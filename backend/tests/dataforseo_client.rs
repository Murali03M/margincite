mod common;

use std::time::Duration;

use common::dataforseo::{self as df, LOGIN, PASSWORD};
use margincite::integrations::dataforseo::client::{SerpQuery, SERP_DEPTH};
use margincite::integrations::dataforseo::{Credentials, DataForSeoClient, DataForSeoError};
use serde_json::{json, Value};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const SERP_PATH: &str = "/v3/serp/google/organic/live/advanced";

fn credentials() -> Credentials<'static> {
    Credentials {
        login: LOGIN,
        password: PASSWORD,
    }
}

fn query() -> SerpQuery<'static> {
    SerpQuery {
        keyword: "blue widgets",
        location_code: 2840,
        language_code: "en",
        device: "desktop",
    }
}

async fn serp_with(response: ResponseTemplate) -> Result<Vec<Value>, DataForSeoError> {
    let mock = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path(SERP_PATH))
        .respond_with(response)
        .mount(&mock)
        .await;
    DataForSeoClient::new(&mock.uri())
        .serp(&credentials(), &query())
        .await
}

fn json_body(status: u16, body: Value) -> ResponseTemplate {
    ResponseTemplate::new(status).set_body_json(body)
}

#[tokio::test]
async fn a_request_carries_one_task_with_basic_auth() {
    let mock = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path(SERP_PATH))
        .respond_with(json_body(200, df::serp(json!([df::organic(1, "a.com")]))))
        .mount(&mock)
        .await;
    let items = DataForSeoClient::new(&mock.uri())
        .serp(&credentials(), &query())
        .await
        .unwrap();
    assert_eq!(items.len(), 1);
    let requests = mock.received_requests().await.unwrap();
    assert_eq!(requests.len(), 1);
    assert_eq!(
        requests[0]
            .headers
            .get("authorization")
            .unwrap()
            .to_str()
            .unwrap(),
        df::basic()
    );
    let sent: Value = serde_json::from_slice(&requests[0].body).unwrap();
    assert_eq!(
        sent,
        json!([{ "keyword": "blue widgets", "location_code": 2840, "language_code": "en",
                 "device": "desktop", "depth": SERP_DEPTH, "load_async_ai_overview": true }])
    );
    assert_eq!(SERP_DEPTH, 100);
}

#[tokio::test]
async fn a_result_page_with_no_items_is_an_empty_answer() {
    let body = df::answer(json!([{ "keyword": "k", "items_count": 0, "items": null }]));
    assert_eq!(serp_with(json_body(200, body)).await, Ok(vec![]));
}

#[tokio::test]
async fn http_errors_become_fixed_sentences() {
    for (status, expected) in [
        (401, DataForSeoError::Rejected),
        (403, DataForSeoError::Rejected),
        (402, DataForSeoError::OutOfCredit),
        (429, DataForSeoError::RateLimited),
        (500, DataForSeoError::Failed),
        (404, DataForSeoError::Failed),
    ] {
        let found = serp_with(ResponseTemplate::new(status).set_body_string("nope")).await;
        assert_eq!(found, Err(expected), "HTTP {status}");
    }
}

#[tokio::test]
async fn status_codes_in_the_answer_decide_even_on_http_200() {
    let task = df::task_failed;
    let top = df::top_failed;
    for (body, expected) in [
        (
            task(40200, "payment required."),
            DataForSeoError::OutOfCredit,
        ),
        (
            task(40203, "the cost limit has been exceeded."),
            DataForSeoError::OutOfCredit,
        ),
        (
            task(40210, "insufficient funds."),
            DataForSeoError::OutOfCredit,
        ),
        (
            top(40200, "payment required."),
            DataForSeoError::OutOfCredit,
        ),
        (
            task(40202, "the rate-limit per minute has been exceeded."),
            DataForSeoError::RateLimited,
        ),
        (
            task(40209, "too many simultaneous queries."),
            DataForSeoError::RateLimited,
        ),
        (
            task(40100, "you are not authorized."),
            DataForSeoError::Rejected,
        ),
        (
            top(40101, "authentication failed."),
            DataForSeoError::Rejected,
        ),
        (task(40201, "unusual activity."), DataForSeoError::Failed),
        (task(50000, "internal error."), DataForSeoError::Failed),
        (
            task(50401, "internal error - timeout."),
            DataForSeoError::TimedOut,
        ),
        (task(40501, "invalid field."), DataForSeoError::Failed),
    ] {
        let found = serp_with(json_body(200, body.clone())).await;
        assert_eq!(found, Err(expected), "{body}");
    }
}

#[tokio::test]
async fn an_unexpected_shape_is_unrecognised() {
    for body in [
        json!({ "hello": "world" }),
        json!({ "status_code": 20000, "tasks": [] }),
        // OK status but no result.
        df::task_failed(20000, "Ok."),
        // A result with no items key at all.
        df::answer(json!([{ "keyword": "k" }])),
        df::answer(json!([])),
        df::answer(json!({ "items": [] })),
    ] {
        let found = serp_with(json_body(200, body.clone())).await;
        assert_eq!(found, Err(DataForSeoError::Unrecognised), "{body}");
    }
    let found = serp_with(ResponseTemplate::new(200).set_body_string("<html>")).await;
    assert_eq!(found, Err(DataForSeoError::Unrecognised));
}

#[tokio::test]
async fn a_slow_answer_times_out_and_a_closed_port_fails() {
    let mock = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(json_body(200, df::serp(json!([]))).set_delay(Duration::from_millis(800)))
        .mount(&mock)
        .await;
    let slow = DataForSeoClient::with_timeout(&mock.uri(), Duration::from_millis(100));
    assert_eq!(
        slow.serp(&credentials(), &query()).await,
        Err(DataForSeoError::TimedOut)
    );
    let closed = DataForSeoClient::new("http://127.0.0.1:9");
    assert_eq!(
        closed.serp(&credentials(), &query()).await,
        Err(DataForSeoError::Failed)
    );
}

#[tokio::test]
async fn an_answer_over_the_size_cap_fails_without_being_read() {
    let big = "x".repeat(21 * 1024 * 1024);
    let found = serp_with(ResponseTemplate::new(200).set_body_string(big)).await;
    assert_eq!(found, Err(DataForSeoError::Failed));
}

#[test]
fn the_sentences_are_fixed_and_plain() {
    for err in [
        DataForSeoError::Rejected,
        DataForSeoError::OutOfCredit,
        DataForSeoError::RateLimited,
        DataForSeoError::TimedOut,
        DataForSeoError::Failed,
        DataForSeoError::Unrecognised,
    ] {
        let text = err.to_string();
        assert!(text.ends_with('.') && !text.contains("HTTP") && !text.contains("status"));
    }
    assert_eq!(
        DataForSeoError::OutOfCredit.to_string(),
        "Your DataForSEO balance has run out."
    );
    assert!(DataForSeoError::Rejected.is_fatal() && DataForSeoError::OutOfCredit.is_fatal());
    assert!(!DataForSeoError::Failed.is_fatal());
    assert!(
        DataForSeoError::RateLimited.is_temporary() && !DataForSeoError::Rejected.is_temporary()
    );
}
