mod common;

use common::dataforseo::{self as df, LOGIN, PASSWORD};
use margincite::integrations::dataforseo::keywords::{fits, read_metrics, KeywordMetrics};
use margincite::integrations::dataforseo::{Credentials, DataForSeoClient, DataForSeoError};
use serde_json::{json, Value};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const VOLUME: &str = "/v3/keywords_data/google_ads/search_volume/live";
const IDEAS: &str = "/v3/keywords_data/google_ads/keywords_for_keywords/live";

fn credentials() -> Credentials<'static> {
    Credentials {
        login: LOGIN,
        password: PASSWORD,
    }
}

async fn serve(at: &str, body: Value) -> (MockServer, DataForSeoClient) {
    let mock = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path(at))
        .respond_with(ResponseTemplate::new(200).set_body_json(body))
        .mount(&mock)
        .await;
    let client = DataForSeoClient::new(&mock.uri());
    (mock, client)
}

fn metrics(
    keyword: &str,
    volume: Option<i32>,
    competition: Option<&str>,
    cpc: Option<f64>,
) -> KeywordMetrics {
    KeywordMetrics {
        keyword: keyword.into(),
        search_volume: volume,
        competition: competition.map(str::to_string),
        cpc,
    }
}

#[tokio::test]
async fn a_volume_request_is_one_task_with_the_keywords_and_the_location() {
    let body = df::answer(json!([
        df::ads_item("blue widgets", Some(1900), Some("HIGH"), Some(1.25)),
        df::ads_item("rare thing", None, None, None),
    ]));
    let (mock, client) = serve(VOLUME, body).await;
    let keywords = vec!["Blue Widgets".to_string(), "rare thing".to_string()];
    let found = client
        .search_volume(&credentials(), &keywords, 2840, "en")
        .await
        .unwrap();
    assert_eq!(
        found,
        vec![
            metrics("blue widgets", Some(1900), Some("HIGH"), Some(1.25)),
            metrics("rare thing", None, None, None),
        ]
    );
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
        json!([{ "keywords": ["Blue Widgets", "rare thing"], "location_code": 2840, "language_code": "en" }])
    );
}

#[tokio::test]
async fn an_ideas_request_names_the_seed_and_reads_every_item() {
    let body = df::answer(json!([
        df::ads_item("widgets", Some(5000), Some("LOW"), Some(0.4)),
        df::ads_item("widget shop", Some(10), Some("MEDIUM"), None),
    ]));
    let (mock, client) = serve(IDEAS, body).await;
    let found = client
        .keyword_ideas(&credentials(), "widgets", 2826, "en")
        .await
        .unwrap();
    assert_eq!(found.len(), 2);
    assert_eq!(
        found[1],
        metrics("widget shop", Some(10), Some("MEDIUM"), None)
    );
    let requests = mock.received_requests().await.unwrap();
    let sent: Value = serde_json::from_slice(&requests[0].body).unwrap();
    assert_eq!(
        sent,
        json!([{ "keywords": ["widgets"], "location_code": 2826, "language_code": "en" }])
    );
}

#[tokio::test]
async fn an_empty_result_is_no_keywords_and_the_errors_are_the_usual_sentences() {
    let (_mock, client) = serve(VOLUME, df::answer(json!([]))).await;
    let none = client
        .search_volume(&credentials(), &["a".to_string()], 2840, "en")
        .await;
    assert_eq!(none, Ok(vec![]));
    for (body, expected) in [
        (
            df::task_failed(40202, "the rate-limit per minute has been exceeded."),
            DataForSeoError::RateLimited,
        ),
        (
            df::task_failed(40200, "payment required."),
            DataForSeoError::OutOfCredit,
        ),
        (df::task_failed(40100, "nope"), DataForSeoError::Rejected),
        (json!({ "x": 1 }), DataForSeoError::Unrecognised),
    ] {
        let (_mock, client) = serve(IDEAS, body).await;
        let found = client.keyword_ideas(&credentials(), "a", 2840, "en").await;
        assert_eq!(found, Err(expected));
    }
}

#[test]
fn items_are_read_defensively() {
    let result = vec![
        df::ads_item("good", Some(10), Some("LOW"), Some(0.1)),
        // Nothing known.
        json!({ "keyword": "none", "search_volume": null, "competition": null, "cpc": null }),
        // Not a keyword item, wrong types, a negative cost, and an empty word for competition.
        json!({ "nokeyword": true }),
        json!("text"),
        json!({ "keyword": "odd", "search_volume": "many", "competition": "", "cpc": -3.0 }),
        json!({ "keyword": "a\u{0}b", "search_volume": 7.0, "competition": 5, "cpc": "1" }),
    ];
    let found = read_metrics(&result);
    assert_eq!(
        found,
        vec![
            metrics("good", Some(10), Some("LOW"), Some(0.1)),
            metrics("none", None, None, None),
            metrics("odd", None, None, None),
            metrics("ab", Some(7), None, None),
        ]
    );
}

#[test]
fn only_keywords_the_provider_accepts_are_sent() {
    assert!(fits("blue widgets"));
    assert!(fits(&"x".repeat(80)));
    assert!(!fits(&"x".repeat(81)));
    assert!(fits("a b c d e f g h i j"));
    assert!(!fits("a b c d e f g h i j k"));
    assert!(!fits("   "));
}
