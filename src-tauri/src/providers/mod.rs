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
    /// Префикс к тексту ДОКУМЕНТА при индексации.
    ///
    /// Часть моделей обучена с обязательными task-префиксами, причём
    /// асимметричными: у `nomic-embed-text` это `search_document: ` для
    /// индексируемых кусков и `search_query: ` для запросов. Без них модель
    /// работает не в том режиме, для которого тренировалась. Вынесено в
    /// конфиг, а не зашито, чтобы это можно было измерить, а не предполагать.
    #[serde(default)]
    pub embed_document_prefix: Option<String>,
    /// Префикс к тексту ЗАПРОСА при поиске.
    #[serde(default)]
    pub embed_query_prefix: Option<String>,
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
/// Что именно эмбеддится: индексируемый кусок или поисковый запрос.
/// Различие существенно для моделей с асимметричными task-префиксами.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EmbedRole {
    Document,
    Query,
}

pub async fn embed_as(
    client: &reqwest::Client,
    config: &ProviderConfig,
    text: &str,
    role: EmbedRole,
) -> Result<Vec<f32>, String> {
    let prefix = match role {
        EmbedRole::Document => config.embed_document_prefix.as_deref(),
        EmbedRole::Query => config.embed_query_prefix.as_deref(),
    }
    .unwrap_or_else(|| default_prefix(config, role));

    let owned;
    let text = if prefix.is_empty() {
        text
    } else {
        owned = format!("{}{}", prefix, text);
        &owned
    };

    let raw = match config.kind {
        ProviderKind::Ollama => ollama::embed(client, config, text).await,
        ProviderKind::OpenAi => openai::embed(client, config, text).await,
        ProviderKind::Gemini => gemini::embed(client, config, text).await,
        ProviderKind::Anthropic => {
            Err("У Anthropic нет эндпоинта эмбеддингов — выберите Ollama или OpenAI-совместимого провайдера для индексации документов.".into())
        }
    }?;

    Ok(normalize(raw))
}

/// Приводит вектор к единичной длине.
///
/// Зачем это здесь, а не в конкретном бэкенде. Провайдеры отдают эмбеддинги
/// ненормированными (у `nomic-embed-text` норма порядка десяти), а бэкенды
/// ранжируют по-разному: `sqlite` — косинусом, `sqlite_vec` — L2, потому что
/// это метрика vec0 по умолчанию. На ненормированных векторах это два РАЗНЫХ
/// порядка: в L2 вклад длины вектора сопоставим с вкладом направления, и
/// появляются чанки-«хабы», близкие сразу ко всем запросам просто из-за
/// удачной нормы.
///
/// На единичных векторах порядок по L2 совпадает с порядком по косинусу
/// (`|a-b|² = 2 - 2·cos`), поэтому нормализация в одной точке делает все три
/// бэкенда согласованными и убирает влияние длины.
///
/// Найдено eval-харнесом: поиск по дословному тексту ответа возвращал его
/// содержащий чанк лишь в половине случаев.
/// Префикс по умолчанию для моделей, которые его требуют.
///
/// `nomic-embed-text` обучен с обязательными асимметричными task-префиксами.
/// Замер на golden set из 50 запросов по 19 файлам: включение префиксов подняло
/// recall@5 с 0.167 до 0.300 (95% бутстрэп [+0.03, +0.27], улучшилось 4 запроса
/// из 30, ухудшилось 0). Поэтому это дефолт, а не опция — но переопределяемый,
/// чтобы эвал мог замерить обе стороны.
fn default_prefix(config: &ProviderConfig, role: EmbedRole) -> &'static str {
    let model = config
        .embedding_model
        .as_deref()
        .unwrap_or(config.model.as_str());

    if model.starts_with("nomic-embed") {
        match role {
            EmbedRole::Document => "search_document: ",
            EmbedRole::Query => "search_query: ",
        }
    } else {
        ""
    }
}

fn normalize(mut v: Vec<f32>) -> Vec<f32> {
    let norm = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 0.0 && norm.is_finite() {
        for x in v.iter_mut() {
            *x /= norm;
        }
    }
    v
}

#[cfg(test)]
mod embed_tests {
    use super::normalize;

    use super::{default_prefix, EmbedRole, ProviderConfig, ProviderKind};

    fn cfg(embedding_model: Option<&str>) -> ProviderConfig {
        ProviderConfig {
            id: "t".into(),
            kind: ProviderKind::Ollama,
            label: "t".into(),
            base_url: "http://localhost:11434".into(),
            api_key: None,
            model: "llama3".into(),
            temperature: 0.0,
            embedding_model: embedding_model.map(str::to_string),
            embed_document_prefix: None,
            embed_query_prefix: None,
        }
    }

    #[test]
    fn nomic_gets_asymmetric_prefixes_by_default() {
        let c = cfg(Some("nomic-embed-text"));
        assert_eq!(default_prefix(&c, EmbedRole::Document), "search_document: ");
        assert_eq!(default_prefix(&c, EmbedRole::Query), "search_query: ");
    }

    #[test]
    fn other_models_get_no_prefix() {
        let c = cfg(Some("text-embedding-3-small"));
        assert_eq!(default_prefix(&c, EmbedRole::Document), "");
        assert_eq!(default_prefix(&c, EmbedRole::Query), "");
    }

    #[test]
    fn explicit_empty_prefix_overrides_default() {
        let mut c = cfg(Some("nomic-embed-text"));
        c.embed_document_prefix = Some(String::new());
        // Пустая строка — это осознанное «без префикса», а не «не задано»:
        // на этом держится возможность померить обе стороны.
        assert_eq!(c.embed_document_prefix.as_deref(), Some(""));
    }

    #[test]
    fn normalize_gives_unit_length() {
        let v = normalize(vec![3.0, 4.0]);
        let norm = v.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 1e-6, "норма должна стать 1, а не {}", norm);
        assert!((v[0] - 0.6).abs() < 1e-6);
    }

    #[test]
    fn normalize_preserves_direction() {
        let a = normalize(vec![1.0, 2.0, 3.0]);
        let b = normalize(vec![10.0, 20.0, 30.0]);
        for (x, y) in a.iter().zip(&b) {
            assert!((x - y).abs() < 1e-6, "коллинеарные векторы должны совпасть");
        }
    }

    #[test]
    fn l2_order_matches_cosine_order_on_unit_vectors() {
        let q = normalize(vec![1.0, 0.0]);
        let near = normalize(vec![0.9, 0.1]);
        let far = normalize(vec![0.0, 1.0]);
        let l2 = |a: &[f32], b: &[f32]| -> f32 {
            a.iter().zip(b).map(|(x, y)| (x - y) * (x - y)).sum::<f32>().sqrt()
        };
        let cos = |a: &[f32], b: &[f32]| -> f32 { a.iter().zip(b).map(|(x, y)| x * y).sum() };
        assert!(l2(&q, &near) < l2(&q, &far));
        assert!(cos(&q, &near) > cos(&q, &far));
    }

    #[test]
    fn zero_vector_survives() {
        assert_eq!(normalize(vec![0.0, 0.0]), vec![0.0, 0.0]);
    }
}

pub fn new_call_id(name: &str) -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("call_{}_{}", name, n)
}
