//! AI visibility helpers on top of `GoogleWorld`: the same mock server plays
//! all five engines under `/claude`, `/openai`, `/grok`, `/perplexity` and `/gemini`.
//! The response bodies are written from the services' documented shapes.

use std::sync::Arc;
use std::time::Duration;

use margincite::ai::engines::EngineFactory;
use margincite::ai::run::AiRunJob;
use margincite::jobs::queue::Job;
use margincite::jobs::worker::JobHandler;
use serde_json::{json, Value};
use uuid::Uuid;
use wiremock::matchers::{body_string_contains, method, path};
use wiremock::{Mock, ResponseTemplate};

use super::google::GoogleWorld;
use super::{engine_bases, test_config, ENGINE_SETTINGS, GOOGLE_SETTINGS};

pub const CLAUDE_KEY: &str = "sk-ant-secret-KEY-111";
pub const OPENAI_KEY: &str = "sk-openai-secret-KEY-222";
pub const PERPLEXITY_KEY: &str = "pplx-secret-KEY-333";
pub const GEMINI_KEY: &str = "gem-secret-KEY-444";
pub const GROK_KEY: &str = "xai-secret-KEY-555";
/// In the order of `Engines`: claude, chatgpt, grok, gemini, perplexity are
/// not the order here; tests index by name.
pub const KEYS: [&str; 5] = [CLAUDE_KEY, OPENAI_KEY, PERPLEXITY_KEY, GEMINI_KEY, GROK_KEY];

pub const Q1: &str = "What is the best widget shop for a small business?";
pub const Q2: &str = "Where can I buy handmade widgets online?";

/// Google's settings plus a key for every engine.
pub fn ai_settings() -> Vec<(&'static str, &'static str)> {
    GOOGLE_SETTINGS
        .iter()
        .chain(ENGINE_SETTINGS.iter())
        .copied()
        .collect()
}

// ------------------------------------------------------- response bodies

/// A Claude answer that searched the web and cited these `(url, title)`s.
pub fn claude_body(text: &str, cites: &[(&str, &str)]) -> Value {
    let results: Vec<Value> = cites
        .iter()
        .map(|(url, title)| {
            json!({ "type": "web_search_result", "url": url, "title": title,
                    "encrypted_content": "EqgfCioIARgB", "page_age": null })
        })
        .collect();
    let citations: Vec<Value> = cites
        .iter()
        .map(|(url, title)| {
            json!({ "type": "web_search_result_location", "url": url, "title": title,
                    "cited_text": "a sentence from the page", "encrypted_index": "Eo8BCioIAhgB" })
        })
        .collect();
    json!({
        "id": "msg_01ABC", "type": "message", "role": "assistant", "model": "claude-opus-5-5",
        "stop_reason": "end_turn", "stop_sequence": null,
        "content": [
            { "type": "text", "text": "Let me search for that." },
            { "type": "server_tool_use", "id": "srvtoolu_01", "name": "web_search",
              "input": { "query": "best widget shop" } },
            { "type": "web_search_tool_result", "tool_use_id": "srvtoolu_01", "content": results },
            { "type": "text", "text": text, "citations": citations }
        ],
        "usage": { "input_tokens": 1200, "output_tokens": 340,
                   "server_tool_use": { "web_search_requests": 2 } }
    })
}

pub fn openai_body(text: &str, cites: &[(&str, &str)]) -> Value {
    let annotations: Vec<Value> = cites
        .iter()
        .map(|(url, title)| {
            json!({ "type": "url_citation", "start_index": 0, "end_index": 10,
                    "url": url, "title": title })
        })
        .collect();
    json!({
        "id": "resp_01", "object": "response", "status": "completed", "model": "gpt-5.6",
        "output": [
            { "type": "web_search_call", "id": "ws_01", "status": "completed" },
            { "type": "message", "id": "msg_01", "status": "completed", "role": "assistant",
              "content": [{ "type": "output_text", "text": text, "annotations": annotations }] }
        ],
        "usage": { "input_tokens": 900, "output_tokens": 210, "total_tokens": 1110 }
    })
}

/// A Grok answer in the Responses shape xAI documents: a `message` item whose
/// `output_text` part has `url_citation` annotations (titled with the visible
/// citation number), plus the top-level `citations` list of the sources.
pub fn grok_body(text: &str, cites: &[(&str, &str)]) -> Value {
    let annotations: Vec<Value> = cites
        .iter()
        .enumerate()
        .map(|(n, (url, _))| {
            json!({ "type": "url_citation", "url": url, "title": (n + 1).to_string(),
                    "start_index": 0, "end_index": 10 })
        })
        .collect();
    let urls: Vec<&str> = cites.iter().map(|(url, _)| *url).collect();
    json!({
        "id": "resp_grok_01", "object": "response", "model": "grok-4.7",
        "output": [
            { "type": "web_search_call", "id": "ws_g1", "status": "completed" },
            { "type": "message", "id": "msg_g1", "role": "assistant", "status": "completed",
              "content": [{ "type": "output_text", "text": text, "logprobs": [],
                            "annotations": annotations }] }
        ],
        "citations": urls,
        "usage": { "input_tokens": 700, "output_tokens": 150 }
    })
}

pub fn perplexity_body(text: &str, cites: &[(&str, &str)]) -> Value {
    let results: Vec<Value> = cites
        .iter()
        .map(|(url, title)| json!({ "title": title, "url": url, "date": null }))
        .collect();
    let urls: Vec<&str> = cites.iter().map(|(url, _)| *url).collect();
    json!({
        "id": "pplx-01", "model": "sonar", "object": "chat.completion",
        "citations": urls, "search_results": results,
        "choices": [{ "index": 0, "finish_reason": "stop",
                      "message": { "role": "assistant", "content": text } }],
        "usage": { "prompt_tokens": 40, "completion_tokens": 300, "total_tokens": 340 }
    })
}

/// `cites` are `(redirect uri, title)`; Gemini's title is the site's domain.
pub fn gemini_body(text: &str, cites: &[(&str, &str)]) -> Value {
    let chunks: Vec<Value> = cites
        .iter()
        .map(|(uri, title)| json!({ "web": { "uri": uri, "title": title } }))
        .collect();
    json!({
        "candidates": [{
            "content": { "role": "model", "parts": [{ "text": text }] },
            "finishReason": "STOP",
            "groundingMetadata": {
                "webSearchQueries": ["best widget shop"],
                "groundingChunks": chunks
            }
        }],
        "usageMetadata": { "promptTokenCount": 25, "candidatesTokenCount": 180, "totalTokenCount": 205 }
    })
}

// ------------------------------------------------------------- the world

impl GoogleWorld {
    /// A world whose app has a key for every engine, all answered by the mock.
    pub async fn with_ai(pool: sqlx::PgPool) -> GoogleWorld {
        GoogleWorld::with_settings(pool, &ai_settings()).await
    }

    pub fn ai_path(&self, tail: &str) -> String {
        format!("/api/v1/sites/{}/ai{tail}", self.site_id)
    }

    pub async fn mount(&self, path_: &str, status: u16, body: Value, only_for: Option<&str>) {
        let mut mock = Mock::given(method("POST")).and(path(path_.to_string()));
        if let Some(text) = only_for {
            mock = mock.and(body_string_contains(text));
        }
        mock.respond_with(ResponseTemplate::new(status).set_body_json(body))
            .mount(&self.mock)
            .await;
    }

    /// Every engine answers every question with `text`; none cites anything
    /// unless `cites` says so. `only_for` limits the mocks to one question.
    ///
    /// The server answers with the first mock that matches, so mount the
    /// specific mocks (with `only_for`) before the general ones.
    pub async fn mock_engines(&self, text: &str, cites: &[(&str, &str)], only_for: Option<&str>) {
        // Gemini's title is the cited site's domain.
        let hosts: Vec<String> = cites
            .iter()
            .map(|(url, _)| margincite::ai::engines::domain_of(url))
            .collect();
        let gemini_cites: Vec<(&str, &str)> = cites
            .iter()
            .zip(&hosts)
            .map(|((url, _), host)| (*url, host.as_str()))
            .collect();
        self.mount(
            "/claude/v1/messages",
            200,
            claude_body(text, cites),
            only_for,
        )
        .await;
        self.mount(
            "/openai/v1/responses",
            200,
            openai_body(text, cites),
            only_for,
        )
        .await;
        self.mount("/grok/v1/responses", 200, grok_body(text, cites), only_for)
            .await;
        self.mount(
            "/perplexity/chat/completions",
            200,
            perplexity_body(text, cites),
            only_for,
        )
        .await;
        self.mount(
            "/gemini/v1beta/models/gemini-3.5-flash:generateContent",
            200,
            gemini_body(text, &gemini_cites),
            only_for,
        )
        .await;
    }

    pub async fn add_prompt(&self, text: &str) -> (u16, Value) {
        self.send(
            "test-token",
            reqwest::Method::POST,
            &self.ai_path("/prompts"),
            Some(json!({ "text": text })),
        )
        .await
    }

    pub async fn put_ai_settings(&self, brand: &str, aliases: &[&str], competitors: &[&str]) {
        let (status, body) = self
            .send(
                "test-token",
                reqwest::Method::PUT,
                &self.ai_path("/settings"),
                Some(
                    json!({ "brand_name": brand, "aliases": aliases, "competitors": competitors }),
                ),
            )
            .await;
        assert_eq!(status, 200, "{body}");
    }

    pub async fn start_run(&self) -> (u16, Value) {
        self.post(&self.ai_path("/runs")).await
    }

    /// Builds the engines the job asks, from the mock.
    pub fn ai_engines(&self, timeout: Duration) -> Arc<EngineFactory> {
        let bases = engine_bases(&self.mock.uri());
        Arc::new(EngineFactory::new(bases, timeout))
    }

    pub fn ai_handler(&self) -> AiRunJob {
        AiRunJob::new(
            self.pool.clone(),
            Arc::new(test_config(&ai_settings())),
            self.ai_engines(Duration::from_secs(10)),
        )
    }

    pub fn ai_job(&self, run_id: Option<Uuid>) -> Job {
        let mut payload = json!({ "workspace_id": self.workspace_id, "site_id": self.site_id });
        if let Some(id) = run_id {
            payload["run_id"] = json!(id);
        }
        Job {
            id: Uuid::new_v4(),
            kind: "ai_run".into(),
            payload,
            attempts: 1,
        }
    }

    pub async fn run_ai_job(&self, run_id: Option<Uuid>) -> anyhow::Result<()> {
        self.ai_handler().run(&self.ai_job(run_id)).await
    }

    /// Starts a run over the API and lets the worker handle it.
    pub async fn run_to_end(&self) -> Uuid {
        let (status, body) = self.start_run().await;
        assert_eq!(status, 202, "{body}");
        let id: Uuid = body["data"]["id"].as_str().unwrap().parse().unwrap();
        self.run_ai_job(Some(id)).await.unwrap();
        id
    }

    pub async fn ai_run(&self, id: Uuid) -> Value {
        let (status, body) = self.get(&format!("/api/v1/ai/runs/{id}")).await;
        assert_eq!(status, 200, "{body}");
        body["data"].clone()
    }

    pub async fn ai_answers(&self, run_id: Uuid) -> Vec<Value> {
        let (status, body) = self.get(&format!("/api/v1/ai/runs/{run_id}/answers")).await;
        assert_eq!(status, 200, "{body}");
        body["data"].as_array().unwrap().clone()
    }

    pub async fn ai_metrics(&self) -> Value {
        let (status, body) = self
            .get(&format!("/api/v1/sites/{}/metrics", self.site_id))
            .await;
        assert_eq!(status, 200, "{body}");
        body["data"]["ai_visibility"].clone()
    }

    pub async fn queued_ai_jobs(&self) -> Vec<Value> {
        let rows: Vec<(Value,)> = sqlx::query_as(
            "SELECT payload FROM jobs WHERE kind = 'ai_run' AND status = 'queued' ORDER BY run_at",
        )
        .fetch_all(&self.pool)
        .await
        .unwrap();
        rows.into_iter().map(|r| r.0).collect()
    }

    pub async fn requests_with(&self, path_: &str) -> Vec<wiremock::Request> {
        self.requests_to(path_).await
    }
}
