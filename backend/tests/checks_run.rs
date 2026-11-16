use std::sync::Arc;
use std::time::Duration;

use margincite::checks::{self, store};
use margincite::crawl::client::{CrawlClient, Limits, Network};
use margincite::crawl::render::NoRenderer;
use margincite::crawl::repo;
use margincite::crawl::runner::{self, CrawlDeps, CrawlSettings};
use sqlx::PgPool;
use uuid::Uuid;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn deps(pool: PgPool) -> CrawlDeps {
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
        renderer: Arc::new(NoRenderer),
        settings: CrawlSettings {
            concurrency: 2,
            delay: Duration::ZERO,
            ..CrawlSettings::default()
        },
        config: None,
    }
}

async fn begin(pool: &PgPool, server: &MockServer) -> (Uuid, Uuid) {
    let workspace_id = Uuid::new_v4();
    let site_id = Uuid::new_v4();
    sqlx::query("INSERT INTO workspaces (id, name) VALUES ($1, 'test')")
        .bind(workspace_id)
        .execute(pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO sites (id, workspace_id, root_url, host, path_prefix, display_name)
         VALUES ($1, $2, $3, '127.0.0.1', '/', '127.0.0.1')",
    )
    .bind(site_id)
    .bind(workspace_id)
    .bind(format!("{}/", server.uri()))
    .execute(pool)
    .await
    .unwrap();
    (workspace_id, site_id)
}

async fn mount_html(server: &MockServer, route: &str, html: &str) {
    Mock::given(method("GET"))
        .and(path(route))
        .respond_with(ResponseTemplate::new(200).set_body_raw(html, "text/html"))
        .mount(server)
        .await;
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

async fn findings_of(pool: &PgPool, crawl_id: Uuid) -> Vec<(String, String)> {
    sqlx::query_as("SELECT check_id, target FROM findings WHERE crawl_id = $1 ORDER BY 1, 2")
        .bind(crawl_id)
        .fetch_all(pool)
        .await
        .unwrap()
}

async fn broken_site(server: &MockServer) {
    mount_html(server, "/", r#"<a href="/missing">gone</a>"#).await;
    Mock::given(method("GET"))
        .and(path("/missing"))
        .respond_with(ResponseTemplate::new(404))
        .mount(server)
        .await;
}

#[sqlx::test]
async fn a_finished_crawl_has_findings_for_its_broken_page(pool: PgPool) {
    let server = MockServer::start().await;
    broken_site(&server).await;
    let (workspace_id, site_id) = begin(&pool, &server).await;
    let crawl = crawl(&deps(pool.clone()), workspace_id, site_id).await;
    assert_eq!(crawl.status, "completed");
    assert_eq!(crawl.error, None);
    let found = findings_of(&pool, crawl.id).await;
    assert!(found
        .iter()
        .any(|(check, target)| check == "broken_page" && target.ends_with("/missing")));
    assert!(found
        .iter()
        .any(|(check, _)| check == "broken_internal_link"));
}

#[sqlx::test]
async fn findings_for_a_crawl_are_replaced_not_duplicated_when_checks_run_twice(pool: PgPool) {
    let server = MockServer::start().await;
    broken_site(&server).await;
    let (workspace_id, site_id) = begin(&pool, &server).await;
    let crawl = crawl(&deps(pool.clone()), workspace_id, site_id).await;
    let before = findings_of(&pool, crawl.id).await;
    let ctx = repo::load_context(&pool, workspace_id, crawl.id)
        .await
        .unwrap();
    let again = checks::run_all(&ctx);
    store::replace_findings(&pool, workspace_id, crawl.id, &again)
        .await
        .unwrap();
    store::replace_findings(&pool, workspace_id, crawl.id, &again)
        .await
        .unwrap();
    assert_eq!(findings_of(&pool, crawl.id).await, before);
}

#[sqlx::test]
async fn a_failed_crawl_has_no_findings(pool: PgPool) {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/robots.txt"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;
    let (workspace_id, site_id) = begin(&pool, &server).await;
    let crawl = crawl(&deps(pool.clone()), workspace_id, site_id).await;
    assert_eq!(crawl.status, "failed");
    assert!(findings_of(&pool, crawl.id).await.is_empty());
}

#[sqlx::test]
async fn a_crawl_with_no_pages_still_runs_and_stores_site_level_findings(pool: PgPool) {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/robots.txt"))
        .respond_with(
            ResponseTemplate::new(200).set_body_raw("User-agent: *\nDisallow: /\n", "text/plain"),
        )
        .mount(&server)
        .await;
    let (workspace_id, site_id) = begin(&pool, &server).await;
    let crawl = crawl(&deps(pool.clone()), workspace_id, site_id).await;
    assert_eq!(crawl.status, "completed");
    assert_eq!(crawl.pages_crawled, 0);
    assert_eq!(crawl.error, None);
    let found = findings_of(&pool, crawl.id).await;
    let blocked = found
        .iter()
        .filter(|(check, _)| check == "ai_crawler_blocked")
        .count();
    assert_eq!(blocked, 9);
    assert!(found.iter().any(|(check, _)| check == "sitemap_missing"));
}

#[sqlx::test]
async fn findings_carry_the_workspace_of_their_crawl(pool: PgPool) {
    let server = MockServer::start().await;
    broken_site(&server).await;
    let (workspace_id, site_id) = begin(&pool, &server).await;
    let crawl = crawl(&deps(pool.clone()), workspace_id, site_id).await;
    let (total, mine): (i64, i64) = sqlx::query_as(
        "SELECT COUNT(*), COUNT(*) FILTER (WHERE workspace_id = $2) FROM findings WHERE crawl_id = $1",
    )
    .bind(crawl.id)
    .bind(workspace_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(total > 0);
    assert_eq!(total, mine);
}

#[sqlx::test]
async fn the_context_restores_text_redirects_and_robots(pool: PgPool) {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/robots.txt"))
        .respond_with(
            ResponseTemplate::new(200).set_body_raw("User-agent: *\nAllow: /\n", "text/plain"),
        )
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/old"))
        .respond_with(ResponseTemplate::new(301).insert_header("location", "/"))
        .mount(&server)
        .await;
    mount_html(
        &server,
        "/",
        "<title>Hi</title><p>hello there friend</p><a href=\"/old\">o</a>",
    )
    .await;
    let (workspace_id, site_id) = begin(&pool, &server).await;
    let crawl = crawl(&deps(pool.clone()), workspace_id, site_id).await;
    let ctx = repo::load_context(&pool, workspace_id, crawl.id)
        .await
        .unwrap();
    assert_eq!(ctx.robots_txt.as_deref(), Some("User-agent: *\nAllow: /\n"));
    let home = ctx.pages.iter().find(|page| page.depth == 0).unwrap();
    let facts = home.facts.as_ref().unwrap();
    assert!(facts.text.contains("hello there friend"));
    assert_eq!(facts.title.as_deref(), Some("Hi"));
    assert!(ctx.pages.iter().any(|page| !page.redirects.is_empty()));
    let other = repo::load_context(&pool, Uuid::new_v4(), crawl.id).await;
    assert!(other.is_err());
}

#[sqlx::test]
async fn a_finished_crawl_has_suggestions_and_a_score(pool: PgPool) {
    let server = MockServer::start().await;
    broken_site(&server).await;
    let (workspace_id, site_id) = begin(&pool, &server).await;
    let crawl = crawl(&deps(pool.clone()), workspace_id, site_id).await;
    assert_eq!(crawl.html_pages, 1);
    assert!(crawl.score.is_some_and(|score| score < 100));
    let counts = margincite::suggestions::repo::counts(&pool, workspace_id, site_id)
        .await
        .unwrap();
    assert!(counts.open > 0);
}

#[sqlx::test]
async fn a_blocked_site_has_suggestions_but_no_score(pool: PgPool) {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/robots.txt"))
        .respond_with(
            ResponseTemplate::new(200).set_body_raw("User-agent: *\nDisallow: /\n", "text/plain"),
        )
        .mount(&server)
        .await;
    let (workspace_id, site_id) = begin(&pool, &server).await;
    let crawl = crawl(&deps(pool.clone()), workspace_id, site_id).await;
    assert_eq!(crawl.score, None);
    assert_eq!(crawl.html_pages, 0);
    let counts = margincite::suggestions::repo::counts(&pool, workspace_id, site_id)
        .await
        .unwrap();
    assert!(counts.open >= 10);
}
