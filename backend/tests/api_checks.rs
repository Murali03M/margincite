mod common;

use margincite::checks::catalog;
use margincite::checks::store::{replace_findings, run_for_crawl};
use margincite::checks::Finding;
use margincite::crawl::repo::{self, Finish, NewPage};
use margincite::suggestions::reconcile;
use serde_json::{json, Value};
use sqlx::PgPool;
use uuid::Uuid;

const A: &str = "https://example.com/a";

struct World {
    app: common::TestApp,
    pool: PgPool,
    workspace_id: Uuid,
    site_id: Uuid,
}

impl World {
    async fn new(pool: PgPool) -> World {
        let app = common::spawn_app(pool.clone()).await;
        let me: Value = app
            .client
            .get(format!("http://{}/api/v1/me", app.addr))
            .header("authorization", "Bearer test-token")
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        let workspace_id: Uuid = me["data"]["workspace_id"]
            .as_str()
            .unwrap()
            .parse()
            .unwrap();
        let site_id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO sites (id, workspace_id, root_url, host, path_prefix, display_name)
             VALUES ($1, $2, 'https://example.com/', 'example.com', '/', 'example.com')",
        )
        .bind(site_id)
        .bind(workspace_id)
        .execute(&pool)
        .await
        .unwrap();
        World {
            app,
            pool,
            workspace_id,
            site_id,
        }
    }

    async fn checks_as(&self, token: &str) -> (u16, Value) {
        let url = format!(
            "http://{}/api/v1/sites/{}/checks",
            self.app.addr, self.site_id
        );
        let response = self
            .app
            .client
            .get(url)
            .header("authorization", format!("Bearer {token}"))
            .send()
            .await
            .unwrap();
        (response.status().as_u16(), response.json().await.unwrap())
    }

    async fn checks(&self) -> Value {
        let (status, body) = self.checks_as("test-token").await;
        assert_eq!(status, 200, "{body}");
        body["data"].clone()
    }

    async fn add_keyword(&self, keyword: &str, target: Option<&str>) {
        sqlx::query(
            "INSERT INTO site_keywords (id, workspace_id, site_id, keyword, target_url)
             VALUES ($1, $2, $3, $4, $5)",
        )
        .bind(Uuid::new_v4())
        .bind(self.workspace_id)
        .bind(self.site_id)
        .bind(keyword)
        .bind(target)
        .execute(&self.pool)
        .await
        .unwrap();
    }

    /// A completed crawl of page `A` with this title and text.
    async fn crawl(&self, title: &str, text: &str) -> Uuid {
        let (pool, ws) = (&self.pool, self.workspace_id);
        let crawl_id = repo::create_crawl(pool, ws, self.site_id, "manual")
            .await
            .unwrap();
        let page = NewPage {
            url: A.to_string(),
            depth: 0,
            in_sitemap: true,
            status_code: Some(200),
            content_type: Some("text/html".into()),
            fetch_error: None,
            redirects: json!([]),
            response_ms: Some(100),
            bytes: Some(1000),
            is_https: true,
            x_robots_tag: None,
            needed_javascript: false,
            facts: Some(json!({
                "title": title, "meta_description": null, "h1s": [], "headings": [],
                "canonical": null, "robots_meta": null, "has_viewport": true, "word_count": 2,
                "structured_data": [], "structured_data_errors": 0, "links": [], "images": [],
                "has_scripts": false, "published": null, "modified": null, "author": null
            })),
            text: Some(text.to_string()),
        };
        repo::upsert_page(pool, ws, crawl_id, &page).await.unwrap();
        let done = Finish {
            status: "completed".into(),
            stop_reason: "finished".into(),
            pages_crawled: 1,
            pages_failed: 0,
            error: None,
        };
        repo::finish(pool, ws, crawl_id, &done).await.unwrap();
        crawl_id
    }
}

fn entry<'a>(checks: &'a Value, id: &str) -> &'a Value {
    checks["checks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["check_id"] == id)
        .unwrap_or_else(|| panic!("{id} missing"))
}

fn status<'a>(checks: &'a Value, id: &str) -> &'a str {
    entry(checks, id)["status"].as_str().unwrap()
}

#[sqlx::test]
async fn a_site_with_no_finished_crawl_has_nothing_run(pool: PgPool) {
    let w = World::new(pool).await;
    let data = w.checks().await;
    assert!(data["crawl_id"].is_null());
    let all = data["checks"].as_array().unwrap();
    assert_eq!(all.len(), catalog::all().len());
    assert!(all
        .iter()
        .all(|c| c["status"] == "not_run" && c["open"] == 0));
    assert_eq!(
        entry(&data, "title_missing")["reason"],
        "Crawl the site to run this check."
    );
    assert_eq!(
        entry(&data, "keyword_not_in_title")["reason"],
        "Add target keywords to run this check."
    );
}

#[sqlx::test]
async fn crawled_checks_are_passed_or_failed_and_keyword_checks_are_not_run(pool: PgPool) {
    let w = World::new(pool.clone()).await;
    let crawl_id = w.crawl("Home", "some words").await;
    let found = [Finding::page("title_missing", A, json!({}))];
    replace_findings(&pool, w.workspace_id, crawl_id, &found)
        .await
        .unwrap();
    reconcile::apply(&pool, w.workspace_id, w.site_id, crawl_id)
        .await
        .unwrap();
    let data = w.checks().await;
    assert_eq!(data["crawl_id"], crawl_id.to_string());
    let failed = entry(&data, "title_missing");
    assert_eq!(
        (failed["status"].as_str(), failed["open"].as_i64()),
        (Some("failed"), Some(1))
    );
    assert!(failed["reason"].is_null());
    assert_eq!(failed["title"], "Pages with no title");
    assert_eq!(failed["category"], "technical");
    assert_eq!(status(&data, "h1_missing"), "passed");
    for id in [
        "keyword_not_in_title",
        "keyword_not_in_h1",
        "keyword_not_in_description",
        "keyword_not_in_body",
        "keyword_stuffing",
        "keyword_cannibalization",
    ] {
        assert_eq!(status(&data, id), "not_run", "{id}");
    }
}

#[sqlx::test]
async fn keyword_checks_run_after_a_crawl_and_pass_once_fixed(pool: PgPool) {
    let w = World::new(pool.clone()).await;
    w.add_keyword("widgets", Some(A)).await;
    let first = w.crawl("Our shop", "we sell things").await;
    run_for_crawl(&pool, w.workspace_id, first).await.unwrap();
    let data = w.checks().await;
    for id in [
        "keyword_not_in_title",
        "keyword_not_in_h1",
        "keyword_not_in_description",
        "keyword_not_in_body",
    ] {
        assert_eq!(status(&data, id), "failed", "{id}");
        assert_eq!(entry(&data, id)["open"], 1);
    }
    assert_eq!(status(&data, "keyword_stuffing"), "passed");
    let cannibal = entry(&data, "keyword_cannibalization");
    assert_eq!(cannibal["status"], "not_run");
    assert_eq!(
        cannibal["reason"],
        "Connect Google Search Console to run this check."
    );

    let second = w.crawl("Widgets for sale", "we sell widgets").await;
    run_for_crawl(&pool, w.workspace_id, second).await.unwrap();
    let data = w.checks().await;
    assert_eq!(data["crawl_id"], second.to_string());
    assert_eq!(status(&data, "keyword_not_in_title"), "passed");
    assert_eq!(status(&data, "keyword_not_in_body"), "passed");
    assert_eq!(status(&data, "keyword_not_in_h1"), "failed");
}

#[sqlx::test]
async fn another_workspace_cannot_list_the_checks(pool: PgPool) {
    let w = World::new(pool).await;
    assert_eq!(w.checks_as("other-token").await.0, 404);
}
