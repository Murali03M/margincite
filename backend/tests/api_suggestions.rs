mod common;

use margincite::checks::store::replace_findings;
use margincite::checks::Finding;
use margincite::crawl::repo::{self, Finish, NewPage};
use margincite::suggestions::reconcile;
use serde_json::{json, Value};
use sqlx::PgPool;
use uuid::Uuid;

const A: &str = "https://example.com/a";
const B: &str = "https://example.com/b";

fn auth(builder: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
    builder.header("authorization", "Bearer test-token")
}

fn other(builder: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
    builder.header("authorization", "Bearer other-token")
}

struct World {
    app: common::TestApp,
    pool: PgPool,
    workspace_id: Uuid,
    site_id: Uuid,
}

impl World {
    async fn new(pool: PgPool) -> World {
        let app = common::spawn_app(pool.clone()).await;
        let me: Value = auth(app.client.get(format!("http://{}/api/v1/me", app.addr)))
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

    fn url(&self, path: &str) -> String {
        format!("http://{}{path}", self.app.addr)
    }

    async fn get(&self, path: &str) -> (u16, Value) {
        let response = auth(self.app.client.get(self.url(path)))
            .send()
            .await
            .unwrap();
        (response.status().as_u16(), response.json().await.unwrap())
    }

    async fn patch(&self, path: &str, body: Value) -> (u16, Value) {
        let response = auth(self.app.client.patch(self.url(path)))
            .json(&body)
            .send()
            .await
            .unwrap();
        (response.status().as_u16(), response.json().await.unwrap())
    }

    /// A finished crawl of `pages` with these findings, reconciled.
    async fn crawl(&self, pages: &[&str], findings: &[Finding]) -> Uuid {
        let (pool, ws) = (&self.pool, self.workspace_id);
        let crawl_id = repo::create_crawl(pool, ws, self.site_id, "manual")
            .await
            .unwrap();
        for url in pages {
            let page = NewPage {
                url: url.to_string(),
                depth: 1,
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
                facts: Some(json!({})),
                text: None,
            };
            repo::upsert_page(pool, ws, crawl_id, &page).await.unwrap();
        }
        let done = Finish {
            status: "completed".into(),
            stop_reason: "finished".into(),
            pages_crawled: pages.len() as i32,
            pages_failed: 0,
            error: None,
        };
        repo::finish(pool, ws, crawl_id, &done).await.unwrap();
        replace_findings(pool, ws, crawl_id, findings)
            .await
            .unwrap();
        reconcile::apply(pool, ws, self.site_id, crawl_id)
            .await
            .unwrap();
        crawl_id
    }

    async fn suggestion_id(&self, check: &str) -> Uuid {
        sqlx::query_scalar("SELECT id FROM suggestions WHERE site_id = $1 AND check_id = $2")
            .bind(self.site_id)
            .bind(check)
            .fetch_one(&self.pool)
            .await
            .unwrap()
    }
}

fn on_page(check: &'static str, target: &str) -> Finding {
    Finding::page(check, target, json!({ "page": target }))
}

fn mixed() -> Vec<Finding> {
    vec![
        on_page("title_missing", A),
        on_page("title_missing", B),
        on_page("description_missing", A),
        on_page("h1_missing", A),
        on_page("structured_data_missing", A),
        on_page("thin_content", A),
        Finding::site("sitemap_missing", "https://example.com/", json!({})),
    ]
}

#[sqlx::test]
async fn the_summary_of_a_new_site_has_no_crawl_and_no_score(pool: PgPool) {
    let w = World::new(pool).await;
    let (status, body) = w.get(&format!("/api/v1/sites/{}/summary", w.site_id)).await;
    assert_eq!(status, 200);
    let data = &body["data"];
    assert_eq!(data["site"]["display_name"], "example.com");
    assert!(data["latest_crawl"].is_null());
    assert!(data["score"].is_null());
    assert_eq!(
        data["counts"],
        json!({"open": 0, "done": 0, "dismissed": 0})
    );
    assert_eq!(data["top"], json!([]));
    assert_eq!(data["by_category"].as_array().unwrap().len(), 3);
    assert_eq!(data["info"]["html_pages"], 0);
    assert_eq!(data["info"]["reach_basis"], "pages");
}

#[sqlx::test]
async fn the_summary_after_a_crawl_has_a_score_counts_and_five_top_suggestions(pool: PgPool) {
    let w = World::new(pool).await;
    let crawl_id = w.crawl(&[A, B], &mixed()).await;
    let (_, body) = w.get(&format!("/api/v1/sites/{}/summary", w.site_id)).await;
    let data = &body["data"];
    assert_eq!(data["latest_crawl"]["id"], crawl_id.to_string());
    let score = data["score"].as_i64().unwrap();
    assert!((0..100).contains(&score));
    assert_eq!(data["counts"]["open"], 7);
    let top = data["top"].as_array().unwrap();
    assert_eq!(top.len(), 5);
    let priorities: Vec<f64> = top
        .iter()
        .map(|s| s["priority"].as_f64().unwrap())
        .collect();
    assert!(priorities.windows(2).all(|pair| pair[0] >= pair[1]));
    assert_eq!(top[0]["check_id"], "title_missing");
    let by: Value = data["by_category"].clone();
    let open_of = |name: &str| {
        by.as_array()
            .unwrap()
            .iter()
            .find(|c| c["category"] == name)
            .unwrap()["open"]
            .clone()
    };
    assert_eq!(open_of("technical"), 5);
    assert_eq!(open_of("ai_readiness"), 1);
    assert_eq!(open_of("content"), 1);
    assert_eq!(data["info"]["html_pages"], 2);
    assert_eq!(data["info"]["pages_crawled"], 2);
}

#[sqlx::test]
async fn suggestions_come_back_ranked_and_paged(pool: PgPool) {
    let w = World::new(pool).await;
    w.crawl(&[A, B], &mixed()).await;
    let base = format!("/api/v1/sites/{}/suggestions", w.site_id);
    let (_, first) = w.get(&format!("{base}?limit=4")).await;
    assert_eq!(first["meta"], json!({"total": 6, "page": 1, "limit": 4}));
    let list = first["data"].as_array().unwrap();
    assert_eq!(list.len(), 4);
    assert_eq!(list[0]["check_id"], "title_missing");
    let one = &list[0];
    for key in [
        "id",
        "check_id",
        "category",
        "severity",
        "effort",
        "title",
        "open",
        "done",
        "dismissed",
        "priority",
    ] {
        assert!(!one[key].is_null(), "{key}");
    }
    let (_, second) = w.get(&format!("{base}?limit=4&page=2")).await;
    assert_eq!(second["data"].as_array().unwrap().len(), 2);
}

#[sqlx::test]
async fn filtering_by_category_and_status_works(pool: PgPool) {
    let w = World::new(pool).await;
    w.crawl(&[A, B], &mixed()).await;
    let base = format!("/api/v1/sites/{}/suggestions", w.site_id);
    let (_, content) = w.get(&format!("{base}?category=content")).await;
    let ids: Vec<_> = content["data"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["check_id"].clone())
        .collect();
    assert_eq!(ids, [json!("thin_content")]);
    let id = w.suggestion_id("thin_content").await;
    w.patch(
        &format!("/api/v1/suggestions/{id}"),
        json!({"status": "done"}),
    )
    .await;
    let (_, open) = w.get(&format!("{base}?category=content")).await;
    assert_eq!(open["data"], json!([]));
    let (_, done) = w.get(&format!("{base}?category=content&status=done")).await;
    assert_eq!(done["data"].as_array().unwrap().len(), 1);
    let (_, all) = w.get(&format!("{base}?status=all")).await;
    assert_eq!(all["meta"]["total"], 6);
    assert_eq!(w.get(&format!("{base}?status=bogus")).await.0, 422);
    assert_eq!(w.get(&format!("{base}?category=bogus")).await.0, 422);
}

#[sqlx::test]
async fn a_suggestion_shows_its_why_fix_and_items(pool: PgPool) {
    let w = World::new(pool).await;
    w.crawl(&[A, B], &mixed()).await;
    let id = w.suggestion_id("title_missing").await;
    let (status, body) = w.get(&format!("/api/v1/suggestions/{id}?limit=1")).await;
    assert_eq!(status, 200);
    let data = &body["data"];
    assert_eq!(data["suggestion"]["check_id"], "title_missing");
    assert_eq!(data["suggestion"]["open"], 2);
    assert!(!data["why"].as_str().unwrap().is_empty());
    assert!(!data["fix"].as_str().unwrap().is_empty());
    assert_eq!(data["items"].as_array().unwrap().len(), 1);
    assert_eq!(body["meta"]["total"], 2);
    let item = &data["items"][0];
    assert_eq!(item["target"], A);
    assert_eq!(item["target_kind"], "page");
    assert_eq!(item["status"], "open");
    assert_eq!(item["evidence"]["page"], A);
    assert!(item["first_seen"].as_str().unwrap().contains('T'));
    assert!(item["resolved_at"].is_null());
}

#[sqlx::test]
async fn marking_one_target_done_changes_one_item(pool: PgPool) {
    let w = World::new(pool).await;
    w.crawl(&[A, B], &mixed()).await;
    let id = w.suggestion_id("title_missing").await;
    let (status, body) = w
        .patch(
            &format!("/api/v1/suggestions/{id}"),
            json!({"status": "done", "targets": [A]}),
        )
        .await;
    assert_eq!((status, &body["data"]), (200, &json!({"changed": 1})));
    let (_, after) = w.get(&format!("/api/v1/suggestions/{id}")).await;
    assert_eq!(after["data"]["suggestion"]["open"], 1);
    assert_eq!(after["data"]["suggestion"]["done"], 1);
    let (_, done_only) = w
        .get(&format!("/api/v1/suggestions/{id}?status=done"))
        .await;
    assert_eq!(done_only["data"]["items"].as_array().unwrap().len(), 1);
    assert!(!done_only["data"]["items"][0]["resolved_at"].is_null());
}

#[sqlx::test]
async fn marking_the_whole_suggestion_dismissed_changes_every_item(pool: PgPool) {
    let w = World::new(pool).await;
    w.crawl(&[A, B], &mixed()).await;
    let id = w.suggestion_id("title_missing").await;
    let (_, body) = w
        .patch(
            &format!("/api/v1/suggestions/{id}"),
            json!({"status": "dismissed"}),
        )
        .await;
    assert_eq!(body["data"]["changed"], 2);
    let (_, again) = w
        .patch(
            &format!("/api/v1/suggestions/{id}"),
            json!({"status": "dismissed"}),
        )
        .await;
    assert_eq!(again["data"]["changed"], 0);
}

#[sqlx::test]
async fn an_unknown_status_is_422(pool: PgPool) {
    let w = World::new(pool).await;
    w.crawl(&[A], &mixed()).await;
    let id = w.suggestion_id("title_missing").await;
    let path = format!("/api/v1/suggestions/{id}");
    assert_eq!(w.patch(&path, json!({"status": "finished"})).await.0, 422);
    assert_eq!(
        w.patch(&path, json!({"status": "done", "targets": []}))
            .await
            .0,
        422
    );
    let (_, body) = w.get(&path).await;
    assert_eq!(body["data"]["suggestion"]["open"], 2);
}

#[sqlx::test]
async fn a_page_lists_the_findings_against_it(pool: PgPool) {
    let w = World::new(pool).await;
    let crawl_id = w.crawl(&[A, B], &mixed()).await;
    let page_id: Uuid = sqlx::query_scalar("SELECT id FROM pages WHERE crawl_id = $1 AND url = $2")
        .bind(crawl_id)
        .bind(B)
        .fetch_one(&w.pool)
        .await
        .unwrap();
    let (status, body) = w.get(&format!("/api/v1/pages/{page_id}/findings")).await;
    assert_eq!(status, 200);
    let found = body["data"].as_array().unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0]["check_id"], "title_missing");
    assert_eq!(found[0]["category"], "technical");
    assert_eq!(found[0]["severity"], 3);
    assert!(!found[0]["title"].as_str().unwrap().is_empty());
    assert_eq!(found[0]["evidence"]["page"], B);
}

#[sqlx::test]
async fn another_workspaces_suggestion_summary_and_findings_are_404(pool: PgPool) {
    let w = World::new(pool).await;
    let crawl_id = w.crawl(&[A], &mixed()).await;
    let id = w.suggestion_id("title_missing").await;
    let page_id: Uuid = sqlx::query_scalar("SELECT id FROM pages WHERE crawl_id = $1")
        .bind(crawl_id)
        .fetch_one(&w.pool)
        .await
        .unwrap();
    let paths = [
        format!("/api/v1/sites/{}/summary", w.site_id),
        format!("/api/v1/sites/{}/suggestions", w.site_id),
        format!("/api/v1/suggestions/{id}"),
        format!("/api/v1/pages/{page_id}/findings"),
    ];
    for path in &paths {
        let response = other(w.app.client.get(w.url(path))).send().await.unwrap();
        assert_eq!(response.status(), 404, "{path}");
    }
    let response = other(
        w.app
            .client
            .patch(w.url(&format!("/api/v1/suggestions/{id}"))),
    )
    .json(&json!({"status": "done"}))
    .send()
    .await
    .unwrap();
    assert_eq!(response.status(), 404);
    let (_, body) = w.get(&format!("/api/v1/suggestions/{id}")).await;
    assert_eq!(body["data"]["suggestion"]["open"], 2);
    let response = w.app.client.get(w.url(&paths[0])).send().await.unwrap();
    assert_eq!(response.status(), 401);
}

#[sqlx::test]
async fn crawled_text_in_evidence_is_returned_as_json_text(pool: PgPool) {
    let w = World::new(pool).await;
    let hostile = "<img src=x onerror=alert(1)>";
    let finding = Finding::page("title_duplicate", A, json!({ "title": hostile }));
    w.crawl(&[A], &[finding]).await;
    let id = w.suggestion_id("title_duplicate").await;
    let response = auth(
        w.app
            .client
            .get(w.url(&format!("/api/v1/suggestions/{id}"))),
    )
    .send()
    .await
    .unwrap();
    assert_eq!(response.headers()["content-type"], "application/json");
    let raw = response.text().await.unwrap();
    assert!(raw.trim_start().starts_with('{'));
    let parsed: Value = serde_json::from_str(&raw).unwrap();
    assert_eq!(parsed["data"]["items"][0]["evidence"]["title"], hostile);
}
