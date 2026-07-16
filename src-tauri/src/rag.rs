//! Локальный RAG-индекс: чанки документов + их эмбеддинги в SQLite.

use crate::providers::{self, ProviderConfig};
use rusqlite::{params, Connection, Result as SqlResult};
use std::fs;
use std::path::Path;
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

fn chunk_text(text: &str, size: usize, overlap: usize) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    if chars.is_empty() {
        return Vec::new();
    }

    let mut chunks = Vec::new();
    let mut start = 0;
    while start < chars.len() {
        let end = (start + size).min(chars.len());
        chunks.push(chars[start..end].iter().collect());
        if end == chars.len() {
            break;
        }
        start += size - overlap;
    }
    chunks
}

fn extract_content(path: &Path) -> Option<String> {
    let ext = path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_lowercase();

    match ext.as_str() {
        "pdf" => pdf_extract::extract_text(path).ok(),
        "txt" | "md" | "json" | "rs" | "ts" | "tsx" | "js" | "py" | "toml" | "csv" => {
            fs::read_to_string(path).ok()
        }
        _ => None,
    }
}

/// Индексирует документы. Перед вставкой удаляет прежние чанки того же пути,
/// чтобы повторная индексация не плодила дубликаты.
pub async fn process_documents(
    app: &tauri::AppHandle,
    client: &reqwest::Client,
    config: &ProviderConfig,
    paths: &[String],
) -> Result<String, String> {
    let conn = init_db(app).map_err(|e| e.to_string())?;
    let mut total_chunks = 0;

    for path_str in paths {
        let path = Path::new(path_str);
        if !path.exists() {
            continue;
        }

        let Some(content) = extract_content(path) else {
            continue;
        };
        if content.trim().is_empty() {
            continue;
        }

        // Чистим прошлую версию этого файла.
        conn.execute("DELETE FROM documents WHERE path = ?1", params![path_str])
            .map_err(|e| e.to_string())?;

        for chunk in chunk_text(&content, 1000, 200) {
            let embedding = providers::embed(client, config, &chunk).await?;
            let embedding_json = serde_json::to_string(&embedding).unwrap_or_else(|_| "[]".into());

            conn.execute(
                "INSERT INTO documents (path, content, embedding) VALUES (?1, ?2, ?3)",
                params![path_str, chunk, embedding_json],
            )
            .map_err(|e| e.to_string())?;
            total_chunks += 1;
        }
    }

    Ok(format!("Проиндексировано фрагментов: {}", total_chunks))
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

/// Ищет наиболее релевантные фрагменты. `scope` (если задан) ограничивает поиск
/// конкретными путями — используем это, чтобы искать только по свежим вложениям.
pub async fn search_documents(
    app: &tauri::AppHandle,
    client: &reqwest::Client,
    config: &ProviderConfig,
    query: &str,
    scope: &[String],
) -> Result<String, String> {
    let conn = init_db(app).map_err(|e| e.to_string())?;
    let query_embedding = providers::embed(client, config, query).await?;

    let scope_set: std::collections::HashSet<&str> = scope.iter().map(String::as_str).collect();

    // path тоже читаем, чтобы отфильтровать по scope на стороне Rust.
    let mut stmt = conn
        .prepare("SELECT content, embedding, path FROM documents")
        .map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map([], |row| {
            let content: String = row.get(0)?;
            let embedding_json: String = row.get(1)?;
            let path: String = row.get(2)?;
            let embedding: Vec<f32> = serde_json::from_str(&embedding_json).unwrap_or_default();
            Ok((content, embedding, path))
        })
        .map_err(|e| e.to_string())?;

    let mut results: Vec<(String, f32)> = Vec::new();
    for row in rows.flatten() {
        let (content, embedding, path) = row;
        if !scope_set.is_empty() && !scope_set.contains(path.as_str()) {
            continue;
        }
        if embedding.is_empty() {
            continue;
        }
        let score = cosine_similarity(&query_embedding, &embedding);
        results.push((content, score));
    }

    results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    let context = results
        .into_iter()
        .take(4)
        .map(|(c, _)| c)
        .collect::<Vec<_>>()
        .join("\n---\n");

    Ok(context)
}
