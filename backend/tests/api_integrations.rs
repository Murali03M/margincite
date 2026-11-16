mod common;

use serde_json::Value;
use sqlx::PgPool;

async fn get(app: &common::TestApp, token: Option<&str>) -> (u16, Value) {
    let mut request = app
        .client
        .get(format!("http://{}/api/v1/integrations", app.addr));
    if let Some(token) = token {
        request = request.header("authorization", format!("Bearer {token}"));
    }
    let response = request.send().await.unwrap();
    (response.status().as_u16(), response.json().await.unwrap())
}

#[sqlx::test]
async fn needs_a_signed_in_caller(pool: PgPool) {
    let app = common::spawn_app(pool).await;
    assert_eq!(get(&app, None).await.0, 401);
}

#[sqlx::test]
async fn nothing_is_configured_by_default(pool: PgPool) {
    let app = common::spawn_app(pool).await;
    let (status, body) = get(&app, Some("test-token")).await;
    assert_eq!(status, 200);
    let data = &body["data"];
    assert_eq!(data["secret_key_set"], false);
    assert_eq!(data["google"]["configured"], false);
    let engines = data["engines"].as_array().unwrap();
    let names: Vec<_> = engines
        .iter()
        .map(|e| e["engine"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["claude", "chatgpt", "grok", "gemini", "perplexity"]);
    assert!(engines.iter().all(|e| e["configured"] == false));
    assert_eq!(engines[0]["model"], "claude-opus-5-5");
}

#[sqlx::test]
async fn reports_configured_engines_and_never_a_key(pool: PgPool) {
    let app = common::spawn_app_with(
        pool,
        &[
            (
                "MARGINCITE_SECRET_KEY",
                "BwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwc=",
            ),
            ("GOOGLE_CLIENT_ID", "client-id-value"),
            ("GOOGLE_CLIENT_SECRET", "client-secret-value"),
            ("ANTHROPIC_API_KEY", "sk-anthropic-hidden"),
            ("GEMINI_API_KEY", "gemini-hidden"),
            ("GEMINI_MODEL", "gemini-test"),
        ],
    )
    .await;
    let (status, body) = get(&app, Some("test-token")).await;
    assert_eq!(status, 200);
    let data = &body["data"];
    assert_eq!(data["secret_key_set"], true);
    assert_eq!(data["google"]["configured"], true);
    let engines = data["engines"].as_array().unwrap();
    let configured: Vec<_> = engines
        .iter()
        .map(|e| e["configured"].as_bool().unwrap())
        .collect();
    assert_eq!(configured, [true, false, false, true, false]);
    assert_eq!(engines[3]["model"], "gemini-test");
    let text = body.to_string();
    for secret in [
        "sk-anthropic-hidden",
        "gemini-hidden",
        "client-secret-value",
        "client-id-value",
        "BwcHBw",
    ] {
        assert!(!text.contains(secret), "{secret} leaked");
    }
}
