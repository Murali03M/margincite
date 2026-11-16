//! Grok through xAI's Responses API with the web search tool.
//!
//! Documented by xAI (docs.x.ai/developers/tools/web-search and
//! /developers/tools/citations): `POST /v1/responses` with `model`, `input`
//! (a list of `{role, content}`) and `tools: [{type: "web_search"}]`; a
//! `message` output item whose `content[]` parts of type `output_text` carry
//! `text` and `annotations[]` of type `url_citation` (`url`, `start_index`,
//! `end_index`, and a `title` that is the visible citation number such as
//! "1"); a top-level `citations` list of URL strings naming every source the
//! agent met, cited inline or not.
//!
//! Assumed, because the pages do not show them: token counts as
//! `usage.input_tokens` / `usage.output_tokens` (the Responses convention),
//! searches counted as `output[]` items of type `web_search_call`, and
//! top-level `citations` entries that are objects with a `url` (the plan asks
//! for them to be accepted; nothing documents them). A `title` that is only
//! digits is a citation number, not a title, so it is dropped.

use std::time::Duration;

use async_trait::async_trait;
use serde_json::{json, Value};

use super::{
    count, http_client, join, push_citation, send, str_of, Engine, EngineAnswer, EngineError,
    EngineId,
};

pub struct GrokEngine {
    client: reqwest::Client,
    api_key: String,
    model: String,
    base: String,
}

impl GrokEngine {
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
    read_top_level_citations(&mut answer, response);
    let usage = response.get("usage");
    answer.input_tokens = count(usage.and_then(|u| u.get("input_tokens")));
    answer.output_tokens = count(usage.and_then(|u| u.get("output_tokens")));
    if answer.text.trim().is_empty() {
        return Err(EngineError::Unreadable);
    }
    Ok(answer)
}

/// A title that is a bare citation number says nothing about the page.
fn real_title(title: Option<&str>) -> Option<&str> {
    title.filter(|t| !t.trim().is_empty() && !t.trim().chars().all(|c| c.is_ascii_digit()))
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
                push_citation(
                    &mut answer.citations,
                    url,
                    real_title(str_of(note, "title")),
                    None,
                );
            }
        }
    }
}

/// The rest of the sources, after the inline ones: strings (documented) or
/// objects with a `url` (assumed).
fn read_top_level_citations(answer: &mut EngineAnswer, response: &Value) {
    for entry in response
        .get("citations")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        match entry {
            Value::String(url) => push_citation(&mut answer.citations, url, None, None),
            Value::Object(_) => {
                if let Some(url) = str_of(entry, "url") {
                    push_citation(
                        &mut answer.citations,
                        url,
                        real_title(str_of(entry, "title")),
                        None,
                    );
                }
            }
            _ => {}
        }
    }
}

#[async_trait]
impl Engine for GrokEngine {
    fn id(&self) -> EngineId {
        EngineId::Grok
    }

    fn model(&self) -> &str {
        &self.model
    }

    async fn ask(&self, prompt: &str) -> Result<EngineAnswer, EngineError> {
        let body = json!({
            "model": self.model,
            "input": [{ "role": "user", "content": prompt }],
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
