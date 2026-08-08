//! Клиент Pinecone (serverless), REST data plane + control plane.
//!
//! Важное ограничение, вокруг которого построен весь модуль: serverless-индексы
//! **не умеют удалять по фильтру метаданных** — только по ID или префиксу ID.
//! Поэтому ID фрагмента собирается как `{хеш пути}#{номер}`, и переиндексация
//! файла делается через list-by-prefix → delete-by-ids.
//! Фильтр по метаданным в `query` при этом работает штатно.

use super::{path_key, Chunk, SearchHit, StoreCtx};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

const CONTROL_PLANE: &str = "https://api.pinecone.io";
const API_VERSION: &str = "2025-04";
/// Pinecone принимает максимум 1000 векторов за один upsert.
const UPSERT_BATCH: usize = 100;

/// Хост индекса резолвится через control plane и не меняется — кешируем,
/// чтобы не платить лишний round-trip на каждый поиск.
fn host_cache() -> &'static Mutex<HashMap<String, String>> {
    static CACHE: OnceLock<Mutex<HashMap<String, String>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

async fn describe_index(ctx: &StoreCtx<'_>) -> Result<Value, String> {
    let index = ctx.config.index()?;
    let response = ctx
        .client
        .get(format!("{}/indexes/{}", CONTROL_PLANE, index))
        .header("Api-Key", ctx.config.key()?)
        .header("X-Pinecone-Api-Version", API_VERSION)
        .send()
        .await
        .map_err(|e| format!("Не удалось подключиться к Pinecone: {}", e))?;

    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        return Err(match status.as_u16() {
            401 => "Pinecone: неверный API-ключ.".to_string(),
            404 => format!("Pinecone: индекс «{}» не найден.", index),
            _ => format!("Pinecone вернул {}: {}", status, body.chars().take(300).collect::<String>()),
        });
    }

    response
        .json()
        .await
        .map_err(|e| format!("Не удалось разобрать ответ Pinecone: {}", e))
}

async fn resolve_host(ctx: &StoreCtx<'_>) -> Result<String, String> {
    let index = ctx.config.index()?.to_string();

    if let Some(host) = host_cache().lock().unwrap().get(&index) {
        return Ok(host.clone());
    }

    let info = describe_index(ctx).await?;
    let host = info
        .get("host")
        .and_then(Value::as_str)
        .ok_or("Pinecone не вернул host индекса.")?
        .to_string();

    host_cache().lock().unwrap().insert(index, host.clone());
    Ok(host)
}

async fn read_error(response: reqwest::Response) -> String {
    let status = response.status();
    let body = response.text().await.unwrap_or_default();
    let detail = serde_json::from_str::<Value>(&body)
        .ok()
        .and_then(|v| {
            v.pointer("/error/message")
                .or_else(|| v.pointer("/message"))
                .and_then(|m| m.as_str().map(str::to_string))
        })
        .unwrap_or_else(|| body.chars().take(300).collect());
    format!("Pinecone вернул {}: {}", status, detail)
}

pub async fn upsert(ctx: &StoreCtx<'_>, chunks: &[Chunk]) -> Result<(), String> {
    if chunks.is_empty() {
        return Ok(());
    }

    let host = resolve_host(ctx).await?;

    // Размерность индекса фиксирована при создании. Если она не совпадает с
    // моделью эмбеддингов, Pinecone ответит невнятной 400 — проверим заранее.
    let info = describe_index(ctx).await?;
    if let Some(dim) = info.get("dimension").and_then(Value::as_u64) {
        let actual = chunks[0].embedding.len() as u64;
        if dim != actual {
            return Err(format!(
                "Размерность индекса Pinecone — {}, а модель эмбеддингов выдаёт {}. \
                 Создайте индекс с dimension={} или смените модель эмбеддингов.",
                dim, actual, actual
            ));
        }
    }

    for batch in chunks.chunks(UPSERT_BATCH) {
        let vectors: Vec<Value> = batch
            .iter()
            .map(|c| {
                json!({
                    "id": format!("{}#{}", path_key(&c.path), c.index),
                    "values": c.embedding,
                    "metadata": {
                        "path": c.path,
                        "text": c.content,
                        "chunk_index": c.index,
                        "char_start": c.char_start,
                        "char_end": c.char_end
                    }
                })
            })
            .collect();

        let response = ctx
            .client
            .post(format!("https://{}/vectors/upsert", host))
            .header("Api-Key", ctx.config.key()?)
            .header("X-Pinecone-Api-Version", API_VERSION)
            .json(&json!({ "vectors": vectors, "namespace": ctx.config.ns() }))
            .send()
            .await
            .map_err(|e| format!("Не удалось записать в Pinecone: {}", e))?;

        if !response.status().is_success() {
            return Err(read_error(response).await);
        }
    }

    Ok(())
}

/// Собирает все ID фрагментов файла через list-by-prefix (с пагинацией).
async fn list_ids_by_prefix(ctx: &StoreCtx<'_>, host: &str, prefix: &str) -> Result<Vec<String>, String> {
    let mut ids = Vec::new();
    let mut token: Option<String> = None;

    loop {
        let mut request = ctx
            .client
            .get(format!("https://{}/vectors/list", host))
            .header("Api-Key", ctx.config.key()?)
            .header("X-Pinecone-Api-Version", API_VERSION)
            .query(&[("prefix", prefix), ("limit", "100")]);

        let ns = ctx.config.ns();
        if !ns.is_empty() {
            request = request.query(&[("namespace", ns)]);
        }
        if let Some(t) = &token {
            request = request.query(&[("paginationToken", t.as_str())]);
        }

        let response = request
            .send()
            .await
            .map_err(|e| format!("Не удалось получить список ID из Pinecone: {}", e))?;

        if !response.status().is_success() {
            return Err(read_error(response).await);
        }

        let body: Value = response.json().await.map_err(|e| e.to_string())?;

        if let Some(vectors) = body.get("vectors").and_then(Value::as_array) {
            for v in vectors {
                if let Some(id) = v.get("id").and_then(Value::as_str) {
                    ids.push(id.to_string());
                }
            }
        }

        token = body
            .pointer("/pagination/next")
            .and_then(Value::as_str)
            .map(str::to_string);

        if token.is_none() {
            break;
        }
    }

    Ok(ids)
}

pub async fn delete_by_path(ctx: &StoreCtx<'_>, path: &str) -> Result<(), String> {
    let host = resolve_host(ctx).await?;
    let prefix = format!("{}#", path_key(path));

    // Обходной путь для serverless: удаление по фильтру метаданных там не
    // поддерживается, поэтому сначала выясняем ID по префиксу.
    let ids = list_ids_by_prefix(ctx, &host, &prefix).await?;
    if ids.is_empty() {
        return Ok(());
    }

    for batch in ids.chunks(1000) {
        let response = ctx
            .client
            .post(format!("https://{}/vectors/delete", host))
            .header("Api-Key", ctx.config.key()?)
            .header("X-Pinecone-Api-Version", API_VERSION)
            .json(&json!({ "ids": batch, "namespace": ctx.config.ns() }))
            .send()
            .await
            .map_err(|e| format!("Не удалось удалить из Pinecone: {}", e))?;

        if !response.status().is_success() {
            return Err(read_error(response).await);
        }
    }

    Ok(())
}

pub async fn query(
    ctx: &StoreCtx<'_>,
    embedding: &[f32],
    scope: &[String],
    top_k: usize,
) -> Result<Vec<SearchHit>, String> {
    let host = resolve_host(ctx).await?;

    let mut body = json!({
        "vector": embedding,
        "topK": top_k,
        "includeMetadata": true,
        "namespace": ctx.config.ns(),
    });

    // В отличие от delete, фильтр по метаданным в query на serverless работает.
    if !scope.is_empty() {
        body["filter"] = json!({ "path": { "$in": scope } });
    }

    let response = ctx
        .client
        .post(format!("https://{}/query", host))
        .header("Api-Key", ctx.config.key()?)
        .header("X-Pinecone-Api-Version", API_VERSION)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("Не удалось выполнить поиск в Pinecone: {}", e))?;

    if !response.status().is_success() {
        return Err(read_error(response).await);
    }

    let payload: Value = response.json().await.map_err(|e| e.to_string())?;
    let matches = payload
        .get("matches")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    Ok(matches
        .iter()
        .map(|m| SearchHit {
            // ID уже имеет вид `{path_key}#{index}` — ровно та схема, что и
            // у локальных бэкендов, поэтому берём его как есть.
            chunk_id: m
                .get("id")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            content: m
                .pointer("/metadata/text")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            path: m
                .pointer("/metadata/path")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            // Числа в metadata Pinecone возвращает как f64, не как integer.
            char_start: m
                .pointer("/metadata/char_start")
                .and_then(Value::as_f64)
                .unwrap_or(0.0) as usize,
            char_end: m
                .pointer("/metadata/char_end")
                .and_then(Value::as_f64)
                .unwrap_or(0.0) as usize,
            score: m.get("score").and_then(Value::as_f64).unwrap_or(0.0) as f32,
        })
        .filter(|hit| !hit.content.is_empty())
        .collect())
}

pub async fn health(ctx: &StoreCtx<'_>) -> Result<String, String> {
    let info = describe_index(ctx).await?;

    let dimension = info.get("dimension").and_then(Value::as_u64).unwrap_or(0);
    let metric = info
        .get("metric")
        .and_then(Value::as_str)
        .unwrap_or("неизвестна");
    let ready = info
        .pointer("/status/ready")
        .and_then(Value::as_bool)
        .unwrap_or(false);

    if !ready {
        let state = info
            .pointer("/status/state")
            .and_then(Value::as_str)
            .unwrap_or("Unknown");
        return Err(format!("Индекс Pinecone ещё не готов (состояние: {}).", state));
    }

    Ok(format!(
        "Pinecone · индекс «{}» · dimension {} · метрика {}",
        ctx.config.index()?,
        dimension,
        metric
    ))
}
