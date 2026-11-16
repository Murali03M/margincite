//! Each engine against a mock that answers in the service's documented shape.
//! No test here reaches a real service.

mod common;

use std::time::Duration;

use common::ai::{
    claude_body, gemini_body, grok_body, openai_body, perplexity_body, CLAUDE_KEY, GEMINI_KEY,
    GROK_KEY, OPENAI_KEY, PERPLEXITY_KEY,
};
use margincite::ai::engines::gemini::GeminiEngine;
use margincite::ai::engines::grok::GrokEngine;
use margincite::ai::engines::openai::OpenAiEngine;
use margincite::ai::engines::perplexity::PerplexityEngine;
use margincite::ai::engines::{ClaudeEngine, Engine, EngineError};
use margincite::ai::suggest::suggest_prompts;
use serde_json::{json, Value};
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const T: Duration = Duration::from_secs(5);

fn claude(server: &MockServer) -> ClaudeEngine {
    ClaudeEngine::new(CLAUDE_KEY, "claude-opus-5-5", &server.uri(), T)
}

async fn mount_claude(server: &MockServer, body: Value, times: Option<u64>) {
    let mock = Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .respond_with(ResponseTemplate::new(200).set_body_json(body));
    match times {
        Some(n) => mock.up_to_n_times(n).mount(server).await,
        None => mock.mount(server).await,
    }
}

async fn bodies(server: &MockServer) -> Vec<Value> {
    server
        .received_requests()
        .await
        .unwrap()
        .iter()
        .map(|r| serde_json::from_slice(&r.body).unwrap())
        .collect()
}

// ------------------------------------------------------------------ Claude

#[tokio::test]
async fn claude_sends_the_documented_request_and_reads_text_citations_and_counts() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .and(header("x-api-key", CLAUDE_KEY))
        .and(header("anthropic-version", "2026-10-21"))
        .and(header("anthropic-beta", "server-side-fallback-2026-07-01"))
        .and(header("content-type", "application/json"))
        .respond_with(ResponseTemplate::new(200).set_body_json(claude_body(
            "Acme is a solid choice.",
            &[
                ("https://acme.com/a", "Acme"),
                ("https://www.rival.com/", "Rival"),
                ("https://acme.com/a", "Acme again"),
            ],
        )))
        .expect(1)
        .mount(&server)
        .await;
    let answer = claude(&server).ask("best widgets?").await.unwrap();
    assert_eq!(
        answer.text,
        "Let me search for that.Acme is a solid choice."
    );
    let urls: Vec<&str> = answer.citations.iter().map(|c| c.url.as_str()).collect();
    assert_eq!(urls, ["https://acme.com/a", "https://www.rival.com/"]);
    assert_eq!(answer.citations[0].title.as_deref(), Some("Acme"));
    assert_eq!(answer.citations[1].domain, "rival.com");
    assert_eq!(
        (answer.input_tokens, answer.output_tokens, answer.searches),
        (1200, 340, 2)
    );
    let sent = &bodies(&server).await[0];
    assert_eq!(sent["model"], "claude-opus-5-5");
    assert_eq!(sent["max_tokens"], 4096);
    assert_eq!(sent["output_config"], json!({"effort": "low"}));
    assert_eq!(sent["fallbacks"], "default");
    assert_eq!(
        sent["tools"],
        json!([{"type": "web_search_20260209", "name": "web_search", "max_uses": 5}])
    );
    assert_eq!(
        sent["messages"],
        json!([{"role": "user", "content": "best widgets?"}])
    );
    for forbidden in ["thinking", "temperature", "top_p", "top_k"] {
        assert!(
            sent.get(forbidden).is_none(),
            "{forbidden} must not be sent"
        );
    }
}

#[tokio::test]
async fn claude_continues_a_paused_turn_with_the_content_unchanged() {
    let server = MockServer::start().await;
    let mut paused = claude_body("First half. ", &[("https://a.com/", "A")]);
    paused["stop_reason"] = json!("pause_turn");
    let mut done = claude_body("Second half.", &[("https://b.com/", "B")]);
    done["content"] = json!([{ "type": "text", "text": "Second half.",
        "citations": [{"type": "web_search_result_location", "url": "https://b.com/", "title": "B"}] }]);
    done["usage"] = json!({"input_tokens": 10, "output_tokens": 5,
                           "server_tool_use": {"web_search_requests": 1}});
    mount_claude(&server, paused.clone(), Some(1)).await;
    mount_claude(&server, done, None).await;

    let answer = claude(&server).ask("q").await.unwrap();
    assert!(answer.text.ends_with("First half. Second half."));
    assert_eq!(answer.citations.len(), 2);
    assert_eq!(answer.searches, 3, "searches add up across turns");
    assert_eq!(answer.input_tokens, 1210);

    let sent = bodies(&server).await;
    assert_eq!(sent.len(), 2);
    let messages = sent[1]["messages"].as_array().unwrap();
    assert_eq!(messages.len(), 2, "no extra user message");
    assert_eq!(messages[0]["role"], "user");
    assert_eq!(messages[1]["role"], "assistant");
    assert_eq!(messages[1]["content"], paused["content"]);
    for body in &sent {
        for forbidden in ["thinking", "temperature", "top_p", "top_k"] {
            assert!(body.get(forbidden).is_none());
        }
    }
}

#[tokio::test]
async fn claude_stops_continuing_after_three_pauses_and_keeps_its_text() {
    let server = MockServer::start().await;
    let mut paused = claude_body("Part. ", &[]);
    paused["stop_reason"] = json!("pause_turn");
    mount_claude(&server, paused, None).await;
    let answer = claude(&server).ask("q").await.unwrap();
    assert_eq!(
        bodies(&server).await.len(),
        4,
        "one request and three continuations"
    );
    assert!(answer.text.contains("Part."));
    assert!(!answer.notes.is_empty());
}

#[tokio::test]
async fn claude_refusal_is_an_error_with_its_category() {
    let server = MockServer::start().await;
    let mut refused = claude_body("", &[]);
    refused["stop_reason"] = json!("refusal");
    refused["stop_details"] = json!({"type": "refusal", "category": "cyber"});
    mount_claude(&server, refused.clone(), Some(1)).await;
    let err = claude(&server).ask("q").await.unwrap_err();
    assert_eq!(err, EngineError::Refused(Some("cyber".into())));
    assert!(err.to_string().contains("cyber"));

    refused.as_object_mut().unwrap().remove("stop_details");
    mount_claude(&server, refused, None).await;
    assert_eq!(
        claude(&server).ask("q").await.unwrap_err(),
        EngineError::Refused(None)
    );
}

#[tokio::test]
async fn claude_search_error_inside_a_result_is_noted_and_the_answer_kept() {
    let server = MockServer::start().await;
    let mut body = claude_body("Still an answer.", &[]);
    body["content"][2]["content"] = json!({
        "type": "web_search_tool_result_error", "error_code": "max_uses_exceeded" });
    mount_claude(&server, body, None).await;
    let answer = claude(&server).ask("q").await.unwrap();
    assert!(answer.text.contains("Still an answer."));
    assert_eq!(answer.notes, ["A web search failed (max_uses_exceeded)."]);
}

#[tokio::test]
async fn claude_keeps_the_text_it_has_when_it_runs_out_of_tokens() {
    let server = MockServer::start().await;
    let mut body = claude_body("Half an ans", &[]);
    body["stop_reason"] = json!("max_tokens");
    mount_claude(&server, body, None).await;
    let answer = claude(&server).ask("q").await.unwrap();
    assert!(answer.text.ends_with("Half an ans"));
}

#[tokio::test]
async fn claude_with_no_text_at_all_is_an_unreadable_answer() {
    let server = MockServer::start().await;
    let mut body = claude_body("x", &[]);
    body["content"] = json!([]);
    mount_claude(&server, body, None).await;
    assert_eq!(
        claude(&server).ask("q").await.unwrap_err(),
        EngineError::Unreadable
    );
}

// ----------------------------------------------------------------- ChatGPT

#[tokio::test]
async fn chatgpt_reads_message_text_and_url_citations() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/responses"))
        .and(header(
            "authorization",
            format!("Bearer {OPENAI_KEY}").as_str(),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(openai_body(
            "Try Acme.",
            &[
                ("https://acme.com/", "Acme"),
                ("https://rival.com/x", "Rival"),
            ],
        )))
        .mount(&server)
        .await;
    let engine = OpenAiEngine::new(OPENAI_KEY, "gpt-5.6", &server.uri(), T);
    let answer = engine.ask("best widgets?").await.unwrap();
    assert_eq!(answer.text, "Try Acme.");
    assert_eq!(answer.citations.len(), 2);
    assert_eq!(answer.citations[1].domain, "rival.com");
    assert_eq!(
        (answer.input_tokens, answer.output_tokens, answer.searches),
        (900, 210, 1)
    );
    let sent = &bodies(&server).await[0];
    assert_eq!(
        *sent,
        json!({"model": "gpt-5.6", "input": "best widgets?", "tools": [{"type": "web_search"}]})
    );
}

#[tokio::test]
async fn chatgpt_without_a_message_is_unreadable() {
    let server = MockServer::start().await;
    let mut body = openai_body("x", &[]);
    body["output"] = json!([{"type": "web_search_call", "id": "ws_1"}]);
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(body))
        .mount(&server)
        .await;
    let engine = OpenAiEngine::new(OPENAI_KEY, "gpt-5.6", &server.uri(), T);
    assert_eq!(engine.ask("q").await.unwrap_err(), EngineError::Unreadable);
}

// -------------------------------------------------------------------- Grok

async fn grok_with(body: Value) -> Result<margincite::ai::engines::EngineAnswer, EngineError> {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/responses"))
        .respond_with(ResponseTemplate::new(200).set_body_json(body))
        .mount(&server)
        .await;
    GrokEngine::new(GROK_KEY, "grok-4.7", &server.uri(), T)
        .ask("q")
        .await
}

#[tokio::test]
async fn grok_sends_the_documented_request_and_merges_both_citation_shapes() {
    let server = MockServer::start().await;
    // The top-level list also names a source that was seen but not cited.
    let mut body = grok_body(
        "Try Acme.",
        &[
            ("https://acme.com/", "Acme"),
            ("https://rival.com/x", "Rival"),
        ],
    );
    body["citations"]
        .as_array_mut()
        .unwrap()
        .push(json!("https://seen-but-not-cited.example.net/page"));
    Mock::given(method("POST"))
        .and(path("/v1/responses"))
        .and(header(
            "authorization",
            format!("Bearer {GROK_KEY}").as_str(),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(body))
        .mount(&server)
        .await;
    let engine = GrokEngine::new(GROK_KEY, "grok-4.7", &server.uri(), T);
    let answer = engine.ask("best widgets?").await.unwrap();
    assert_eq!(answer.text, "Try Acme.");
    let urls: Vec<_> = answer.citations.iter().map(|c| c.url.as_str()).collect();
    assert_eq!(
        urls,
        [
            "https://acme.com/",
            "https://rival.com/x",
            "https://seen-but-not-cited.example.net/page"
        ],
        "inline citations first, then the rest of the top-level list, no duplicates"
    );
    assert_eq!(answer.citations[1].domain, "rival.com");
    assert_eq!(
        answer.citations[0].title, None,
        "a bare citation number is not a title"
    );
    assert_eq!(
        (answer.input_tokens, answer.output_tokens, answer.searches),
        (700, 150, 1)
    );
    assert_eq!(
        bodies(&server).await[0],
        json!({
            "model": "grok-4.7",
            "input": [{"role": "user", "content": "best widgets?"}],
            "tools": [{"type": "web_search"}]
        })
    );
}

#[tokio::test]
async fn grok_reads_a_real_title_and_citations_given_as_objects() {
    let mut body = grok_body("Text.", &[]);
    body["output"][1]["content"][0]["annotations"] = json!([
        {"type": "url_citation", "url": "https://a.com/", "title": "A real page title"},
        {"type": "other", "url": "https://ignored.com/"}
    ]);
    body["citations"] = json!([
        {"url": "https://b.com/", "title": "B"},
        "https://a.com/",
        {"no_url": true},
        42
    ]);
    let answer = grok_with(body).await.unwrap();
    let got: Vec<_> = answer
        .citations
        .iter()
        .map(|c| (c.url.as_str(), c.title.as_deref()))
        .collect();
    assert_eq!(
        got,
        [
            ("https://a.com/", Some("A real page title")),
            ("https://b.com/", Some("B"))
        ]
    );
}

#[tokio::test]
async fn grok_with_only_top_level_citations_still_has_them() {
    let mut body = grok_body("Text.", &[]);
    body["citations"] = json!(["https://x.com/i/status/1", "https://x.ai/news"]);
    let answer = grok_with(body).await.unwrap();
    assert_eq!(answer.citations.len(), 2);
    assert_eq!(answer.citations[1].domain, "x.ai");
}

#[tokio::test]
async fn grok_without_a_message_or_with_a_wrong_shape_is_unreadable() {
    for body in [
        json!({"output": [{"type": "web_search_call"}]}),
        json!({"output": "nope"}),
        json!({"choices": []}),
        json!({"output": [{"type": "message", "content": [{"type": "output_text", "text": "  "}]}]}),
    ] {
        assert_eq!(grok_with(body).await.unwrap_err(), EngineError::Unreadable);
    }
}

#[tokio::test]
async fn grok_http_failures_become_plain_errors_that_never_repeat_the_body() {
    for (status, message) in [
        (401, "The engine rejected the API key (HTTP 401)."),
        (429, "The engine refused the request because the key's rate limit or quota is used up (HTTP 429). Check the plan and billing for this key with the provider."),
        (503, "The engine answered with HTTP 503."),
    ] {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(status).set_body_string(GROK_KEY))
            .mount(&server)
            .await;
        let err = GrokEngine::new(GROK_KEY, "grok-4.7", &server.uri(), T)
            .ask("q")
            .await
            .unwrap_err();
        assert_eq!(err.to_string(), message);
        assert!(!err.to_string().contains(GROK_KEY));
    }
}

// -------------------------------------------------------------- Perplexity

async fn perplexity_with(
    body: Value,
) -> Result<margincite::ai::engines::EngineAnswer, EngineError> {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .and(header(
            "authorization",
            format!("Bearer {PERPLEXITY_KEY}").as_str(),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(body))
        .mount(&server)
        .await;
    let engine = PerplexityEngine::new(PERPLEXITY_KEY, "sonar", &server.uri(), T);
    let result = engine.ask("q").await;
    let sent = &bodies(&server).await[0];
    assert_eq!(
        *sent,
        json!({"model": "sonar", "messages": [{"role": "user", "content": "q"}]})
    );
    result
}

#[tokio::test]
async fn perplexity_prefers_search_results_with_titles() {
    let answer = perplexity_with(perplexity_body(
        "Acme it is.",
        &[
            ("https://acme.com/", "Acme"),
            ("https://www.rival.com/p", "Rival"),
        ],
    ))
    .await
    .unwrap();
    assert_eq!(answer.text, "Acme it is.");
    assert_eq!(answer.citations.len(), 2);
    assert_eq!(answer.citations[0].title.as_deref(), Some("Acme"));
    assert_eq!(answer.citations[1].domain, "rival.com");
    assert_eq!((answer.input_tokens, answer.output_tokens), (40, 300));
}

#[tokio::test]
async fn perplexity_falls_back_to_plain_citation_strings() {
    let body = json!({
        "citations": ["https://a.com/1", "https://b.com/2", "https://a.com/1"],
        "choices": [{"message": {"role": "assistant", "content": "Text"}}]
    });
    let answer = perplexity_with(body).await.unwrap();
    let urls: Vec<&str> = answer.citations.iter().map(|c| c.url.as_str()).collect();
    assert_eq!(urls, ["https://a.com/1", "https://b.com/2"]);
    assert!(answer.citations[0].title.is_none());
}

#[tokio::test]
async fn perplexity_reads_citations_from_inside_the_message_too() {
    let body = json!({
        "choices": [{"message": {"role": "assistant", "content": "Text",
                                 "search_results": [{"url": "https://m.com/", "title": "M"}]}}]
    });
    let answer = perplexity_with(body).await.unwrap();
    assert_eq!(answer.citations[0].domain, "m.com");
    let body = json!({
        "choices": [{"message": {"role": "assistant", "content": "Text",
                                 "citations": ["https://n.com/"]}}]
    });
    assert_eq!(
        perplexity_with(body).await.unwrap().citations[0].domain,
        "n.com"
    );
}

#[tokio::test]
async fn perplexity_with_no_choices_is_unreadable() {
    let result = perplexity_with(json!({"choices": []})).await;
    assert_eq!(result.unwrap_err(), EngineError::Unreadable);
}

// ------------------------------------------------------------------ Gemini

#[tokio::test]
async fn gemini_reads_text_and_takes_the_domain_from_the_title_without_following_the_redirect() {
    let server = MockServer::start().await;
    let redirect_a = format!("{}/grounding-api-redirect/AAA", server.uri());
    let redirect_b = format!("{}/grounding-api-redirect/BBB", server.uri());
    Mock::given(method("POST"))
        .and(path("/v1beta/models/gem-x:generateContent"))
        .and(header("x-goog-api-key", GEMINI_KEY))
        .respond_with(ResponseTemplate::new(200).set_body_json(gemini_body(
            "Acme again.",
            &[
                (&redirect_a, "www.acme.com"),
                (&redirect_b, "Some Article Title"),
            ],
        )))
        .mount(&server)
        .await;
    let engine = GeminiEngine::new(GEMINI_KEY, "gem-x", &server.uri(), T);
    let answer = engine.ask("q").await.unwrap();
    assert_eq!(answer.text, "Acme again.");
    assert_eq!(answer.citations[0].domain, "acme.com");
    assert_eq!(answer.citations[0].url, redirect_a);
    assert_eq!(
        answer.citations[1].domain, "",
        "a title that is not a host names no domain"
    );
    assert_eq!(
        (answer.input_tokens, answer.output_tokens, answer.searches),
        (25, 180, 1)
    );
    let requests = server.received_requests().await.unwrap();
    assert_eq!(
        requests.len(),
        1,
        "the redirect addresses are never fetched"
    );
    let sent: Value = serde_json::from_slice(&requests[0].body).unwrap();
    assert_eq!(
        sent,
        json!({"contents": [{"parts": [{"text": "q"}]}], "tools": [{"google_search": {}}]})
    );
}

#[tokio::test]
async fn gemini_with_no_candidates_is_unreadable() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"candidates": []})))
        .mount(&server)
        .await;
    let engine = GeminiEngine::new(GEMINI_KEY, "gem-x", &server.uri(), T);
    assert_eq!(engine.ask("q").await.unwrap_err(), EngineError::Unreadable);
}

// ------------------------------------------------------------ every engine

#[tokio::test]
async fn failures_become_plain_errors_that_never_repeat_the_service_or_the_key() {
    let server = MockServer::start().await;
    let echoing =
        json!({"error": {"message": format!("Incorrect API key provided: {OPENAI_KEY}")}});
    Mock::given(method("POST"))
        .and(path("/v1/responses"))
        .respond_with(ResponseTemplate::new(401).set_body_json(echoing))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(ResponseTemplate::new(500).set_body_string("<html>boom</html>"))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .respond_with(ResponseTemplate::new(200).set_body_string("not json {"))
        .mount(&server)
        .await;
    let openai = OpenAiEngine::new(OPENAI_KEY, "m", &server.uri(), T);
    let err = openai.ask("q").await.unwrap_err();
    assert_eq!(err, EngineError::Http(401));
    assert!(!err.to_string().contains(OPENAI_KEY));
    assert!(err.to_string().contains("API key"));
    let pplx = PerplexityEngine::new(PERPLEXITY_KEY, "m", &server.uri(), T);
    assert_eq!(pplx.ask("q").await.unwrap_err(), EngineError::Http(500));
    assert_eq!(
        claude(&server).ask("q").await.unwrap_err(),
        EngineError::Unreadable
    );
}

#[tokio::test]
async fn a_slow_engine_times_out() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(openai_body("late", &[]))
                .set_delay(Duration::from_secs(3)),
        )
        .mount(&server)
        .await;
    let engine = OpenAiEngine::new(OPENAI_KEY, "m", &server.uri(), Duration::from_millis(300));
    assert_eq!(engine.ask("q").await.unwrap_err(), EngineError::Timeout);
}

#[tokio::test]
async fn an_engine_that_cannot_be_reached_is_a_plain_error() {
    let engine = OpenAiEngine::new(OPENAI_KEY, "m", "http://127.0.0.1:9", T);
    assert_eq!(engine.ask("q").await.unwrap_err(), EngineError::Transport);
}

#[tokio::test]
async fn an_engine_never_follows_a_redirect_with_its_key() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/responses"))
        .respond_with(
            ResponseTemplate::new(307)
                .insert_header("location", format!("{}/elsewhere", server.uri()).as_str()),
        )
        .mount(&server)
        .await;
    let engine = OpenAiEngine::new(OPENAI_KEY, "m", &server.uri(), T);
    assert_eq!(engine.ask("q").await.unwrap_err(), EngineError::Http(307));
    assert_eq!(server.received_requests().await.unwrap().len(), 1);
}

// ------------------------------------------------------------- suggestions

fn suggestion_body(questions: Value) -> Value {
    let mut body = claude_body("x", &[]);
    body["content"] =
        json!([{ "type": "text", "text": json!({"questions": questions}).to_string() }]);
    body
}

#[tokio::test]
async fn suggestions_use_structured_output_without_web_search_and_drop_brand_questions() {
    let server = MockServer::start().await;
    mount_claude(
        &server,
        suggestion_body(json!([
            "Which widget shop ships to Canada quickly?",
            "Is Acme worth the price for widgets?",
            "How do I choose widgets for a small bakery?",
            "Which widget shop ships to canada quickly?"
        ])),
        None,
    )
    .await;
    let found = suggest_prompts(
        &claude(&server),
        "Widget shop",
        "We sell widgets",
        &["widgets".to_string()],
        "Acme",
    )
    .await
    .unwrap();
    assert_eq!(
        found,
        [
            "Which widget shop ships to Canada quickly?",
            "How do I choose widgets for a small bakery?"
        ]
    );
    let sent = &bodies(&server).await[0];
    assert!(sent.get("tools").is_none(), "no web search for suggestions");
    assert!(sent.get("fallbacks").is_none());
    assert_eq!(sent["output_config"]["effort"], "low");
    assert_eq!(sent["output_config"]["format"]["type"], "json_schema");
    assert_eq!(
        sent["output_config"]["format"]["schema"],
        json!({
            "type": "object",
            "properties": {"questions": {"type": "array", "items": {"type": "string"}}},
            "required": ["questions"],
            "additionalProperties": false
        })
    );
    for forbidden in ["thinking", "temperature", "top_p", "top_k"] {
        assert!(sent.get(forbidden).is_none());
    }
    let prompt = sent["messages"][0]["content"].as_str().unwrap();
    assert!(prompt.contains("Widget shop") && prompt.contains("We sell widgets"));
    assert!(prompt.contains("widgets"));
}

#[tokio::test]
async fn suggestions_that_are_not_json_or_are_refused_are_errors() {
    let server = MockServer::start().await;
    let mut junk = suggestion_body(json!([]));
    junk["content"] = json!([{"type": "text", "text": "Sure! Here you go"}]);
    mount_claude(&server, junk, Some(1)).await;
    let engine = claude(&server);
    assert_eq!(
        suggest_prompts(&engine, "s", "d", &[], "")
            .await
            .unwrap_err(),
        EngineError::Unreadable
    );
    let mut refused = suggestion_body(json!([]));
    refused["stop_reason"] = json!("refusal");
    mount_claude(&server, refused, None).await;
    assert!(matches!(
        suggest_prompts(&engine, "s", "d", &[], "")
            .await
            .unwrap_err(),
        EngineError::Refused(_)
    ));
}
