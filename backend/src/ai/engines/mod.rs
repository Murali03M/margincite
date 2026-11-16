//! The AI engines Margincite asks. Each one is a small HTTP client with
//! its base address given at construction, so tests can point it at a mock.
//!
//! Response bodies are read as `serde_json::Value` on purpose: the services
//! add fields often, and a missing or new field must never fail an answer
//! that has text in it.

pub mod claude;
pub mod gemini;
pub mod grok;
pub mod openai;
pub mod perplexity;

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use serde_json::Value;
use url::Url;

use crate::body::{read_capped, MAX_BODY_BYTES};
use crate::config::EngineSettings;

pub use claude::ClaudeEngine;

/// How long one request to an engine may take.
pub const ENGINE_TIMEOUT: Duration = Duration::from_secs(180);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EngineId {
    Claude,
    ChatGpt,
    Grok,
    Gemini,
    Perplexity,
}

impl EngineId {
    pub fn as_str(self) -> &'static str {
        match self {
            EngineId::Claude => "claude",
            EngineId::ChatGpt => "chatgpt",
            EngineId::Grok => "grok",
            EngineId::Gemini => "gemini",
            EngineId::Perplexity => "perplexity",
        }
    }

    pub fn from_name(name: &str) -> Option<EngineId> {
        [
            EngineId::Claude,
            EngineId::ChatGpt,
            EngineId::Grok,
            EngineId::Gemini,
            EngineId::Perplexity,
        ]
        .into_iter()
        .find(|id| id.as_str() == name)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Citation {
    pub url: String,
    pub title: Option<String>,
    /// The cited site's host, lower case, without `www.`. Empty when the
    /// engine does not say.
    pub domain: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EngineAnswer {
    pub text: String,
    /// In the order the engine gave them, each address once.
    pub citations: Vec<Citation>,
    pub input_tokens: i32,
    pub output_tokens: i32,
    pub searches: i32,
    /// Things that went wrong without spoiling the answer, such as a failed
    /// web search inside an answer that still has text.
    pub notes: Vec<String>,
}

/// Why an engine gave no answer. The messages are written for people and
/// never repeat anything the service said, so a key it echoed back cannot
/// reach a stored row or a response.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum EngineError {
    #[error("{}", refused_message(.0))]
    Refused(Option<String>),
    #[error("{}", http_message(*.0))]
    Http(u16),
    #[error("The engine did not answer in time.")]
    Timeout,
    #[error("The engine could not be reached.")]
    Transport,
    #[error("The engine's answer could not be read.")]
    Unreadable,
}

fn refused_message(category: &Option<String>) -> String {
    match category {
        Some(category) => format!("The engine declined to answer this question ({category})."),
        None => "The engine declined to answer this question.".to_string(),
    }
}

fn http_message(status: u16) -> String {
    match status {
        401 | 403 => format!("The engine rejected the API key (HTTP {status})."),
        429 => "The engine refused the request because the key's rate limit or quota is used up (HTTP 429). Check the plan and billing for this key with the provider.".to_string(),
        other => format!("The engine answered with HTTP {other}."),
    }
}

#[async_trait]
pub trait Engine: Send + Sync {
    fn id(&self) -> EngineId;
    fn model(&self) -> &str;
    async fn ask(&self, prompt: &str) -> Result<EngineAnswer, EngineError>;
}

/// Where each engine lives. Tests replace these.
#[derive(Debug, Clone)]
pub struct EngineBases {
    pub claude: String,
    pub openai: String,
    pub grok: String,
    pub gemini: String,
    pub perplexity: String,
}

impl EngineBases {
    pub fn production() -> Self {
        Self {
            claude: "https://api.anthropic.com".into(),
            openai: "https://api.openai.com".into(),
            grok: "https://api.x.ai".into(),
            gemini: "https://generativelanguage.googleapis.com".into(),
            perplexity: "https://api.perplexity.ai".into(),
        }
    }
}

/// Builds the engines one workspace may ask. Keys saved in the app apply from
/// the next request, so nothing is built until it is needed.
#[derive(Debug, Clone)]
pub struct EngineFactory {
    pub bases: EngineBases,
    pub timeout: Duration,
}

impl EngineFactory {
    pub fn new(bases: EngineBases, timeout: Duration) -> Self {
        Self { bases, timeout }
    }

    pub fn production() -> Self {
        Self::new(EngineBases::production(), ENGINE_TIMEOUT)
    }

    /// The engines with a key for this workspace: its own key first, the
    /// server's environment second.
    pub async fn for_workspace(
        &self,
        pool: &sqlx::PgPool,
        config: &crate::config::Config,
        workspace_id: uuid::Uuid,
    ) -> Result<Engines, crate::error::AppError> {
        let settings =
            crate::settings::resolve::engine_settings(pool, config, workspace_id).await?;
        Ok(Engines::new(&settings, &self.bases, self.timeout))
    }
}

/// The engines that have a key, in the order of `config::PROVIDERS`: claude, chatgpt, grok, gemini, perplexity.
pub struct Engines {
    claude: Option<Arc<ClaudeEngine>>,
    list: Vec<Arc<dyn Engine>>,
}

impl Engines {
    pub fn new(settings: &EngineSettings, bases: &EngineBases, timeout: Duration) -> Self {
        let claude = settings.claude.api_key.as_ref().map(|key| {
            Arc::new(ClaudeEngine::new(
                key,
                &settings.claude.model,
                &bases.claude,
                timeout,
            ))
        });
        let mut list: Vec<Arc<dyn Engine>> = Vec::new();
        if let Some(claude) = &claude {
            list.push(claude.clone());
        }
        if let Some(key) = &settings.chatgpt.api_key {
            list.push(Arc::new(openai::OpenAiEngine::new(
                key,
                &settings.chatgpt.model,
                &bases.openai,
                timeout,
            )));
        }
        if let Some(key) = &settings.grok.api_key {
            list.push(Arc::new(grok::GrokEngine::new(
                key,
                &settings.grok.model,
                &bases.grok,
                timeout,
            )));
        }
        if let Some(key) = &settings.gemini.api_key {
            list.push(Arc::new(gemini::GeminiEngine::new(
                key,
                &settings.gemini.model,
                &bases.gemini,
                timeout,
            )));
        }
        if let Some(key) = &settings.perplexity.api_key {
            list.push(Arc::new(perplexity::PerplexityEngine::new(
                key,
                &settings.perplexity.model,
                &bases.perplexity,
                timeout,
            )));
        }
        Self { claude, list }
    }

    /// Claude, for the question suggestions, when it has a key.
    pub fn claude(&self) -> Option<&Arc<ClaudeEngine>> {
        self.claude.as_ref()
    }

    pub fn configured(&self) -> &[Arc<dyn Engine>] {
        &self.list
    }

    pub fn ids(&self) -> Vec<&'static str> {
        self.list
            .iter()
            .map(|engine| engine.id().as_str())
            .collect()
    }

    pub fn get(&self, id: &str) -> Option<&Arc<dyn Engine>> {
        self.list.iter().find(|engine| engine.id().as_str() == id)
    }
}

// ----------------------------------------------------------------- helpers

/// A client that never follows redirects, so a key is only ever sent to the
/// address it was configured for.
pub(crate) fn http_client(timeout: Duration) -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(timeout)
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .expect("an HTTP client with a timeout builds")
}

pub(crate) fn join(base: &str, path: &str) -> String {
    format!("{}{path}", base.trim_end_matches('/'))
}

/// Sends the request and returns its JSON body. Anything but a 2xx with a
/// JSON object is an `EngineError` that says nothing about the body.
pub(crate) async fn send(request: reqwest::RequestBuilder) -> Result<Value, EngineError> {
    let response = request.send().await.map_err(|err| {
        if err.is_timeout() {
            EngineError::Timeout
        } else {
            tracing::warn!(
                timeout = err.is_timeout(),
                connect = err.is_connect(),
                "engine request failed"
            );
            EngineError::Transport
        }
    })?;
    let status = response.status();
    if !status.is_success() {
        return Err(EngineError::Http(status.as_u16()));
    }
    let body = read_capped(response, MAX_BODY_BYTES).await.map_err(|err| {
        if err.is_timeout() {
            EngineError::Timeout
        } else {
            EngineError::Unreadable
        }
    })?;
    let value: Value = serde_json::from_slice(&body).map_err(|_| EngineError::Unreadable)?;
    if value.is_object() {
        Ok(value)
    } else {
        Err(EngineError::Unreadable)
    }
}

/// The host of an address, lower case without `www.`; empty when it has none.
pub fn domain_of(address: &str) -> String {
    Url::parse(address)
        .ok()
        .and_then(|url| url.host_str().map(bare_host))
        .unwrap_or_default()
}

pub fn bare_host(host: &str) -> String {
    let host = host.trim_end_matches('.').to_ascii_lowercase();
    host.strip_prefix("www.").unwrap_or(&host).to_string()
}

/// Adds a citation unless its address is already there.
pub(crate) fn push_citation(
    citations: &mut Vec<Citation>,
    url: &str,
    title: Option<&str>,
    domain: Option<String>,
) {
    let url = url.trim();
    if url.is_empty() || citations.iter().any(|seen| seen.url == url) {
        return;
    }
    let title = title
        .map(str::trim)
        .filter(|title| !title.is_empty())
        .map(str::to_string);
    citations.push(Citation {
        url: url.to_string(),
        title,
        domain: domain.unwrap_or_else(|| domain_of(url)),
    });
}

pub(crate) fn count(value: Option<&Value>) -> i32 {
    value
        .and_then(Value::as_i64)
        .map_or(0, |n| n.clamp(0, i64::from(i32::MAX)) as i32)
}

pub(crate) fn str_of<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value.get(key).and_then(Value::as_str)
}
