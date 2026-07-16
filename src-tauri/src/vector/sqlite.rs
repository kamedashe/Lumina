//! Локальное векторное хранилище на SQLite.
//!
//! Полный перебор с косинусным сходством: просто, работает офлайн и ничего
//! не отправляет наружу. Стоимость поиска линейна по числу фрагментов —
//! для личной базы документов этого достаточно, для сотен тысяч — берите Pinecone.

use super::{Chunk, SearchHit, StoreCtx};
use rusqlite::{params, Connection, Result as SqlResult};
use tauri::Manager;

pub fn init_db(app: &tauri::AppHandle) -> SqlResult<Connection> {
    let app_dir = app
        .path()
        .app_data_dir()
        .expect("Не удалось получить директорию данных приложения");
    if !app_dir.exists() {
        std::fs::create_dir_all(&app_dir).expect("Не удалось создать директорию данных");
    }

    let conn = Connection::open(app_dir.join("lumina.db"))?;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS documents (
            id INTEGER PRIMARY KEY,
            path TEXT NOT NULL,
            content TEXT NOT NULL,
            embedding BLOB
        )",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_documents_path ON documents(path)",
        [],
    )?;
    Ok(conn)
}

pub fn upsert(ctx: &StoreCtx<'_>, chunks: &[Chunk]) -> Result<(), String> {
    let conn = init_db(ctx.app).map_err(|e| e.to_string())?;

    for chunk in chunks {
        let embedding_json = serde_json::to_string(&chunk.embedding).unwrap_or_else(|_| "[]".into());
        conn.execute(
            "INSERT INTO documents (path, content, embedding) VALUES (?1, ?2, ?3)",
            params![chunk.path, chunk.content, embedding_json],
        )
        .map_err(|e| e.to_string())?;
    }

    Ok(())
}

pub fn delete_by_path(ctx: &StoreCtx<'_>, path: &str) -> Result<(), String> {
    let conn = init_db(ctx.app).map_err(|e| e.to_string())?;
    conn.execute("DELETE FROM documents WHERE path = ?1", params![path])
        .map_err(|e| e.to_string())?;
    Ok(())
}

fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() {
        return 0.0;
    }
    let dot: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
    let norm_a = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm_a == 0.0 || norm_b == 0.0 {
        0.0
    } else {
        dot / (norm_a * norm_b)
    }
}

pub fn query(
    ctx: &StoreCtx<'_>,
    embedding: &[f32],
    scope: &[String],
    top_k: usize,
) -> Result<Vec<SearchHit>, String> {
    let conn = init_db(ctx.app).map_err(|e| e.to_string())?;
    let scope_set: std::collections::HashSet<&str> = scope.iter().map(String::as_str).collect();

    let mut stmt = conn
        .prepare("SELECT content, embedding, path FROM documents")
        .map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map([], |row| {
            let content: String = row.get(0)?;
            let embedding_json: String = row.get(1)?;
            let path: String = row.get(2)?;
            let vector: Vec<f32> = serde_json::from_str(&embedding_json).unwrap_or_default();
            Ok((content, vector, path))
        })
        .map_err(|e| e.to_string())?;

    let mut results: Vec<SearchHit> = Vec::new();
    for row in rows.flatten() {
        let (content, vector, path) = row;
        if !scope_set.is_empty() && !scope_set.contains(path.as_str()) {
            continue;
        }
        if vector.is_empty() {
            continue;
        }
        results.push(SearchHit {
            score: cosine_similarity(embedding, &vector),
            content,
            path,
        });
    }

    results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    results.truncate(top_k);
    Ok(results)
}

pub fn health(ctx: &StoreCtx<'_>) -> Result<String, String> {
    let conn = init_db(ctx.app).map_err(|e| e.to_string())?;
    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM documents", [], |row| row.get(0))
        .map_err(|e| e.to_string())?;
    Ok(format!("Локальный SQLite · фрагментов в индексе: {}", count))
}
