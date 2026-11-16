mod common;

use common::settings::*;
use serde_json::{json, Value};
use sqlx::PgPool;
use wiremock::matchers::{method, path};
use wiremock::{Mock, ResponseTemplate};

// -------------------------------------------------------------- DataForSEO

#[sqlx::test]
async fn dataforseo_is_tested_against_the_account_endpoint(pool: PgPool) {
    let w = world(pool).await;
    Mock::given(method("GET"))
        .and(path("/dataforseo/v3/appendix/user_data"))
        .respond_with(ResponseTemplate::new(401).set_body_string(SEO_PASSWORD))
        .mount(&w.mock)
        .await;
    let (s, body, raw) = put(
        &w,
        "/api/v1/settings/dataforseo",
        json!({ "login": "me@example.com", "password": SEO_PASSWORD }),
    )
    .await;
    assert_eq!(s, 200, "{raw}");
    assert_eq!(body["data"]["status"], "error");
    assert_eq!(body["data"]["error"], "The login or password was rejected.");
    assert_eq!(body["data"]["balance"], Value::Null);
    assert_eq!(body["data"]["login_set"], true);
    assert_no_secret(&raw);
}

#[sqlx::test]
async fn dataforseo_trouble_is_reported_in_plain_words_whatever_the_provider_said(pool: PgPool) {
    let w = world(pool).await;
    let echo = format!("{SEO_PASSWORD} me@example.com");
    let task = |code: i64| json!({ "status_code": 20000, "tasks": [{ "status_code": code, "status_message": echo, "result": null }] });
    let top = json!({ "status_code": 40200, "status_message": echo, "tasks": [] });
    for (status, body, expected) in [
        (
            402,
            json!({ "echo": echo }),
            "The account is out of credit.",
        ),
        (200, task(40200), "The account is out of credit."),
        (200, task(40210), "The account is out of credit."),
        (200, task(40203), "The account is out of credit."),
        (200, top, "The account is out of credit."),
        (200, task(40100), "The login or password was rejected."),
        (
            403,
            json!({ "echo": echo }),
            "The login or password was rejected.",
        ),
        (
            429,
            json!({ "echo": echo }),
            "DataForSEO is limiting requests right now. Try again later.",
        ),
        (
            200,
            task(40202),
            "DataForSEO is limiting requests right now. Try again later.",
        ),
        (
            500,
            json!({ "echo": echo }),
            "DataForSEO could not answer this request.",
        ),
        (
            200,
            task(50000),
            "DataForSEO could not answer this request.",
        ),
        (
            200,
            json!({ "echo": echo }),
            "The provider's answer could not be read.",
        ),
    ] {
        w.mock.reset().await;
        Mock::given(method("GET"))
            .and(path("/dataforseo/v3/appendix/user_data"))
            .respond_with(ResponseTemplate::new(status).set_body_json(body.clone()))
            .mount(&w.mock)
            .await;
        let (s, reply, raw) = put(
            &w,
            "/api/v1/settings/dataforseo",
            json!({ "login": "me@example.com", "password": SEO_PASSWORD }),
        )
        .await;
        assert_eq!(s, 200, "{status} {body}: {raw}");
        assert_eq!(reply["data"]["status"], "error", "{status} {body}");
        assert_eq!(reply["data"]["error"], expected, "{status} {body}");
        assert_eq!(reply["data"]["balance"], Value::Null);
        assert_no_secret(&raw);
        assert!(!raw.contains("me@example.com"), "{raw}");
    }
}

#[tokio::test]
async fn a_dataforseo_login_that_cannot_be_tested_says_it_could_not_be_reached() {
    let found =
        margincite::settings::test_provider::dataforseo("http://127.0.0.1:9", "a", "b").await;
    assert_eq!(
        found,
        Err("DataForSEO could not answer this request.".to_string())
    );
}
