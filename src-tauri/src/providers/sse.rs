//! Разбор потоковых ответов: SSE (OpenAI/Anthropic/Gemini) и NDJSON (Ollama).

use futures_util::StreamExt;

/// Одно SSE-событие.
pub struct SseEvent {
    pub event: Option<String>,
    pub data: String,
}

/// Читает тело ответа и вызывает `on_event` для каждого распарсенного SSE-события.
/// `data: [DONE]` фильтруется здесь же.
pub async fn read_sse<F>(response: reqwest::Response, mut on_event: F) -> Result<(), String>
where
    F: FnMut(SseEvent) -> Result<(), String>,
{
    let mut stream = response.bytes_stream();
    let mut buffer = String::new();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| format!("Обрыв потока: {}", e))?;
        buffer.push_str(&String::from_utf8_lossy(&chunk));

        // События разделены пустой строкой. Нормализуем CRLF, чтобы делитель был один.
        while let Some(idx) = find_boundary(&buffer) {
            let (raw, rest) = buffer.split_at(idx);
            let raw = raw.to_string();
            buffer = rest.trim_start_matches(['\r', '\n']).to_string();

            if let Some(event) = parse_block(&raw) {
                if event.data.trim() == "[DONE]" {
                    return Ok(());
                }
                on_event(event)?;
            }
        }
    }

    // Хвост без завершающего разделителя.
    if let Some(event) = parse_block(&buffer) {
        if event.data.trim() != "[DONE]" {
            on_event(event)?;
        }
    }

    Ok(())
}

fn find_boundary(buffer: &str) -> Option<usize> {
    let lf = buffer.find("\n\n");
    let crlf = buffer.find("\r\n\r\n");
    match (lf, crlf) {
        (Some(a), Some(b)) => Some(a.min(b)),
        (Some(a), None) => Some(a),
        (None, Some(b)) => Some(b),
        (None, None) => None,
    }
}

fn parse_block(raw: &str) -> Option<SseEvent> {
    let mut event = None;
    let mut data_lines: Vec<&str> = Vec::new();

    for line in raw.lines() {
        let line = line.trim_end_matches('\r');
        if let Some(rest) = line.strip_prefix("event:") {
            event = Some(rest.trim().to_string());
        } else if let Some(rest) = line.strip_prefix("data:") {
            data_lines.push(rest.strip_prefix(' ').unwrap_or(rest));
        }
        // Комментарии (": heartbeat") и прочие поля игнорируем.
    }

    if data_lines.is_empty() {
        return None;
    }

    Some(SseEvent {
        event,
        data: data_lines.join("\n"),
    })
}

/// Читает NDJSON-поток (Ollama): по одному JSON-объекту на строку.
pub async fn read_ndjson<F>(response: reqwest::Response, mut on_line: F) -> Result<(), String>
where
    F: FnMut(serde_json::Value) -> Result<(), String>,
{
    let mut stream = response.bytes_stream();
    let mut buffer = String::new();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| format!("Обрыв потока: {}", e))?;
        buffer.push_str(&String::from_utf8_lossy(&chunk));

        while let Some(idx) = buffer.find('\n') {
            let line = buffer[..idx].trim().to_string();
            buffer = buffer[idx + 1..].to_string();
            if line.is_empty() {
                continue;
            }
            let value: serde_json::Value = serde_json::from_str(&line)
                .map_err(|e| format!("Некорректный JSON от Ollama: {}", e))?;
            on_line(value)?;
        }
    }

    let tail = buffer.trim();
    if !tail.is_empty() {
        let value: serde_json::Value =
            serde_json::from_str(tail).map_err(|e| format!("Некорректный JSON от Ollama: {}", e))?;
        on_line(value)?;
    }

    Ok(())
}

/// Превращает неуспешный ответ в читаемую ошибку.
pub async fn error_from_response(response: reqwest::Response, provider: &str) -> String {
    let status = response.status();
    let body = response.text().await.unwrap_or_default();

    // Провайдеры кладут человекочитаемое сообщение в разные места — достаём, что найдём.
    let detail = serde_json::from_str::<serde_json::Value>(&body)
        .ok()
        .and_then(|v| {
            v.pointer("/error/message")
                .or_else(|| v.pointer("/error"))
                .or_else(|| v.pointer("/message"))
                .and_then(|m| m.as_str().map(str::to_string))
        })
        .unwrap_or_else(|| body.chars().take(400).collect());

    match status.as_u16() {
        401 | 403 => format!("{}: неверный или отсутствующий API-ключ ({}).", provider, status),
        404 => format!("{}: модель или эндпоинт не найдены ({}). {}", provider, status, detail),
        429 => format!("{}: превышен лимит запросов. {}", provider, detail),
        _ => format!("{} вернул {}: {}", provider, status, detail),
    }
}
