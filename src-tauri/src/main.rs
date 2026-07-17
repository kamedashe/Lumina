#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod providers;
mod rag;
mod tools;
mod vector;

use providers::{ChatMessage, ProviderConfig, StreamEvent};
use vector::{StoreCtx, VectorStoreConfig};
use rquickjs::{prelude::*, Context, Runtime};
use serde_json::Value;
use tauri::ipc::Channel;
use tauri::Manager;
use window_vibrancy::apply_acrylic;

/// Максимум итераций агентного цикла за один запрос пользователя —
/// защита от бесконечного дёргания инструментов.
const MAX_AGENT_STEPS: usize = 6;

fn http_client() -> reqwest::Client {
    reqwest::Client::builder()
        // Облачные модели с длинным ответом легко перекрывают дефолтные 30с.
        .timeout(std::time::Duration::from_secs(300))
        .build()
        .expect("Не удалось создать HTTP-клиент")
}

/// Основная команда: прогоняет агентный цикл и стримит события во фронтенд
/// через Channel. История ведётся на стороне фронтенда и передаётся целиком.
///
/// `embedding_provider` — чем считать эмбеддинги, если активный провайдер их
/// не умеет. Фронтенд подставляет сюда настроенного пользователем Ollama.
#[tauri::command]
async fn run_agent(
    app: tauri::AppHandle,
    config: ProviderConfig,
    store: VectorStoreConfig,
    embedding_provider: Option<ProviderConfig>,
    system: String,
    messages: Vec<ChatMessage>,
    attachments: Vec<String>,
    on_event: Channel<StreamEvent>,
) -> Result<(), String> {
    let client = http_client();
    let mut history = messages;

    // Если есть вложения — индексируем и подмешиваем контекст в последний вопрос.
    if !attachments.is_empty() {
        let _ = on_event.send(StreamEvent::Status {
            message: "Индексирую файлы…".into(),
        });

        // Для эмбеддингов у Anthropic своего эндпоинта нет — при необходимости
        // используем локальный Ollama как фолбэк.
        let embed_config = embedding_config(&config, &embedding_provider);

        if let Err(e) =
            rag::process_documents(&app, &client, &embed_config, &store, &attachments).await
        {
            let _ = on_event.send(StreamEvent::Error { message: e });
            let _ = on_event.send(StreamEvent::Done {
                stop_reason: "error".into(),
            });
            return Ok(());
        }

        if let Some(last) = history.last_mut() {
            let query = last.content.clone();
            if let Ok(context) =
                rag::search_documents(&app, &client, &embed_config, &store, &query, &attachments)
                    .await
            {
                if !context.trim().is_empty() {
                    last.content = format!(
                        "Контекст из прикреплённых файлов:\n{}\n\nВопрос пользователя: {}",
                        context, query
                    );
                }
            }
        }
    }

    let tool_defs = tools::definitions();

    let result: Result<(), String> = async {
        for _ in 0..MAX_AGENT_STEPS {
            let turn = providers::stream_turn(
                &client,
                &config,
                &system,
                &history,
                &tool_defs,
                &on_event,
            )
            .await?;

            if !turn.wants_tools() {
                let _ = on_event.send(StreamEvent::Done {
                    stop_reason: if turn.stop_reason.is_empty() {
                        "end_turn".into()
                    } else {
                        turn.stop_reason.clone()
                    },
                });
                return Ok(());
            }

            // Фиксируем ход ассистента с запрошенными вызовами.
            history.push(ChatMessage::assistant(turn.text.clone(), turn.tool_calls.clone()));

            // Выполняем каждый инструмент и возвращаем результат в диалог.
            for call in &turn.tool_calls {
                let args: Value = serde_json::from_str(&call.arguments).unwrap_or(Value::Null);

                let (content, is_error) = if tools::is_local(&call.name) {
                    tools::run_local(&call.name, &args)
                } else if call.name == "search_documents" {
                    let query = args
                        .get("query")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_string();
                    let embed_config = embedding_config(&config, &embedding_provider);
                    match rag::search_documents(&app, &client, &embed_config, &store, &query, &[])
                        .await
                    {
                        Ok(ctx) if ctx.trim().is_empty() => {
                            ("Ничего не найдено в проиндексированных документах.".into(), false)
                        }
                        Ok(ctx) => (ctx, false),
                        Err(e) => (e, true),
                    }
                } else {
                    (format!("Неизвестный инструмент: {}", call.name), true)
                };

                let _ = on_event.send(StreamEvent::ToolResult {
                    id: call.id.clone(),
                    name: call.name.clone(),
                    content: content.clone(),
                    is_error,
                });

                history.push(ChatMessage::tool_result(call, content));
            }
        }

        // Исчерпали лимит шагов — просим модель подвести итог без новых инструментов.
        let turn =
            providers::stream_turn(&client, &config, &system, &history, &[], &on_event).await?;
        let _ = on_event.send(StreamEvent::Done {
            stop_reason: if turn.stop_reason.is_empty() {
                "max_steps".into()
            } else {
                turn.stop_reason
            },
        });
        Ok(())
    }
    .await;

    if let Err(message) = result {
        let _ = on_event.send(StreamEvent::Error { message });
        let _ = on_event.send(StreamEvent::Done {
            stop_reason: "error".into(),
        });
    }

    Ok(())
}

/// Выбирает, чем считать эмбеддинги.
///
/// У Anthropic эндпоинта эмбеддингов нет вообще, поэтому для него нужен другой
/// провайдер. Приоритет — тот, что явно передал фронтенд (настроенный
/// пользователем Ollama с его base_url); localhost остаётся лишь последним
/// рубежом, когда ни одного подходящего провайдера не настроено.
fn embedding_config(
    config: &ProviderConfig,
    explicit: &Option<ProviderConfig>,
) -> ProviderConfig {
    if let Some(provider) = explicit {
        return provider.clone();
    }

    if matches!(config.kind, providers::ProviderKind::Anthropic) {
        return ProviderConfig {
            id: "ollama-embed-fallback".into(),
            kind: providers::ProviderKind::Ollama,
            label: "Ollama (эмбеддинги)".into(),
            base_url: "http://localhost:11434".into(),
            api_key: None,
            model: "nomic-embed-text".into(),
            temperature: 0.0,
            embedding_model: Some("nomic-embed-text".into()),
        };
    }

    config.clone()
}

#[tauri::command]
async fn list_models(config: ProviderConfig) -> Result<Vec<String>, String> {
    let client = http_client();
    providers::list_models(&client, &config).await
}

/// Быстрая проверка доступности провайдера — для индикатора в настройках.
#[tauri::command]
async fn test_provider(config: ProviderConfig) -> Result<bool, String> {
    let client = http_client();
    providers::list_models(&client, &config).await.map(|_| true)
}

/// Проверка векторного хранилища: для Pinecone заодно показывает размерность
/// индекса, чтобы сразу увидеть несовпадение с моделью эмбеддингов.
#[tauri::command]
async fn test_vector_store(app: tauri::AppHandle, store: VectorStoreConfig) -> Result<String, String> {
    let client = http_client();
    let ctx = StoreCtx {
        app: &app,
        client: &client,
        config: &store,
    };
    vector::health(&ctx).await
}

#[tauri::command]
async fn run_plugin(code: String) -> Result<String, String> {
    let rt = Runtime::new().map_err(|e: rquickjs::Error| e.to_string())?;
    let context = Context::full(&rt).map_err(|e: rquickjs::Error| e.to_string())?;

    context.with(|ctx| {
        let global = ctx.globals();
        global
            .set(
                "print",
                Func::new(|msg: String| {
                    println!("Plugin: {}", msg);
                }),
            )
            .map_err(|e: rquickjs::Error| e.to_string())?;

        let res: rquickjs::Value = ctx.eval(code).map_err(|e: rquickjs::Error| e.to_string())?;

        let string_ctor: rquickjs::Function = ctx
            .globals()
            .get("String")
            .map_err(|e: rquickjs::Error| e.to_string())?;
        let result: rquickjs::String = string_ctor
            .call((res,))
            .map_err(|e: rquickjs::Error| e.to_string())?;

        result.to_string().map_err(|e: rquickjs::Error| e.to_string())
    })
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
            let window = app
                .get_webview_window("main")
                .ok_or_else(|| tauri::Error::AssetNotFound("Main window not found".to_string()))?;

            #[cfg(target_os = "windows")]
            {
                let _ = apply_acrylic(&window, Some((16, 16, 20, 120)));
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            run_agent,
            list_models,
            test_provider,
            test_vector_store,
            run_plugin
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
