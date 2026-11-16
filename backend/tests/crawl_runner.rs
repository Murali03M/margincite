use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use axum::extract::{OriginalUri, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::Router;
use margincite::crawl::client::{CrawlClient, Limits, Network};
use margincite::crawl::render::{NoRenderer, Renderer};
use margincite::crawl::repo;
use margincite::crawl::runner::{self, CrawlDeps, CrawlSettings};
use sqlx::PgPool;
use url::Url;
use uuid::Uuid;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

struct FakeRender {
    html: String,
    fail: bool,
}

#[async_trait]
impl Renderer for FakeRender {
    async fn render(&self, _url: &Url) -> Result<String, String> {
        if self.fail {
            Err("render failed".into())
        } else {
            Ok(self.html.clone())
        }
    }
}

#[derive(Clone)]
struct Gate {
    current: Arc<AtomicUsize>,
    max: Arc<AtomicUsize>,
}

async fn gated(State(gate): State<Gate>, OriginalUri(uri): OriginalUri) -> impl IntoResponse {
    if matches!(uri.path(), "/robots.txt" | "/sitemap.xml" | "/llms.txt") {
        return (StatusCode::NOT_FOUND, "").into_response();
    }
    let now = gate.current.fetch_add(1, Ordering::SeqCst) + 1;
    gate.max.fetch_max(now, Ordering::SeqCst);
    tokio::time::sleep(Duration::from_millis(200)).await;
    gate.current.fetch_sub(1, Ordering::SeqCst);
    let body = if uri.path() == "/" {
        "<a href=\"/a\">a</a><a href=\"/b\">b</a><a href=\"/c\">c</a><a href=\"/d\">d</a>"
    } else {
        "page"
    };
    (StatusCode::OK, [("content-type", "text/html")], body).into_response()
}

fn deps(pool: PgPool, renderer: Arc<dyn Renderer>) -> CrawlDeps {
    CrawlDeps {
        pool,
        client: CrawlClient::with_limits(
            "https://example.com/bot",
            Network::AllowPrivate,
            Limits {
                timeout: Duration::from_secs(5),
                max_bytes: 1024 * 1024,
                max_redirects: 5,
            },
        ),
        renderer,
        settings: CrawlSettings {
            concurrency: 2,
            delay: Duration::ZERO,
            ..CrawlSettings::default()
        },
        config: None,
    }
}

async fn begin(pool: &PgPool, server: &MockServer, cap: i32) -> (Uuid, Uuid) {
    let workspace_id = Uuid::new_v4();
    let site_id = Uuid::new_v4();
    let root = format!("{}/", server.uri());
    sqlx::query("INSERT INTO workspaces (id, name) VALUES ($1, 'test')")
        .bind(workspace_id)
        .execute(pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO sites (id, workspace_id, root_url, host, path_prefix, display_name, page_cap)
         VALUES ($1, $2, $3, '127.0.0.1', '/', '127.0.0.1', $4)",
    )
    .bind(site_id)
    .bind(workspace_id)
    .bind(root)
    .bind(cap)
    .execute(pool)
    .await
    .unwrap();
    (workspace_id, site_id)
}

async fn crawl(deps: &CrawlDeps, workspace_id: Uuid, site_id: Uuid) -> repo::Crawl {
    let crawl_id = repo::create_crawl(&deps.pool, workspace_id, site_id, "manual")
        .await
        .unwrap();
    runner::run_crawl(deps, workspace_id, crawl_id)
        .await
        .unwrap();
    repo::get_crawl(&deps.pool, workspace_id, crawl_id)
        .await
        .unwrap()
}

async fn mount_page(server: &MockServer, route: &str, html: &str) {
    Mock::given(method("GET"))
        .and(path(route))
        .respond_with(ResponseTemplate::new(200).set_body_raw(html, "text/html"))
        .mount(server)
        .await;
}

fn no_render() -> Arc<dyn Renderer> {
    Arc::new(NoRenderer)
}

#[sqlx::test]
async fn crawls_every_linked_page_once(pool: PgPool) {
    let server = MockServer::start().await;
    mount_page(&server, "/", r#"<a href="/a">a</a>"#).await;
    mount_page(&server, "/a", r#"<a href="/b">b</a>"#).await;
    mount_page(&server, "/b", r#"<a href="/c">c</a>"#).await;
    mount_page(&server, "/c", r#"<a href="/d">d</a>"#).await;
    mount_page(&server, "/d", r#"<a href="/">home</a>"#).await;
    let (workspace_id, site_id) = begin(&pool, &server, 500).await;
    let crawl = crawl(&deps(pool.clone(), no_render()), workspace_id, site_id).await;
    assert_eq!(crawl.pages_crawled, 5);
    assert_eq!(crawl.stop_reason.as_deref(), Some("finished"));
}

#[sqlx::test]
async fn records_depth_from_the_home_page(pool: PgPool) {
    let server = MockServer::start().await;
    mount_page(&server, "/", r#"<a href="/a">a</a>"#).await;
    mount_page(&server, "/a", r#"<a href="/a/b">b</a>"#).await;
    mount_page(&server, "/a/b", "end").await;
    let (workspace_id, site_id) = begin(&pool, &server, 500).await;
    let crawl = crawl(&deps(pool.clone(), no_render()), workspace_id, site_id).await;
    let pages = repo::list_pages(&pool, workspace_id, crawl.id)
        .await
        .unwrap();
    let depth = |suffix: &str| {
        pages
            .iter()
            .find(|page| page.url.ends_with(suffix))
            .unwrap()
            .depth
    };
    assert_eq!(depth("/"), 0);
    assert_eq!(depth("/a"), 1);
    assert_eq!(depth("/a/b"), 2);
}

#[sqlx::test]
async fn finds_pages_only_the_sitemap_lists(pool: PgPool) {
    let server = MockServer::start().await;
    mount_page(&server, "/", "home").await;
    mount_page(&server, "/only", "listed").await;
    let xml = format!(
        "<urlset><url><loc>{}/only</loc></url></urlset>",
        server.uri()
    );
    Mock::given(method("GET"))
        .and(path("/sitemap.xml"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(xml, "application/xml"))
        .mount(&server)
        .await;
    let (workspace_id, site_id) = begin(&pool, &server, 500).await;
    let crawl = crawl(&deps(pool.clone(), no_render()), workspace_id, site_id).await;
    let pages = repo::list_pages(&pool, workspace_id, crawl.id)
        .await
        .unwrap();
    let only = pages
        .iter()
        .find(|page| page.url.ends_with("/only"))
        .unwrap();
    assert!(only.in_sitemap);
}

#[sqlx::test]
async fn skips_what_robots_disallows(pool: PgPool) {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/robots.txt"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            "User-agent: MarginciteBot\nDisallow: /private\n",
            "text/plain",
        ))
        .mount(&server)
        .await;
    mount_page(
        &server,
        "/",
        r#"<a href="/private">no</a><a href="/ok">ok</a>"#,
    )
    .await;
    mount_page(&server, "/ok", "ok").await;
    Mock::given(method("GET"))
        .and(path("/private"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&server)
        .await;
    let (workspace_id, site_id) = begin(&pool, &server, 500).await;
    crawl(&deps(pool, no_render()), workspace_id, site_id).await;
    server.verify().await;
}

#[sqlx::test]
async fn stays_inside_the_site(pool: PgPool) {
    let server = MockServer::start().await;
    mount_page(
        &server,
        "/",
        r#"<a href="https://other.example/no">no</a><a href="/ok">ok</a>"#,
    )
    .await;
    mount_page(&server, "/ok", "ok").await;
    let (workspace_id, site_id) = begin(&pool, &server, 500).await;
    let crawl = crawl(&deps(pool.clone(), no_render()), workspace_id, site_id).await;
    let pages = repo::list_pages(&pool, workspace_id, crawl.id)
        .await
        .unwrap();
    assert!(pages.iter().all(|page| page.url.contains("127.0.0.1")));
    assert_eq!(pages.len(), 2);
}

#[sqlx::test]
async fn stops_at_the_page_cap(pool: PgPool) {
    let server = MockServer::start().await;
    let mut links = String::new();
    for n in 1..10 {
        let path = format!("/p{n}");
        links.push_str(&format!(r#"<a href="{path}">p</a>"#));
        mount_page(&server, &path, "page").await;
    }
    mount_page(&server, "/", &links).await;
    let (workspace_id, site_id) = begin(&pool, &server, 3).await;
    let crawl = crawl(&deps(pool.clone(), no_render()), workspace_id, site_id).await;
    assert_eq!(crawl.pages_crawled, 3);
    assert_eq!(crawl.stop_reason.as_deref(), Some("page_cap"));
}

#[sqlx::test]
async fn a_broken_page_is_recorded_and_the_crawl_continues(pool: PgPool) {
    let server = MockServer::start().await;
    mount_page(
        &server,
        "/",
        r#"<a href="/bad">bad</a><a href="/ok">ok</a>"#,
    )
    .await;
    mount_page(&server, "/ok", "ok").await;
    Mock::given(method("GET"))
        .and(path("/bad"))
        .respond_with(ResponseTemplate::new(500).set_body_raw("nope", "text/html"))
        .mount(&server)
        .await;
    let (workspace_id, site_id) = begin(&pool, &server, 500).await;
    let crawl = crawl(&deps(pool.clone(), no_render()), workspace_id, site_id).await;
    assert_eq!(crawl.status, "completed");
    let pages = repo::list_pages(&pool, workspace_id, crawl.id)
        .await
        .unwrap();
    assert!(pages
        .iter()
        .any(|page| page.url.ends_with("/bad") && page.status_code == Some(500)));
}

#[sqlx::test]
async fn a_redirect_to_an_internal_address_is_refused_and_recorded(pool: PgPool) {
    let server = MockServer::start().await;
    mount_page(&server, "/", r#"<a href="/go">go</a><a href="/ok">ok</a>"#).await;
    mount_page(&server, "/ok", "ok").await;
    Mock::given(method("GET"))
        .and(path("/go"))
        .respond_with(
            ResponseTemplate::new(302).insert_header("location", "http://169.254.169.254/"),
        )
        .mount(&server)
        .await;
    let (workspace_id, site_id) = begin(&pool, &server, 500).await;
    let crawl = crawl(&deps(pool.clone(), no_render()), workspace_id, site_id).await;
    assert_eq!(crawl.status, "completed");
    let pages = repo::list_pages(&pool, workspace_id, crawl.id)
        .await
        .unwrap();
    let bounced = pages.iter().find(|page| page.url.ends_with("/go")).unwrap();
    assert_eq!(bounced.fetch_error.as_deref(), Some("blocked"));
}

#[sqlx::test]
async fn robots_blocking_everything_completes_with_no_pages(pool: PgPool) {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/robots.txt"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_raw("User-agent: MarginciteBot\nDisallow: /\n", "text/plain"),
        )
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&server)
        .await;
    let (workspace_id, site_id) = begin(&pool, &server, 500).await;
    let crawl = crawl(&deps(pool, no_render()), workspace_id, site_id).await;
    assert_eq!(crawl.status, "completed");
    assert_eq!(crawl.stop_reason.as_deref(), Some("robots_blocked"));
    assert_eq!(crawl.pages_crawled, 0);
    server.verify().await;
}

#[sqlx::test]
async fn an_unreachable_site_fails_with_a_plain_reason(pool: PgPool) {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/robots.txt"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;
    let (workspace_id, site_id) = begin(&pool, &server, 500).await;
    let crawl = crawl(&deps(pool, no_render()), workspace_id, site_id).await;
    assert_eq!(crawl.status, "failed");
    assert_eq!(crawl.stop_reason.as_deref(), Some("site_unreachable"));
    assert_eq!(
        crawl.error.as_deref(),
        Some("The site could not be reached.")
    );
}

#[sqlx::test]
async fn never_runs_more_than_two_requests_at_once(pool: PgPool) {
    let gate = Gate {
        current: Arc::new(AtomicUsize::new(0)),
        max: Arc::new(AtomicUsize::new(0)),
    };
    let app = Router::new().fallback(get(gated)).with_state(gate.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    let workspace_id = Uuid::new_v4();
    let site_id = Uuid::new_v4();
    sqlx::query("INSERT INTO workspaces (id, name) VALUES ($1, 'test')")
        .bind(workspace_id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO sites (id, workspace_id, root_url, host, path_prefix, display_name, page_cap)
         VALUES ($1, $2, $3, '127.0.0.1', '/', '127.0.0.1', 500)",
    )
    .bind(site_id)
    .bind(workspace_id)
    .bind(format!("http://{addr}/"))
    .execute(&pool)
    .await
    .unwrap();
    crawl(&deps(pool, no_render()), workspace_id, site_id).await;
    let max = gate.max.load(Ordering::SeqCst);
    assert!(max <= 2, "in flight {max}");
    assert!(max >= 2, "expected overlap, in flight {max}");
}

#[sqlx::test]
async fn a_javascript_only_page_is_marked_and_rendered(pool: PgPool) {
    let server = MockServer::start().await;
    mount_page(
        &server,
        "/",
        r#"<div id="root"></div><script src="/app.js"></script>"#,
    )
    .await;
    let renderer = Arc::new(FakeRender {
        html: format!("<body>{}</body>", "word ".repeat(60)),
        fail: false,
    });
    let (workspace_id, site_id) = begin(&pool, &server, 500).await;
    let crawl = crawl(&deps(pool.clone(), renderer), workspace_id, site_id).await;
    let pages = repo::list_pages(&pool, workspace_id, crawl.id)
        .await
        .unwrap();
    assert!(pages[0].needed_javascript);
    assert!(pages[0].text.as_deref().unwrap().split_whitespace().count() > 50);
}

#[sqlx::test]
async fn a_failed_render_keeps_the_raw_page_and_the_mark(pool: PgPool) {
    let server = MockServer::start().await;
    mount_page(
        &server,
        "/",
        r#"<div id="root"></div><script src="/app.js"></script>"#,
    )
    .await;
    let renderer = Arc::new(FakeRender {
        html: String::new(),
        fail: true,
    });
    let (workspace_id, site_id) = begin(&pool, &server, 500).await;
    let crawl = crawl(&deps(pool.clone(), renderer), workspace_id, site_id).await;
    let pages = repo::list_pages(&pool, workspace_id, crawl.id)
        .await
        .unwrap();
    assert!(pages[0].needed_javascript);
    assert_eq!(
        pages[0]
            .text
            .as_deref()
            .unwrap_or("")
            .split_whitespace()
            .count(),
        0
    );
}

#[sqlx::test]
async fn a_restarted_crawl_does_not_repeat_or_duplicate_pages(pool: PgPool) {
    let server = MockServer::start().await;
    for (route, html) in [
        ("/", r#"<a href="/a">a</a>"#),
        ("/a", r#"<a href="/b">b</a>"#),
        ("/b", r#"<a href="/c">c</a>"#),
        ("/c", "end"),
    ] {
        Mock::given(method("GET"))
            .and(path(route))
            .respond_with(ResponseTemplate::new(200).set_body_raw(html, "text/html"))
            .expect(1)
            .mount(&server)
            .await;
    }
    let (workspace_id, site_id) = begin(&pool, &server, 2).await;
    let deps = deps(pool.clone(), no_render());
    let crawl_id = repo::create_crawl(&deps.pool, workspace_id, site_id, "manual")
        .await
        .unwrap();
    runner::run_crawl(&deps, workspace_id, crawl_id)
        .await
        .unwrap();
    sqlx::query("UPDATE sites SET page_cap = 10 WHERE id = $1")
        .bind(site_id)
        .execute(&pool)
        .await
        .unwrap();
    runner::run_crawl(&deps, workspace_id, crawl_id)
        .await
        .unwrap();
    let pages = repo::list_pages(&pool, workspace_id, crawl_id)
        .await
        .unwrap();
    assert_eq!(pages.len(), 4);
    server.verify().await;
}

#[sqlx::test]
async fn stores_robots_text_and_whether_llms_txt_exists(pool: PgPool) {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/robots.txt"))
        .respond_with(
            ResponseTemplate::new(200).set_body_raw("User-agent: *\nAllow: /\n", "text/plain"),
        )
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/llms.txt"))
        .respond_with(ResponseTemplate::new(200).set_body_raw("hello", "text/plain"))
        .mount(&server)
        .await;
    mount_page(&server, "/", "home").await;
    let (workspace_id, site_id) = begin(&pool, &server, 500).await;
    let crawl = crawl(&deps(pool, no_render()), workspace_id, site_id).await;
    assert!(crawl.robots_txt_found);
    assert!(crawl.llms_txt_found);
}

#[sqlx::test]
async fn tracking_parameters_do_not_create_duplicate_pages(pool: PgPool) {
    let server = MockServer::start().await;
    mount_page(
        &server,
        "/",
        r#"<a href="/a?utm_source=x">a</a><a href="/a?gclid=1">b</a>"#,
    )
    .await;
    mount_page(&server, "/a", "once").await;
    let (workspace_id, site_id) = begin(&pool, &server, 500).await;
    let crawl = crawl(&deps(pool.clone(), no_render()), workspace_id, site_id).await;
    let pages = repo::list_pages(&pool, workspace_id, crawl.id)
        .await
        .unwrap();
    let copies = pages.iter().filter(|page| page.url.contains("/a")).count();
    assert_eq!(copies, 1);
}

#[sqlx::test]
async fn a_crawl_stops_at_its_time_limit_and_keeps_its_pages(pool: PgPool) {
    let server = MockServer::start().await;
    let links: String = (0..30)
        .map(|n| format!(r#"<a href="/p{n}">p</a>"#))
        .collect();
    mount_page(&server, "/", &links).await;
    Mock::given(method("GET"))
        .and(wiremock::matchers::path_regex(r"^/p\d+$"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_raw("page", "text/html")
                .set_delay(Duration::from_millis(300)),
        )
        .mount(&server)
        .await;
    let (workspace_id, site_id) = begin(&pool, &server, 500).await;
    let mut limited = deps(pool.clone(), no_render());
    limited.settings.time_limit = Duration::from_millis(800);
    let crawl = crawl(&limited, workspace_id, site_id).await;
    assert_eq!(crawl.status, "completed");
    assert_eq!(crawl.stop_reason.as_deref(), Some("time_limit"));
    assert!(crawl.pages_crawled >= 2, "{}", crawl.pages_crawled);
    assert!(crawl.pages_crawled < 31, "{}", crawl.pages_crawled);
    let pages = repo::list_pages(&pool, workspace_id, crawl.id)
        .await
        .unwrap();
    assert_eq!(pages.len() as i32, crawl.pages_crawled);
}

async fn google_jobs(pool: &PgPool) -> i64 {
    sqlx::query_scalar("SELECT COUNT(*) FROM jobs WHERE kind = 'google_sync' AND status = 'queued'")
        .fetch_one(pool)
        .await
        .unwrap()
}

#[sqlx::test]
async fn a_completed_crawl_queues_a_search_console_sync_for_a_connected_site(pool: PgPool) {
    let server = MockServer::start().await;
    mount_page(&server, "/", "home").await;
    let (workspace_id, site_id) = begin(&pool, &server, 500).await;
    let deps = deps(pool.clone(), no_render());
    // Not connected: nothing queued.
    crawl(&deps, workspace_id, site_id).await;
    assert_eq!(google_jobs(&pool).await, 0);
    sqlx::query(
        "INSERT INTO google_connections (site_id, workspace_id, refresh_token_sealed, property, status)
         VALUES ($1, $2, 'sealed', 'sc-domain:example.com', 'connected')",
    )
    .bind(site_id)
    .bind(workspace_id)
    .execute(&pool)
    .await
    .unwrap();
    crawl(&deps, workspace_id, site_id).await;
    crawl(&deps, workspace_id, site_id).await;
    assert_eq!(
        google_jobs(&pool).await,
        1,
        "one queued however many crawls finish"
    );
}
