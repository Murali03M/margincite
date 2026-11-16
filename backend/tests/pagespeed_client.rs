use std::time::Duration;

use margincite::integrations::pagespeed::client::{PageSpeedClient, PageSpeedError};
use serde_json::{json, Value};
use wiremock::matchers::{method, path, query_param, query_param_is_missing};
use wiremock::{Mock, MockServer, ResponseTemplate};

const KEY: &str = "psi-key-secret-777";
const PAGE: &str = "https://example.com/a";

fn lab(score: Value) -> Value {
    json!({
        "lighthouseResult": {
            "categories": { "performance": { "score": score } },
            "audits": {
                "largest-contentful-paint": { "numericValue": 3100.4 },
                "cumulative-layout-shift": { "numericValue": 0.0834 },
                "total-blocking-time": { "numericValue": 449.6 }
            }
        }
    })
}

fn with_field(mut body: Value) -> Value {
    body["loadingExperience"] = json!({ "metrics": {
        "LARGEST_CONTENTFUL_PAINT_MS": { "percentile": 2400 },
        "CUMULATIVE_LAYOUT_SHIFT_SCORE": { "percentile": 12 },
        "INTERACTION_TO_NEXT_PAINT": { "percentile": 180 }
    }});
    body
}

async fn serve(status: u16, body: Value) -> (MockServer, PageSpeedClient) {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/runPagespeed"))
        .respond_with(ResponseTemplate::new(status).set_body_json(body))
        .mount(&server)
        .await;
    let client = PageSpeedClient::new(&server.uri());
    (server, client)
}

#[tokio::test]
async fn a_lab_only_answer_is_read_and_rounded() {
    let (_server, client) = serve(200, lab(json!(0.62))).await;
    let found = client.measure(PAGE, None).await.unwrap();
    assert_eq!(found.performance, Some(62));
    assert_eq!(found.lcp_ms, Some(3100));
    assert_eq!(found.cls, Some(0.0834));
    assert_eq!(found.tbt_ms, Some(450));
    assert_eq!(found.inp_ms, None);
    assert!(!found.field_data);
}

#[tokio::test]
async fn field_data_is_preferred_and_flagged() {
    let (_server, client) = serve(200, with_field(lab(json!(0.9)))).await;
    let found = client.measure(PAGE, None).await.unwrap();
    assert_eq!(found.performance, Some(90));
    assert_eq!(found.lcp_ms, Some(2400));
    assert_eq!(found.cls, Some(0.12));
    assert_eq!(found.inp_ms, Some(180));
    assert_eq!(found.tbt_ms, Some(450));
    assert!(found.field_data);
}

#[tokio::test]
async fn site_wide_field_data_is_not_read_as_this_pages() {
    let mut body = with_field(lab(json!(0.9)));
    body["loadingExperience"]["origin_fallback"] = json!(true);
    let (_server, client) = serve(200, body).await;
    let found = client.measure(PAGE, None).await.unwrap();
    assert!(!found.field_data);
    assert_eq!(found.lcp_ms, Some(3100));
    assert_eq!(found.cls, Some(0.0834));
    assert_eq!(found.inp_ms, None);
}

#[tokio::test]
async fn a_page_with_its_own_field_data_says_so_even_when_the_fallback_flag_is_false() {
    let mut body = with_field(lab(json!(0.9)));
    body["loadingExperience"]["origin_fallback"] = json!(false);
    let (_server, client) = serve(200, body).await;
    let found = client.measure(PAGE, None).await.unwrap();
    assert!(found.field_data);
    assert_eq!(found.lcp_ms, Some(2400));
}

#[tokio::test]
async fn a_missing_score_is_unknown_not_zero() {
    let (_server, client) = serve(200, lab(Value::Null)).await;
    let found = client.measure(PAGE, None).await.unwrap();
    assert_eq!(found.performance, None);
    assert_eq!(found.lcp_ms, Some(3100));
}

#[tokio::test]
async fn an_answer_with_nothing_measured_is_an_error() {
    let body = json!({ "lighthouseResult": { "categories": {}, "audits": {} } });
    let (_server, client) = serve(200, body).await;
    assert_eq!(
        client.measure(PAGE, None).await,
        Err(PageSpeedError::Failed)
    );
}

#[tokio::test]
async fn the_request_asks_for_a_phone_test_and_sends_the_key_only_when_set() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/runPagespeed"))
        .and(query_param("url", PAGE))
        .and(query_param("strategy", "mobile"))
        .and(query_param("category", "performance"))
        .and(query_param("key", KEY))
        .respond_with(ResponseTemplate::new(200).set_body_json(lab(json!(0.5))))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/runPagespeed"))
        .and(query_param_is_missing("key"))
        .respond_with(ResponseTemplate::new(200).set_body_json(lab(json!(0.4))))
        .expect(1)
        .mount(&server)
        .await;
    let client = PageSpeedClient::new(&server.uri());
    assert_eq!(
        client.measure(PAGE, Some(KEY)).await.unwrap().performance,
        Some(50)
    );
    assert_eq!(
        client.measure(PAGE, None).await.unwrap().performance,
        Some(40)
    );
}

#[tokio::test]
async fn refusals_limits_and_failures_become_fixed_plain_errors() {
    for (status, expected) in [
        (401, PageSpeedError::Rejected),
        (403, PageSpeedError::Rejected),
        (429, PageSpeedError::RateLimited),
        (400, PageSpeedError::Failed),
        (500, PageSpeedError::Failed),
        (503, PageSpeedError::Failed),
    ] {
        let (_server, client) = serve(status, json!({ "error": KEY })).await;
        let err = client.measure(PAGE, Some(KEY)).await.unwrap_err();
        assert_eq!(err, expected, "{status}");
        assert_no_secret(&err);
    }
}

#[tokio::test]
async fn a_limit_without_a_key_says_to_add_a_key() {
    let (_server, client) = serve(429, json!({ "error": "quota" })).await;
    let err = client.measure(PAGE, None).await.unwrap_err();
    assert_eq!(err, PageSpeedError::NoKeyLimit);
    assert_eq!(
        err.to_string(),
        "Google is not accepting page speed tests without an API key right now. Add a free PageSpeed key in Settings."
    );
}

#[tokio::test]
async fn a_400_that_says_the_key_is_bad_is_a_rejected_key() {
    let body = json!({ "error": { "message": "API key not valid. Please pass a valid API key." } });
    let (_server, client) = serve(400, body).await;
    assert_eq!(
        client.measure(PAGE, Some("bad")).await.unwrap_err(),
        PageSpeedError::Rejected
    );
}

fn assert_no_secret(err: &PageSpeedError) {
    for text in [err.to_string(), format!("{err:?}")] {
        assert!(!text.contains(KEY) && !text.contains("key="), "{text}");
    }
}

#[tokio::test]
async fn a_slow_answer_is_a_timeout_that_never_carries_the_key() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(lab(json!(0.5)))
                .set_delay(Duration::from_millis(800)),
        )
        .mount(&server)
        .await;
    let client = PageSpeedClient::with_timeout(&server.uri(), Duration::from_millis(100));
    let err = client.measure(PAGE, Some(KEY)).await.unwrap_err();
    assert_eq!(err, PageSpeedError::TimedOut);
    assert_no_secret(&err);
}

#[tokio::test]
async fn an_unexpected_shape_is_unrecognised() {
    for body in [json!({ "unexpected": true }), json!([1, 2])] {
        let (_server, client) = serve(200, body).await;
        assert_eq!(
            client.measure(PAGE, None).await.unwrap_err(),
            PageSpeedError::Unrecognised
        );
    }
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_string("<html>not json</html>"))
        .mount(&server)
        .await;
    let client = PageSpeedClient::new(&server.uri());
    assert_eq!(
        client.measure(PAGE, None).await.unwrap_err(),
        PageSpeedError::Unrecognised
    );
}

#[tokio::test]
async fn an_unreachable_service_and_an_oversized_answer_never_carry_the_key() {
    let client = PageSpeedClient::new("http://127.0.0.1:9");
    let err = client.measure(PAGE, Some(KEY)).await.unwrap_err();
    assert_no_secret(&err);
    assert_eq!(err, PageSpeedError::Failed);

    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(vec![b'a'; 21 * 1024 * 1024]))
        .mount(&server)
        .await;
    let client = PageSpeedClient::new(&server.uri());
    let err = client.measure(PAGE, Some(KEY)).await.unwrap_err();
    assert_eq!(err, PageSpeedError::Failed);
}
