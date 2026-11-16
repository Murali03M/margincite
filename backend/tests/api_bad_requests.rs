//! A request that cannot be read answers in the usual envelope with a plain
//! message, and the message never quotes what was sent.

mod common;

use common::google::GoogleWorld;
use reqwest::Method;
use serde_json::{json, Value};
use sqlx::PgPool;

fn assert_plain_422(status: u16, body: &Value, sent: &str) {
    assert_eq!(status, 422, "{body}");
    assert_eq!(body["success"], false, "{body}");
    assert!(body["data"].is_null(), "{body}");
    let error = body["error"].as_str().expect("an error message");
    assert!(!error.is_empty());
    assert!(!error.contains(sent), "{error}");
    assert!(!error.contains("deserialize"), "{error}");
}

#[sqlx::test]
async fn a_site_settings_body_of_the_wrong_type_uses_the_envelope(pool: PgPool) {
    let w = GoogleWorld::new(pool).await;
    let (status, body) = w
        .send(
            "test-token",
            Method::PUT,
            &format!("/api/v1/sites/{}/settings", w.site_id),
            Some(json!({ "page_cap": "abc-secret-marker" })),
        )
        .await;
    assert_plain_422(status, &body, "abc-secret-marker");
}

#[sqlx::test]
async fn an_ideas_body_of_the_wrong_type_uses_the_envelope(pool: PgPool) {
    let w = GoogleWorld::new(pool).await;
    let (status, body) = w
        .send(
            "test-token",
            Method::POST,
            &format!("/api/v1/sites/{}/keywords/ideas", w.site_id),
            Some(json!({ "seed": 5 })),
        )
        .await;
    assert_plain_422(status, &body, "invalid type");
}

#[sqlx::test]
async fn a_usage_query_that_is_not_a_number_uses_the_envelope(pool: PgPool) {
    let w = GoogleWorld::new(pool).await;
    let (status, body) = w.get("/api/v1/usage?days=abc-secret-marker").await;
    assert_plain_422(status, &body, "abc-secret-marker");
}

#[sqlx::test]
async fn a_body_that_is_not_json_uses_the_envelope(pool: PgPool) {
    let w = GoogleWorld::new(pool).await;
    let response = w
        .app
        .client
        .put(w.url(&format!("/api/v1/sites/{}/settings", w.site_id)))
        .header("authorization", "Bearer test-token")
        .header("content-type", "application/json")
        .body("{not json")
        .send()
        .await
        .unwrap();
    let status = response.status().as_u16();
    let body: Value = response.json().await.unwrap_or(Value::Null);
    assert_plain_422(status, &body, "not json");
}

const MARK: &str = "abc-secret-marker";
const NO_SUCH_ID: &str = "00000000-0000-4000-8000-000000000001";

/// Every body an older endpoint reads, sent with a field of the wrong type.
#[sqlx::test]
async fn older_endpoints_answer_a_body_of_the_wrong_type_in_the_envelope(pool: PgPool) {
    let w = GoogleWorld::new(pool).await;
    let site = w.site_id;
    let cases: Vec<(Method, String, Value)> = vec![
        (
            Method::POST,
            "/api/v1/sites".into(),
            json!({ "url": [MARK] }),
        ),
        (
            Method::POST,
            format!("/api/v1/sites/{site}/keywords"),
            json!({ "keyword": [MARK] }),
        ),
        (
            Method::PATCH,
            format!("/api/v1/keywords/{NO_SUCH_ID}"),
            json!({ "target_url": [MARK] }),
        ),
        (
            Method::PUT,
            format!("/api/v1/sites/{site}/ai/settings"),
            json!({ "brand_name": [MARK] }),
        ),
        (
            Method::POST,
            format!("/api/v1/sites/{site}/ai/prompts"),
            json!({ "text": [MARK] }),
        ),
        (
            Method::PUT,
            format!("/api/v1/sites/{site}/integrations/bing"),
            json!({ "api_key": [MARK] }),
        ),
        (
            Method::PUT,
            format!("/api/v1/sites/{site}/integrations/google"),
            json!({ "property": [MARK] }),
        ),
        (
            Method::PATCH,
            format!("/api/v1/suggestions/{NO_SUCH_ID}"),
            json!({ "status": [MARK] }),
        ),
    ];
    for (method, path, body) in cases {
        let (status, answer) = w
            .send("test-token", method.clone(), &path, Some(body))
            .await;
        assert_eq!(status, 422, "{method} {path}: {answer}");
        assert_plain_422(status, &answer, MARK);
    }
}

/// Every list that takes a page number, sent one that is not a number.
#[sqlx::test]
async fn older_lists_answer_a_page_that_is_not_a_number_in_the_envelope(pool: PgPool) {
    let w = GoogleWorld::new(pool).await;
    let site = w.site_id;
    let paths = [
        format!("/api/v1/crawls/{NO_SUCH_ID}/pages"),
        format!("/api/v1/sites/{site}/ai/runs"),
        format!("/api/v1/sites/{site}/suggestions"),
        format!("/api/v1/suggestions/{NO_SUCH_ID}"),
        format!("/api/v1/sites/{site}/search/queries"),
        format!("/api/v1/sites/{site}/search/pages"),
        format!("/api/v1/sites/{site}/backlinks"),
    ];
    for path in paths {
        let (status, answer) = w.get(&format!("{path}?page={MARK}")).await;
        assert_eq!(status, 422, "{path}: {answer}");
        assert_plain_422(status, &answer, MARK);
    }
}
