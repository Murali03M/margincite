//! No answer from Google, Bing or an AI engine is read past 20 MB.

mod common;

use std::time::Duration;

use common::ai::claude_body;
use common::bing::BING_KEY;
use common::google::hosts;
use margincite::ai::engines::{ClaudeEngine, Engine, EngineError};
use margincite::body::{read_capped, BodyError, MAX_BODY_BYTES};
use margincite::integrations::bing::client::{BingClient, BingError};
use margincite::integrations::google::client::{GoogleClient, GoogleError};
use serde_json::json;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;
use wiremock::matchers::any;
use wiremock::{Mock, MockServer, ResponseTemplate};

/// A valid answer, followed by enough blanks to pass the limit. Read whole,
/// it would parse fine, so only the limit can stop it.
fn too_big(answer: &serde_json::Value) -> Vec<u8> {
    let mut body = answer.to_string().into_bytes();
    body.resize(MAX_BODY_BYTES + 1, b' ');
    body
}

async fn mock_too_big(answer: serde_json::Value) -> MockServer {
    let server = MockServer::start().await;
    Mock::given(any())
        .respond_with(ResponseTemplate::new(200).set_body_bytes(too_big(&answer)))
        .mount(&server)
        .await;
    server
}

/// A server that sends `chunks` with no content-length, then closes.
async fn chunked_server(chunks: Vec<Vec<u8>>) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut buf = [0u8; 1024];
        let _ = tokio::io::AsyncReadExt::read(&mut socket, &mut buf).await;
        let head = "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n";
        socket.write_all(head.as_bytes()).await.unwrap();
        for chunk in chunks {
            let _ = socket
                .write_all(format!("{:x}\r\n", chunk.len()).as_bytes())
                .await;
            let _ = socket.write_all(&chunk).await;
            let _ = socket.write_all(b"\r\n").await;
        }
        let _ = socket.write_all(b"0\r\n\r\n").await;
    });
    format!("http://{addr}/")
}

#[tokio::test]
async fn a_body_announced_over_the_limit_is_refused() {
    let server = mock_too_big(json!({})).await;
    let response = reqwest::get(server.uri()).await.unwrap();
    assert!(matches!(
        read_capped(response, MAX_BODY_BYTES).await,
        Err(BodyError::TooLarge)
    ));
}

#[tokio::test]
async fn a_streamed_body_is_refused_once_it_adds_up_past_the_limit() {
    let url = chunked_server(vec![vec![b'a'; 600], vec![b'b'; 600]]).await;
    let response = reqwest::get(url).await.unwrap();
    assert!(response.content_length().is_none());
    assert!(matches!(
        read_capped(response, 1000).await,
        Err(BodyError::TooLarge)
    ));
}

#[tokio::test]
async fn a_body_within_the_limit_is_read_whole() {
    let url = chunked_server(vec![vec![b'a'; 600], vec![b'b'; 400]]).await;
    let response = reqwest::get(url).await.unwrap();
    assert_eq!(read_capped(response, 1000).await.unwrap().len(), 1000);
}

#[tokio::test]
async fn an_engine_answer_over_the_limit_is_an_unreadable_answer_not_a_crash() {
    let server = mock_too_big(claude_body("Acme is great.", &[])).await;
    let engine = ClaudeEngine::new("k", "m", &server.uri(), Duration::from_secs(30));
    assert_eq!(
        engine.ask("a question").await.unwrap_err(),
        EngineError::Unreadable
    );
}

#[tokio::test]
async fn a_bing_answer_over_the_limit_is_a_retryable_error() {
    let server = mock_too_big(json!({ "d": [] })).await;
    let client = BingClient::new(&server.uri());
    let err = client.user_sites(BING_KEY).await.unwrap_err();
    assert!(matches!(err, BingError::Transient(_)), "{err:?}");
    assert!(!err.to_string().contains(BING_KEY));
}

#[tokio::test]
async fn a_google_answer_over_the_limit_is_a_retryable_error() {
    let server = mock_too_big(json!({ "siteEntry": [] })).await;
    let client = GoogleClient::new(hosts(&server));
    let err = client.list_properties("token").await.unwrap_err();
    assert!(matches!(err, GoogleError::Transient(_)), "{err:?}");
}
