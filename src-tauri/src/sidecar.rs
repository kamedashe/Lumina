//! Мост к LangGraph-графу, живущему в Python-процессе.
//!
//! Разделение обязанностей: граф отвечает за оркестрацию (переформулировать
//! запрос, оценить найденное, при необходимости зайти на второй круг), а весь
//! ввод-вывод остаётся здесь. Когда узлу графа нужен векторный поиск или вызов
//! модели, он присылает callback, который обслуживается кодом ниже.
//!
//! Благодаря этому в Python нет ни ключей API, ни клиентов провайдеров, ни
//! знания о том, какое из трёх векторных хранилищ активно.
//!
//! Процесс поднимается на время одного поиска. Долгоживущий сэкономил бы
//! ~1.5 с на импорт LangGraph, но потребовал бы следить за перезапуском после
//! падений и утечкой состояния между запросами; на фоне нескольких вызовов
//! модели внутри графа эта экономия не окупает усложнения.

use crate::providers::{self, ChatMessage, ProviderConfig, StreamEvent};
use crate::rag;
use crate::vector::VectorStoreConfig;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::ipc::Channel;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;

/// Потолок на весь прогон графа: несколько вызовов модели плюс поиск.
const GRAPH_TIMEOUT: Duration = Duration::from_secs(180);

pub struct SidecarCtx<'a> {
    pub data_dir: &'a Path,
    pub client: &'a reqwest::Client,
    /// Чем граф думает: переформулировка запроса и оценка релевантности.
    pub provider: &'a ProviderConfig,
    /// Чем считаются эмбеддинги (у Anthropic своих нет — там будет Ollama).
    pub embed_provider: &'a ProviderConfig,
    pub store: &'a VectorStoreConfig,
}

pub struct GraphResult {
    pub context: String,
    /// Пошаговый след графа — показываем в UI, чтобы работа была видна.
    pub trace: Vec<String>,
}

/// Ищет `graph.py`. В dev он лежит в репозитории, в собранном приложении —
/// рядом с исполняемым файлом.
fn locate_script() -> Result<PathBuf, String> {
    let mut candidates: Vec<PathBuf> = Vec::new();

    // Репозиторий: src-tauri/ → ../sidecar/graph.py
    candidates.push(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("sidecar")
            .join("graph.py"),
    );

    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            candidates.push(dir.join("sidecar").join("graph.py"));
            candidates.push(dir.join("graph.py"));
        }
    }

    candidates
        .into_iter()
        .find(|p| p.exists())
        .ok_or_else(|| "Не найден sidecar/graph.py — LangGraph-граф недоступен.".to_string())
}

/// Интерпретатор Python. `LUMINA_PYTHON` позволяет указать venv.
fn python_command() -> String {
    std::env::var("LUMINA_PYTHON").unwrap_or_else(|_| {
        if cfg!(target_os = "windows") {
            "python".to_string()
        } else {
            "python3".to_string()
        }
    })
}

/// Векторный поиск по запросу графа.
async fn handle_retrieve(ctx: &SidecarCtx<'_>, payload: &Value) -> Result<Value, String> {
    let query = payload.get("query").and_then(Value::as_str).unwrap_or_default();
    let top_k = payload.get("top_k").and_then(Value::as_u64).unwrap_or(8) as usize;
    let scope: Vec<String> = payload
        .get("scope")
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();

    // Через rag::retrieve_hits, а не напрямую в vector: путь поиска должен
    // быть один — тот же, что мерит eval-харнес. Иначе граф ходил бы мимо
    // измеряемого кода.
    let hits = rag::retrieve_hits(
        ctx.data_dir,
        ctx.client,
        ctx.embed_provider,
        ctx.store,
        query,
        &scope,
        top_k,
    )
    .await?;

    // SearchHit сериализуется целиком: у графа появляются chunk_id и спаны,
    // а прежние поля `content` и `score` остаются на месте — graph.py
    // продолжает работать без правок.
    Ok(json!({ "hits": hits }))
}

/// Одиночный вызов модели для узла графа.
async fn handle_llm(ctx: &SidecarCtx<'_>, payload: &Value) -> Result<Value, String> {
    let system = payload.get("system").and_then(Value::as_str).unwrap_or_default();
    let prompt = payload.get("prompt").and_then(Value::as_str).unwrap_or_default();

    // Служебные вызовы графа не должны попадать в чат пользователя —
    // отдаём stream_turn канал, который всё выбрасывает.
    let silent: Channel<StreamEvent> = Channel::new(|_| Ok(()));

    let turn = providers::stream_turn(
        ctx.client,
        ctx.provider,
        system,
        &[ChatMessage::user(prompt)],
        &[], // узлам графа инструменты не нужны
        &silent,
    )
    .await?;

    Ok(json!({ "text": turn.text }))
}

/// Проверяет готовность sidecar: находится ли скрипт, есть ли Python и
/// установлен ли LangGraph. Используется индикатором в настройках.
pub async fn health() -> Result<String, String> {
    let script = locate_script()?;
    let python = python_command();

    // У пакета langgraph нет атрибута __version__ — берём версию из метаданных
    // дистрибутива. Импорт самого модуля всё равно нужен: он подтверждает, что
    // пакет не просто установлен, а импортируется без ошибок.
    let probe = "import sys, langgraph; \
                 from importlib.metadata import version; \
                 print(f\"{version('langgraph')}|{sys.version.split()[0]}\")";

    let output = Command::new(&python)
        .arg("-c")
        .arg(probe)
        .output()
        .await
        .map_err(|e| format!("Не удалось запустить «{}»: {}. Установите Python 3.", python, e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(if stderr.contains("ModuleNotFoundError") {
            "LangGraph не установлен. Выполните: pip install -r sidecar/requirements.txt".to_string()
        } else {
            format!("Python вернул ошибку: {}", stderr.trim().lines().last().unwrap_or(""))
        });
    }

    let raw = String::from_utf8_lossy(&output.stdout);
    let info = raw.trim();
    let (langgraph_version, python_version) = info.split_once('|').unwrap_or((info, "?"));

    Ok(format!(
        "LangGraph {} · Python {} · {}",
        langgraph_version,
        python_version,
        script.file_name().unwrap_or_default().to_string_lossy()
    ))
}

/// Прогоняет граф и возвращает собранный контекст.
pub async fn search(
    ctx: &SidecarCtx<'_>,
    query: &str,
    scope: &[String],
) -> Result<GraphResult, String> {
    let script = locate_script()?;
    let python = python_command();

    let mut child = Command::new(&python)
        // -u отключает буферизацию: иначе ответы графа зависают в буфере.
        .arg("-u")
        .arg(&script)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .map_err(|e| {
            format!(
                "Не удалось запустить «{}»: {}. Установите Python и зависимости: \
                 pip install -r sidecar/requirements.txt",
                python, e
            )
        })?;

    let mut stdin = child.stdin.take().ok_or("нет stdin у sidecar")?;
    let stdout = child.stdout.take().ok_or("нет stdout у sidecar")?;
    let stderr = child.stderr.take().ok_or("нет stderr у sidecar")?;

    // Диагностика падений: без stderr «процесс молча умер» неотлаживаемо.
    let errors = Arc::new(Mutex::new(String::new()));
    {
        let errors = Arc::clone(&errors);
        tokio::spawn(async move {
            let mut lines = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let mut buf = errors.lock().unwrap();
                buf.push_str(&line);
                buf.push('\n');
            }
        });
    }

    let request = json!({ "type": "search", "query": query, "scope": scope });
    stdin
        .write_all(format!("{}\n", request).as_bytes())
        .await
        .map_err(|e| format!("Не удалось отправить запрос графу: {}", e))?;

    let mut reader = BufReader::new(stdout).lines();

    let outcome = tokio::time::timeout(GRAPH_TIMEOUT, async {
        while let Some(line) = reader
            .next_line()
            .await
            .map_err(|e| format!("Обрыв связи с графом: {}", e))?
        {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            let message: Value = match serde_json::from_str(line) {
                Ok(v) => v,
                // Посторонний вывод (предупреждения библиотек) просто пропускаем.
                Err(_) => continue,
            };

            match message.get("type").and_then(Value::as_str) {
                Some("callback") => {
                    let call_id = message
                        .get("call_id")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string();
                    let kind = message.get("kind").and_then(Value::as_str).unwrap_or_default();
                    let payload = message.get("payload").cloned().unwrap_or(Value::Null);

                    let result = match kind {
                        "retrieve" => handle_retrieve(ctx, &payload).await,
                        "llm" => handle_llm(ctx, &payload).await,
                        other => Err(format!("неизвестный callback: {}", other)),
                    };

                    // Ошибку тоже возвращаем графу: узлы умеют её пережить
                    // (например, поиск без переформулировки).
                    let response = match result {
                        Ok(data) => json!({ "call_id": call_id, "ok": true, "data": data }),
                        Err(e) => json!({ "call_id": call_id, "ok": false, "error": e }),
                    };

                    stdin
                        .write_all(format!("{}\n", response).as_bytes())
                        .await
                        .map_err(|e| format!("Не удалось ответить графу: {}", e))?;
                }

                Some("result") => {
                    return Ok(GraphResult {
                        context: message
                            .get("context")
                            .and_then(Value::as_str)
                            .unwrap_or_default()
                            .to_string(),
                        trace: message
                            .get("trace")
                            .and_then(Value::as_array)
                            .map(|arr| {
                                arr.iter()
                                    .filter_map(|v| v.as_str().map(str::to_string))
                                    .collect()
                            })
                            .unwrap_or_default(),
                    });
                }

                Some("error") => {
                    return Err(format!(
                        "Граф вернул ошибку: {}",
                        message
                            .get("message")
                            .and_then(Value::as_str)
                            .unwrap_or("без описания")
                    ));
                }

                _ => continue,
            }
        }

        // Поток закончился без результата — почти всегда это падение импорта.
        let detail = errors.lock().unwrap().trim().to_string();
        Err(if detail.is_empty() {
            "Граф завершился, не вернув результат.".to_string()
        } else {
            format!("Граф упал: {}", detail.lines().last().unwrap_or(&detail))
        })
    })
    .await;

    let _ = child.kill().await;

    match outcome {
        Ok(result) => result,
        Err(_) => Err(format!(
            "Граф не уложился в {} с.",
            GRAPH_TIMEOUT.as_secs()
        )),
    }
}
