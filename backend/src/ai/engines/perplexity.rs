//! Perplexity's chat completions (search is built in).
//!
//! `POST /chat/completions`, text from `choices[0].message.content`
//! (documented). Citations from `search_results[].url`/`title` when present,
//! else `citations[]` strings, at the top level or inside
//! `choices[0].message` (the plan says both places occur; the top level is
//! documented, the one inside the message is assumed). Token counts from
//! `usage.prompt_tokens` / `usage.completion_tokens` (documented); searches
//! from `usage.num_search_queries` (assumed, 0 when absent).

use std::time::Duration;

use async_trait::async_trait;
use serde_json::{json, Value};

use super::{
    count, http_client, join, push_citation, send, str_of, Engine, EngineAnswer, EngineError,
    EngineId,
};

pub struct PerplexityEngine {
    client: reqwest::Client,
    api_key: String,
    model: String,
    base: String,
}

impl PerplexityEngine {
    pub fn new(api_key: &str, model: &str, base_url: &str, timeout: Duration) -> Self {
        Self {
            client: http_client(timeout),
            api_key: api_key.to_string(),
            model: model.to_string(),
            base: base_url.to_string(),
        }
    }
}

pub fn parse(response: &Value) -> Result<EngineAnswer, EngineError> {
    let message = response
        .get("choices")
        .and_then(Value::as_array)
        .and_then(|choices| choices.first())
        .and_then(|choice| choice.get("message"))
        .ok_or(EngineError::Unreadable)?;
    let text = str_of(message, "content").ok_or(EngineError::Unreadable)?;
    if text.trim().is_empty() {
        return Err(EngineError::Unreadable);
    }
    let mut answer = EngineAnswer {
        text: text.to_string(),
        ..EngineAnswer::default()
    };
    for place in [response, message] {
        read_sources(&mut answer, place);
    }
    let usage = response.get("usage");
    answer.input_tokens = count(usage.and_then(|u| u.get("prompt_tokens")));
    answer.output_tokens = count(usage.and_then(|u| u.get("completion_tokens")));
    answer.searches = count(usage.and_then(|u| u.get("num_search_queries")));
    Ok(answer)
}

/// `search_results` when this place has any, otherwise its `citations`.
fn read_sources(answer: &mut EngineAnswer, place: &Value) {
    let results = place
        .get("search_results")
        .and_then(Value::as_array)
        .filter(|rows| !rows.is_empty());
    if let Some(rows) = results {
        for row in rows {
            if let Some(url) = str_of(row, "url") {
                push_citation(&mut answer.citations, url, str_of(row, "title"), None);
            }
        }
        return;
    }
    for url in place
        .get("citations")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
    {
        push_citation(&mut answer.citations, url, None, None);
    }
}

#[async_trait]
impl Engine for PerplexityEngine {
    fn id(&self) -> EngineId {
        EngineId::Perplexity
    }

    fn model(&self) -> &str {
        &self.model
    }

    async fn ask(&self, prompt: &str) -> Result<EngineAnswer, EngineError> {
        let body = json!({
            "model": self.model,
            "messages": [{ "role": "user", "content": prompt }],
        });
        let response = send(
            self.client
                .post(join(&self.base, "/chat/completions"))
                .bearer_auth(&self.api_key)
                .json(&body),
        )
        .await?;
        parse(&response)
    }
}
