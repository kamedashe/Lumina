//! Клиент локального Ollama (/api/chat с поддержкой инструментов).

use super::sse::{self, read_ndjson};
use super::{AssistantTurn, ChatMessage, ProviderConfig, Role, StreamEvent, ToolCall, ToolDef};
use serde_json::{json, Value};
use tauri::ipc::Channel;

fn role_str(role: &Role) -> &'static str {
    match role {
        Role::System => "system",
        Role::User => "user",
        Role::Assistant => "assistant",
        Role::Tool => "tool",
    }
}

fn to_wire_messages(system: &str, messages: &[ChatMessage]) -> Vec<Value> {
    let mut out = Vec::with_capacity(messages.len() + 1);
    if !system.trim().is_empty() {
        out.push(json!({ "role": "system", "content": system }));
    }

    for m in messages {
        let mut obj = json!({ "role": role_str(&m.role), "content": m.content });

        if !m.tool_calls.is_empty() {
            obj["tool_calls"] = Value::Array(
                m.tool_calls
                    .iter()
                    .map(|c| {
                        let args: Value =
                            serde_json::from_str(&c.arguments).unwrap_or_else(|_| json!({}));
                        // Ollama ждёт arguments объектом, а не строкой.
                        json!({ "function": { "name": c.name, "arguments": args } })
                    })
                    .collect(),
            );
        }

        if let Some(name) = &m.tool_name {
            obj["tool_name"] = json!(name);
        }

        out.push(obj);
    }

    out
}

pub async fn stream_turn(
    client: &reqwest::Client,
    config: &ProviderConfig,
    system: &str,
    messages: &[ChatMessage],
    tools: &[ToolDef],
    channel: &Channel<StreamEvent>,
) -> Result<AssistantTurn, String> {
    let mut body = json!({
        "model": config.model,
        "messages": to_wire_messages(system, messages),
        "stream": true,
        "options": { "temperature": config.temperature },
    });

    if !tools.is_empty() {
        body["tools"] = Value::Array(
            tools
                .iter()
                .map(|t| {
                    json!({
                        "type": "function",
                        "function": {
                            "name": t.name,
                            "description": t.description,
                            "parameters": t.parameters
                        }
                    })
                })
                .collect(),
        );
    }

    let response = client
        .post(format!("{}/api/chat", config.base()))
        .json(&body)
        .send()
        .await
        .map_err(|e| {
            format!(
                "Не удалось подключиться к Ollama по {}: {}. Проверьте, что он запущен.",
                config.base(),
                e
            )
        })?;

    if !response.status().is_success() {
        return Err(sse::error_from_response(response, "Ollama").await);
    }

    let mut turn = AssistantTurn::default();

    read_ndjson(response, |chunk| {
        if let Some(err) = chunk.get("error").and_then(Value::as_str) {
            return Err(format!("Ollama: {}", err));
        }

        if let Some(text) = chunk.pointer("/message/content").and_then(Value::as_str) {
            if !text.is_empty() {
                turn.text.push_str(text);
                let _ = channel.send(StreamEvent::Text {
                    delta: text.to_string(),
                });
            }
        }

        // Ollama отдаёт вызов целиком и без id — генерируем свой.
        if let Some(calls) = chunk.pointer("/message/tool_calls").and_then(Value::as_array) {
            for call in calls {
                let name = call
                    .pointer("/function/name")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string();
                if name.is_empty() {
                    continue;
                }

                let args = call
                    .pointer("/function/arguments")
                    .cloned()
                    .unwrap_or_else(|| json!({}));
                let id = super::new_call_id(&name);

                let _ = channel.send(StreamEvent::ToolCallStart {
                    id: id.clone(),
                    name: name.clone(),
                });
                turn.tool_calls.push(ToolCall {
                    id,
                    name,
                    arguments: args.to_string(),
                });
            }
        }

        if chunk.get("done").and_then(Value::as_bool) == Some(true) {
            turn.stop_reason = chunk
                .get("done_reason")
                .and_then(Value::as_str)
                .unwrap_or("stop")
                .to_string();
        }

        Ok(())
    })
    .await?;

    Ok(turn)
}

pub async fn list_models(
    client: &reqwest::Client,
    config: &ProviderConfig,
) -> Result<Vec<String>, String> {
    let response = client
        .get(format!("{}/api/tags", config.base()))
        .send()
        .await
        .map_err(|e| {
            format!(
                "Не удалось подключиться к Ollama по {}: {}. Проверьте, что он запущен.",
                config.base(),
                e
            )
        })?;

    if !response.status().is_success() {
        return Err(sse::error_from_response(response, "Ollama").await);
    }

    let body: Value = response.json().await.map_err(|e| e.to_string())?;
    let mut models: Vec<String> = body
        .get("models")
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(|m| m.get("name").and_then(Value::as_str).map(str::to_string))
                .collect()
        })
        .unwrap_or_default();

    models.sort();
    Ok(models)
}

pub async fn embed(
    client: &reqwest::Client,
    config: &ProviderConfig,
    text: &str,
) -> Result<Vec<f32>, String> {
    let model = config
        .embedding_model
        .as_deref()
        .unwrap_or("nomic-embed-text");

    let response = client
        .post(format!("{}/api/embeddings", config.base()))
        .json(&json!({ "model": model, "prompt": text }))
        .send()
        .await
        // Сюда попадаем, когда до Ollama вообще не достучались. Голая ошибка
        // reqwest («error sending request for url…») пользователю ничего не
        // говорит — подсказываем, что именно проверить.
        .map_err(|_| {
            format!(
                "Ollama не отвечает на {}. Запустите его (команда `ollama serve` \
                 или приложение Ollama) и убедитесь, что установлена модель \
                 эмбеддингов: ollama pull {}",
                config.base(),
                model
            )
        })?;

    if !response.status().is_success() {
        // Раньше на ЛЮБОЙ неуспех предлагалось `ollama pull`. Это сбивало с
        // толку: переполнение контекста (500) выглядело как отсутствующая
        // модель. Причины разные, и подсказки должны быть разными.
        let status = response.status();
        let detail = sse::error_from_response(response, "Ollama").await;

        if detail.contains("context length") || detail.contains("input length") {
            return Err(format!(
                "{} Фрагмент не влезает в контекст модели «{}». Размер чанка \
                 задан в СИМВОЛАХ, а контекст модели — в токенах, и на кириллице \
                 один символ часто равен токену. Уменьшите размер чанка или \
                 возьмите модель с большим контекстом.",
                detail, model
            ));
        }
        if status == reqwest::StatusCode::NOT_FOUND {
            return Err(format!(
                "{} Модель «{}» не установлена — выполните: ollama pull {}",
                detail, model, model
            ));
        }
        return Err(detail);
    }

    let body: Value = response.json().await.map_err(|e| e.to_string())?;
    body.get("embedding")
        .and_then(Value::as_array)
        .map(|arr| arr.iter().filter_map(Value::as_f64).map(|v| v as f32).collect())
        .ok_or_else(|| "Ответ эмбеддинга не содержит вектора.".to_string())
}
