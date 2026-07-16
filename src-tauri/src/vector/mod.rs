//! Абстракция векторного хранилища.
//!
//! Локальный SQLite (по умолчанию, данные не покидают машину) и облачный
//! Pinecone (ANN-поиск, переживает переустановку, общий индекс между устройствами).

pub mod pinecone;
pub mod sqlite;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum VectorStoreKind {
    /// Локальный SQLite с полным перебором и косинусным сходством.
    Sqlite,
    /// Облачный Pinecone (serverless).
    Pinecone,
}

impl Default for VectorStoreKind {
    fn default() -> Self {
        Self::Sqlite
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct VectorStoreConfig {
    #[serde(default)]
    pub kind: VectorStoreKind,
    #[serde(default)]
    pub api_key: Option<String>,
    /// Имя индекса Pinecone. Хост резолвится автоматически через control plane.
    #[serde(default)]
    pub index_name: Option<String>,
    #[serde(default)]
    pub namespace: Option<String>,
}

impl VectorStoreConfig {
    pub fn key(&self) -> Result<&str, String> {
        self.api_key
            .as_deref()
            .filter(|k| !k.trim().is_empty())
            .ok_or_else(|| "Не задан API-ключ Pinecone.".to_string())
    }

    pub fn index(&self) -> Result<&str, String> {
        self.index_name
            .as_deref()
            .filter(|k| !k.trim().is_empty())
            .ok_or_else(|| "Не задано имя индекса Pinecone.".to_string())
    }

    pub fn ns(&self) -> &str {
        self.namespace.as_deref().unwrap_or("")
    }
}

/// Фрагмент документа с посчитанным эмбеддингом.
#[derive(Clone, Debug)]
pub struct Chunk {
    pub path: String,
    /// Порядковый номер фрагмента внутри файла.
    pub index: usize,
    pub content: String,
    pub embedding: Vec<f32>,
}

#[derive(Clone, Debug)]
pub struct SearchHit {
    pub content: String,
    #[allow(dead_code)]
    pub path: String,
    pub score: f32,
}

pub struct StoreCtx<'a> {
    pub app: &'a tauri::AppHandle,
    pub client: &'a reqwest::Client,
    pub config: &'a VectorStoreConfig,
}

/// Стабильный ключ пути. Нужен, чтобы собирать ID вида `{ключ}#{номер}`:
/// serverless-индексы Pinecone не умеют удалять по фильтру метаданных, только
/// по ID и префиксу ID, поэтому переиндексация файла опирается на этот префикс.
///
/// FNV-1a, а не DefaultHasher — у последнего не гарантирована стабильность
/// между версиями Rust, а ID должны переживать обновление приложения.
pub fn path_key(path: &str) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in path.as_bytes() {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{:016x}", hash)
}

pub async fn upsert(ctx: &StoreCtx<'_>, chunks: &[Chunk]) -> Result<(), String> {
    match ctx.config.kind {
        VectorStoreKind::Sqlite => sqlite::upsert(ctx, chunks),
        VectorStoreKind::Pinecone => pinecone::upsert(ctx, chunks).await,
    }
}

/// Удаляет все фрагменты файла перед его повторной индексацией.
pub async fn delete_by_path(ctx: &StoreCtx<'_>, path: &str) -> Result<(), String> {
    match ctx.config.kind {
        VectorStoreKind::Sqlite => sqlite::delete_by_path(ctx, path),
        VectorStoreKind::Pinecone => pinecone::delete_by_path(ctx, path).await,
    }
}

/// Поиск ближайших фрагментов. Непустой `scope` ограничивает поиск этими путями.
pub async fn query(
    ctx: &StoreCtx<'_>,
    embedding: &[f32],
    scope: &[String],
    top_k: usize,
) -> Result<Vec<SearchHit>, String> {
    match ctx.config.kind {
        VectorStoreKind::Sqlite => sqlite::query(ctx, embedding, scope, top_k),
        VectorStoreKind::Pinecone => pinecone::query(ctx, embedding, scope, top_k).await,
    }
}

/// Проверка доступности хранилища — для индикатора в настройках.
/// Возвращает человекочитаемое описание состояния.
pub async fn health(ctx: &StoreCtx<'_>) -> Result<String, String> {
    match ctx.config.kind {
        VectorStoreKind::Sqlite => sqlite::health(ctx),
        VectorStoreKind::Pinecone => pinecone::health(ctx).await,
    }
}
