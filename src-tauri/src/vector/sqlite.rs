//! Локальное векторное хранилище на SQLite.
//!
//! Полный перебор с косинусным сходством: просто, работает офлайн и ничего
//! не отправляет наружу. Стоимость поиска линейна по числу фрагментов —
//! для личной базы документов этого достаточно, для сотен тысяч — берите Pinecone.

use super::{path_key, Chunk, SearchHit, StoreCtx};
use rusqlite::{params, Connection, Result as SqlResult};
use std::path::Path;

/// Версия схемы. При её смене таблица пересоздаётся: у старых строк нет
/// символьных смещений, и восстановить их неоткуда — нужна переиндексация.
const SCHEMA_VERSION: i64 = 2;

pub fn init_db(data_dir: &Path) -> SqlResult<Connection> {
    if !data_dir.exists() {
        std::fs::create_dir_all(data_dir).expect("Не удалось создать директорию данных");
    }

    let conn = Connection::open(data_dir.join("lumina.db"))?;

    let version: i64 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    if version != SCHEMA_VERSION {
        conn.execute("DROP TABLE IF EXISTS documents", [])?;
        conn.pragma_update(None, "user_version", SCHEMA_VERSION)?;
    }

    conn.execute(
        "CREATE TABLE IF NOT EXISTS documents (
            id INTEGER PRIMARY KEY,
            path TEXT NOT NULL,
            chunk_index INTEGER NOT NULL,
            char_start INTEGER NOT NULL,
            char_end INTEGER NOT NULL,
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
    let conn = init_db(ctx.data_dir).map_err(|e| e.to_string())?;

    for chunk in chunks {
        let embedding_json = serde_json::to_string(&chunk.embedding).unwrap_or_else(|_| "[]".into());
        conn.execute(
            "INSERT INTO documents
                (path, chunk_index, char_start, char_end, content, embedding)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                chunk.path,
                chunk.index as i64,
                chunk.char_start as i64,
                chunk.char_end as i64,
                chunk.content,
                embedding_json
            ],
        )
        .map_err(|e| e.to_string())?;
    }

    Ok(())
}

pub fn delete_by_path(ctx: &StoreCtx<'_>, path: &str) -> Result<(), String> {
    let conn = init_db(ctx.data_dir).map_err(|e| e.to_string())?;
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
    let conn = init_db(ctx.data_dir).map_err(|e| e.to_string())?;
    let scope_set: std::collections::HashSet<&str> = scope.iter().map(String::as_str).collect();

    let mut stmt = conn
        .prepare(
            "SELECT content, embedding, path, chunk_index, char_start, char_end FROM documents",
        )
        .map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map([], |row| {
            let content: String = row.get(0)?;
            let embedding_json: String = row.get(1)?;
            let path: String = row.get(2)?;
            let index: i64 = row.get(3)?;
            let char_start: i64 = row.get(4)?;
            let char_end: i64 = row.get(5)?;
            let vector: Vec<f32> = serde_json::from_str(&embedding_json).unwrap_or_default();
            Ok((content, vector, path, index, char_start, char_end))
        })
        .map_err(|e| e.to_string())?;

    let mut results: Vec<SearchHit> = Vec::new();
    for row in rows.flatten() {
        let (content, vector, path, index, char_start, char_end) = row;
        if !scope_set.is_empty() && !scope_set.contains(path.as_str()) {
            continue;
        }
        if vector.is_empty() {
            continue;
        }
        results.push(SearchHit {
            chunk_id: format!("{}#{}", path_key(&path), index),
            score: cosine_similarity(embedding, &vector),
            char_start: char_start as usize,
            char_end: char_end as usize,
            content,
            path,
        });
    }

    results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    results.truncate(top_k);
    Ok(results)
}

pub fn health(ctx: &StoreCtx<'_>) -> Result<String, String> {
    let conn = init_db(ctx.data_dir).map_err(|e| e.to_string())?;
    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM documents", [], |row| row.get(0))
        .map_err(|e| e.to_string())?;
    Ok(format!("Локальный SQLite · фрагментов в индексе: {}", count))
}

