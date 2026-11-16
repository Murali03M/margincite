mod common;

use margincite::crawl::repo::{self, Finish, NewPage};
use serde_json::{json, Value};
use sqlx::PgPool;
use uuid::Uuid;

fn auth(builder: reqwest::RequestBuilder, token: &str) -> reqwest::RequestBuilder {
    builder.header("authorization", format!("Bearer {token}"))
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
        let workspace_id = workspace_of(&app, "test-token").await;
        let site_id = add_site(&pool, workspace_id, "https://example.com/", "/").await;
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

    async fn send(
        &self,
        token: &str,
        method: reqwest::Method,
        path: &str,
        body: Option<Value>,
    ) -> (u16, Value) {
        let mut request = auth(self.app.client.request(method, self.url(path)), token);
        if let Some(body) = body {
            request = request.json(&body);
        }
        let response = request.send().await.unwrap();
        (response.status().as_u16(), response.json().await.unwrap())
    }

    async fn get(&self, path: &str) -> (u16, Value) {
        self.send("test-token", reqwest::Method::GET, path, None)
            .await
    }

    async fn post(&self, body: Value) -> (u16, Value) {
        let path = format!("/api/v1/sites/{}/keywords", self.site_id);
        self.send("test-token", reqwest::Method::POST, &path, Some(body))
            .await
    }

    async fn add(&self, keyword: &str) -> Value {
        let (status, body) = self.post(json!({ "keyword": keyword })).await;
        assert_eq!(status, 201, "{body}");
        body["data"].clone()
    }

    async fn list(&self) -> Vec<Value> {
        let (status, body) = self
            .get(&format!("/api/v1/sites/{}/keywords", self.site_id))
            .await;
        assert_eq!(status, 200);
        body["data"].as_array().unwrap().clone()
    }

    /// A completed crawl whose pages carry these `(url, depth, title)`.
    async fn crawl(&self, pages: &[(&str, i32, &str)]) {
        let (pool, ws) = (&self.pool, self.workspace_id);
        let crawl_id = repo::create_crawl(pool, ws, self.site_id, "manual")
            .await
            .unwrap();
        for (url, depth, title) in pages {
            let page = NewPage {
                url: url.to_string(),
                depth: *depth,
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
                facts: Some(facts(title)),
                text: Some("some words".into()),
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
    }
}

pub fn facts(title: &str) -> Value {
    json!({
        "title": title, "meta_description": null, "h1s": [], "headings": [],
        "canonical": null, "robots_meta": null, "has_viewport": true, "word_count": 2,
        "structured_data": [], "structured_data_errors": 0, "links": [], "images": [],
        "has_scripts": false, "published": null, "modified": null, "author": null
    })
}

async fn workspace_of(app: &common::TestApp, token: &str) -> Uuid {
    let me: Value = auth(
        app.client.get(format!("http://{}/api/v1/me", app.addr)),
        token,
    )
    .send()
    .await
    .unwrap()
    .json()
    .await
    .unwrap();
    me["data"]["workspace_id"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap()
}

async fn add_site(pool: &PgPool, workspace_id: Uuid, root: &str, prefix: &str) -> Uuid {
    let site_id = Uuid::new_v4();
    let host = root
        .trim_start_matches("https://")
        .trim_end_matches('/')
        .split('/')
        .next()
        .unwrap()
        .to_string();
    sqlx::query(
        "INSERT INTO sites (id, workspace_id, root_url, host, path_prefix, display_name)
         VALUES ($1, $2, $3, $4, $5, $4)",
    )
    .bind(site_id)
    .bind(workspace_id)
    .bind(root)
    .bind(host)
    .bind(prefix)
    .execute(pool)
    .await
    .unwrap();
    site_id
}

#[sqlx::test]
async fn add_list_and_delete_a_keyword(pool: PgPool) {
    let w = World::new(pool).await;
    assert!(w.list().await.is_empty());
    let added = w.add("  blue   widgets ").await;
    assert_eq!(added["keyword"], "blue widgets");
    for null in [
        "target_url",
        "landing_url",
        "landing_source",
        "position",
        "clicks",
        "impressions",
    ] {
        assert!(added[null].is_null(), "{null}");
    }
    let listed = w.list().await;
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0]["id"], added["id"]);
    let id = added["id"].as_str().unwrap();
    let (status, body) = w
        .send(
            "test-token",
            reqwest::Method::DELETE,
            &format!("/api/v1/keywords/{id}"),
            None,
        )
        .await;
    assert_eq!((status, &body["data"]["deleted"]), (200, &json!(true)));
    assert!(w.list().await.is_empty());
    let (again, _) = w
        .send(
            "test-token",
            reqwest::Method::DELETE,
            &format!("/api/v1/keywords/{id}"),
            None,
        )
        .await;
    assert_eq!(again, 404);
}

#[sqlx::test]
async fn bad_keywords_are_422(pool: PgPool) {
    let w = World::new(pool).await;
    for bad in ["", "   ", "!!! ???", &"a".repeat(101)] {
        let (status, body) = w.post(json!({ "keyword": bad })).await;
        assert_eq!(status, 422, "{bad:?}");
        assert!(body["error"].is_string());
    }
    let (status, _) = w.post(json!({ "keyword": "a".repeat(100) })).await;
    assert_eq!(status, 201);
}

#[sqlx::test]
async fn a_target_url_must_be_on_the_site(pool: PgPool) {
    let w = World::new(pool).await;
    for bad in ["https://other.com/page", "not a url", "ftp://example.com/x"] {
        let (status, _) = w.post(json!({ "keyword": "k", "target_url": bad })).await;
        assert_eq!(status, 422, "{bad}");
    }
    let (status, body) = w.post(json!({ "keyword": "k1", "target_url": "https://www.example.com/pricing?utm_source=x#top" })).await;
    assert_eq!(
        (status, &body["data"]["target_url"]),
        (201, &json!("https://www.example.com/pricing"))
    );
    let (status, body) = w
        .post(json!({ "keyword": "k2", "target_url": "/blog/a" }))
        .await;
    assert_eq!(
        (status, &body["data"]["target_url"]),
        (201, &json!("https://example.com/blog/a"))
    );
    let (status, body) = w.post(json!({ "keyword": "k3", "target_url": "  " })).await;
    assert_eq!((status, body["data"]["target_url"].is_null()), (201, true));
}

#[sqlx::test]
async fn a_target_url_outside_the_sites_path_prefix_is_422(pool: PgPool) {
    let w = World::new(pool.clone()).await;
    let docs = add_site(&pool, w.workspace_id, "https://example.com/docs/", "/docs").await;
    let path = format!("/api/v1/sites/{docs}/keywords");
    let post = |target: &'static str| {
        let (w, path) = (&w, path.clone());
        async move {
            w.send(
                "test-token",
                reqwest::Method::POST,
                &path,
                Some(json!({"keyword": "k", "target_url": target})),
            )
            .await
            .0
        }
    };
    assert_eq!(post("https://example.com/blog").await, 422);
    assert_eq!(post("https://example.com/docs/intro").await, 201);
}

#[sqlx::test]
async fn duplicates_are_409_whatever_the_case(pool: PgPool) {
    let w = World::new(pool).await;
    w.add("Blue Widgets").await;
    let (status, body) = w.post(json!({ "keyword": "blue  WIDGETS" })).await;
    assert_eq!(status, 409);
    assert_eq!(body["error"], "This keyword is already on the list.");
}

#[sqlx::test]
async fn the_51st_keyword_is_409(pool: PgPool) {
    let w = World::new(pool).await;
    for n in 0..50 {
        w.add(&format!("keyword {n}")).await;
    }
    let (status, body) = w.post(json!({ "keyword": "one too many" })).await;
    assert_eq!(status, 409);
    assert_eq!(body["error"], "A site can have at most 50 keywords.");
    assert_eq!(w.list().await.len(), 50);
}

#[sqlx::test]
async fn patch_sets_and_clears_the_target_url(pool: PgPool) {
    let w = World::new(pool).await;
    let id = w.add("widgets").await["id"].as_str().unwrap().to_string();
    let path = format!("/api/v1/keywords/{id}");
    let (status, body) = w
        .send(
            "test-token",
            reqwest::Method::PATCH,
            &path,
            Some(json!({"target_url": "https://example.com/widgets"})),
        )
        .await;
    assert_eq!(
        (status, &body["data"]["target_url"]),
        (200, &json!("https://example.com/widgets"))
    );
    let (status, _) = w
        .send(
            "test-token",
            reqwest::Method::PATCH,
            &path,
            Some(json!({"target_url": "https://elsewhere.com/"})),
        )
        .await;
    assert_eq!(status, 422);
    assert_eq!(
        w.list().await[0]["target_url"],
        "https://example.com/widgets"
    );
    let (status, body) = w
        .send(
            "test-token",
            reqwest::Method::PATCH,
            &path,
            Some(json!({"target_url": null})),
        )
        .await;
    assert_eq!((status, body["data"]["target_url"].is_null()), (200, true));
}

#[sqlx::test]
async fn another_workspaces_keywords_and_sites_are_404(pool: PgPool) {
    let w = World::new(pool).await;
    let id = w.add("widgets").await["id"].as_str().unwrap().to_string();
    let other = "other-token";
    let site = format!("/api/v1/sites/{}/keywords", w.site_id);
    let key = format!("/api/v1/keywords/{id}");
    let m = reqwest::Method::GET;
    assert_eq!(w.send(other, m, &site, None).await.0, 404);
    assert_eq!(
        w.send(
            other,
            reqwest::Method::POST,
            &site,
            Some(json!({"keyword": "x"}))
        )
        .await
        .0,
        404
    );
    assert_eq!(
        w.send(
            other,
            reqwest::Method::PATCH,
            &key,
            Some(json!({"target_url": null}))
        )
        .await
        .0,
        404
    );
    assert_eq!(
        w.send(other, reqwest::Method::DELETE, &key, None).await.0,
        404
    );
    assert_eq!(w.list().await.len(), 1);
}

#[sqlx::test]
async fn landing_pages_follow_the_four_rules(pool: PgPool) {
    let w = World::new(pool).await;
    for keyword in ["pricing plans", "zebra"] {
        w.add(keyword).await;
    }
    w.post(json!({"keyword": "contact", "target_url": "https://example.com/reach-us"}))
        .await;
    let none = w.list().await;
    assert!(none
        .iter()
        .all(|k| k["landing_url"].is_null() && k["landing_source"].is_null()));
    w.crawl(&[
        ("https://example.com/", 0, "Home"),
        ("https://example.com/pricing", 1, "Pricing and plans"),
        ("https://example.com/reach-us", 1, "Talk to us"),
    ])
    .await;
    let by_keyword = |rows: &[Value], name: &str| {
        let row = rows.iter().find(|k| k["keyword"] == name).unwrap();
        (row["landing_url"].clone(), row["landing_source"].clone())
    };
    let rows = w.list().await;
    assert_eq!(
        by_keyword(&rows, "pricing plans"),
        (json!("https://example.com/pricing"), json!("best_match"))
    );
    assert_eq!(
        by_keyword(&rows, "zebra"),
        (json!("https://example.com/"), json!("home"))
    );
    assert_eq!(
        by_keyword(&rows, "contact"),
        (json!("https://example.com/reach-us"), json!("set"))
    );
}

#[sqlx::test]
async fn deleting_a_site_deletes_its_keywords(pool: PgPool) {
    let w = World::new(pool.clone()).await;
    w.add("widgets").await;
    let (status, _) = w
        .send(
            "test-token",
            reqwest::Method::DELETE,
            &format!("/api/v1/sites/{}", w.site_id),
            None,
        )
        .await;
    assert_eq!(status, 200);
    let left: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM site_keywords")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(left, 0);
}

// ------------------------------------------- the checks follow the keywords

const HOME: &str = "https://example.com/";
const WIDGETS_PAGE: &str = "https://example.com/widgets";

/// The title check's `(status, open items)` as the checks list shows it.
async fn title_check(w: &World) -> (String, i64) {
    let (status, body) = w.get(&format!("/api/v1/sites/{}/checks", w.site_id)).await;
    assert_eq!(status, 200, "{body}");
    let entry = body["data"]["checks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["check_id"] == "keyword_not_in_title")
        .unwrap()
        .clone();
    (
        entry["status"].as_str().unwrap().to_string(),
        entry["open"].as_i64().unwrap(),
    )
}

#[sqlx::test]
async fn a_new_keyword_is_checked_at_once_not_shown_as_passed(pool: PgPool) {
    let w = World::new(pool).await;
    w.crawl(&[(HOME, 0, "Our shop")]).await;
    w.add("widgets").await;
    assert_eq!(title_check(&w).await, ("failed".to_string(), 1));
}

#[sqlx::test]
async fn changing_a_keywords_page_runs_the_checks_again(pool: PgPool) {
    let w = World::new(pool).await;
    w.crawl(&[(HOME, 0, "Our shop"), (WIDGETS_PAGE, 1, "Widgets for sale")])
        .await;
    let (status, body) = w
        .post(json!({ "keyword": "widgets", "target_url": HOME }))
        .await;
    assert_eq!(status, 201, "{body}");
    assert_eq!(title_check(&w).await.0, "failed");
    let id = body["data"]["id"].as_str().unwrap();
    let (status, _) = w
        .send(
            "test-token",
            reqwest::Method::PATCH,
            &format!("/api/v1/keywords/{id}"),
            Some(json!({ "target_url": WIDGETS_PAGE })),
        )
        .await;
    assert_eq!(status, 200);
    assert_eq!(title_check(&w).await.0, "passed");
}

#[sqlx::test]
async fn deleting_a_keyword_runs_the_checks_again(pool: PgPool) {
    let w = World::new(pool).await;
    w.crawl(&[(HOME, 0, "Our shop")]).await;
    let widgets = w.add("widgets").await["id"].as_str().unwrap().to_string();
    w.add("gadgets").await;
    assert_eq!(title_check(&w).await, ("failed".to_string(), 2));
    let (status, _) = w
        .send(
            "test-token",
            reqwest::Method::DELETE,
            &format!("/api/v1/keywords/{widgets}"),
            None,
        )
        .await;
    assert_eq!(status, 200);
    assert_eq!(title_check(&w).await, ("failed".to_string(), 1));
}
