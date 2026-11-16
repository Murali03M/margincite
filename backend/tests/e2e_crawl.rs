mod common;

use std::sync::Arc;
use std::time::Duration;

use margincite::crawl::client::{CrawlClient, Limits, Network};
use margincite::crawl::render::NoRenderer;
use margincite::crawl::runner::{CrawlDeps, CrawlJob, CrawlSettings};
use margincite::jobs::worker::{self, JobHandler};
use sqlx::PgPool;
use tokio_util::sync::CancellationToken;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn token(builder: reqwest::RequestBuilder, token: &str) -> reqwest::RequestBuilder {
    builder.header("authorization", format!("Bearer {token}"))
}

#[sqlx::test]
async fn a_site_can_be_added_crawled_and_read_back(pool: PgPool) {
    let server = MockServer::start().await;
    mount(&server).await;
    let app = common::spawn_app(pool.clone()).await;
    let shutdown = CancellationToken::new();
    let deps = CrawlDeps {
        pool: pool.clone(),
        client: CrawlClient::with_limits(
            "https://example.com/margincite-bot",
            Network::AllowPrivate,
            Limits {
                timeout: Duration::from_secs(5),
                max_bytes: 1024 * 1024,
                max_redirects: 4,
            },
        )
        .pin_host("fixture.test", *server.address()),
        renderer: Arc::new(NoRenderer),
        settings: CrawlSettings {
            concurrency: 2,
            delay: Duration::ZERO,
            ..CrawlSettings::default()
        },
        config: None,
    };
    let worker = tokio::spawn(worker::run(
        pool.clone(),
        vec![Arc::new(CrawlJob { deps }) as Arc<dyn JobHandler>],
        shutdown.clone(),
    ));

    let created = token(
        app.client.post(format!("http://{}/api/v1/sites", app.addr)),
        "test-token",
    )
    .json(&serde_json::json!({
        "url": format!("http://fixture.test:{}/", server.address().port())
    }))
    .send()
    .await
    .unwrap();
    assert_eq!(created.status(), 201);
    let created: serde_json::Value = created.json().await.unwrap();
    let site_id = created["data"]["id"].as_str().unwrap();
    let crawl_id = created["data"]["first_crawl_id"].as_str().unwrap();

    let crawl = wait_until_done(&app, crawl_id).await;
    assert_eq!(crawl["status"], "completed");
    assert_eq!(crawl["stop_reason"], "finished");
    assert_eq!(crawl["pages_crawled"], 5);
    assert_eq!(crawl["pages_failed"], 1);

    let pages = token(
        app.client.get(format!(
            "http://{}/api/v1/crawls/{crawl_id}/pages",
            app.addr
        )),
        "test-token",
    )
    .send()
    .await
    .unwrap()
    .json::<serde_json::Value>()
    .await
    .unwrap();
    let urls: Vec<_> = pages["data"]
        .as_array()
        .unwrap()
        .iter()
        .map(|page| page["url"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(urls.len(), 5);
    assert!(urls.iter().all(|url| !url.contains("/private")));
    assert!(
        pages["data"]
            .as_array()
            .unwrap()
            .iter()
            .any(|page| page["url"].as_str().unwrap().ends_with("/only")
                && page["in_sitemap"] == true)
    );

    let home = pages["data"]
        .as_array()
        .unwrap()
        .iter()
        .find(|page| page["url"].as_str().unwrap().ends_with('/'))
        .unwrap();
    let page_id = home["id"].as_str().unwrap();
    let page = token(
        app.client
            .get(format!("http://{}/api/v1/pages/{page_id}", app.addr)),
        "test-token",
    )
    .send()
    .await
    .unwrap()
    .json::<serde_json::Value>()
    .await
    .unwrap();
    assert_eq!(page["data"]["facts"]["title"], "Home");
    assert!(page["data"]["facts"]["word_count"].as_u64().unwrap() > 0);
    assert!(page["data"]["text"].as_str().unwrap().contains("Welcome"));

    for path in [
        format!("/api/v1/sites/{site_id}"),
        format!("/api/v1/crawls/{crawl_id}"),
        format!("/api/v1/pages/{page_id}"),
    ] {
        let response = token(
            app.client.get(format!("http://{}{path}", app.addr)),
            "other-token",
        )
        .send()
        .await
        .unwrap();
        assert_eq!(response.status(), 404, "{path}");
    }

    shutdown.cancel();
    worker.await.unwrap();
}

async fn wait_until_done(app: &common::TestApp, crawl_id: &str) -> serde_json::Value {
    for _ in 0..40 {
        let body: serde_json::Value = token(
            app.client
                .get(format!("http://{}/api/v1/crawls/{crawl_id}", app.addr)),
            "test-token",
        )
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
        if body["data"]["status"] == "completed" || body["data"]["status"] == "failed" {
            return body["data"].clone();
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    panic!("crawl {crawl_id} did not finish");
}

async fn mount(server: &MockServer) {
    Mock::given(method("GET"))
        .and(path("/robots.txt"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            "User-agent: MarginciteBot\nDisallow: /private\n",
            "text/plain",
        ))
        .mount(server)
        .await;
    let xml = format!(
        "<urlset><url><loc>http://fixture.test:{}/only</loc></url></urlset>",
        server.address().port()
    );
    Mock::given(method("GET"))
        .and(path("/sitemap.xml"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(xml, "application/xml"))
        .mount(server)
        .await;
    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            "<title>Home</title><body>Welcome home <a href=\"/a\">a</a><a href=\"/b\">b</a><a href=\"/bad\">bad</a><a href=\"/private\">no</a></body>",
            "text/html",
        ))
        .mount(server)
        .await;
    for (route, html) in [
        ("/a", "page a"),
        ("/b", "page b"),
        ("/only", "only in the sitemap"),
    ] {
        Mock::given(method("GET"))
            .and(path(route))
            .respond_with(ResponseTemplate::new(200).set_body_raw(html, "text/html"))
            .mount(server)
            .await;
    }
    Mock::given(method("GET"))
        .and(path("/bad"))
        .respond_with(ResponseTemplate::new(500).set_body_raw("broken", "text/html"))
        .mount(server)
        .await;
    Mock::given(method("GET"))
        .and(path("/private"))
        .respond_with(ResponseTemplate::new(200).set_body_raw("secret", "text/html"))
        .expect(0)
        .mount(server)
        .await;
}
