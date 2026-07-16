//! Унифицированный слой LLM-провайдеров.
//!
//! Каждый провайдер приводится к одному интерфейсу: список сообщений + описания
//! инструментов на входе, поток событий + запрошенные вызовы инструментов на выходе.

pub mod anthropic;
pub mod gemini;
pub mod ollama;
pub mod openai;
pub mod sse;

use serde::{Deserialize, Serialize};
use tauri::ipc::Channel;

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProviderKind {
    /// Локальный Ollama.
    Ollama,
    /// Любой OpenAI-совместимый эндпоинт: OpenAI, OpenRouter, Groq, LM Studio, vLLM.
    OpenAi,
    Anthropic,
    Gemini,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ProviderConfig {
    pub id: String,
    pub kind: ProviderKind,
    pub label: String,
    pub base_url: String,
    #[serde(default)]
    pub api_key: Option<String>,
    pub model: String,
    #[serde(default = "default_temperature")]
    pub temperature: f32,
    /// Модель для эмбеддингов. Используется только RAG-индексом.
    #[serde(default)]
    pub embedding_model: Option<String>,
}

fn default_temperature() -> f32 {
    0.7
}

impl ProviderConfig {
    pub fn base(&self) -> &str {
        self.base_url.trim_end_matches('/')
    }

    pub fn key(&self) -> Result<&str, String> {
        self.api_key
            .as_deref()
            .filter(|k| !k.trim().is_empty())
            .ok_or_else(|| format!("Не задан API-ключ для «{}».", self.label))
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    /// Аргументы как JSON-строка: провайдеры отдают их по кускам, склеиваем текстом.
    pub arguments: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    System,
    User,
    Assistant,
    Tool,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ChatMessage {
    pub role: Role,
    #[serde(default)]
    pub content: String,
    /// Заполняется, когда ассистент запросил инструменты.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tool_calls: Vec<ToolCall>,
    /// Заполняется у Role::Tool — id вызова, на который это ответ.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    /// Имя инструмента у Role::Tool. Нужно Gemini и Anthropic.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_name: Option<String>,
}

impl ChatMessage {
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: Role::User,
            content: content.into(),
            tool_calls: Vec::new(),
            tool_call_id: None,
            tool_name: None,
        }
    }

    pub fn assistant(content: impl Into<String>, tool_calls: Vec<ToolCall>) -> Self {
        Self {
            role: Role::Assistant,
            content: content.into(),
            tool_calls,
            tool_call_id: None,
            tool_name: None,
        }
    }

    pub fn tool_result(call: &ToolCall, content: impl Into<String>) -> Self {
        Self {
            role: Role::Tool,
            content: content.into(),
            tool_calls: Vec::new(),
            tool_call_id: Some(call.id.clone()),
            tool_name: Some(call.name.clone()),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ToolDef {
    pub name: String,
    pub description: String,
    /// JSON Schema объекта параметров.
    pub parameters: serde_json::Value,
}

/// События, уходящие во фронтенд по мере генерации.
#[derive(Serialize, Clone, Debug)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StreamEvent {
    /// Кусок текста ответа.
    Text { delta: String },
    /// Модель начала запрашивать инструмент.
    ToolCallStart { id: String, name: String },
    /// Инструмент выполнен локально, вот его результат.
    ToolResult {
        id: String,
        name: String,
        content: String,
        is_error: bool,
    },
    /// Служебный статус для индикатора («Индексирую файлы…»).
    Status { message: String },
    /// Ход завершён.
    Done { stop_reason: String },
    Error { message: String },
}

/// Что провайдер вернул за один ход.
#[derive(Debug, Default)]
pub struct AssistantTurn {
    pub text: String,
    pub tool_calls: Vec<ToolCall>,
    pub stop_reason: String,
}

impl AssistantTurn {
    pub fn wants_tools(&self) -> bool {
        !self.tool_calls.is_empty()
    }
}

/// Один ход диалога: отправить историю, стримить ответ, вернуть то, что модель хочет дальше.
pub async fn stream_turn(
    client: &reqwest::Client,
    config: &ProviderConfig,
    system: &str,
    messages: &[ChatMessage],
    tools: &[ToolDef],
    channel: &Channel<StreamEvent>,
) -> Result<AssistantTurn, String> {
    match config.kind {
        ProviderKind::Ollama => {
            ollama::stream_turn(client, config, system, messages, tools, channel).await
        }
        ProviderKind::OpenAi => {
            openai::stream_turn(client, config, system, messages, tools, channel).await
        }
        ProviderKind::Anthropic => {
            anthropic::stream_turn(client, config, system, messages, tools, channel).await
        }
        ProviderKind::Gemini => {
            gemini::stream_turn(client, config, system, messages, tools, channel).await
        }
    }
}

/// Список моделей провайдера. Для облачных — запрос к их /models, для Ollama — /api/tags.
pub async fn list_models(
    client: &reqwest::Client,
    config: &ProviderConfig,
) -> Result<Vec<String>, String> {
    match config.kind {
        ProviderKind::Ollama => ollama::list_models(client, config).await,
        ProviderKind::OpenAi => openai::list_models(client, config).await,
        ProviderKind::Anthropic => anthropic::list_models(client, config).await,
        ProviderKind::Gemini => gemini::list_models(client, config).await,
    }
}

/// Эмбеддинг текста. Anthropic своего эмбеддинг-эндпоинта не имеет —
/// для таких провайдеров RAG откатывается на локальный Ollama.
pub async fn embed(
    client: &reqwest::Client,
    config: &ProviderConfig,
    text: &str,
) -> Result<Vec<f32>, String> {
    match config.kind {
        ProviderKind::Ollama => ollama::embed(client, config, text).await,
        ProviderKind::OpenAi => openai::embed(client, config, text).await,
        ProviderKind::Gemini => gemini::embed(client, config, text).await,
        ProviderKind::Anthropic => {
            Err("У Anthropic нет эндпоинта эмбеддингов — выберите Ollama или OpenAI-совместимого провайдера для индексации документов.".into())
        }
    }
}

pub fn new_call_id(name: &str) -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("call_{}_{}", name, n)
}
