//! Инструменты агента: их описания для модели и локальное выполнение.
//!
//! Это заменяет прежний парсинг `@@ACTION: ...@@` регулярками. Модель через
//! нативный tool-calling запрашивает инструмент, мы выполняем его здесь и
//! возвращаем результат обратно в диалог — как и положено агентному циклу.

use crate::providers::ToolDef;
use serde_json::{json, Value};
use std::fs;
use std::path::Path;
use std::process::Command;

/// Описания всех доступных инструментов для передачи модели.
pub fn definitions() -> Vec<ToolDef> {
    vec![
        ToolDef {
            name: "list_files".into(),
            description: "Показать файлы и папки в указанной директории.".into(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Путь к директории." }
                },
                "required": ["path"]
            }),
        },
        ToolDef {
            name: "read_file".into(),
            description: "Прочитать содержимое текстового файла.".into(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Путь к файлу." }
                },
                "required": ["path"]
            }),
        },
        ToolDef {
            name: "write_file".into(),
            description: "Создать файл или перезаписать его новым содержимым.".into(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Путь к файлу." },
                    "content": { "type": "string", "description": "Содержимое файла." }
                },
                "required": ["path", "content"]
            }),
        },
        ToolDef {
            name: "get_current_dir".into(),
            description: "Получить текущую рабочую директорию приложения.".into(),
            parameters: json!({ "type": "object", "properties": {} }),
        },
        ToolDef {
            name: "list_processes".into(),
            description: "Получить список запущенных процессов системы (первые 15).".into(),
            parameters: json!({ "type": "object", "properties": {} }),
        },
        ToolDef {
            name: "search_documents".into(),
            description:
                "Поиск по проиндексированным вложенным документам (RAG). Используй, когда пользователь спрашивает о содержимом прикреплённых файлов.".into(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "query": { "type": "string", "description": "Поисковый запрос." }
                },
                "required": ["query"]
            }),
        },
    ]
}

/// Инструменты, которые исполняются синхронно и не требуют доступа к БД/сети.
/// `search_documents` обрабатывается отдельно в вызывающем коде (нужен async + БД).
pub fn is_local(name: &str) -> bool {
    matches!(
        name,
        "list_files" | "read_file" | "write_file" | "get_current_dir" | "list_processes"
    )
}

/// Выполнить локальный инструмент. Возвращает (текст результата, признак ошибки).
pub fn run_local(name: &str, args: &Value) -> (String, bool) {
    match name {
        "list_files" => match args.get("path").and_then(Value::as_str) {
            Some(path) => match list_files(path) {
                Ok(out) => (out, false),
                Err(e) => (e, true),
            },
            None => ("Не указан параметр path.".into(), true),
        },

        "read_file" => match args.get("path").and_then(Value::as_str) {
            Some(path) => match fs::read_to_string(path) {
                Ok(content) => {
                    // Ограничиваем, чтобы не переполнить контекст модели.
                    let trimmed: String = content.chars().take(8000).collect();
                    (trimmed, false)
                }
                Err(e) => (format!("Не удалось прочитать {}: {}", path, e), true),
            },
            None => ("Не указан параметр path.".into(), true),
        },

        "write_file" => {
            let path = args.get("path").and_then(Value::as_str);
            let content = args.get("content").and_then(Value::as_str);
            match (path, content) {
                (Some(path), Some(content)) => match fs::write(path, content) {
                    Ok(_) => (format!("Файл записан: {}", path), false),
                    Err(e) => (format!("Не удалось записать {}: {}", path, e), true),
                },
                _ => ("Нужны параметры path и content.".into(), true),
            }
        }

        "get_current_dir" => match std::env::current_dir() {
            Ok(path) => (path.to_string_lossy().to_string(), false),
            Err(e) => (format!("Ошибка: {}", e), true),
        },

        "list_processes" => (list_processes(), false),

        other => (format!("Неизвестный инструмент: {}", other), true),
    }
}

fn list_files(path: &str) -> Result<String, String> {
    let dir = Path::new(path);
    if !dir.exists() {
        return Err(format!("Директория не существует: {}", path));
    }

    let mut entries = Vec::new();
    let read = fs::read_dir(dir).map_err(|e| format!("Не удалось прочитать {}: {}", path, e))?;
    for entry in read.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        let kind = if entry.path().is_dir() { "[DIR] " } else { "[FILE]" };
        entries.push(format!("{} {}", kind, name));
    }

    if entries.is_empty() {
        return Ok("Директория пуста.".into());
    }
    entries.sort();
    Ok(entries.join("\n"))
}

fn list_processes() -> String {
    #[cfg(target_os = "windows")]
    let output = Command::new("tasklist").output();

    #[cfg(not(target_os = "windows"))]
    let output = Command::new("ps").args(["-eo", "pid,comm,%mem"]).output();

    match output {
        Ok(o) => String::from_utf8_lossy(&o.stdout)
            .lines()
            .skip(3)
            .take(15)
            .collect::<Vec<_>>()
            .join("\n"),
        Err(e) => format!("Не удалось получить список процессов: {}", e),
    }
}
