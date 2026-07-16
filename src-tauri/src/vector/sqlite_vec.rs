//! Локальное хранилище на расширении sqlite-vec (виртуальная таблица vec0).
//!
//! Важно понимать, что это даёт, а что нет: sqlite-vec до v1 делает **полный
//! перебор**, а не ANN — приблизительных индексов там пока нет. Ускорение
//! относительно нашего наивного бэкенда даёт не алгоритм, а константа:
//! векторы лежат компактными бинарными блобами вместо JSON (который иначе
//! парсится на каждой строке при каждом запросе), а расстояния считает SIMD-код
//! на C, а не скалярный Rust.

use super::{Chunk, SearchHit, StoreCtx};
use rusqlite::{ffi::sqlite3_auto_extension, params, Connection};
use sqlite_vec::sqlite3_vec_init;
use std::sync::Once;
use tauri::Manager;

/// Расширение регистрируется глобально и ровно один раз за процесс —
/// после этого его подхватывает каждое новое соединение.
fn register_extension() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| unsafe {
        sqlite3_auto_extension(Some(std::mem::transmute(sqlite3_vec_init as *const ())));
    });
}

/// vec0 принимает вектор бинарным блобом из little-endian f32.
fn to_blob(embedding: &[f32]) -> Vec<u8> {
    embedding.iter().flat_map(|x| x.to_le_bytes()).collect()
}

fn open(app: &tauri::AppHandle) -> Result<Connection, String> {
    register_extension();

    let app_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Нет доступа к директории данных: {}", e))?;
    if !app_dir.exists() {
        std::fs::create_dir_all(&app_dir).map_err(|e| e.to_string())?;
    }

    let conn = Connection::open(app_dir.join("lumina-vec.db")).map_err(|e| e.to_string())?;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS vec_meta (key TEXT PRIMARY KEY, value TEXT NOT NULL)",
        [],
    )
    .map_err(|e| e.to_string())?;
    Ok(conn)
}

fn stored_dimension(conn: &Connection) -> Option<usize> {
    conn.query_row(
        "SELECT value FROM vec_meta WHERE key = 'dimension'",
        [],
        |row| row.get::<_, String>(0),
    )
    .ok()
    .and_then(|v| v.parse().ok())
}

/// Размерность в vec0 фиксируется при создании таблицы. Если сменилась модель
/// эмбеддингов, старые векторы всё равно несопоставимы с новыми — пересоздаём.
fn ensure_table(conn: &Connection, dimension: usize, allow_recreate: bool) -> Result<(), String> {
    match stored_dimension(conn) {
        Some(existing) if existing == dimension => Ok(()),
        Some(existing) => {
            if !allow_recreate {
                return Err(format!(
                    "Индекс построен для размерности {}, а модель эмбеддингов выдаёт {}. \
                     Переиндексируйте документы (прикрепите их заново).",
                    existing, dimension
                ));
            }
            conn.execute("DROP TABLE IF EXISTS vec_documents", [])
                .map_err(|e| e.to_string())?;
            create_table(conn, dimension)
        }
        None => create_table(conn, dimension),
    }
}

fn create_table(conn: &Connection, dimension: usize) -> Result<(), String> {
    // path — метаданные (можно фильтровать), content — вспомогательная колонка
    // с префиксом «+»: хранится рядом, не индексируется, достаётся без join.
    conn.execute(
        &format!(
            "CREATE VIRTUAL TABLE IF NOT EXISTS vec_documents USING vec0(
                embedding float[{}],
                path TEXT,
                +content TEXT
            )",
            dimension
        ),
        [],
    )
    .map_err(|e| format!("Не удалось создать vec0-таблицу: {}", e))?;

    conn.execute(
        "INSERT OR REPLACE INTO vec_meta (key, value) VALUES ('dimension', ?1)",
        params![dimension.to_string()],
    )
    .map_err(|e| e.to_string())?;

    Ok(())
}

pub fn upsert(ctx: &StoreCtx<'_>, chunks: &[Chunk]) -> Result<(), String> {
    if chunks.is_empty() {
        return Ok(());
    }

    let mut conn = open(ctx.app)?;
    ensure_table(&conn, chunks[0].embedding.len(), true)?;

    let tx = conn.transaction().map_err(|e| e.to_string())?;
    {
        let mut stmt = tx
            .prepare("INSERT INTO vec_documents (embedding, path, content) VALUES (?1, ?2, ?3)")
            .map_err(|e| e.to_string())?;

        for chunk in chunks {
            stmt.execute(params![to_blob(&chunk.embedding), chunk.path, chunk.content])
                .map_err(|e| format!("Не удалось записать фрагмент: {}", e))?;
        }
    }
    tx.commit().map_err(|e| e.to_string())?;

    Ok(())
}

pub fn delete_by_path(ctx: &StoreCtx<'_>, path: &str) -> Result<(), String> {
    let conn = open(ctx.app)?;
    if stored_dimension(&conn).is_none() {
        return Ok(()); // таблицы ещё нет — удалять нечего
    }

    // Сначала собираем rowid по метаданным, потом удаляем по ним: удаление
    // по rowid — самый предсказуемый путь для виртуальной таблицы.
    let rowids: Vec<i64> = {
        let mut stmt = conn
            .prepare("SELECT rowid FROM vec_documents WHERE path = ?1")
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(params![path], |row| row.get::<_, i64>(0))
            .map_err(|e| e.to_string())?;
        rows.flatten().collect()
    };

    for rowid in rowids {
        conn.execute("DELETE FROM vec_documents WHERE rowid = ?1", params![rowid])
            .map_err(|e| e.to_string())?;
    }

    Ok(())
}

pub fn query(
    ctx: &StoreCtx<'_>,
    embedding: &[f32],
    scope: &[String],
    top_k: usize,
) -> Result<Vec<SearchHit>, String> {
    let conn = open(ctx.app)?;
    if stored_dimension(&conn).is_none() {
        return Ok(Vec::new()); // ничего не индексировали
    }
    ensure_table(&conn, embedding.len(), false)?;

    let blob = to_blob(embedding);
    let mut hits: Vec<SearchHit> = Vec::new();

    if scope.is_empty() {
        let mut stmt = conn
            .prepare(
                "SELECT content, path, distance FROM vec_documents
                 WHERE embedding MATCH ?1 AND k = ?2
                 ORDER BY distance",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(params![blob, top_k as i64], map_hit)
            .map_err(|e| e.to_string())?;
        hits.extend(rows.flatten());
    } else {
        // TEXT-метаданные в vec0 поддерживают только «=» и «!=» — оператора IN
        // нет, поэтому по каждому пути идёт свой KNN-запрос, а результаты
        // сливаются и пересортировываются. Вложений обычно единицы, это дёшево.
        let mut stmt = conn
            .prepare(
                "SELECT content, path, distance FROM vec_documents
                 WHERE embedding MATCH ?1 AND k = ?2 AND path = ?3
                 ORDER BY distance",
            )
            .map_err(|e| e.to_string())?;

        for path in scope {
            let rows = stmt
                .query_map(params![blob, top_k as i64, path], map_hit)
                .map_err(|e| e.to_string())?;
            hits.extend(rows.flatten());
        }
    }

    // vec0 отдаёт расстояние (меньше — ближе), а SearchHit ожидает score,
    // где больше — лучше. Инвертируем и сортируем по убыванию.
    for hit in hits.iter_mut() {
        hit.score = -hit.score;
    }
    hits.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    hits.truncate(top_k);

    Ok(hits)
}

fn map_hit(row: &rusqlite::Row<'_>) -> rusqlite::Result<SearchHit> {
    Ok(SearchHit {
        content: row.get(0)?,
        path: row.get(1)?,
        score: row.get::<_, f64>(2)? as f32,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Проверяет ровно те конструкции vec0, на которых держится модуль:
    /// создание таблицы с метаданными и вспомогательной колонкой, вставку
    /// блобов, KNN через MATCH + k, фильтр по TEXT-метаданным и удаление.
    fn setup() -> Connection {
        register_extension();
        let conn = Connection::open_in_memory().unwrap();
        conn.execute(
            "CREATE VIRTUAL TABLE vec_documents USING vec0(
                embedding float[4],
                path TEXT,
                +content TEXT
            )",
            [],
        )
        .unwrap();
        conn
    }

    fn insert(conn: &Connection, embedding: [f32; 4], path: &str, content: &str) {
        conn.execute(
            "INSERT INTO vec_documents (embedding, path, content) VALUES (?1, ?2, ?3)",
            params![to_blob(&embedding), path, content],
        )
        .unwrap();
    }

    #[test]
    fn extension_loads() {
        register_extension();
        let conn = Connection::open_in_memory().unwrap();
        let version: String = conn
            .query_row("SELECT vec_version()", [], |row| row.get(0))
            .expect("расширение sqlite-vec должно загрузиться");
        assert!(!version.is_empty());
    }

    #[test]
    fn knn_returns_nearest_first() {
        let conn = setup();
        insert(&conn, [1.0, 0.0, 0.0, 0.0], "a.txt", "ближний");
        insert(&conn, [0.0, 1.0, 0.0, 0.0], "b.txt", "дальний");

        let mut stmt = conn
            .prepare(
                "SELECT content, path, distance FROM vec_documents
                 WHERE embedding MATCH ?1 AND k = ?2
                 ORDER BY distance",
            )
            .unwrap();
        let hits: Vec<SearchHit> = stmt
            .query_map(params![to_blob(&[1.0f32, 0.0, 0.0, 0.0]), 2i64], map_hit)
            .unwrap()
            .flatten()
            .collect();

        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].content, "ближний");
        assert!(hits[0].score < hits[1].score, "distance должен расти");
    }

    #[test]
    fn metadata_filter_scopes_by_path() {
        let conn = setup();
        insert(&conn, [1.0, 0.0, 0.0, 0.0], "a.txt", "из a");
        insert(&conn, [0.9, 0.1, 0.0, 0.0], "b.txt", "из b");

        let mut stmt = conn
            .prepare(
                "SELECT content, path, distance FROM vec_documents
                 WHERE embedding MATCH ?1 AND k = ?2 AND path = ?3
                 ORDER BY distance",
            )
            .unwrap();
        let hits: Vec<SearchHit> = stmt
            .query_map(params![to_blob(&[1.0f32, 0.0, 0.0, 0.0]), 10i64, "b.txt"], map_hit)
            .unwrap()
            .flatten()
            .collect();

        assert_eq!(hits.len(), 1, "должен вернуться только b.txt");
        assert_eq!(hits[0].path, "b.txt");
    }

    #[test]
    fn delete_by_rowid_removes_only_that_path() {
        let conn = setup();
        insert(&conn, [1.0, 0.0, 0.0, 0.0], "a.txt", "остаётся");
        insert(&conn, [0.0, 1.0, 0.0, 0.0], "b.txt", "удаляется");

        let rowids: Vec<i64> = {
            let mut stmt = conn
                .prepare("SELECT rowid FROM vec_documents WHERE path = ?1")
                .unwrap();
            stmt.query_map(params!["b.txt"], |row| row.get(0))
                .unwrap()
                .flatten()
                .collect()
        };
        assert_eq!(rowids.len(), 1);

        for rowid in rowids {
            conn.execute("DELETE FROM vec_documents WHERE rowid = ?1", params![rowid])
                .unwrap();
        }

        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM vec_documents", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 1);
    }
}

pub fn health(ctx: &StoreCtx<'_>) -> Result<String, String> {
    let conn = open(ctx.app)?;

    let version: String = conn
        .query_row("SELECT vec_version()", [], |row| row.get(0))
        .map_err(|e| format!("Расширение sqlite-vec не загрузилось: {}", e))?;

    let Some(dimension) = stored_dimension(&conn) else {
        return Ok(format!(
            "sqlite-vec {} · индекс пуст (прикрепите документы)",
            version
        ));
    };

    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM vec_documents", [], |row| row.get(0))
        .unwrap_or(0);

    Ok(format!(
        "sqlite-vec {} · dimension {} · фрагментов: {}",
        version, dimension, count
    ))
}
