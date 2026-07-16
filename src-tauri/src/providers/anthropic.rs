//! Клиент Anthropic Messages API (/v1/messages).

use super::sse::{self, read_sse};
use super::{AssistantTurn, ChatMessage, ProviderConfig, Role, StreamEvent, ToolCall, ToolDef};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use tauri::ipc::Channel;

const API_VERSION: &str = "2023-06-01";

/// Потолок ответа. Со стримингом это безопасно для всех актуальных моделей
/// (у самой скромной, Haiku 4.5, лимит вывода 64K).
const MAX_TOKENS: u32 = 16000;

/// Начиная с Opus 4.7 семплирующие параметры удалены из API: `temperature`
/// возвращает 400. Sonnet 5 отвергает недефолтные значения. Для этих моделей
/// параметр не отправляем вообще, стилем управляет промпт.
fn supports_temperature(model: &str) -> bool {
    const NO_SAMPLING: [&str; 5] = [
        "claude-opus-4-8",
        "claude-opus-4-7",
        "claude-sonnet-5",
        "claude-fable-5",
        "claude-mythos-5",
    ];
    !NO_SAMPLING.iter().any(|m| model.starts_with(m))
}

/// Anthropic ждёт чередование user/assistant, а результаты инструментов — это
/// user-сообщение из tool_result-блоков. Поэтому подряд идущие ответы
/// инструментов надо схлопнуть в один ход.
fn to_wire_messages(messages: &[ChatMessage]) -> Result<Vec<Value>, String> {
    let mut out: Vec<Value> = Vec::new();

    for m in messages {
        match m.role {
            // Системный промпт передаётся отдельным полем — здесь его быть не должно.
            Role::System => continue,

            Role::User => out.push(json!({ "role": "user", "content": m.content })),

            Role::Assistant => {
                let mut blocks: Vec<Value> = Vec::new();
                if !m.content.trim().is_empty() {
                    blocks.push(json!({ "type": "text", "text": m.content }));
                }
                for call in &m.tool_calls {
                    let input: Value = serde_json::from_str(&call.arguments)
                        .unwrap_or_else(|_| json!({}));
                    blocks.push(json!({
                        "type": "tool_use",
                        "id": call.id,
                        "name": call.name,
                        "input": input
                    }));
                }
                if blocks.is_empty() {
                    continue;
                }
                out.push(json!({ "role": "assistant", "content": blocks }));
            }

            Role::Tool => {
                let id = m
                    .tool_call_id
                    .as_ref()
                    .ok_or("tool_result без tool_use_id")?;
                let block = json!({
                    "type": "tool_result",
                    "tool_use_id": id,
                    "content": m.content
                });

                // Дописываем в предыдущий user-ход, если он тоже состоит из tool_result.
                let appended = out
                    .last_mut()
                    .filter(|last| last["role"] == "user" && last["content"].is_array())
                    .map(|last| {
                        last["content"].as_array_mut().unwrap().push(block.clone());
                    })
                    .is_some();

                if !appended {
                    out.push(json!({ "role": "user", "content": [block] }));
                }
            }
        }
    }

    Ok(out)
}

fn to_wire_tools(tools: &[ToolDef]) -> Vec<Value> {
    tools
        .iter()
        .map(|t| {
            json!({
                "name": t.name,
                "description": t.description,
                "input_schema": t.parameters
            })
        })
        .collect()
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
        "max_tokens": MAX_TOKENS,
        "messages": to_wire_messages(messages)?,
        "stream": true,
    });

    if !system.trim().is_empty() {
        body["system"] = json!(system);
    }
    if supports_temperature(&config.model) {
        body["temperature"] = json!(config.temperature);
    }
    if !tools.is_empty() {
        body["tools"] = Value::Array(to_wire_tools(tools));
    }

    let response = client
        .post(format!("{}/v1/messages", config.base()))
        .header("x-api-key", config.key()?)
        .header("anthropic-version", API_VERSION)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("Не удалось подключиться к Anthropic: {}", e))?;

    if !response.status().is_success() {
        return Err(sse::error_from_response(response, "Anthropic").await);
    }

    let mut turn = AssistantTurn::default();
    // Блоки идентифицируются индексом; аргументы tool_use приходят кусками JSON.
    let mut blocks: BTreeMap<u64, ToolCall> = BTreeMap::new();

    read_sse(response, |event| {
        let payload: Value = match serde_json::from_str(&event.data) {
            Ok(v) => v,
            Err(_) => return Ok(()),
        };

        let kind = payload
            .get("type")
            .and_then(Value::as_str)
            .or(event.event.as_deref())
            .unwrap_or("");

        match kind {
            "content_block_start" => {
                let index = payload.get("index").and_then(Value::as_u64).unwrap_or(0);
                if payload.pointer("/content_block/type").and_then(Value::as_str)
                    == Some("tool_use")
                {
                    let id = payload
                        .pointer("/content_block/id")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string();
                    let name = payload
                        .pointer("/content_block/name")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string();

                    let _ = channel.send(StreamEvent::ToolCallStart {
                        id: id.clone(),
                        name: name.clone(),
                    });
                    blocks.insert(
                        index,
                        ToolCall {
                            id,
                            name,
                            arguments: String::new(),
                        },
                    );
                }
            }

            "content_block_delta" => {
                let index = payload.get("index").and_then(Value::as_u64).unwrap_or(0);
                match payload.pointer("/delta/type").and_then(Value::as_str) {
                    Some("text_delta") => {
                        if let Some(text) = payload.pointer("/delta/text").and_then(Value::as_str) {
                            turn.text.push_str(text);
                            let _ = channel.send(StreamEvent::Text {
                                delta: text.to_string(),
                            });
                        }
                    }
                    Some("input_json_delta") => {
                        if let Some(part) =
                            payload.pointer("/delta/partial_json").and_then(Value::as_str)
                        {
                            if let Some(call) = blocks.get_mut(&index) {
                                call.arguments.push_str(part);
                            }
                        }
                    }
                    // thinking_delta и прочие типы пока не показываем.
                    _ => {}
                }
            }

            "message_delta" => {
                if let Some(reason) = payload.pointer("/delta/stop_reason").and_then(Value::as_str) {
                    turn.stop_reason = reason.to_string();
                }
            }

            "error" => {
                let message = payload
                    .pointer("/error/message")
                    .and_then(Value::as_str)
                    .unwrap_or("неизвестная ошибка потока");
                return Err(format!("Anthropic: {}", message));
            }

            _ => {}
        }

        Ok(())
    })
    .await?;

    turn.tool_calls = blocks
        .into_values()
        .map(|mut c| {
            if c.arguments.trim().is_empty() {
                c.arguments = "{}".to_string();
            }
            c
        })
        .collect();

    Ok(turn)
}

pub async fn list_models(
    client: &reqwest::Client,
    config: &ProviderConfig,
) -> Result<Vec<String>, String> {
    let response = client
        .get(format!("{}/v1/models?limit=100", config.base()))
        .header("x-api-key", config.key()?)
        .header("anthropic-version", API_VERSION)
        .send()
        .await
        .map_err(|e| format!("Не удалось получить модели Anthropic: {}", e))?;

    if !response.status().is_success() {
        return Err(sse::error_from_response(response, "Anthropic").await);
    }

    let body: Value = response.json().await.map_err(|e| e.to_string())?;
    Ok(body
        .get("data")
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(|m| m.get("id").and_then(Value::as_str).map(str::to_string))
                .collect()
        })
        .unwrap_or_default())
}
