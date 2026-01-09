#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use tauri::Manager;
use serde::{Deserialize, Serialize};
use std::process::Command;
use scraper::{Html, Selector};
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

#[derive(Deserialize, Debug)]
struct DuckDuckGoResponse {
    #[serde(rename = "AbstractText")]
    abstract_text: String,
    #[serde(rename = "RelatedTopics")]
    related_topics: Vec<DuckDuckGoTopic>,
}

#[derive(Deserialize, Debug)]
struct DuckDuckGoTopic {
    #[serde(rename = "Text")]
    text: Option<String>,
}

#[tauri::command]
async fn web_search(query: String) -> Result<String, String> {
    let client = reqwest::Client::new();
    // Используем HTML версию DuckDuckGo для получения реальных результатов
    let url = format!("https://html.duckduckgo.com/html/?q={}", query);
    
    let res = client.get(&url)
        .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
        .send()
        .await
        .map_err(|e| format!("Search request failed: {}", e))?;

    if res.status().is_success() {
        let body = res.text().await.map_err(|e| format!("Failed to get body: {}", e))?;
        
        let mut search_results = String::new();
        
        // Ограничиваем область видимости document, чтобы он не жил во время await
        {
            let document = Html::parse_document(&body);
            
            // Селекторы для HTML версии DuckDuckGo
            let result_selector = Selector::parse(".result").map_err(|e| e.to_string())?;
            let title_selector = Selector::parse(".result__a").map_err(|e| e.to_string())?;
            let snippet_selector = Selector::parse(".result__snippet").map_err(|e| e.to_string())?;

            for (i, element) in document.select(&result_selector).take(5).enumerate() {
                let title = element
                    .select(&title_selector)
                    .next()
                    .map(|e| e.text().collect::<String>())
                    .unwrap_or_default();
                
                let snippet = element
                    .select(&snippet_selector)
                    .next()
                    .map(|e| e.text().collect::<String>())
                    .unwrap_or_default();

                if !title.is_empty() {
                    search_results.push_str(&format!("{}. TITLE: {}\nSNIPPET: {}\n\n", i + 1, title.trim(), snippet.trim()));
                }
            }
        } // document умирает здесь

        if search_results.is_empty() {
            // Если HTML парсинг не удался, пробуем старый API как бэкап
            let backup_url = format!("https://api.duckduckgo.com/?q={}&format=json&no_html=1", query);
            let backup_res = client.get(&backup_url).send().await.map_err(|e| e.to_string())?;
            if backup_res.status().is_success() {
                let ddg_json: DuckDuckGoResponse = backup_res.json().await.map_err(|e| e.to_string())?;
                if !ddg_json.abstract_text.is_empty() {
                    return Ok(ddg_json.abstract_text);
                }
            }
            Ok("No results found.".to_string())
        } else {
            Ok(search_results)
        }
    } else {
        Err(format!("Search Error: {}", res.status()))
    }
}

// --- КОМАНДЫ (Те же, что и были) ---
#[tauri::command]
async fn chat_with_ollama(prompt: String, model: String, temperature: Option<f32>) -> Result<String, String> {
    let client = reqwest::Client::new();
    let request = OllamaRequest {
        model: model,
        prompt: prompt,
        stream: false,
        options: temperature.map(|t| OllamaOptions { temperature: t }),
    };
    let res = client.post("http://localhost:11434/api/generate")
        .json(&request)
        .send()
        .await
        .map_err(|e| format!("Connection error: {}", e))?;

    if res.status().is_success() {
        let response_body: OllamaResponse = res.json().await.map_err(|e| e.to_string())?;
        Ok(response_body.response)
    } else {
        Err(format!("Ollama Error: {}", res.status()))
    }
}

#[tauri::command]
async fn get_ollama_models() -> Result<Vec<String>, String> {
    let client = reqwest::Client::new();
    let res = client.get("http://localhost:11434/api/tags")
        .send()
        .await
        .map_err(|e| format!("Could not connect to Ollama: {}. Make sure it is running.", e))?;
        
    if res.status().is_success() {
        let list: OllamaModelList = res.json().await.map_err(|e| format!("Failed to parse models: {}", e))?;
        Ok(list.models.into_iter().map(|m| m.name).collect())
    } else {
        Err(format!("Ollama returned error: {}", res.status()))
    }
}

#[tauri::command]
async fn get_system_processes() -> Vec<String> {
    let output = Command::new("tasklist").output();
    match output {
        Ok(o) => String::from_utf8_lossy(&o.stdout).lines().skip(3).take(10).map(|s| s.to_string()).collect(),
        Err(_) => vec!["Error".to_string()]
    }
}

// --- MAIN С НАСТРОЙКОЙ ДИЗАЙНА ---
fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
            let window = app.get_webview_window("main")
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
        .invoke_handler(tauri::generate_handler![chat_with_ollama, get_ollama_models, get_system_processes, web_search])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}