//! Claude over raw HTTP (there is no official Rust SDK).
//!
//! Request and response shapes are the ones in the plan, which come from
//! Anthropic's documentation for API version 2026-10-21 and the
//! `server-side-fallback-2026-07-01` beta: documented.
//! `thinking`, `temperature`, `top_p` and `top_k` are rejected on this
//! version, so they are never sent.

use std::time::Duration;

use async_trait::async_trait;
use serde_json::{json, Value};

use super::{
    count, http_client, join, push_citation, send, str_of, Engine, EngineAnswer, EngineError,
    EngineId,
};

pub(crate) const VERSION: &str = "2026-10-21";
const BETA: &str = "server-side-fallback-2026-07-01";
const MAX_TOKENS: u32 = 4096;
const MAX_SEARCHES: u32 = 5;
/// A paused turn is sent back at most this many times.
pub const MAX_CONTINUATIONS: usize = 3;

pub struct ClaudeEngine {
    client: reqwest::Client,
    api_key: String,
    model: String,
    base: String,
}

/// What one response from the messages API contributed.
struct Turn {
    text: String,
    stop_reason: String,
    stop_category: Option<String>,
    content: Value,
}

impl ClaudeEngine {
    pub fn new(api_key: &str, model: &str, base_url: &str, timeout: Duration) -> Self {
        Self {
            client: http_client(timeout),
            api_key: api_key.to_string(),
            model: model.to_string(),
            base: base_url.to_string(),
        }
    }

    async fn call(&self, body: &Value) -> Result<Value, EngineError> {
        send(
            self.client
                .post(join(&self.base, "/v1/messages"))
                .header("x-api-key", &self.api_key)
                .header("anthropic-version", VERSION)
                .header("anthropic-beta", BETA)
                .header("content-type", "application/json")
                .body(body.to_string()),
        )
        .await
    }

    fn search_body(&self, messages: &[Value]) -> Value {
        json!({
            "model": self.model,
            "max_tokens": MAX_TOKENS,
            "output_config": { "effort": "low" },
            "fallbacks": "default",
            "tools": [{ "type": "web_search_20260209", "name": "web_search", "max_uses": MAX_SEARCHES }],
            "messages": messages,
        })
    }

    /// One request without web search that must answer with JSON matching
    /// `schema` (structured outputs). Returns the first text block.
    ///
    /// The `output_config.format` shape (`type: json_schema` and `schema`) is
    /// from the plan; if the API rejects it the call fails with `Http(400)`.
    pub async fn complete_json(&self, prompt: &str, schema: &Value) -> Result<String, EngineError> {
        let body = json!({
            "model": self.model,
            "max_tokens": MAX_TOKENS,
            "output_config": {
                "effort": "low",
                "format": { "type": "json_schema", "schema": schema },
            },
            "messages": [{ "role": "user", "content": prompt }],
        });
        let response = self.call(&body).await?;
        let turn = read_turn(&response);
        refused(&turn)?;
        first_text_block(&response).ok_or(EngineError::Unreadable)
    }
}

fn refused(turn: &Turn) -> Result<(), EngineError> {
    if turn.stop_reason == "refusal" {
        let category = turn
            .stop_category
            .clone()
            .filter(|c| !c.is_empty() && c.len() <= 40)
            .filter(|c| {
                c.chars()
                    .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
            });
        return Err(EngineError::Refused(category));
    }
    Ok(())
}

fn first_text_block(response: &Value) -> Option<String> {
    response
        .get("content")?
        .as_array()?
        .iter()
        .find(|block| str_of(block, "type") == Some("text"))
        .and_then(|block| str_of(block, "text"))
        .map(str::to_string)
}

fn read_turn(response: &Value) -> Turn {
    let blocks = response
        .get("content")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let text = blocks
        .iter()
        .filter(|block| str_of(block, "type") == Some("text"))
        .filter_map(|block| str_of(block, "text"))
        .collect::<String>();
    Turn {
        text,
        stop_reason: str_of(response, "stop_reason")
            .unwrap_or("end_turn")
            .to_string(),
        stop_category: response
            .get("stop_details")
            .and_then(|details| str_of(details, "category"))
            .map(str::to_string),
        content: Value::Array(blocks),
    }
}

/// Adds what a turn's content holds to the answer: citations from text
/// blocks, and a note for each web search that failed.
fn absorb(answer: &mut EngineAnswer, response: &Value, turn: &Turn) {
    answer.text.push_str(&turn.text);
    let usage = response.get("usage");
    answer.input_tokens = answer
        .input_tokens
        .saturating_add(count(usage.and_then(|u| u.get("input_tokens"))));
    answer.output_tokens = answer
        .output_tokens
        .saturating_add(count(usage.and_then(|u| u.get("output_tokens"))));
    answer.searches = answer.searches.saturating_add(count(
        usage
            .and_then(|u| u.get("server_tool_use"))
            .and_then(|u| u.get("web_search_requests")),
    ));
    for block in turn.content.as_array().into_iter().flatten() {
        match str_of(block, "type") {
            Some("text") => {
                for cite in block
                    .get("citations")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter(|c| str_of(c, "type") == Some("web_search_result_location"))
                {
                    if let Some(url) = str_of(cite, "url") {
                        push_citation(&mut answer.citations, url, str_of(cite, "title"), None);
                    }
                }
            }
            Some("web_search_tool_result") => {
                // A list of results is fine; an object is an error such as
                // `{ "type": "web_search_tool_result_error", "error_code": "..." }`.
                if let Some(error) = block.get("content").filter(|c| c.is_object()) {
                    let code = str_of(error, "error_code").unwrap_or("unknown");
                    let code: String = code
                        .chars()
                        .filter(|c| c.is_ascii_alphanumeric() || *c == '_')
                        .take(40)
                        .collect();
                    answer.notes.push(format!("A web search failed ({code})."));
                }
            }
            _ => {}
        }
    }
}

#[async_trait]
impl Engine for ClaudeEngine {
    fn id(&self) -> EngineId {
        EngineId::Claude
    }

    fn model(&self) -> &str {
        &self.model
    }

    async fn ask(&self, prompt: &str) -> Result<EngineAnswer, EngineError> {
        let mut messages = vec![json!({ "role": "user", "content": prompt })];
        let mut answer = EngineAnswer::default();
        let mut continuations = 0;
        loop {
            let response = self.call(&self.search_body(&messages)).await?;
            let turn = read_turn(&response);
            refused(&turn)?;
            absorb(&mut answer, &response, &turn);
            if turn.stop_reason == "pause_turn" && continuations < MAX_CONTINUATIONS {
                continuations += 1;
                // The paused content goes back unchanged, as the next
                // assistant message, with nothing after it.
                messages.push(json!({ "role": "assistant", "content": turn.content }));
                continue;
            }
            if turn.stop_reason == "pause_turn" {
                answer
                    .notes
                    .push("The answer was still paused after the last continuation.".into());
            }
            if turn.stop_reason == "max_tokens" {
                answer
                    .notes
                    .push("The answer was cut short at the token limit.".into());
            }
            if answer.text.trim().is_empty() {
                return Err(EngineError::Unreadable);
            }
            return Ok(answer);
        }
    }
}
