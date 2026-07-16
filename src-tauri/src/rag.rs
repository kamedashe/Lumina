//! RAG: извлечение текста, нарезка на фрагменты и оркестрация поиска.
//! Само хранение векторов делегируется модулю `vector` (SQLite или Pinecone).

use crate::providers::{self, ProviderConfig};
use crate::vector::{self, Chunk, StoreCtx, VectorStoreConfig};
use std::fs;
use std::path::Path;

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

/// Индексирует документы. Прежние фрагменты того же пути удаляются, поэтому
/// повторная индексация обновляет файл, а не плодит дубликаты.
pub async fn process_documents(
    app: &tauri::AppHandle,
    client: &reqwest::Client,
    provider: &ProviderConfig,
    store: &VectorStoreConfig,
    paths: &[String],
) -> Result<String, String> {
    let ctx = StoreCtx {
        app,
        client,
        config: store,
    };
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

        vector::delete_by_path(&ctx, path_str).await?;

        let mut batch: Vec<Chunk> = Vec::new();
        for (index, text) in chunk_text(&content, 1000, 200).into_iter().enumerate() {
            let embedding = providers::embed(client, provider, &text).await?;
            batch.push(Chunk {
                path: path_str.clone(),
                index,
                content: text,
                embedding,
            });
        }

        total_chunks += batch.len();
        vector::upsert(&ctx, &batch).await?;
    }

    Ok(format!("Проиндексировано фрагментов: {}", total_chunks))
}

/// Ищет релевантные фрагменты и склеивает их в контекст для модели.
/// Непустой `scope` ограничивает поиск конкретными файлами.
pub async fn search_documents(
    app: &tauri::AppHandle,
    client: &reqwest::Client,
    provider: &ProviderConfig,
    store: &VectorStoreConfig,
    query: &str,
    scope: &[String],
) -> Result<String, String> {
    let ctx = StoreCtx {
        app,
        client,
        config: store,
    };

    let embedding = providers::embed(client, provider, query).await?;
    let hits = vector::query(&ctx, &embedding, scope, 4).await?;

    Ok(hits
        .into_iter()
        .map(|h| h.content)
        .collect::<Vec<_>>()
        .join("\n---\n"))
}
