//! Gemini with Google Search grounding.
//!
//! `POST /v1beta/models/{model}:generateContent`, key in `x-goog-api-key`,
//! text from `candidates[0].content.parts[].text`, citations from
//! `candidates[0].groundingMetadata.groundingChunks[].web` (`uri`, `title`)
//! (documented). The `uri` is a Google redirect that is never followed; the
//! cited site is read from `title`, which Google fills with the site's
//! domain (documented behaviour, but a title that is not a host name leaves
//! the domain empty). Tokens from `usageMetadata.promptTokenCount` and
//! `candidatesTokenCount` (documented); searches counted as
//! `groundingMetadata.webSearchQueries` entries (documented field).

use std::time::Duration;

use async_trait::async_trait;
use serde_json::{json, Value};

use super::{
    bare_host, count, http_client, join, push_citation, send, str_of, Engine, EngineAnswer,
    EngineError, EngineId,
};

pub struct GeminiEngine {
    client: reqwest::Client,
    api_key: String,
    model: String,
    base: String,
}

impl GeminiEngine {
    pub fn new(api_key: &str, model: &str, base_url: &str, timeout: Duration) -> Self {
        Self {
            client: http_client(timeout),
            api_key: api_key.to_string(),
            model: model.to_string(),
            base: base_url.to_string(),
        }
    }
}

/// The title as a host name, when it is one (`example.com`, no spaces).
fn host_from_title(title: &str) -> String {
    let title = title
        .trim()
        .trim_start_matches("https://")
        .trim_start_matches("http://");
    let title = title.split('/').next().unwrap_or("");
    let looks_like_host = title.contains('.')
        && !title.starts_with('.')
        && !title.ends_with('.')
        && title
            .chars()
            .all(|c| c.is_alphanumeric() || c == '.' || c == '-');
    if looks_like_host {
        bare_host(title)
    } else {
        String::new()
    }
}

pub fn parse(response: &Value) -> Result<EngineAnswer, EngineError> {
    let candidate = response
        .get("candidates")
        .and_then(Value::as_array)
        .and_then(|candidates| candidates.first())
        .ok_or(EngineError::Unreadable)?;
    let text: String = candidate
        .get("content")
        .and_then(|content| content.get("parts"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|part| str_of(part, "text"))
        .collect();
    if text.trim().is_empty() {
        return Err(EngineError::Unreadable);
    }
    let mut answer = EngineAnswer {
        text,
        ..EngineAnswer::default()
    };
    let grounding = candidate.get("groundingMetadata");
    for web in grounding
        .and_then(|g| g.get("groundingChunks"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|chunk| chunk.get("web"))
    {
        if let Some(uri) = str_of(web, "uri") {
            let title = str_of(web, "title");
            let domain = title.map(host_from_title).unwrap_or_default();
            push_citation(&mut answer.citations, uri, title, Some(domain));
        }
    }
    answer.searches = grounding
        .and_then(|g| g.get("webSearchQueries"))
        .and_then(Value::as_array)
        .map_or(0, |queries| queries.len().min(i32::MAX as usize) as i32);
    let usage = response.get("usageMetadata");
    answer.input_tokens = count(usage.and_then(|u| u.get("promptTokenCount")));
    answer.output_tokens = count(usage.and_then(|u| u.get("candidatesTokenCount")));
    Ok(answer)
}

#[async_trait]
impl Engine for GeminiEngine {
    fn id(&self) -> EngineId {
        EngineId::Gemini
    }

    fn model(&self) -> &str {
        &self.model
    }

    async fn ask(&self, prompt: &str) -> Result<EngineAnswer, EngineError> {
        let body = json!({
            "contents": [{ "parts": [{ "text": prompt }] }],
            "tools": [{ "google_search": {} }],
        });
        let path = format!("/v1beta/models/{}:generateContent", self.model);
        let response = send(
            self.client
                .post(join(&self.base, &path))
                .header("x-goog-api-key", &self.api_key)
                .json(&body),
        )
        .await?;
        parse(&response)
    }
}
