//! Helpers shared by the `api_settings*` test files.

use super::google::GoogleWorld;
use super::SECRET_KEY;
use reqwest::Method;
use serde_json::{json, Value};
use sqlx::PgPool;
use wiremock::matchers::{method, path};
use wiremock::{Mock, ResponseTemplate};

pub const KEY_ONLY: &[(&str, &str)] = &[("MARGINCITE_SECRET_KEY", SECRET_KEY)];

/// Fake secrets that are easy to search for in any output.
pub const OPENAI_SECRET: &str = "sk-saved-openai-SECRET-9001";
pub const CLAUDE_SECRET: &str = "sk-saved-claude-SECRET-9002";
pub const GOOGLE_SECRET: &str = "saved-google-SECRET-9003";
pub const GOOGLE_ID: &str = "saved-google-ID-9004";
pub const SEO_PASSWORD: &str = "saved-seo-PASSWORD-9005";
pub const PSI_SECRET: &str = "saved-psi-SECRET-9006";
pub const ALL_SECRETS: [&str; 7] = [
    OPENAI_SECRET,
    CLAUDE_SECRET,
    GOOGLE_SECRET,
    GOOGLE_ID,
    SEO_PASSWORD,
    PSI_SECRET,
    "SECRET_KEY_NEVER",
];

pub async fn world(pool: PgPool) -> GoogleWorld {
    GoogleWorld::with_settings(pool, KEY_ONLY).await
}

pub async fn call(
    w: &GoogleWorld,
    token: &str,
    m: Method,
    p: &str,
    body: Option<Value>,
) -> (u16, Value, String) {
    let mut request = w
        .app
        .client
        .request(m, w.url(p))
        .header("authorization", format!("Bearer {token}"));
    if let Some(body) = body {
        request = request.json(&body);
    }
    let response = request.send().await.unwrap();
    let status = response.status().as_u16();
    let raw = response.text().await.unwrap();
    (
        status,
        serde_json::from_str(&raw).unwrap_or(Value::Null),
        raw,
    )
}

pub async fn put(w: &GoogleWorld, p: &str, body: Value) -> (u16, Value, String) {
    call(w, "test-token", Method::PUT, p, Some(body)).await
}

pub async fn settings(w: &GoogleWorld) -> (Value, String) {
    let (status, body, raw) = call(w, "test-token", Method::GET, "/api/v1/settings", None).await;
    assert_eq!(status, 200, "{raw}");
    (body["data"].clone(), raw)
}

pub fn provider<'a>(data: &'a Value, name: &str) -> &'a Value {
    data["providers"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["provider"] == name)
        .unwrap()
}

pub fn assert_no_secret(raw: &str) {
    for secret in ALL_SECRETS.iter().chain([&SECRET_KEY, &"BwcHBw"]) {
        assert!(!raw.contains(secret), "{secret} leaked in {raw}");
    }
}

pub async fn mock_models_ok(w: &GoogleWorld) {
    for (m, p) in [
        ("GET", "/claude/v1/models"),
        ("GET", "/openai/v1/models"),
        ("GET", "/gemini/v1beta/models"),
        ("GET", "/grok/v1/models"),
        ("POST", "/perplexity/chat/completions"),
    ] {
        Mock::given(method(m))
            .and(path(p))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "data": [] })))
            .mount(&w.mock)
            .await;
    }
}
