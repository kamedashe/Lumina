#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use rusqlite::{params, Connection, Result as SqlResult};
use rquickjs::{Context, Runtime, prelude::*};
use scraper::{Html, Selector};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use std::process::Command;
use tauri::Manager;
// Импортируем библиотеку для размытия
use window_vibrancy::apply_acrylic;

// --- СТРУКТУРЫ OLLAMA (Те же, что и были) ---
#[derive(Serialize)]
struct OllamaRequest {
    model: String,
    prompt: String,
    stream: bool,
    options: Option<OllamaOptions>,
}

#[derive(Serialize)]
struct OllamaOptions {
    temperature: f32,
}

#[derive(Deserialize, Debug)]
struct OllamaResponse {
    response: String,
}

#[derive(Deserialize, Debug)]
struct OllamaModelList {
    models: Vec<OllamaModel>,
}

#[derive(Deserialize, Debug)]
struct OllamaModel {
    name: String,
}

#[derive(Serialize)]
struct OllamaEmbeddingRequest {
    model: String,
    prompt: String,
}

#[derive(Deserialize, Debug)]
struct OllamaEmbeddingResponse {
    embedding: Vec<f64>,
}



// --- КОМАНДЫ (Те же, что и были) ---
#[tauri::command]
async fn chat_with_ollama(
    prompt: String,
    model: String,
    temperature: Option<f32>,
) -> Result<String, String> {
    let client = reqwest::Client::new();
    let request = OllamaRequest {
        model: model,
        prompt: prompt,
        stream: false,
        options: temperature.map(|t| OllamaOptions { temperature: t }),
    };
    let res = client
        .post("http://localhost:11434/api/generate")
        .json(&request)
        .send()
        .await
        .map_err(|e| format!("Connection error: {}", e))?;

    if res.status().is_success() {
        let response_body: OllamaResponse = res.json().await.map_err(|e| e.to_string())?;
        Ok(response_body.response)
    } else {
        let status = res.status();
        let body = res.text().await.unwrap_or_default();
        if status == 400 {
            return Err("Ollama Error 400: Bad Request. You are likely using an embedding-only model (like 'nomic-embed-text') for chat. Please switch to a text model (e.g., 'mistral', 'llama3').".to_string());
        }
        Err(format!("Ollama Error {}: {}", status, body))
    }
}

#[tauri::command]
async fn get_ollama_models() -> Result<Vec<String>, String> {
    let client = reqwest::Client::new();
    let res = client
        .get("http://localhost:11434/api/tags")
        .send()
        .await
        .map_err(|e| {
            format!(
                "Could not connect to Ollama: {}. Make sure it is running.",
                e
            )
        })?;

    if res.status().is_success() {
        let list: OllamaModelList = res
            .json()
            .await
            .map_err(|e| format!("Failed to parse models: {}", e))?;
        Ok(list.models.into_iter().map(|m| m.name).collect())
    } else {
        Err(format!("Ollama returned error: {}", res.status()))
    }
}

#[tauri::command]
async fn get_system_processes() -> Vec<String> {
    let output = Command::new("tasklist").output();
    match output {
        Ok(o) => String::from_utf8_lossy(&o.stdout)
            .lines()
            .skip(3)
            .take(10)
            .map(|s| s.to_string())
            .collect(),
        Err(_) => vec!["Error".to_string()],
    }
}

// --- ИНИЦИАЛИЗАЦИЯ БД ---
fn init_db(app: &tauri::AppHandle) -> SqlResult<Connection> {
    let app_dir = app.path().app_data_dir().expect("Failed to get app data dir");
    if !app_dir.exists() {
        std::fs::create_dir_all(&app_dir).expect("Failed to create app data dir");
    }
    let db_path = app_dir.join("lumina.db");
    
    let conn = Connection::open(db_path)?;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS documents (
            id INTEGER PRIMARY KEY,
            path TEXT NOT NULL,
            content TEXT NOT NULL,
            embedding BLOB
        )",
        [],
    )?;
    Ok(conn)
}

async fn get_embedding(text: &str) -> Result<Vec<f64>, String> {
    let client = reqwest::Client::new();
    println!("Requesting embedding for text (first 50 chars): {}", &text.chars().take(50).collect::<String>());
    
    let res = client
        .post("http://localhost:11434/api/embeddings")
        .json(&OllamaEmbeddingRequest {
            model: "nomic-embed-text".to_string(),
            prompt: text.to_string(),
        })
        .send()
        .await
        .map_err(|e| {
            println!("Embedding connection error: {}", e);
            format!("Connection error: {}. Make sure Ollama is running.", e)
        })?;

    if res.status().is_success() {
        let body: OllamaEmbeddingResponse = res.json().await.map_err(|e| {
            println!("Embedding parse error: {}", e);
            e.to_string()
        })?;
        Ok(body.embedding)
    } else {
        let status = res.status();
        let err_body = res.text().await.unwrap_or_default();
        println!("Embedding API error: {} - {}", status, err_body);
        Err(format!("Embedding model 'nomic-embed-text' not found or error: {}. Try running 'ollama pull nomic-embed-text'", err_body))
    }
}

fn chunk_text(text: &str, size: usize, overlap: usize) -> Vec<String> {
    let mut chunks = Vec::new();
    let chars: Vec<char> = text.chars().collect();
    if chars.is_empty() {
        return chunks;
    }
    
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

#[tauri::command]
async fn process_documents(app: tauri::AppHandle, paths: Vec<String>) -> Result<String, String> {
    let conn = init_db(&app).map_err(|e| e.to_string())?;
    let mut total_chunks = 0;

    for path_str in paths {
        let path = Path::new(&path_str);
        if !path.exists() {
            continue;
        }

        let extension = path
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_lowercase();

        let content = match extension.as_str() {
            "pdf" => pdf_extract::extract_text(&path).unwrap_or_else(|e| {
                println!("Failed to extract PDF text from {}: {}", path_str, e);
                String::new()
            }),
            "txt" | "md" | "json" | "rs" | "ts" | "tsx" | "js" => {
                fs::read_to_string(&path).unwrap_or_default()
            }
            _ => continue,
        };

        if content.trim().is_empty() {
            continue;
        }

        // Разбиваем текст на чанки по ~1000 символов с перекрытием 200 символов
        let chunks = chunk_text(&content, 1000, 200);
        
        for chunk in chunks {
            // Для каждого чанка получаем свой эмбеддинг
            let embedding_res = get_embedding(&chunk).await;
            
            let embedding_json = match embedding_res {
                Ok(vec) => serde_json::to_string(&vec).unwrap_or("[]".to_string()),
                Err(e) => {
                    println!("Embedding failed for chunk: {}", e);
                    "[]".to_string()
                }
            };

            conn.execute(
                "INSERT INTO documents (path, content, embedding) VALUES (?1, ?2, ?3)",
                params![path_str, chunk, embedding_json],
            )
            .map_err(|e| e.to_string())?;
            
            total_chunks += 1;
        }
    }

    Ok(format!(
        "Indexed {} text chunks from documents into local DB.",
        total_chunks
    ))
}

fn cosine_similarity(a: &[f64], b: &[f64]) -> f64 {
    let dot_product: f64 = a.iter().zip(b).map(|(x, y)| x * y).sum();
    let norm_a: f64 = a.iter().map(|x| x.powi(2)).sum::<f64>().sqrt();
    let norm_b: f64 = b.iter().map(|x| x.powi(2)).sum::<f64>().sqrt();
    if norm_a == 0.0 || norm_b == 0.0 {
        0.0
    } else {
        dot_product / (norm_a * norm_b)
    }
}

#[tauri::command]
async fn search_documents(app: tauri::AppHandle, query: String) -> Result<String, String> {
    let conn = init_db(&app).map_err(|e| e.to_string())?;
    let query_embedding = get_embedding(&query).await?;

    let mut stmt = conn
        .prepare("SELECT content, embedding FROM documents")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |row| {
            let content: String = row.get(0)?;
            let embedding_json: String = row.get(1)?;
            let embedding: Vec<f64> = serde_json::from_str(&embedding_json).unwrap_or_default();
            Ok((content, embedding))
        })
        .map_err(|e| e.to_string())?;

    let mut results: Vec<(String, f64)> = Vec::new();

    for row in rows {
        if let Ok((content, embedding)) = row {
            if !embedding.is_empty() {
                let score = cosine_similarity(&query_embedding, &embedding);
                results.push((content, score));
            }
        }
    }

    // Сортируем по убыванию релевантности
    results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    // Берем топ-3
    let context = results
        .iter()
        .take(3)
        .map(|(c, _)| c.clone())
        .collect::<Vec<String>>()
        .join("\n---\n");

    Ok(context)
}

#[tauri::command]
async fn run_plugin(code: String) -> Result<String, String> {
    let rt = Runtime::new().map_err(|e: rquickjs::Error| e.to_string())?;
    let context = Context::full(&rt).map_err(|e: rquickjs::Error| e.to_string())?;

    let result = context.with(|ctx| {
        // Прокидываем console.log
        let global = ctx.globals();
        
        global.set("print", Func::new(|msg: String| {
            println!("Plugin says: {}", msg);
        })).map_err(|e: rquickjs::Error| e.to_string())?;

        let res: rquickjs::Value = ctx.eval(code).map_err(|e: rquickjs::Error| e.to_string())?;
        
        let global = ctx.globals();
        let string_ctor: rquickjs::Function = global.get("String").map_err(|e: rquickjs::Error| e.to_string())?;
        let result_str: rquickjs::String = string_ctor.call((res,)).map_err(|e: rquickjs::Error| e.to_string())?;
        
        Ok::<String, String>(result_str.to_string().map_err(|e: rquickjs::Error| e.to_string())?)
    });

    result
}

#[tauri::command]
fn get_current_dir() -> Result<String, String> {
    std::env::current_dir()
        .map(|path| path.to_string_lossy().to_string())
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn list_dir_contents(path: String) -> Result<Vec<String>, String> {
    let dir_path = Path::new(&path);
    if !dir_path.exists() {
        return Err(format!("Directory does not exist: {}", path));
    }
    
    let mut entries = Vec::new();
    match fs::read_dir(dir_path) {
        Ok(read_dir) => {
            for entry in read_dir {
                if let Ok(entry) = entry {
                    let file_name = entry.file_name().to_string_lossy().to_string();
                    let file_type = if entry.path().is_dir() { "[DIR]" } else { "[FILE]" };
                    entries.push(format!("{} {}", file_type, file_name));
                }
            }
            Ok(entries)
        }
        Err(e) => Err(format!("Failed to read directory: {}", e)),
    }
}

#[tauri::command]
async fn generate_report() -> Result<String, String> {
    let current_dir = std::env::current_dir().map_err(|e| e.to_string())?;
    let mut report = String::new();
    report.push_str(&format!("Report for directory: {:?}\n\n", current_dir));

    fn visit_dirs(dir: &Path, prefix: &str, report: &mut String) -> std::io::Result<()> {
        if dir.is_dir() {
            let mut entries = fs::read_dir(dir)?
                .map(|res| res.map(|e| e.path()))
                .collect::<Result<Vec<_>, std::io::Error>>()?;
            
            // Sort entries for consistent output
            entries.sort();

            for (i, path) in entries.iter().enumerate() {
                let file_name = path.file_name().unwrap_or_default().to_string_lossy();
                
                // Skip ignored directories
                if file_name == "node_modules" || file_name == "target" || file_name == ".git" || file_name == ".roo" {
                    continue;
                }

                let is_last = i == entries.len() - 1;
                let connector = if is_last { "└── " } else { "├── " };
                
                report.push_str(&format!("{}{}{}\n", prefix, connector, file_name));

                if path.is_dir() {
                    let new_prefix = format!("{}{}", prefix, if is_last { "    " } else { "│   " });
                    visit_dirs(path, &new_prefix, report)?;
                }
            }
        }
        Ok(())
    }

    visit_dirs(&current_dir, "", &mut report).map_err(|e| e.to_string())?;
    Ok(report)
}

// --- MAIN С НАСТРОЙКОЙ ДИЗАЙНА ---
fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
            let window = app
                .get_webview_window("main")
                .ok_or_else(|| tauri::Error::AssetNotFound("Main window not found".to_string()))?;

            // ВКЛЮЧАЕМ РАЗМЫТИЕ (Только для Windows)
            #[cfg(target_os = "windows")]
            {
                // Попробуй apply_acrylic() для сильного размытия или apply_blur() для мягкого
                // apply_mica() - это стиль Windows 11 (очень стильно, но работает только на Win11)

                // Вариант 1: Acrylic (Сильное размытие, выглядит дорого)
                let _ = apply_acrylic(&window, Some((20, 20, 20, 100)));

                // Если Acrylic глючит, раскомментируй строку ниже, а верхнюю убери:
                // let _ = apply_blur(&window, Some((10, 10, 10, 0)));
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            chat_with_ollama,
            get_ollama_models,
            get_system_processes,
            process_documents,
            search_documents,
            run_plugin,
            list_dir_contents,
            get_current_dir,
            generate_report
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
