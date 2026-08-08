//! RAG: извлечение текста, нарезка на фрагменты и оркестрация поиска.
//! Само хранение векторов делегируется модулю `vector` (SQLite или Pinecone).

use crate::providers::{self, EmbedRole, ProviderConfig};
use crate::vector::{self, Chunk, SearchHit, StoreCtx, VectorStoreConfig};
use std::fs;
use std::path::Path;

/// Параметры нарезки по умолчанию. Вынесены в константы, потому что в
/// eval-харнесе это свипаемая ручка: размер чанка задаётся конфигом прогона.
pub const DEFAULT_CHUNK_SIZE: usize = 1000;
pub const DEFAULT_CHUNK_OVERLAP: usize = 200;

/// Сколько фрагментов подмешивать в контекст модели.
pub const AGENT_TOP_K: usize = 4;

/// Фрагмент текста вместе с его границами в исходном документе.
struct TextChunk {
    content: String,
    char_start: usize,
    char_end: usize,
}

/// Режет текст на перекрывающиеся окна.
///
/// Границы считаются в СИМВОЛАХ, а не в байтах: разметка golden set ищет
/// цитату питоновским `str.find()`, который возвращает смещение в кодовых
/// точках. На кириллице байтовые смещения разошлись бы с ним вдвое, и
/// пересечение спанов молча давало бы неверные метрики.
fn chunk_text(text: &str, size: usize, overlap: usize) -> Vec<TextChunk> {
    let chars: Vec<char> = text.chars().collect();
    if chars.is_empty() || size == 0 {
        return Vec::new();
    }

    // Защита от конфига со свипа: overlap >= size означал бы шаг 0
    // и бесконечный цикл.
    let step = size.saturating_sub(overlap).max(1);

    let mut chunks = Vec::new();
    let mut start = 0;
    while start < chars.len() {
        let end = (start + size).min(chars.len());
        chunks.push(TextChunk {
            content: chars[start..end].iter().collect(),
            char_start: start,
            char_end: end,
        });
        if end == chars.len() {
            break;
        }
        start += step;
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
    data_dir: &Path,
    client: &reqwest::Client,
    provider: &ProviderConfig,
    store: &VectorStoreConfig,
    paths: &[String],
    chunk_size: usize,
    chunk_overlap: usize,
) -> Result<String, String> {
    let ctx = StoreCtx {
        data_dir,
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
        for (index, tc) in chunk_text(&content, chunk_size, chunk_overlap)
            .into_iter()
            .enumerate()
        {
            let embedding =
                providers::embed_as(client, provider, &tc.content, EmbedRole::Document).await?;
            batch.push(Chunk {
                path: path_str.clone(),
                index,
                char_start: tc.char_start,
                char_end: tc.char_end,
                content: tc.content,
                embedding,
            });
        }

        total_chunks += batch.len();
        vector::upsert(&ctx, &batch).await?;
    }

    Ok(format!("Проиндексировано фрагментов: {}", total_chunks))
}

/// Единственная точка входа в поиск: и агентный цикл, и LangGraph-сайдкар,
/// и eval-харнес ходят сюда. Одна точка — чтобы харнес мерил ровно тот путь,
/// которым пользуется приложение, а не свою копию логики.
pub async fn retrieve_hits(
    data_dir: &Path,
    client: &reqwest::Client,
    provider: &ProviderConfig,
    store: &VectorStoreConfig,
    query: &str,
    scope: &[String],
    k: usize,
) -> Result<Vec<SearchHit>, String> {
    let ctx = StoreCtx {
        data_dir,
        client,
        config: store,
    };

    let embedding = providers::embed_as(client, provider, query, EmbedRole::Query).await?;
    vector::query(&ctx, &embedding, scope, k).await
}

/// Ищет релевантные фрагменты и склеивает их в контекст для модели.
/// Непустой `scope` ограничивает поиск конкретными файлами.
pub async fn search_documents(
    data_dir: &Path,
    client: &reqwest::Client,
    provider: &ProviderConfig,
    store: &VectorStoreConfig,
    query: &str,
    scope: &[String],
) -> Result<String, String> {
    let hits = retrieve_hits(
        data_dir,
        client,
        provider,
        store,
        query,
        scope,
        AGENT_TOP_K,
    )
    .await?;

    Ok(hits
        .into_iter()
        .map(|h| h.content)
        .collect::<Vec<_>>()
        .join("\n---\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spans_are_contiguous_with_overlap() {
        let text: String = "абвгдеёжзи".repeat(30); // 300 символов кириллицы
        let chunks = chunk_text(&text, 100, 20);

        assert!(chunks.len() > 1);
        assert_eq!(chunks[0].char_start, 0);
        // Шаг = size - overlap, значит следующий чанк начинается на 80.
        assert_eq!(chunks[1].char_start, 80);
        // Соседние чанки перекрываются ровно на overlap.
        assert_eq!(chunks[0].char_end - chunks[1].char_start, 20);
        // Последний чанк упирается в конец текста, а не выходит за него.
        assert_eq!(
            chunks.last().unwrap().char_end,
            text.chars().count()
        );
    }

    #[test]
    fn spans_are_char_offsets_not_bytes() {
        let text = "абвг"; // 4 символа, 8 байт в UTF-8
        let chunks = chunk_text(text, 10, 0);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].char_end, 4, "смещения должны быть в символах");
    }

    #[test]
    fn overlap_not_smaller_than_size_does_not_hang() {
        let text: String = "a".repeat(50);
        let chunks = chunk_text(&text, 10, 10);
        assert!(!chunks.is_empty());
    }
}
