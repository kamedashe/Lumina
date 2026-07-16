//! Клиент Google Gemini (generateContent + function calling).

use super::sse::{self, read_sse};
use super::{AssistantTurn, ChatMessage, ProviderConfig, Role, StreamEvent, ToolCall, ToolDef};
use serde_json::{json, Value};
use tauri::ipc::Channel;

/// Gemini знает только роли "user" и "model"; ответы инструментов идут как user.
fn to_wire_contents(messages: &[ChatMessage]) -> Vec<Value> {
    let mut out: Vec<Value> = Vec::new();

    for m in messages {
        match m.role {
            Role::System => continue, // уходит в systemInstruction

            Role::User => out.push(json!({
                "role": "user",
                "parts": [{ "text": m.content }]
            })),

            Role::Assistant => {
                let mut parts: Vec<Value> = Vec::new();
                if !m.content.trim().is_empty() {
                    parts.push(json!({ "text": m.content }));
                }
                for call in &m.tool_calls {
                    let args: Value =
                        serde_json::from_str(&call.arguments).unwrap_or_else(|_| json!({}));
                    parts.push(json!({
                        "functionCall": { "name": call.name, "args": args }
                    }));
                }
                if parts.is_empty() {
                    continue;
                }
                out.push(json!({ "role": "model", "parts": parts }));
            }

            Role::Tool => {
                let name = m.tool_name.clone().unwrap_or_default();
                let part = json!({
                    "functionResponse": {
                        "name": name,
                        // Gemini требует объект — заворачиваем текстовый результат.
                        "response": { "result": m.content }
                    }
                });

                let appended = out
                    .last_mut()
                    .filter(|last| last["role"] == "user" && last["parts"].is_array())
                    .filter(|last| {
                        // Дописываем только к ходу, который сам состоит из ответов инструментов.
                        last["parts"]
                            .as_array()
                            .map(|p| p.iter().all(|x| x.get("functionResponse").is_some()))
                            .unwrap_or(false)
                    })
                    .map(|last| last["parts"].as_array_mut().unwrap().push(part.clone()))
                    .is_some();

                if !appended {
                    out.push(json!({ "role": "user", "parts": [part] }));
                }
            }
        }
    }

    out
}

fn to_wire_tools(tools: &[ToolDef]) -> Value {
    let declarations: Vec<Value> = tools
        .iter()
        .map(|t| {
            let mut decl = json!({
                "name": t.name,
                "description": t.description,
            });

            // Функции без параметров: Gemini отвергает пустой объект properties,
            // поэтому схему просто не передаём.
            let has_params = t
                .parameters
                .get("properties")
                .and_then(Value::as_object)
                .map(|p| !p.is_empty())
                .unwrap_or(false);

            if has_params {
                decl["parameters"] = sanitize_schema(&t.parameters);
            }

            decl
        })
        .collect();

    json!([{ "functionDeclarations": declarations }])
}

/// Gemini принимает подмножество JSON Schema и падает на незнакомых ключах.
fn sanitize_schema(schema: &Value) -> Value {
    match schema {
        Value::Object(map) => {
            let mut out = serde_json::Map::new();
            for (key, value) in map {
                if matches!(
                    key.as_str(),
                    "additionalProperties" | "$schema" | "definitions" | "$defs" | "default"
                ) {
                    continue;
                }
                out.insert(key.clone(), sanitize_schema(value));
            }
            Value::Object(out)
        }
        Value::Array(items) => Value::Array(items.iter().map(sanitize_schema).collect()),
        other => other.clone(),
    }
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
        "contents": to_wire_contents(messages),
        "generationConfig": { "temperature": config.temperature },
    });

    if !system.trim().is_empty() {
        body["systemInstruction"] = json!({ "parts": [{ "text": system }] });
    }
    if !tools.is_empty() {
        body["tools"] = to_wire_tools(tools);
    }

    let url = format!(
        "{}/v1beta/models/{}:streamGenerateContent?alt=sse",
        config.base(),
        config.model
    );

    let response = client
        .post(url)
        .header("x-goog-api-key", config.key()?)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("Не удалось подключиться к Gemini: {}", e))?;

    if !response.status().is_success() {
        return Err(sse::error_from_response(response, "Gemini").await);
    }

    let mut turn = AssistantTurn::default();

    read_sse(response, |event| {
        let payload: Value = match serde_json::from_str(&event.data) {
            Ok(v) => v,
            Err(_) => return Ok(()),
        };

        if let Some(message) = payload.pointer("/error/message").and_then(Value::as_str) {
            return Err(format!("Gemini: {}", message));
        }

        let Some(candidate) = payload.pointer("/candidates/0") else {
            return Ok(());
        };

        if let Some(parts) = candidate.pointer("/content/parts").and_then(Value::as_array) {
            for part in parts {
                if let Some(text) = part.get("text").and_then(Value::as_str) {
                    if !text.is_empty() {
                        turn.text.push_str(text);
                        let _ = channel.send(StreamEvent::Text {
                            delta: text.to_string(),
                        });
                    }
                }

                // В отличие от OpenAI, Gemini отдаёт вызов целиком одним куском.
                if let Some(call) = part.get("functionCall") {
                    let name = call
                        .get("name")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string();
                    let args = call.get("args").cloned().unwrap_or_else(|| json!({}));
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
        }

        if let Some(reason) = candidate.get("finishReason").and_then(Value::as_str) {
            turn.stop_reason = reason.to_string();
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
        .get(format!("{}/v1beta/models?pageSize=200", config.base()))
        .header("x-goog-api-key", config.key()?)
        .send()
        .await
        .map_err(|e| format!("Не удалось получить модели Gemini: {}", e))?;

    if !response.status().is_success() {
        return Err(sse::error_from_response(response, "Gemini").await);
    }

    let body: Value = response.json().await.map_err(|e| e.to_string())?;
    Ok(body
        .get("models")
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter(|m| {
                    // Оставляем только те, что умеют генерацию — эмбеддеры в чате бесполезны.
                    m.get("supportedGenerationMethods")
                        .and_then(Value::as_array)
                        .map(|methods| {
                            methods
                                .iter()
                                .any(|x| x.as_str() == Some("generateContent"))
                        })
                        .unwrap_or(false)
                })
                .filter_map(|m| m.get("name").and_then(Value::as_str))
                // API отдаёт "models/gemini-2.5-pro" — в запросе нужен голый id.
                .map(|n| n.trim_start_matches("models/").to_string())
                .collect()
        })
        .unwrap_or_default())
}

pub async fn embed(
    client: &reqwest::Client,
    config: &ProviderConfig,
    text: &str,
) -> Result<Vec<f32>, String> {
    let model = config
        .embedding_model
        .as_deref()
        .unwrap_or("text-embedding-004");

    let response = client
        .post(format!(
            "{}/v1beta/models/{}:embedContent",
            config.base(),
            model
        ))
        .header("x-goog-api-key", config.key()?)
        .json(&json!({
            "model": format!("models/{}", model),
            "content": { "parts": [{ "text": text }] }
        }))
        .send()
        .await
        .map_err(|e| format!("Ошибка эмбеддинга (Gemini): {}", e))?;

    if !response.status().is_success() {
        return Err(sse::error_from_response(response, "Gemini").await);
    }

    let body: Value = response.json().await.map_err(|e| e.to_string())?;
    body.pointer("/embedding/values")
        .and_then(Value::as_array)
        .map(|arr| arr.iter().filter_map(Value::as_f64).map(|v| v as f32).collect())
        .ok_or_else(|| "Ответ эмбеддинга не содержит вектора.".to_string())
}
