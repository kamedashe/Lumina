//! OpenAI-совместимый клиент: OpenAI, OpenRouter, Groq, Together, LM Studio, vLLM.
//! Все они говорят на /chat/completions, поэтому различаются только base_url и ключом.

use super::sse::{self, read_sse};
use super::{AssistantTurn, ChatMessage, ProviderConfig, Role, StreamEvent, ToolCall, ToolDef};
use serde_json::{json, Value};
use std::collections::BTreeMap;
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
                        json!({
                            "id": c.id,
                            "type": "function",
                            "function": { "name": c.name, "arguments": c.arguments }
                        })
                    })
                    .collect(),
            );
            // При наличии tool_calls контент должен быть null, иначе часть шлюзов ругается.
            if m.content.is_empty() {
                obj["content"] = Value::Null;
            }
        }

        if let Some(id) = &m.tool_call_id {
            obj["tool_call_id"] = json!(id);
        }

        out.push(obj);
    }

    out
}

fn to_wire_tools(tools: &[ToolDef]) -> Vec<Value> {
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
        "messages": to_wire_messages(system, messages),
        "stream": true,
        "temperature": config.temperature,
    });

    if !tools.is_empty() {
        body["tools"] = Value::Array(to_wire_tools(tools));
    }

    let mut request = client
        .post(format!("{}/chat/completions", config.base()))
        .json(&body);

    // Локальные серверы (LM Studio, vLLM, Ollama-shim) часто работают без ключа.
    if let Some(key) = config.api_key.as_deref().filter(|k| !k.trim().is_empty()) {
        request = request.bearer_auth(key);
    }

    let response = request
        .send()
        .await
        .map_err(|e| format!("Не удалось подключиться к {}: {}", config.label, e))?;

    if !response.status().is_success() {
        return Err(sse::error_from_response(response, &config.label).await);
    }

    let mut turn = AssistantTurn::default();
    // Куски tool_calls приходят с индексом, а не с id — собираем по индексу.
    let mut partial: BTreeMap<u64, ToolCall> = BTreeMap::new();

    read_sse(response, |event| {
        let chunk: Value = match serde_json::from_str(&event.data) {
            Ok(v) => v,
            Err(_) => return Ok(()), // keep-alive и прочий шум
        };

        let Some(choice) = chunk.pointer("/choices/0") else {
            return Ok(());
        };

        if let Some(text) = choice.pointer("/delta/content").and_then(Value::as_str) {
            if !text.is_empty() {
                turn.text.push_str(text);
                let _ = channel.send(StreamEvent::Text {
                    delta: text.to_string(),
                });
            }
        }

        if let Some(calls) = choice.pointer("/delta/tool_calls").and_then(Value::as_array) {
            for call in calls {
                let index = call.get("index").and_then(Value::as_u64).unwrap_or(0);
                let entry = partial.entry(index).or_insert_with(|| ToolCall {
                    id: String::new(),
                    name: String::new(),
                    arguments: String::new(),
                });

                if let Some(id) = call.get("id").and_then(Value::as_str) {
                    entry.id = id.to_string();
                }
                if let Some(name) = call.pointer("/function/name").and_then(Value::as_str) {
                    entry.name.push_str(name);
                    if !entry.name.is_empty() {
                        let _ = channel.send(StreamEvent::ToolCallStart {
                            id: entry.id.clone(),
                            name: entry.name.clone(),
                        });
                    }
                }
                if let Some(args) = call.pointer("/function/arguments").and_then(Value::as_str) {
                    entry.arguments.push_str(args);
                }
            }
        }

        if let Some(reason) = choice.get("finish_reason").and_then(Value::as_str) {
            turn.stop_reason = reason.to_string();
        }

        Ok(())
    })
    .await?;

    turn.tool_calls = partial
        .into_values()
        .map(|mut c| {
            if c.id.is_empty() {
                c.id = super::new_call_id(&c.name);
            }
            if c.arguments.trim().is_empty() {
                c.arguments = "{}".to_string();
            }
            c
        })
        .filter(|c| !c.name.is_empty())
        .collect();

    Ok(turn)
}

pub async fn list_models(
    client: &reqwest::Client,
    config: &ProviderConfig,
) -> Result<Vec<String>, String> {
    let mut request = client.get(format!("{}/models", config.base()));
    if let Some(key) = config.api_key.as_deref().filter(|k| !k.trim().is_empty()) {
        request = request.bearer_auth(key);
    }

    let response = request
        .send()
        .await
        .map_err(|e| format!("Не удалось получить модели {}: {}", config.label, e))?;

    if !response.status().is_success() {
        return Err(sse::error_from_response(response, &config.label).await);
    }

    let body: Value = response.json().await.map_err(|e| e.to_string())?;
    let mut models: Vec<String> = body
        .get("data")
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(|m| m.get("id").and_then(Value::as_str).map(str::to_string))
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
        .unwrap_or("text-embedding-3-small");

    let mut request = client
        .post(format!("{}/embeddings", config.base()))
        .json(&json!({ "model": model, "input": text }));

    if let Some(key) = config.api_key.as_deref().filter(|k| !k.trim().is_empty()) {
        request = request.bearer_auth(key);
    }

    let response = request
        .send()
        .await
        .map_err(|e| format!("Ошибка эмбеддинга ({}): {}", config.label, e))?;

    if !response.status().is_success() {
        return Err(sse::error_from_response(response, &config.label).await);
    }

    let body: Value = response.json().await.map_err(|e| e.to_string())?;
    body.pointer("/data/0/embedding")
        .and_then(Value::as_array)
        .map(|arr| arr.iter().filter_map(Value::as_f64).map(|v| v as f32).collect())
        .ok_or_else(|| "Ответ эмбеддинга не содержит вектора.".to_string())
}
