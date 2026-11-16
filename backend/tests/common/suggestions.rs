//! Shared setup for the suggestion and reconcile tests.

use margincite::checks::store::replace_findings;
use margincite::checks::Finding;
use margincite::crawl::repo::{self, Finish, NewPage};
use margincite::suggestions::reconcile;
use margincite::suggestions::repo as sugg;
use margincite::suggestions::repo::ListFilter;
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

pub const A: &str = "https://example.com/a";
pub const B: &str = "https://example.com/b";
pub const C: &str = "https://example.com/c";

pub struct Site {
    pub workspace_id: Uuid,
    pub site_id: Uuid,
}

pub async fn site(pool: &PgPool) -> Site {
    let workspace_id = Uuid::new_v4();
    let site_id = Uuid::new_v4();
    sqlx::query("INSERT INTO workspaces (id, name) VALUES ($1, 'test')")
        .bind(workspace_id)
        .execute(pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO sites (id, workspace_id, root_url, host, path_prefix, display_name)
         VALUES ($1, $2, 'https://example.com/', 'example.com', '/', 'example.com')",
    )
    .bind(site_id)
    .bind(workspace_id)
    .execute(pool)
    .await
    .unwrap();
    Site {
        workspace_id,
        site_id,
    }
}

pub fn page(url: &str) -> NewPage {
    NewPage {
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
    }
}

pub fn on_page(check: &'static str, target: &str) -> Finding {
    Finding::page(check, target, json!({ "seen": target }))
}

/// One finished crawl that crawled `pages` and found `findings`, then reconciled.
pub async fn crawl(pool: &PgPool, s: &Site, pages: &[&str], findings: &[Finding]) -> repo::Crawl {
    let rows: Vec<NewPage> = pages.iter().map(|url| page(url)).collect();
    crawl_rows(pool, s, &rows, findings).await
}

/// A page the crawl tried and could not read: no status, a fetch error.
pub fn timed_out(url: &str) -> NewPage {
    NewPage {
        status_code: None,
        fetch_error: Some("timed out".into()),
        facts: None,
        ..page(url)
    }
}

pub async fn crawl_rows(
    pool: &PgPool,
    s: &Site,
    pages: &[NewPage],
    findings: &[Finding],
) -> repo::Crawl {
    let crawl_id = repo::create_crawl(pool, s.workspace_id, s.site_id, "manual")
        .await
        .unwrap();
    for row in pages {
        repo::upsert_page(pool, s.workspace_id, crawl_id, row)
            .await
            .unwrap();
    }
    let done = Finish {
        status: "completed".into(),
        stop_reason: "finished".into(),
        pages_crawled: pages.len() as i32,
        pages_failed: 0,
        error: None,
    };
    repo::finish(pool, s.workspace_id, crawl_id, &done)
        .await
        .unwrap();
    replace_findings(pool, s.workspace_id, crawl_id, findings)
        .await
        .unwrap();
    reconcile::apply(pool, s.workspace_id, s.site_id, crawl_id)
        .await
        .unwrap();
    repo::get_crawl(pool, s.workspace_id, crawl_id)
        .await
        .unwrap()
}

pub async fn status_of(pool: &PgPool, s: &Site, check: &str, target: &str) -> Option<String> {
    sqlx::query_scalar(
        "SELECT i.status FROM suggestion_items i JOIN suggestions g ON g.id = i.suggestion_id
         WHERE g.site_id = $1 AND g.check_id = $2 AND i.target = $3",
    )
    .bind(s.site_id)
    .bind(check)
    .bind(target)
    .fetch_optional(pool)
    .await
    .unwrap()
}

pub async fn connect_with_rows(pool: &PgPool, s: &Site, rows: &[(&str, i64)]) {
    sqlx::query(
        "INSERT INTO google_connections (site_id, workspace_id, refresh_token_sealed, property, status)
         VALUES ($1, $2, 'sealed', 'sc-domain:example.com', 'connected')",
    )
    .bind(s.site_id)
    .bind(s.workspace_id)
    .execute(pool)
    .await
    .unwrap();
    for (page, impressions) in rows {
        sqlx::query(
            "INSERT INTO search_rows (workspace_id, site_id, query, page, clicks, impressions, ctr, position)
             VALUES ($1, $2, 'q', $3, 0, $4, 0, 5)",
        )
        .bind(s.workspace_id)
        .bind(s.site_id)
        .bind(page)
        .bind(impressions)
        .execute(pool)
        .await
        .unwrap();
    }
}

pub async fn reach_of(pool: &PgPool, s: &Site, check: &str) -> f64 {
    let (rows, _) = sugg::list(
        pool,
        s.workspace_id,
        s.site_id,
        ListFilter::default(),
        1,
        100,
    )
    .await
    .unwrap();
    rows.iter().find(|r| r.check_id == check).unwrap().reach
}
