#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod providers;
mod rag;
mod sidecar;
mod tools;
mod vector;

use providers::{ChatMessage, ProviderConfig, StreamEvent};
use vector::{StoreCtx, VectorStoreConfig};
use rquickjs::{prelude::*, Context, Runtime};
use serde_json::Value;
use std::path::{Path, PathBuf};
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
    use_graph: bool,
    system: String,
    messages: Vec<ChatMessage>,
    attachments: Vec<String>,
    on_event: Channel<StreamEvent>,
) -> Result<(), String> {
    let client = http_client();
    let data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Нет доступа к директории данных: {}", e))?;
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
            rag::process_documents(
                &data_dir,
                &client,
                &embed_config,
                &store,
                &attachments,
                rag::DEFAULT_CHUNK_SIZE,
                rag::DEFAULT_CHUNK_OVERLAP,
            )
            .await
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
                rag::search_documents(&data_dir, &client, &embed_config, &store, &query, &attachments)
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

                    search_documents_tool(
                        &data_dir,
                        &client,
                        &config,
                        &embed_config,
                        &store,
                        use_graph,
                        &query,
                        &on_event,
                    )
                    .await
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

/// Поиск по документам для агента.
///
/// Когда включён LangGraph-режим, поиск идёт через граф: он переформулирует
/// запрос, оценивает найденное и при нехватке материала заходит на второй круг.
/// Граф — улучшение, а не обязательное звено: любой его сбой (нет Python, нет
/// зависимостей, таймаут) откатывает на прямой векторный поиск, а не роняет ход.
#[allow(clippy::too_many_arguments)]
async fn search_documents_tool(
    data_dir: &Path,
    client: &reqwest::Client,
    provider: &ProviderConfig,
    embed_provider: &ProviderConfig,
    store: &VectorStoreConfig,
    use_graph: bool,
    query: &str,
    on_event: &Channel<StreamEvent>,
) -> (String, bool) {
    if use_graph {
        let _ = on_event.send(StreamEvent::Status {
            message: "LangGraph: уточняю запрос…".into(),
        });

        let ctx = sidecar::SidecarCtx {
            data_dir,
            client,
            provider,
            embed_provider,
            store,
        };

        match sidecar::search(&ctx, query, &[]).await {
            Ok(result) => {
                for step in &result.trace {
                    let _ = on_event.send(StreamEvent::Status {
                        message: format!("LangGraph · {}", step),
                    });
                }
                if result.context.trim().is_empty() {
                    return (
                        "Граф не нашёл релевантных фрагментов в документах.".into(),
                        false,
                    );
                }
                return (result.context, false);
            }
            Err(e) => {
                // Сообщаем и продолжаем обычным поиском — пользователь получит ответ.
                let _ = on_event.send(StreamEvent::Status {
                    message: format!("LangGraph недоступен ({}), обычный поиск", e),
                });
            }
        }
    }

    match rag::search_documents(data_dir, client, embed_provider, store, query, &[]).await {
        Ok(ctx) if ctx.trim().is_empty() => (
            "Ничего не найдено в проиндексированных документах.".into(),
            false,
        ),
        Ok(ctx) => (ctx, false),
        Err(e) => (e, true),
    }
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
            // nomic обучен с асимметричными task-префиксами.
            embed_document_prefix: Some("search_document: ".into()),
            embed_query_prefix: Some("search_query: ".into()),
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
    let data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Нет доступа к директории данных: {}", e))?;
    let ctx = StoreCtx {
        data_dir: &data_dir,
        client: &client,
        config: &store,
    };
    vector::health(&ctx).await
}

/// Проверяет, поднимается ли LangGraph-sidecar: есть ли Python, скрипт и
/// установлены ли зависимости. Возвращает версию LangGraph.
#[tauri::command]
async fn test_langgraph() -> Result<String, String> {
    sidecar::health().await
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


// ─────────────────────────── headless-режим ───────────────────────────
//
// Отдельного бинаря нет намеренно: `src/bin/*.rs` не видит модули из
// `main.rs`, для этого пришлось бы выносить `lib.rs`. Вместо этого — ветка
// в самом начале `main()`, до `tauri::Builder`. Tauri-рантайм не стартует,
// окна нет, JSON уходит в stdout.
//
// ВАЖНО: в release-сборке на Windows действует
// `windows_subsystem = "windows"` (первая строка файла) — консоли нет и
// stdout уходит в никуда даже при перенаправлении. Для эвалов гоняйте
// debug-сборку: `cargo run -- retrieve ...`.

/// Конфигурация прогона эвала. Едет в публичный репозиторий вместе с
/// `evals/`, поэтому ключей внутри быть не должно — они берутся из
/// переменных окружения.
#[derive(serde::Deserialize)]
struct EvalConfig {
    #[serde(default)]
    name: String,
    #[serde(default = "default_chunk_size")]
    chunk_size: usize,
    #[serde(default = "default_chunk_overlap")]
    chunk_overlap: usize,
    #[serde(default = "default_k")]
    k: usize,
    #[serde(default)]
    store: VectorStoreConfig,
    provider: ProviderConfig,
}

fn default_chunk_size() -> usize {
    rag::DEFAULT_CHUNK_SIZE
}
fn default_chunk_overlap() -> usize {
    rag::DEFAULT_CHUNK_OVERLAP
}
fn default_k() -> usize {
    20
}

/// То же место, что вернул бы `AppHandle::path().app_data_dir()`, но без
/// запущенного Tauri. Если разойдётся — headless откроет пустую базу и
/// честно вернёт ноль хитов.
#[cfg(target_os = "windows")]
fn default_data_dir() -> PathBuf {
    PathBuf::from(std::env::var("APPDATA").unwrap_or_default()).join("com.lumina.app")
}

#[cfg(target_os = "macos")]
fn default_data_dir() -> PathBuf {
    PathBuf::from(std::env::var("HOME").unwrap_or_default())
        .join("Library/Application Support")
        .join("com.lumina.app")
}

#[cfg(all(unix, not(target_os = "macos")))]
fn default_data_dir() -> PathBuf {
    std::env::var("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            PathBuf::from(std::env::var("HOME").unwrap_or_default()).join(".local/share")
        })
        .join("com.lumina.app")
}

fn flag(args: &[String], name: &str) -> Option<String> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1))
        .cloned()
}

fn resolve_data_dir(args: &[String]) -> PathBuf {
    flag(args, "--data-dir")
        .map(PathBuf::from)
        .unwrap_or_else(default_data_dir)
}

fn load_eval_config(args: &[String]) -> Result<EvalConfig, String> {
    let path = flag(args, "--config").ok_or("Не задан --config")?;
    let raw = std::fs::read_to_string(&path).map_err(|e| format!("{}: {}", path, e))?;
    let mut cfg: EvalConfig =
        serde_json::from_str(&raw).map_err(|e| format!("{}: {}", path, e))?;

    // Ключи только из окружения: конфиг лежит в публичном репозитории.
    if cfg.provider.api_key.as_deref().unwrap_or("").is_empty() {
        if let Ok(key) = std::env::var("LUMINA_API_KEY") {
            cfg.provider.api_key = Some(key);
        }
    }
    if cfg.store.api_key.as_deref().unwrap_or("").is_empty() {
        if let Ok(key) = std::env::var("PINECONE_API_KEY") {
            cfg.store.api_key = Some(key);
        }
    }

    Ok(cfg)
}

/// `lumina index --config ... --paths a.md b.md`
///
/// Нужен для свипа размера чанка: без него пришлось бы переиндексировать
/// корпус через GUI на каждую конфигурацию.
async fn run_index(args: &[String]) -> Result<(), String> {
    let cfg = load_eval_config(args)?;
    let data_dir = resolve_data_dir(args);

    let paths: Vec<String> = args
        .iter()
        .skip_while(|a| a.as_str() != "--paths")
        .skip(1)
        .take_while(|a| !a.starts_with("--"))
        .cloned()
        .collect();
    if paths.is_empty() {
        return Err("Не заданы --paths".into());
    }

    let client = http_client();
    let report = rag::process_documents(
        &data_dir,
        &client,
        &cfg.provider,
        &cfg.store,
        &paths,
        cfg.chunk_size,
        cfg.chunk_overlap,
    )
    .await?;

    println!(
        "{}",
        serde_json::json!({
            "config": cfg.name,
            "data_dir": data_dir.to_string_lossy(),
            "chunk_size": cfg.chunk_size,
            "chunk_overlap": cfg.chunk_overlap,
            "files": paths.len(),
            "report": report,
        })
    );
    Ok(())
}

/// `lumina retrieve --config ... --query "..." --k 20`
async fn run_retrieve(args: &[String]) -> Result<(), String> {
    let cfg = load_eval_config(args)?;
    let data_dir = resolve_data_dir(args);
    let query = flag(args, "--query").ok_or("Не задан --query")?;
    let k = flag(args, "--k")
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(cfg.k);

    let client = http_client();
    let hits = rag::retrieve_hits(
        &data_dir,
        &client,
        &cfg.provider,
        &cfg.store,
        &query,
        &[],
        k,
    )
    .await?;

    println!(
        "{}",
        serde_json::to_string(&serde_json::json!({
            "config": cfg.name,
            "data_dir": data_dir.to_string_lossy(),
            "query": query,
            "k": k,
            "hits": hits,
        }))
        .map_err(|e| e.to_string())?
    );
    Ok(())
}

fn main() {
    // Headless-подкоманды перехватываются до старта Tauri.
    if let Some(cmd) = std::env::args().nth(1) {
        if cmd == "retrieve" || cmd == "index" {
            let args: Vec<String> = std::env::args().collect();
            let rt = tokio::runtime::Runtime::new().expect("Не удалось создать tokio-runtime");
            let result = if cmd == "retrieve" {
                rt.block_on(run_retrieve(&args))
            } else {
                rt.block_on(run_index(&args))
            };
            if let Err(e) = result {
                eprintln!("{}", e);
                std::process::exit(1);
            }
            std::process::exit(0);
        }
    }

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
            test_langgraph,
            run_plugin
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
