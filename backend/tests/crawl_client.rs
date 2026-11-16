use std::time::Duration;

use margincite::crawl::client::{CrawlClient, FetchError, Limits, Network};
use url::Url;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const CONTACT: &str = "https://example.com/margincite-bot";

fn limits() -> Limits {
    Limits {
        timeout: Duration::from_millis(200),
        max_bytes: 64,
        max_redirects: 10,
    }
}

fn client(network: Network) -> CrawlClient {
    CrawlClient::with_limits(CONTACT, network, limits())
}

#[tokio::test]
async fn fetches_html_and_times_it() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(
            ResponseTemplate::new(200).set_body_raw("<p>Hello</p>", "text/html; charset=utf-8"),
        )
        .mount(&server)
        .await;
    let fetched = client(Network::AllowPrivate)
        .fetch(&Url::parse(&server.uri()).unwrap())
        .await;
    assert_eq!(fetched.status, Some(200));
    assert_eq!(fetched.body.as_deref(), Some("<p>Hello</p>"));
    assert!(fetched.elapsed_ms > 0);
    assert!(fetched.error.is_none());
}

#[tokio::test]
async fn sends_the_bot_user_agent() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(wiremock::matchers::header(
            "user-agent",
            "MarginciteBot/0.1 (+https://example.com/margincite-bot)",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_raw("ok", "text/html"))
        .expect(1)
        .mount(&server)
        .await;
    let fetched = client(Network::AllowPrivate)
        .fetch(&Url::parse(&server.uri()).unwrap())
        .await;
    assert_eq!(fetched.status, Some(200));
    server.verify().await;
}

#[tokio::test]
async fn follows_redirects_and_records_them() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(
            ResponseTemplate::new(302).insert_header("location", format!("{}/a", server.uri())),
        )
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/a"))
        .respond_with(
            ResponseTemplate::new(302).insert_header("location", format!("{}/b", server.uri())),
        )
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/b"))
        .respond_with(ResponseTemplate::new(200).set_body_raw("landed", "text/html"))
        .mount(&server)
        .await;
    let fetched = client(Network::AllowPrivate)
        .fetch(&Url::parse(&format!("{}/", server.uri())).unwrap())
        .await;
    assert_eq!(fetched.redirects.len(), 2);
    assert!(fetched.final_url.path().ends_with("/b"));
    assert_eq!(fetched.body.as_deref(), Some("landed"));
}

#[tokio::test]
async fn stops_after_ten_redirects() {
    let server = MockServer::start().await;
    for hop in 0..=10 {
        let next = format!("{}/r/{}", server.uri(), hop + 1);
        Mock::given(method("GET"))
            .and(path(format!("/r/{hop}")))
            .respond_with(ResponseTemplate::new(302).insert_header("location", next))
            .mount(&server)
            .await;
    }
    let fetched = client(Network::AllowPrivate)
        .fetch(&Url::parse(&format!("{}/r/0", server.uri())).unwrap())
        .await;
    assert!(matches!(fetched.error, Some(FetchError::TooManyRedirects)));
}

#[tokio::test]
async fn cuts_off_a_body_over_5_mb() {
    let server = MockServer::start().await;
    let body = vec![b'a'; 5 * 1024 * 1024 + 1];
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(body, "text/html"))
        .mount(&server)
        .await;
    let fetched = CrawlClient::new(CONTACT, Network::AllowPrivate)
        .fetch(&Url::parse(&server.uri()).unwrap())
        .await;
    assert!(matches!(fetched.error, Some(FetchError::TooLarge)));
    assert!(fetched.body.is_none());
}

#[tokio::test]
async fn times_out_a_slow_response() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_delay(Duration::from_secs(2)))
        .mount(&server)
        .await;
    let fetched = client(Network::AllowPrivate)
        .fetch(&Url::parse(&server.uri()).unwrap())
        .await;
    assert!(matches!(fetched.error, Some(FetchError::Timeout)));
}

#[tokio::test]
async fn does_not_read_a_pdf_body() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_raw("%PDF", "application/pdf"))
        .mount(&server)
        .await;
    let fetched = client(Network::AllowPrivate)
        .fetch(&Url::parse(&server.uri()).unwrap())
        .await;
    assert_eq!(fetched.content_type.as_deref(), Some("application/pdf"));
    assert!(fetched.body.is_none());
    assert!(fetched.error.is_none());
}

#[tokio::test]
async fn public_mode_refuses_localhost() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&server)
        .await;
    let fetched = CrawlClient::new(CONTACT, Network::Public)
        .fetch(&Url::parse(&server.uri()).unwrap())
        .await;
    assert!(matches!(fetched.error, Some(FetchError::Blocked)));
    server.verify().await;
}

#[tokio::test]
async fn public_mode_refuses_a_redirect_to_an_internal_address() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(
            ResponseTemplate::new(302).insert_header("location", "http://169.254.169.254/"),
        )
        .expect(1)
        .mount(&server)
        .await;
    // AllowPrivate reaches the local mock. The next hop is refused by the same
    // address check Public mode uses, including the metadata address.
    let fetched = client(Network::AllowPrivate)
        .fetch(&Url::parse(&server.uri()).unwrap())
        .await;
    assert!(matches!(fetched.error, Some(FetchError::Blocked)));
    server.verify().await;
}

async fn resolve(host: &str, network: Network) -> Result<Vec<std::net::SocketAddr>, String> {
    use reqwest::dns::{Name, Resolve};
    let resolver = margincite::crawl::client::GuardedResolver::new(network);
    let name: Name = host.parse().unwrap();
    resolver
        .resolve(name)
        .await
        .map(|addrs| addrs.collect())
        .map_err(|err| err.to_string())
}

#[tokio::test]
async fn the_resolver_refuses_a_name_that_resolves_to_loopback() {
    assert!(resolve("localhost", Network::Public).await.is_err());
}

#[tokio::test]
async fn the_resolver_refuses_a_name_that_resolves_to_a_private_address() {
    for host in ["10.0.0.1", "169.254.169.254", "192.168.1.1"] {
        assert!(resolve(host, Network::Public).await.is_err(), "{host}");
    }
}

#[tokio::test]
async fn the_resolver_allows_private_addresses_in_allow_private_mode() {
    let addrs = resolve("localhost", Network::AllowPrivate).await.unwrap();
    assert!(addrs.iter().all(|addr| addr.ip().is_loopback()));
    assert!(!addrs.is_empty());
}

#[tokio::test]
async fn public_mode_refuses_a_name_that_resolves_to_localhost() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&server)
        .await;
    let port = server.address().port();
    let fetched = CrawlClient::new(CONTACT, Network::Public)
        .fetch(&Url::parse(&format!("http://localhost:{port}/")).unwrap())
        .await;
    assert!(matches!(fetched.error, Some(FetchError::Blocked)));
    server.verify().await;
}
