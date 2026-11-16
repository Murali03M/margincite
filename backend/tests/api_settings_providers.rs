mod common;

use std::sync::Arc;
use std::time::Duration;

use common::ai::{openai_body, Q1};
use common::google::GoogleWorld;
use common::settings::*;
use common::test_config;
use margincite::ai::run::AiRunJob;
use margincite::jobs::worker::JobHandler;
use reqwest::Method;
use serde_json::{json, Value};
use sqlx::PgPool;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, ResponseTemplate};

// -------------------------------------------------------- testing providers

#[sqlx::test]
async fn each_provider_is_tested_with_one_cheap_request(pool: PgPool) {
    let w = world(pool).await;
    Mock::given(method("GET"))
        .and(path("/claude/v1/models"))
        .and(header("x-api-key", CLAUDE_SECRET))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({})))
        .mount(&w.mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/openai/v1/models"))
        .and(header(
            "authorization",
            format!("Bearer {OPENAI_SECRET}").as_str(),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({})))
        .mount(&w.mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/gemini/v1beta/models"))
        .and(header("x-goog-api-key", "gem-key-1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({})))
        .mount(&w.mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/grok/v1/models"))
        .and(header("authorization", "Bearer xai-key-1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({})))
        .mount(&w.mock)
        .await;
    Mock::given(method("POST"))
        .and(path("/perplexity/chat/completions"))
        .and(header("authorization", "Bearer pplx-key-1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({})))
        .mount(&w.mock)
        .await;
    for (name, key) in [
        ("claude", CLAUDE_SECRET),
        ("chatgpt", OPENAI_SECRET),
        ("gemini", "gem-key-1"),
        ("grok", "xai-key-1"),
        ("perplexity", "pplx-key-1"),
    ] {
        let (s, body, raw) = put(
            &w,
            &format!("/api/v1/settings/providers/{name}"),
            json!({ "api_key": key }),
        )
        .await;
        assert_eq!(s, 200, "{raw}");
        assert_eq!(body["data"]["status"], "ok", "{name}: {raw}");
        assert_eq!(body["data"]["error"], Value::Null);
    }
}

#[sqlx::test]
async fn a_rejected_key_is_still_saved_and_says_so_in_plain_words(pool: PgPool) {
    let w = world(pool).await;
    // The provider echoes the key back; none of it may reach the person.
    Mock::given(method("GET"))
        .and(path("/openai/v1/models"))
        .respond_with(ResponseTemplate::new(401).set_body_string(format!(
            "{{\"error\":\"Incorrect API key provided: {OPENAI_SECRET}\"}}"
        )))
        .mount(&w.mock)
        .await;
    let (s, body, raw) = put(
        &w,
        "/api/v1/settings/providers/chatgpt",
        json!({ "api_key": OPENAI_SECRET }),
    )
    .await;
    assert_eq!(s, 200, "{raw}");
    assert_eq!(body["data"]["status"], "error");
    assert_eq!(body["data"]["error"], "The key was rejected.");
    assert_eq!(body["data"]["key_set"], true);
    assert_no_secret(&raw);
    let (data, raw) = settings(&w).await;
    assert_eq!(provider(&data, "chatgpt")["status"], "error");
    assert_no_secret(&raw);
    let stored: Vec<(Option<String>,)> = sqlx::query_as("SELECT error FROM provider_status")
        .fetch_all(&w.pool)
        .await
        .unwrap();
    assert!(stored
        .iter()
        .all(|(e,)| !e.as_deref().unwrap_or("").contains("SECRET")));
}

#[sqlx::test]
async fn other_failures_say_what_happened_without_the_provider_words(pool: PgPool) {
    let w = world(pool).await;
    Mock::given(method("GET"))
        .and(path("/claude/v1/models"))
        .respond_with(ResponseTemplate::new(500).set_body_string(CLAUDE_SECRET))
        .mount(&w.mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/openai/v1/models"))
        .respond_with(ResponseTemplate::new(200).set_body_string("not json"))
        .mount(&w.mock)
        .await;
    let (_, body, raw) = put(
        &w,
        "/api/v1/settings/providers/claude",
        json!({ "api_key": CLAUDE_SECRET }),
    )
    .await;
    assert_eq!(
        body["data"]["error"],
        "The provider answered with HTTP 500."
    );
    assert_no_secret(&raw);
    let (_, body, _) = put(
        &w,
        "/api/v1/settings/providers/chatgpt",
        json!({ "api_key": OPENAI_SECRET }),
    )
    .await;
    assert_eq!(
        body["data"]["error"],
        "The provider's answer could not be read."
    );
}

#[sqlx::test]
async fn a_provider_that_cannot_be_reached_is_an_error_not_a_crash(pool: PgPool) {
    // Gemini is not mocked: its address answers 404 from the mock; point the
    // dead-base app at a port nothing listens on instead.
    let app = common::spawn_app_with(pool, KEY_ONLY).await;
    let response = app
        .client
        .put(format!(
            "http://{}/api/v1/settings/providers/gemini",
            app.addr
        ))
        .header("authorization", "Bearer test-token")
        .json(&json!({ "api_key": "gem-key-2" }))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let body: Value = response.json().await.unwrap();
    assert_eq!(body["data"]["status"], "error");
    assert_eq!(body["data"]["error"], "The provider could not be reached.");
}

#[sqlx::test]
async fn testing_a_provider_with_no_key_does_nothing_harmful(pool: PgPool) {
    let w = world(pool).await;
    let (s, body, _) = call(
        &w,
        "test-token",
        Method::POST,
        "/api/v1/settings/providers/claude/test",
        None,
    )
    .await;
    assert_eq!(s, 200);
    assert_eq!(body["data"]["status"], "unchecked");
    let (s, _, _) = call(
        &w,
        "test-token",
        Method::POST,
        "/api/v1/settings/providers/nobody/test",
        None,
    )
    .await;
    assert_eq!(s, 404);
}

// --------------------------------------------------- features use the settings

#[sqlx::test]
async fn a_workspace_with_only_a_saved_key_can_run_ai_visibility(pool: PgPool) {
    let w = world(pool).await;
    mock_models_ok(&w).await;
    put(
        &w,
        "/api/v1/settings/providers/chatgpt",
        json!({ "api_key": OPENAI_SECRET }),
    )
    .await;
    w.mount(
        "/openai/v1/responses",
        200,
        openai_body("Acme Widgets is great.", &[]),
        None,
    )
    .await;
    w.add_prompt(Q1).await;
    let (s, body) = w.start_run().await;
    assert_eq!(s, 202, "{body}");
    assert_eq!(body["data"]["engines"], json!(["chatgpt"]));
    let id: uuid::Uuid = body["data"]["id"].as_str().unwrap().parse().unwrap();
    let job = AiRunJob::new(
        w.pool.clone(),
        Arc::new(test_config(KEY_ONLY)),
        w.ai_engines(Duration::from_secs(10)),
    );
    job.run(&w.ai_job(Some(id))).await.unwrap();
    let answers = w.ai_answers(id).await;
    assert_eq!(answers.len(), 1);
    assert_eq!(answers[0]["error"], Value::Null);
    let sent = w.requests_to("/openai/v1/responses").await;
    assert_eq!(
        sent[0]
            .headers
            .get("authorization")
            .unwrap()
            .to_str()
            .unwrap(),
        format!("Bearer {OPENAI_SECRET}")
    );
}

#[sqlx::test]
async fn an_ai_run_is_refused_when_the_workspace_has_no_key(pool: PgPool) {
    let w = world(pool).await;
    w.add_prompt(Q1).await;
    let (s, body) = w.start_run().await;
    assert_eq!(s, 422);
    assert!(body["error"].as_str().unwrap().contains("Settings"));
}

#[sqlx::test]
async fn the_environment_key_still_works_for_ai_runs_and_integrations(pool: PgPool) {
    let w = GoogleWorld::with_ai(pool).await;
    let (_, body) = w.get("/api/v1/integrations").await;
    let engines = body["data"]["engines"].as_array().unwrap();
    assert!(engines.iter().all(|e| e["configured"] == true));
    // And a key saved by one workspace shows in /integrations for it only.
    let (_, other, _) = call(&w, "other-token", Method::GET, "/api/v1/integrations", None).await;
    assert!(other["data"]["engines"]
        .as_array()
        .unwrap()
        .iter()
        .all(|e| e["configured"] == true));
}

#[sqlx::test]
async fn integrations_reflect_the_workspaces_own_settings(pool: PgPool) {
    let w = world(pool).await;
    mock_models_ok(&w).await;
    put(
        &w,
        "/api/v1/settings/providers/gemini",
        json!({ "api_key": "gem-saved-1", "model": "gem-mine" }),
    )
    .await;
    put(
        &w,
        "/api/v1/settings/google",
        json!({ "client_id": GOOGLE_ID, "client_secret": GOOGLE_SECRET }),
    )
    .await;
    let (_, body, raw) = call(&w, "test-token", Method::GET, "/api/v1/integrations", None).await;
    let gemini = body["data"]["engines"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["engine"] == "gemini")
        .unwrap();
    assert_eq!(gemini["configured"], true);
    assert_eq!(gemini["model"], "gem-mine");
    assert_eq!(body["data"]["google"]["configured"], true);
    assert_no_secret(&raw);
    let (_, other, _) = call(&w, "other-token", Method::GET, "/api/v1/integrations", None).await;
    assert_eq!(other["data"]["google"]["configured"], false);
}

#[sqlx::test]
async fn google_sign_in_uses_the_workspaces_credentials(pool: PgPool) {
    let w = world(pool).await;
    let start = format!("/api/v1/sites/{}/integrations/google/start", w.site_id);
    let (s, body, _) = call(&w, "test-token", Method::POST, &start, None).await;
    assert_eq!(s, 422, "not set up yet: {body}");
    assert!(body["error"].as_str().unwrap().contains("Settings"));
    put(
        &w,
        "/api/v1/settings/google",
        json!({ "client_id": GOOGLE_ID, "client_secret": GOOGLE_SECRET }),
    )
    .await;
    let (s, body, _) = call(&w, "test-token", Method::POST, &start, None).await;
    assert_eq!(s, 200, "{body}");
    let url = body["data"]["auth_url"].as_str().unwrap();
    assert!(url.contains(GOOGLE_ID), "{url}");
}

#[sqlx::test]
async fn google_sign_in_still_uses_environment_credentials(pool: PgPool) {
    let w = GoogleWorld::new(pool).await;
    let start = format!("/api/v1/sites/{}/integrations/google/start", w.site_id);
    let (s, body, _) = call(&w, "test-token", Method::POST, &start, None).await;
    assert_eq!(s, 200, "{body}");
    assert!(body["data"]["auth_url"]
        .as_str()
        .unwrap()
        .contains("test-client-id"));
}

#[sqlx::test]
async fn a_workspace_with_only_a_grok_key_can_run_ai_visibility(pool: PgPool) {
    let w = world(pool).await;
    mock_models_ok(&w).await;
    let (s, body, _) = put(
        &w,
        "/api/v1/settings/providers/grok",
        json!({ "api_key": "xai-saved-SECRET-1" }),
    )
    .await;
    assert_eq!(s, 200);
    assert_eq!(body["data"]["model"], "grok-4.7");
    assert_eq!(body["data"]["status"], "ok");
    w.mount(
        "/grok/v1/responses",
        200,
        common::ai::grok_body("Acme Widgets is great.", &[("https://example.com/p", "Me")]),
        None,
    )
    .await;
    w.put_ai_settings("Acme", &[], &[]).await;
    w.add_prompt(Q1).await;
    let (s, body) = w.start_run().await;
    assert_eq!(s, 202, "{body}");
    assert_eq!(body["data"]["engines"], json!(["grok"]));
    let id: uuid::Uuid = body["data"]["id"].as_str().unwrap().parse().unwrap();
    let job = AiRunJob::new(
        w.pool.clone(),
        Arc::new(test_config(KEY_ONLY)),
        w.ai_engines(Duration::from_secs(10)),
    );
    job.run(&w.ai_job(Some(id))).await.unwrap();
    let answers = w.ai_answers(id).await;
    assert_eq!(answers.len(), 1);
    assert_eq!(answers[0]["engine"], "grok");
    assert_eq!(answers[0]["error"], Value::Null);
    assert_eq!(answers[0]["mentioned"], true);
    assert_eq!(answers[0]["cited"], true);
    let metrics = w.ai_metrics().await;
    assert_eq!(metrics["engines"][0]["engine"], "grok");
    let sent = w.requests_to("/grok/v1/responses").await;
    assert_eq!(
        sent[0]
            .headers
            .get("authorization")
            .unwrap()
            .to_str()
            .unwrap(),
        "Bearer xai-saved-SECRET-1"
    );
}
