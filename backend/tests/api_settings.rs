mod common;

use common::google::GoogleWorld;
use common::settings::*;
use common::SECRET_KEY;
use reqwest::Method;
use serde_json::json;
use sqlx::PgPool;
use wiremock::matchers::{method, path};
use wiremock::{Mock, ResponseTemplate};

// ------------------------------------------------------------ the overview

#[sqlx::test]
async fn nothing_is_saved_by_default(pool: PgPool) {
    let w = world(pool).await;
    let (data, _) = settings(&w).await;
    assert_eq!(data["secret_key_ready"], true);
    let names: Vec<_> = data["providers"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["provider"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["claude", "chatgpt", "grok", "gemini", "perplexity"]);
    let claude = provider(&data, "claude");
    assert_eq!(claude["key_set"], false);
    assert_eq!(claude["source"], "none");
    assert_eq!(claude["status"], "unchecked");
    assert_eq!(claude["model"], "claude-opus-5-5");
    assert_eq!(claude["default_model"], "claude-opus-5-5");
    assert_eq!(data["google"]["source"], "none");
    assert_eq!(
        data["google"]["redirect_uri"],
        "http://localhost:5174/api/v1/oauth/google/callback"
    );
    assert_eq!(data["dataforseo"]["login_set"], false);
    assert_eq!(data["pagespeed"]["key_set"], false);
    assert_eq!(
        data["crawler"]["contact"],
        "https://example.com/margincite-bot"
    );
    assert_eq!(data["crawler"]["contact_source"], "environment");
}

#[sqlx::test]
async fn the_overview_needs_a_signed_in_caller(pool: PgPool) {
    let w = world(pool).await;
    let response = w
        .app
        .client
        .get(w.url("/api/v1/settings"))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 401);
}

// ------------------------------------------------------------- round trips

#[sqlx::test]
async fn every_setting_round_trips_and_no_response_holds_a_secret(pool: PgPool) {
    let w = world(pool).await;
    mock_models_ok(&w).await;
    Mock::given(method("GET"))
        .and(path("/dataforseo/v3/appendix/user_data"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "status_code": 20000,
            "tasks": [{ "status_code": 20000,
                        "result": [{ "money": { "total": 100.0, "balance": 42.5 } }] }]
        })))
        .mount(&w.mock)
        .await;
    let mut raws = Vec::new();

    let (s, _, raw) = put(
        &w,
        "/api/v1/settings/providers/chatgpt",
        json!({ "api_key": OPENAI_SECRET, "model": "gpt-test" }),
    )
    .await;
    assert_eq!(s, 200, "{raw}");
    raws.push(raw);
    let (s, _, raw) = put(
        &w,
        "/api/v1/settings/providers/claude",
        json!({ "api_key": CLAUDE_SECRET }),
    )
    .await;
    assert_eq!(s, 200, "{raw}");
    raws.push(raw);
    let (s, _, raw) = put(
        &w,
        "/api/v1/settings/google",
        json!({ "client_id": GOOGLE_ID, "client_secret": GOOGLE_SECRET }),
    )
    .await;
    assert_eq!(s, 200, "{raw}");
    raws.push(raw);
    let (s, _, raw) = put(
        &w,
        "/api/v1/settings/dataforseo",
        json!({ "login": "me@example.com", "password": SEO_PASSWORD }),
    )
    .await;
    assert_eq!(s, 200, "{raw}");
    raws.push(raw);
    let (s, _, raw) = put(
        &w,
        "/api/v1/settings/pagespeed",
        json!({ "api_key": PSI_SECRET }),
    )
    .await;
    assert_eq!(s, 200, "{raw}");
    raws.push(raw);
    let (s, _, raw) = put(
        &w,
        "/api/v1/settings/crawler",
        json!({ "contact": "bot@example.org" }),
    )
    .await;
    assert_eq!(s, 200, "{raw}");
    raws.push(raw);
    let (s, _, raw) = call(
        &w,
        "test-token",
        Method::POST,
        "/api/v1/settings/providers/chatgpt/test",
        None,
    )
    .await;
    assert_eq!(s, 200, "{raw}");
    raws.push(raw);
    let (s, _, raw) = call(
        &w,
        "test-token",
        Method::POST,
        "/api/v1/settings/dataforseo/test",
        None,
    )
    .await;
    assert_eq!(s, 200, "{raw}");
    raws.push(raw);

    let (data, raw) = settings(&w).await;
    raws.push(raw);
    let chatgpt = provider(&data, "chatgpt");
    assert_eq!(chatgpt["key_set"], true);
    assert_eq!(chatgpt["source"], "workspace");
    assert_eq!(chatgpt["model"], "gpt-test");
    assert_eq!(chatgpt["default_model"], "gpt-5.6");
    assert_eq!(chatgpt["status"], "ok");
    assert!(chatgpt["checked_at"].is_string());
    assert_eq!(provider(&data, "claude")["model"], "claude-opus-5-5");
    assert_eq!(data["google"]["client_id_set"], true);
    assert_eq!(data["google"]["client_secret_set"], true);
    assert_eq!(data["google"]["source"], "workspace");
    assert_eq!(data["dataforseo"]["login_set"], true);
    assert_eq!(data["dataforseo"]["status"], "ok");
    assert_eq!(data["dataforseo"]["balance"], 42.5);
    assert_eq!(data["pagespeed"]["key_set"], true);
    assert_eq!(data["crawler"]["contact"], "bot@example.org");
    assert_eq!(data["crawler"]["contact_source"], "workspace");

    // Deleting returns everything to the environment, and answers safely too.
    for p in [
        "/api/v1/settings/providers/chatgpt",
        "/api/v1/settings/providers/claude",
        "/api/v1/settings/google",
        "/api/v1/settings/dataforseo",
        "/api/v1/settings/pagespeed",
        "/api/v1/settings/crawler",
    ] {
        let (s, _, raw) = call(&w, "test-token", Method::DELETE, p, None).await;
        assert_eq!(s, 200, "{p}: {raw}");
        raws.push(raw);
    }
    let (data, raw) = settings(&w).await;
    raws.push(raw);
    assert_eq!(provider(&data, "chatgpt")["key_set"], false);
    assert_eq!(provider(&data, "chatgpt")["model"], "gpt-5.6");
    assert_eq!(provider(&data, "chatgpt")["status"], "unchecked");
    assert_eq!(data["google"]["source"], "none");
    assert_eq!(data["dataforseo"]["login_set"], false);
    assert_eq!(data["crawler"]["contact_source"], "environment");
    for raw in raws {
        assert_no_secret(&raw);
    }
}

#[sqlx::test]
async fn secrets_are_stored_sealed_and_never_as_plain_text(pool: PgPool) {
    let w = world(pool.clone()).await;
    mock_models_ok(&w).await;
    put(
        &w,
        "/api/v1/settings/providers/chatgpt",
        json!({ "api_key": OPENAI_SECRET }),
    )
    .await;
    put(
        &w,
        "/api/v1/settings/google",
        json!({ "client_id": GOOGLE_ID, "client_secret": GOOGLE_SECRET }),
    )
    .await;
    put(
        &w,
        "/api/v1/settings/pagespeed",
        json!({ "api_key": PSI_SECRET }),
    )
    .await;
    let rows: Vec<(String, Option<String>, Option<String>)> =
        sqlx::query_as("SELECT key, value_sealed, value_plain FROM workspace_settings")
            .fetch_all(&pool)
            .await
            .unwrap();
    assert!(rows.len() >= 4);
    for (key, sealed, plain) in rows {
        if key.ends_with("api_key") || key.ends_with("client_secret") {
            assert!(plain.is_none(), "{key} has plain text");
            let sealed = sealed.expect("sealed");
            assert!(!sealed.contains("SECRET"), "{key}");
            let secrets = common::google::secrets();
            assert!(secrets.open(&sealed).unwrap().contains("SECRET"));
        }
    }
}

// ------------------------------------------------------------ the lookup order

#[sqlx::test]
async fn the_workspace_beats_the_environment_and_the_environment_beats_the_default(pool: PgPool) {
    let w = GoogleWorld::with_settings(
        pool,
        &[
            ("MARGINCITE_SECRET_KEY", SECRET_KEY),
            ("OPENAI_API_KEY", "sk-env-openai"),
            ("OPENAI_MODEL", "gpt-env"),
            ("PAGESPEED_API_KEY", "psi-env"),
        ],
    )
    .await;
    mock_models_ok(&w).await;
    let (data, _) = settings(&w).await;
    let chatgpt = provider(&data, "chatgpt");
    assert_eq!(
        (chatgpt["source"].as_str(), chatgpt["model"].as_str()),
        (Some("environment"), Some("gpt-env"))
    );
    assert_eq!(data["pagespeed"]["source"], "environment");
    assert_eq!(provider(&data, "gemini")["model"], "gemini-3.5-flash");

    put(
        &w,
        "/api/v1/settings/providers/chatgpt",
        json!({ "api_key": OPENAI_SECRET, "model": "gpt-mine" }),
    )
    .await;
    let (data, _) = settings(&w).await;
    let chatgpt = provider(&data, "chatgpt");
    assert_eq!(
        (chatgpt["source"].as_str(), chatgpt["model"].as_str()),
        (Some("workspace"), Some("gpt-mine"))
    );
    // A model alone keeps the environment's key.
    call(
        &w,
        "test-token",
        Method::DELETE,
        "/api/v1/settings/providers/chatgpt",
        None,
    )
    .await;
    put(
        &w,
        "/api/v1/settings/providers/chatgpt",
        json!({ "model": "gpt-only-model" }),
    )
    .await;
    let (data, _) = settings(&w).await;
    let chatgpt = provider(&data, "chatgpt");
    assert_eq!(
        (chatgpt["source"].as_str(), chatgpt["model"].as_str()),
        (Some("environment"), Some("gpt-only-model"))
    );
    // An empty model goes back to the environment's.
    put(
        &w,
        "/api/v1/settings/providers/chatgpt",
        json!({ "model": "" }),
    )
    .await;
    let (data, _) = settings(&w).await;
    assert_eq!(provider(&data, "chatgpt")["model"], "gpt-env");
}

#[sqlx::test]
async fn another_workspace_cannot_read_or_change_settings(pool: PgPool) {
    let w = world(pool).await;
    mock_models_ok(&w).await;
    put(
        &w,
        "/api/v1/settings/providers/chatgpt",
        json!({ "api_key": OPENAI_SECRET }),
    )
    .await;
    put(
        &w,
        "/api/v1/settings/crawler",
        json!({ "contact": "mine@example.org" }),
    )
    .await;
    let (_, body, raw) = call(&w, "other-token", Method::GET, "/api/v1/settings", None).await;
    assert_eq!(provider(&body["data"], "chatgpt")["key_set"], false);
    assert_eq!(body["data"]["crawler"]["contact_source"], "environment");
    assert_no_secret(&raw);
    call(
        &w,
        "other-token",
        Method::DELETE,
        "/api/v1/settings/providers/chatgpt",
        None,
    )
    .await;
    call(
        &w,
        "other-token",
        Method::DELETE,
        "/api/v1/settings/crawler",
        None,
    )
    .await;
    let (data, _) = settings(&w).await;
    assert_eq!(provider(&data, "chatgpt")["key_set"], true);
    assert_eq!(data["crawler"]["contact"], "mine@example.org");
}
