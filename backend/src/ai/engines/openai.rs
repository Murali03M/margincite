//! ChatGPT through the Responses API with the web search tool.
//!
//! Request and response shapes follow the plan: `POST /v1/responses`,
//! `output[]` items of type `message` whose `content[]` holds `output_text`
//! parts with `annotations[]` of type `url_citation` (documented). Token
//! counts from `usage.input_tokens` / `usage.output_tokens` (documented);
//! searches counted as `output[]` items of type `web_search_call` (assumed
//! from the documented item type).

use std::time::Duration;

use async_trait::async_trait;
use serde_json::{json, Value};

use super::{
    count, http_client, join, push_citation, send, str_of, Engine, EngineAnswer, EngineError,
    EngineId,
};

pub struct OpenAiEngine {
    client: reqwest::Client,
    api_key: String,
    model: String,
    base: String,
}

impl OpenAiEngine {
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
    let mut answer = EngineAnswer::default();
    let items = response
        .get("output")
        .and_then(Value::as_array)
        .ok_or(EngineError::Unreadable)?;
    for item in items {
        match str_of(item, "type") {
            Some("web_search_call") => answer.searches += 1,
            Some("message") => read_message(&mut answer, item),
            _ => {}
        }
    }
    let usage = response.get("usage");
    answer.input_tokens = count(usage.and_then(|u| u.get("input_tokens")));
    answer.output_tokens = count(usage.and_then(|u| u.get("output_tokens")));
    if answer.text.trim().is_empty() {
        return Err(EngineError::Unreadable);
    }
    Ok(answer)
}

fn read_message(answer: &mut EngineAnswer, item: &Value) {
    for part in item
        .get("content")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|part| str_of(part, "type") == Some("output_text"))
    {
        if let Some(text) = str_of(part, "text") {
            answer.text.push_str(text);
        }
        for note in part
            .get("annotations")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter(|a| str_of(a, "type") == Some("url_citation"))
        {
            if let Some(url) = str_of(note, "url") {
                push_citation(&mut answer.citations, url, str_of(note, "title"), None);
            }
        }
    }
}

#[async_trait]
impl Engine for OpenAiEngine {
    fn id(&self) -> EngineId {
        EngineId::ChatGpt
    }

    fn model(&self) -> &str {
        &self.model
    }

    async fn ask(&self, prompt: &str) -> Result<EngineAnswer, EngineError> {
        let body = json!({
            "model": self.model,
            "input": prompt,
            "tools": [{ "type": "web_search" }],
        });
        let response = send(
            self.client
                .post(join(&self.base, "/v1/responses"))
                .bearer_auth(&self.api_key)
                .json(&body),
        )
        .await?;
        parse(&response)
    }
}
