mod common;

use std::time::Duration;

use margincite::crawl::client::{CrawlClient, Limits, Network};
use margincite::crawl::render::NoRenderer;
use margincite::crawl::runner::{self, CrawlDeps, CrawlSettings};
use sqlx::PgPool;
use uuid::Uuid;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn auth(builder: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
    builder.header("authorization", "Bearer test-token")
}

fn other(builder: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
    builder.header("authorization", "Bearer other-token")
}

async fn add_site(app: &common::TestApp, url: &str) -> serde_json::Value {
    auth(app.client.post(format!("http://{}/api/v1/sites", app.addr)))
        .json(&serde_json::json!({ "url": url }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap()
}

#[sqlx::test]
async fn adding_a_site_starts_its_first_crawl(pool: PgPool) {
    let app = common::spawn_app(pool.clone()).await;
    let body = add_site(&app, "example.com").await;
    let crawl_id = body["data"]["first_crawl_id"].as_str().unwrap();
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM jobs WHERE kind = 'crawl' AND payload->>'crawl_id' = $1 AND status = 'queued'",
    )
    .bind(crawl_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(count, 1);
}

#[sqlx::test]
async fn starting_a_crawl_returns_202_and_queues_a_job(pool: PgPool) {
    let app = common::spawn_app(pool.clone()).await;
    let body = add_site(&app, "example.com").await;
    let site_id = body["data"]["id"].as_str().unwrap();
    sqlx::query("UPDATE crawls SET status = 'completed' WHERE site_id = $1")
        .bind(Uuid::parse_str(site_id).unwrap())
        .execute(&pool)
        .await
        .unwrap();
    let response = auth(
        app.client
            .post(format!("http://{}/api/v1/sites/{site_id}/crawls", app.addr)),
    )
    .send()
    .await
    .unwrap();
    assert_eq!(response.status(), 202);
    let created: serde_json::Value = response.json().await.unwrap();
    let crawl_id = created["data"]["id"].as_str().unwrap();
    let queued: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM jobs WHERE payload->>'crawl_id' = $1 AND status = 'queued'",
    )
    .bind(crawl_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(queued, 1);
}

#[sqlx::test]
async fn a_second_crawl_while_one_runs_is_409(pool: PgPool) {
    let app = common::spawn_app(pool).await;
    let body = add_site(&app, "example.com").await;
    let site_id = body["data"]["id"].as_str().unwrap();
    let response = auth(
        app.client
            .post(format!("http://{}/api/v1/sites/{site_id}/crawls", app.addr)),
    )
    .send()
    .await
    .unwrap();
    assert_eq!(response.status(), 409);
    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["error"], "A crawl of this site is already running.");
}

#[sqlx::test]
async fn the_eleventh_manual_crawl_in_a_day_is_429(pool: PgPool) {
    let app = common::spawn_app(pool.clone()).await;
    let body = add_site(&app, "example.com").await;
    let site_id = Uuid::parse_str(body["data"]["id"].as_str().unwrap()).unwrap();
    let workspace_id: Uuid = sqlx::query_scalar("SELECT workspace_id FROM sites WHERE id = $1")
        .bind(site_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE crawls SET status = 'completed' WHERE site_id = $1")
        .bind(site_id)
        .execute(&pool)
        .await
        .unwrap();
    for _ in 0..10 {
        sqlx::query(
            "INSERT INTO crawls (id, workspace_id, site_id, status, trigger) VALUES ($1, $2, $3, 'completed', 'manual')",
        )
        .bind(Uuid::new_v4())
        .bind(workspace_id)
        .bind(site_id)
        .execute(&pool)
        .await
        .unwrap();
    }
    let response = auth(
        app.client
            .post(format!("http://{}/api/v1/sites/{site_id}/crawls", app.addr)),
    )
    .send()
    .await
    .unwrap();
    assert_eq!(response.status(), 429);
    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(
        body["error"],
        "You have used today's 10 crawls. Try again tomorrow."
    );
}

#[sqlx::test]
async fn scheduled_crawls_do_not_use_the_daily_allowance(pool: PgPool) {
    let app = common::spawn_app(pool.clone()).await;
    let body = add_site(&app, "example.com").await;
    let site_id = Uuid::parse_str(body["data"]["id"].as_str().unwrap()).unwrap();
    let workspace_id: Uuid = sqlx::query_scalar("SELECT workspace_id FROM sites WHERE id = $1")
        .bind(site_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE crawls SET status = 'completed' WHERE site_id = $1")
        .bind(site_id)
        .execute(&pool)
        .await
        .unwrap();
    for _ in 0..10 {
        sqlx::query(
            "INSERT INTO crawls (id, workspace_id, site_id, status, trigger) VALUES ($1, $2, $3, 'completed', 'scheduled')",
        )
        .bind(Uuid::new_v4())
        .bind(workspace_id)
        .bind(site_id)
        .execute(&pool)
        .await
        .unwrap();
    }
    let response = auth(
        app.client
            .post(format!("http://{}/api/v1/sites/{site_id}/crawls", app.addr)),
    )
    .send()
    .await
    .unwrap();
    assert_eq!(response.status(), 202);
}

#[sqlx::test]
async fn pages_are_listed_in_pages_of_at_most_100(pool: PgPool) {
    let app = common::spawn_app(pool.clone()).await;
    let body = add_site(&app, "example.com").await;
    let crawl_id = Uuid::parse_str(body["data"]["first_crawl_id"].as_str().unwrap()).unwrap();
    let workspace_id: Uuid = sqlx::query_scalar("SELECT workspace_id FROM crawls WHERE id = $1")
        .bind(crawl_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    let response = auth(app.client.get(format!(
        "http://{}/api/v1/crawls/{crawl_id}/pages?limit=500",
        app.addr
    )))
    .send()
    .await
    .unwrap();
    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["meta"]["limit"], 100);
    let _ = workspace_id;
}

#[sqlx::test]
async fn the_page_list_leaves_out_the_text_and_one_page_includes_it(pool: PgPool) {
    let app = common::spawn_app(pool.clone()).await;
    let body = add_site(&app, "example.com").await;
    let crawl_id = Uuid::parse_str(body["data"]["first_crawl_id"].as_str().unwrap()).unwrap();
    let workspace_id: Uuid = sqlx::query_scalar("SELECT workspace_id FROM crawls WHERE id = $1")
        .bind(crawl_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    let page_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO pages (id, workspace_id, crawl_id, url, depth, is_https, text)
         VALUES ($1, $2, $3, 'https://example.com/', 0, true, 'hello page')",
    )
    .bind(page_id)
    .bind(workspace_id)
    .bind(crawl_id)
    .execute(&pool)
    .await
    .unwrap();
    let listed: serde_json::Value = auth(app.client.get(format!(
        "http://{}/api/v1/crawls/{crawl_id}/pages",
        app.addr
    )))
    .send()
    .await
    .unwrap()
    .json()
    .await
    .unwrap();
    assert!(listed["data"][0].get("text").is_none());
    let one: serde_json::Value = auth(
        app.client
            .get(format!("http://{}/api/v1/pages/{page_id}", app.addr)),
    )
    .send()
    .await
    .unwrap()
    .json()
    .await
    .unwrap();
    assert_eq!(one["data"]["text"], "hello page");
}

#[sqlx::test]
async fn another_workspaces_crawl_and_pages_are_404(pool: PgPool) {
    let app = common::spawn_app(pool.clone()).await;
    let body = add_site(&app, "example.com").await;
    let crawl_id = body["data"]["first_crawl_id"].as_str().unwrap();
    let crawl = other(
        app.client
            .get(format!("http://{}/api/v1/crawls/{crawl_id}", app.addr)),
    )
    .send()
    .await
    .unwrap();
    assert_eq!(crawl.status(), 404);
    let pages = other(app.client.get(format!(
        "http://{}/api/v1/crawls/{crawl_id}/pages",
        app.addr
    )))
    .send()
    .await
    .unwrap();
    assert_eq!(pages.status(), 404);
}

#[sqlx::test]
async fn finishing_a_crawl_schedules_the_next_in_seven_days(pool: PgPool) {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(ResponseTemplate::new(200).set_body_raw("home", "text/html"))
        .mount(&server)
        .await;
    let (workspace_id, site_id, crawl_id) = local_site(&pool, &server).await;
    let deps = local_deps(pool.clone());
    runner::run_crawl(&deps, workspace_id, crawl_id)
        .await
        .unwrap();
    let wait: f64 = sqlx::query_scalar(
        "SELECT EXTRACT(EPOCH FROM (run_at - now()))::float8 FROM jobs
         WHERE payload->>'site_id' = $1 AND payload->>'trigger' = 'scheduled'",
    )
    .bind(site_id.to_string())
    .fetch_one(&pool)
    .await
    .unwrap();
    let week = 7.0 * 24.0 * 60.0 * 60.0;
    assert!((week - 120.0..week + 120.0).contains(&wait), "{wait}");
}

#[sqlx::test]
async fn a_deleted_site_is_not_rescheduled(pool: PgPool) {
    let workspace_id = Uuid::new_v4();
    let site_id = Uuid::new_v4();
    sqlx::query("INSERT INTO workspaces (id, name) VALUES ($1, 'gone')")
        .bind(workspace_id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO sites (id, workspace_id, root_url, host, path_prefix, display_name)
         VALUES ($1, $2, 'https://example.com/', 'example.com', '/', 'example.com')",
    )
    .bind(site_id)
    .bind(workspace_id)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("DELETE FROM sites WHERE id = $1")
        .bind(site_id)
        .execute(&pool)
        .await
        .unwrap();
    runner::schedule_next(&pool, workspace_id, site_id)
        .await
        .unwrap();
    let jobs: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM jobs WHERE payload->>'site_id' = $1")
        .bind(site_id.to_string())
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(jobs, 0);
}

#[sqlx::test]
async fn only_the_two_newest_crawls_keep_their_page_text(pool: PgPool) {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(ResponseTemplate::new(200).set_body_raw("fresh words here", "text/html"))
        .mount(&server)
        .await;
    let (workspace_id, site_id, crawl_id) = local_site(&pool, &server).await;
    for _ in 0..2 {
        let old = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO crawls (id, workspace_id, site_id, status, trigger, created_at)
             VALUES ($1, $2, $3, 'completed', 'scheduled', now() - interval '2 days')",
        )
        .bind(old)
        .bind(workspace_id)
        .bind(site_id)
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO pages (id, workspace_id, crawl_id, url, depth, is_https, text)
             VALUES ($1, $2, $3, 'https://old.example/', 0, true, 'old text')",
        )
        .bind(Uuid::new_v4())
        .bind(workspace_id)
        .bind(old)
        .execute(&pool)
        .await
        .unwrap();
    }
    runner::run_crawl(&local_deps(pool.clone()), workspace_id, crawl_id)
        .await
        .unwrap();
    let kept: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM pages WHERE workspace_id = $1 AND text IS NOT NULL",
    )
    .bind(workspace_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(kept, 2);
}

#[sqlx::test]
async fn page_text_comes_back_as_plain_json_text(pool: PgPool) {
    let app = common::spawn_app(pool.clone()).await;
    let body = add_site(&app, "example.com").await;
    let crawl_id = Uuid::parse_str(body["data"]["first_crawl_id"].as_str().unwrap()).unwrap();
    let workspace_id: Uuid = sqlx::query_scalar("SELECT workspace_id FROM crawls WHERE id = $1")
        .bind(crawl_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    let page_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO pages (id, workspace_id, crawl_id, url, depth, is_https, text)
         VALUES ($1, $2, $3, 'https://example.com/', 0, true, '<script>alert(1)</script>')",
    )
    .bind(page_id)
    .bind(workspace_id)
    .bind(crawl_id)
    .execute(&pool)
    .await
    .unwrap();
    let response = auth(
        app.client
            .get(format!("http://{}/api/v1/pages/{page_id}", app.addr)),
    )
    .send()
    .await
    .unwrap();
    assert_eq!(
        response
            .headers()
            .get("content-type")
            .unwrap()
            .to_str()
            .unwrap(),
        "application/json"
    );
    let raw = response.text().await.unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&raw).unwrap();
    assert_eq!(parsed["data"]["text"], "<script>alert(1)</script>");
    assert!(raw.trim_start().starts_with('{'));
}

fn local_deps(pool: PgPool) -> CrawlDeps {
    CrawlDeps {
        pool,
        client: CrawlClient::with_limits(
            "https://example.com/bot",
            Network::AllowPrivate,
            Limits {
                timeout: Duration::from_secs(5),
                max_bytes: 1024 * 1024,
                max_redirects: 3,
            },
        ),
        renderer: std::sync::Arc::new(NoRenderer),
        settings: CrawlSettings {
            concurrency: 2,
            delay: Duration::ZERO,
            ..CrawlSettings::default()
        },
        config: None,
    }
}

async fn local_site(pool: &PgPool, server: &MockServer) -> (Uuid, Uuid, Uuid) {
    let workspace_id = Uuid::new_v4();
    let site_id = Uuid::new_v4();
    sqlx::query("INSERT INTO workspaces (id, name) VALUES ($1, 'local')")
        .bind(workspace_id)
        .execute(pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO sites (id, workspace_id, root_url, host, path_prefix, display_name)
         VALUES ($1, $2, $3, '127.0.0.1', '/', 'local')",
    )
    .bind(site_id)
    .bind(workspace_id)
    .bind(format!("{}/", server.uri()))
    .execute(pool)
    .await
    .unwrap();
    let crawl_id = margincite::crawl::repo::create_crawl(pool, workspace_id, site_id, "manual")
        .await
        .unwrap();
    (workspace_id, site_id, crawl_id)
}

async fn workspace_of(pool: &PgPool, site_id: Uuid) -> Uuid {
    sqlx::query_scalar("SELECT workspace_id FROM sites WHERE id = $1")
        .bind(site_id)
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn insert_crawls(pool: &PgPool, site_id: Uuid, trigger: &str, count: usize) {
    let workspace_id = workspace_of(pool, site_id).await;
    for _ in 0..count {
        sqlx::query(
            "INSERT INTO crawls (id, workspace_id, site_id, status, trigger) VALUES ($1, $2, $3, 'completed', $4)",
        )
        .bind(Uuid::new_v4())
        .bind(workspace_id)
        .bind(site_id)
        .bind(trigger)
        .execute(pool)
        .await
        .unwrap();
    }
}

#[sqlx::test]
async fn first_crawls_count_toward_the_daily_limit(pool: PgPool) {
    let app = common::spawn_app(pool.clone()).await;
    let body = add_site(&app, "example.com").await;
    let site_id = Uuid::parse_str(body["data"]["id"].as_str().unwrap()).unwrap();
    sqlx::query("UPDATE crawls SET status = 'completed' WHERE site_id = $1")
        .bind(site_id)
        .execute(&pool)
        .await
        .unwrap();
    insert_crawls(&pool, site_id, "first", 9).await;
    let response = auth(
        app.client
            .post(format!("http://{}/api/v1/sites/{site_id}/crawls", app.addr)),
    )
    .send()
    .await
    .unwrap();
    assert_eq!(response.status(), 429);
}

#[sqlx::test]
async fn adding_a_site_is_refused_once_the_daily_limit_is_used(pool: PgPool) {
    let app = common::spawn_app(pool.clone()).await;
    let body = add_site(&app, "example.com").await;
    let site_id = Uuid::parse_str(body["data"]["id"].as_str().unwrap()).unwrap();
    insert_crawls(&pool, site_id, "first", 9).await;
    let response = auth(app.client.post(format!("http://{}/api/v1/sites", app.addr)))
        .json(&serde_json::json!({ "url": "second.example" }))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 429);
    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(
        body["error"],
        "You have used today's 10 crawls. Try again tomorrow."
    );
}

async fn queued_crawl_jobs(pool: &PgPool, site_id: Uuid) -> i64 {
    sqlx::query_scalar(
        "SELECT COUNT(*) FROM jobs WHERE kind = 'crawl' AND status = 'queued' AND payload->>'site_id' = $1",
    )
    .bind(site_id.to_string())
    .fetch_one(pool)
    .await
    .unwrap()
}

#[sqlx::test]
async fn scheduling_twice_leaves_one_queued_crawl_job(pool: PgPool) {
    let app = common::spawn_app(pool.clone()).await;
    let body = add_site(&app, "example.com").await;
    let site_id = Uuid::parse_str(body["data"]["id"].as_str().unwrap()).unwrap();
    let workspace_id = workspace_of(&pool, site_id).await;
    sqlx::query("UPDATE jobs SET status = 'done'")
        .execute(&pool)
        .await
        .unwrap();
    runner::schedule_next(&pool, workspace_id, site_id)
        .await
        .unwrap();
    runner::schedule_next(&pool, workspace_id, site_id)
        .await
        .unwrap();
    assert_eq!(queued_crawl_jobs(&pool, site_id).await, 1);
}

#[sqlx::test]
async fn a_queued_manual_crawl_job_also_blocks_scheduling(pool: PgPool) {
    let app = common::spawn_app(pool.clone()).await;
    let body = add_site(&app, "example.com").await;
    let site_id = Uuid::parse_str(body["data"]["id"].as_str().unwrap()).unwrap();
    let workspace_id = workspace_of(&pool, site_id).await;
    // The first crawl's job is queued and names its crawl, not the site.
    runner::schedule_next(&pool, workspace_id, site_id)
        .await
        .unwrap();
    assert_eq!(queued_crawl_jobs(&pool, site_id).await, 0);
}

#[sqlx::test]
async fn a_crawl_response_reports_robots_txt_found_and_never_the_text(pool: PgPool) {
    let app = common::spawn_app(pool.clone()).await;
    let body = add_site(&app, "example.com").await;
    let crawl_id = Uuid::parse_str(body["data"]["first_crawl_id"].as_str().unwrap()).unwrap();
    let site_id = Uuid::parse_str(body["data"]["id"].as_str().unwrap()).unwrap();
    let url = format!("http://{}/api/v1/crawls/{crawl_id}", app.addr);
    let before: serde_json::Value = auth(app.client.get(&url))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(before["data"]["robots_txt_found"], false);
    assert!(before["data"].get("robots_txt").is_none());
    sqlx::query("UPDATE crawls SET robots_txt = 'User-agent: *' WHERE id = $1")
        .bind(crawl_id)
        .execute(&pool)
        .await
        .unwrap();
    let after: serde_json::Value = auth(app.client.get(&url))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(after["data"]["robots_txt_found"], true);
    let list: serde_json::Value = auth(
        app.client
            .get(format!("http://{}/api/v1/sites/{site_id}/crawls", app.addr)),
    )
    .send()
    .await
    .unwrap()
    .json()
    .await
    .unwrap();
    assert_eq!(list["data"][0]["robots_txt_found"], true);
    assert!(list["data"][0].get("robots_txt").is_none());
}
