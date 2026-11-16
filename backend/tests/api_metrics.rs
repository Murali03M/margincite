mod common;

use common::google::GoogleWorld;
use serde_json::{json, Value};
use sqlx::PgPool;
use time::{Duration, OffsetDateTime};
use uuid::Uuid;

const PROPERTY: &str = "sc-domain:example.com";
const FAILED: &str = "Could not load this number right now.";

async fn metrics(w: &GoogleWorld) -> Value {
    let (status, body) = w.get(&format!("/api/v1/sites/{}/metrics", w.site_id)).await;
    assert_eq!(status, 200, "{body}");
    body["data"].clone()
}

fn day(days_ago: i64) -> time::Date {
    (OffsetDateTime::now_utc() - Duration::days(days_ago)).date()
}

async fn seed_daily(w: &GoogleWorld, days: &[(i64, i64, i64)]) {
    for (days_ago, clicks, impressions) in days {
        sqlx::query(
            "INSERT INTO search_daily (workspace_id, site_id, date, clicks, impressions) VALUES ($1, $2, $3, $4, $5)",
        )
        .bind(w.workspace_id)
        .bind(w.site_id)
        .bind(day(*days_ago))
        .bind(clicks)
        .bind(impressions)
        .execute(&w.pool)
        .await
        .unwrap();
    }
}

async fn seed_previous(w: &GoogleWorld, clicks: i64, impressions: i64, keywords: i64) {
    sqlx::query(
        "INSERT INTO search_previous (site_id, workspace_id, clicks, impressions, keywords) VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(w.site_id)
    .bind(w.workspace_id)
    .bind(clicks)
    .bind(impressions)
    .bind(keywords)
    .execute(&w.pool)
    .await
    .unwrap();
}

async fn mark_synced(w: &GoogleWorld) {
    sqlx::query("UPDATE google_connections SET synced_at = now() WHERE site_id = $1")
        .bind(w.site_id)
        .execute(&w.pool)
        .await
        .unwrap();
}

#[sqlx::test]
async fn a_new_site_returns_every_block_with_its_reason(pool: PgPool) {
    let w = GoogleWorld::new(pool).await;
    let data = metrics(&w).await;
    assert_eq!(
        data["site_health"],
        json!({"score": null, "previous": null, "history": [], "reason": "Crawl the site to see its health."})
    );
    assert_eq!(
        data["ai_visibility"],
        json!({"percent": null, "mentions": null, "citations": null, "answers": 0,
               "last_run_at": null, "engines": [], "reason": "Run an AI visibility check to see this."})
    );
    assert_eq!(
        data["visibility"],
        json!({"percent": null, "in_top_10": null, "keywords": 0, "source": null,
               "reason": "Add target keywords to see visibility."})
    );
    assert_eq!(
        data["organic"],
        json!({"clicks": null, "impressions": null, "keywords": null, "avg_position": null,
               "clicks_previous": null, "keywords_previous": null, "daily": [],
               "source": "search_console",
               "reason": "Connect Google Search Console to see organic traffic."})
    );
    assert_eq!(
        data["backlinks"],
        json!({"total": null, "linked_pages": null, "top_pages": [], "source": "bing",
               "synced_at": null, "reason": "Connect Bing Webmaster Tools to see backlinks."})
    );
}

#[sqlx::test]
async fn site_health_reports_the_latest_score_the_one_before_and_twelve_points(pool: PgPool) {
    let w = GoogleWorld::new(pool).await;
    for n in 0..14 {
        sqlx::query(
            "INSERT INTO crawls (id, workspace_id, site_id, status, trigger, score, finished_at, created_at)
             VALUES ($1, $2, $3, 'completed', 'manual', $4, now() - ($5 * interval '1 day'), now() - ($5 * interval '1 day'))",
        )
        .bind(Uuid::new_v4())
        .bind(w.workspace_id)
        .bind(w.site_id)
        .bind(50 + n)
        .bind(20 - n)
        .execute(&w.pool)
        .await
        .unwrap();
    }
    // A failed crawl and a running one are never part of the history.
    for status in ["failed", "running"] {
        sqlx::query(
            "INSERT INTO crawls (id, workspace_id, site_id, status, trigger, score) VALUES ($1, $2, $3, $4, 'manual', 1)",
        )
        .bind(Uuid::new_v4())
        .bind(w.workspace_id)
        .bind(w.site_id)
        .bind(status)
        .execute(&w.pool)
        .await
        .unwrap();
    }
    let health = metrics(&w).await["site_health"].clone();
    assert_eq!(health["score"], 63);
    assert_eq!(health["previous"], 62);
    assert!(health["reason"].is_null());
    let history = health["history"].as_array().unwrap();
    assert_eq!(history.len(), 12);
    let scores: Vec<i64> = history
        .iter()
        .map(|p| p["score"].as_i64().unwrap())
        .collect();
    assert_eq!(scores, (52..=63).collect::<Vec<i64>>(), "oldest first");
    for point in history {
        assert!(time::OffsetDateTime::parse(
            point["at"].as_str().unwrap(),
            &time::format_description::well_known::Rfc3339
        )
        .is_ok());
    }
}

#[sqlx::test]
async fn site_health_with_one_crawl_has_no_previous_and_one_without_pages_says_why(pool: PgPool) {
    let w = GoogleWorld::new(pool).await;
    w.crawl(&[("https://example.com/", true)]).await;
    let health = metrics(&w).await["site_health"].clone();
    assert!(health["score"].is_i64());
    assert!(health["previous"].is_null());
    assert_eq!(health["history"].as_array().unwrap().len(), 1);
    sqlx::query("UPDATE crawls SET score = NULL WHERE site_id = $1")
        .bind(w.site_id)
        .execute(&w.pool)
        .await
        .unwrap();
    let health = metrics(&w).await["site_health"].clone();
    assert!(health["score"].is_null());
    assert_eq!(
        health["reason"],
        "This crawl found no working pages to score."
    );
    assert_eq!(health["history"], json!([]));
}

#[sqlx::test]
async fn organic_numbers_come_from_search_console(pool: PgPool) {
    let w = GoogleWorld::new(pool).await;
    w.connect(PROPERTY).await;
    mark_synced(&w).await;
    w.seed_rows(&[
        ("blue widgets", "https://example.com/a", 10, 100, 4.0),
        ("blue widgets", "https://example.com/b", 5, 300, 8.0),
        ("red widgets", "https://example.com/a", 1, 100, 2.0),
    ])
    .await;
    // Three days inside the last 28 days of data, one far older.
    seed_daily(
        &w,
        &[(3, 10, 200), (4, 6, 300), (10, 0, 100), (60, 99, 999)],
    )
    .await;
    seed_previous(&w, 12, 640, 7).await;
    let organic = metrics(&w).await["organic"].clone();
    assert_eq!(organic["clicks"], 16);
    assert_eq!(organic["impressions"], 600);
    assert_eq!(organic["keywords"], 2);
    // (4*100 + 8*300 + 2*100) / 500
    assert!((organic["avg_position"].as_f64().unwrap() - 6.0).abs() < 1e-9);
    assert_eq!(organic["clicks_previous"], 12);
    assert_eq!(organic["keywords_previous"], 7);
    assert_eq!(organic["source"], "search_console");
    assert!(organic["reason"].is_null());
    let daily = organic["daily"].as_array().unwrap();
    assert_eq!(daily.len(), 4);
    assert_eq!(daily[0]["clicks"], 99, "oldest first");
    assert!(daily[0]["date"].as_str().unwrap().len() == 10);
    assert_eq!(daily[3]["impressions"], 200);
}

#[sqlx::test]
async fn organic_says_why_it_is_empty_at_each_stage_of_connecting(pool: PgPool) {
    let w = GoogleWorld::new(pool).await;
    let reason = |data: &Value| data["organic"]["reason"].as_str().map(str::to_string);
    w.insert_connection(None, "choose_property").await;
    assert_eq!(
        reason(&metrics(&w).await).as_deref(),
        Some("Choose a Search Console property to see organic traffic.")
    );
    w.insert_connection(Some(PROPERTY), "connected").await;
    assert_eq!(
        reason(&metrics(&w).await).as_deref(),
        Some("Google Search Console is syncing. Check back in a few minutes.")
    );
    w.insert_connection(Some(PROPERTY), "needs_reconnecting")
        .await;
    let data = metrics(&w).await;
    assert_eq!(
        reason(&data).as_deref(),
        Some("Reconnect Google Search Console to see organic traffic.")
    );
    assert!(data["organic"]["clicks"].is_null());
}

#[sqlx::test]
async fn a_synced_site_with_no_searches_shows_zeros(pool: PgPool) {
    let w = GoogleWorld::new(pool).await;
    w.connect(PROPERTY).await;
    mark_synced(&w).await;
    let organic = metrics(&w).await["organic"].clone();
    assert_eq!(
        (organic["clicks"].as_i64(), organic["impressions"].as_i64()),
        (Some(0), Some(0))
    );
    assert_eq!(organic["keywords"], 0);
    assert!(organic["avg_position"].is_null());
    assert!(organic["reason"].is_null());
}

#[sqlx::test]
async fn visibility_is_the_share_of_keywords_in_the_top_ten(pool: PgPool) {
    let w = GoogleWorld::new(pool).await;
    for keyword in ["blue widgets", "red widgets", "green widgets", "no data"] {
        w.add_keyword(keyword).await;
    }
    w.connect(PROPERTY).await;
    mark_synced(&w).await;
    w.seed_rows(&[
        ("blue widgets", "https://example.com/a", 1, 100, 4.0),
        ("Red Widgets", "https://example.com/a", 1, 100, 10.0),
        // 5 at 100 impressions and 30 at 100 averages 17.5: outside the top ten.
        ("green widgets", "https://example.com/a", 1, 100, 5.0),
        ("green widgets", "https://example.com/b", 1, 100, 30.0),
    ])
    .await;
    let visibility = metrics(&w).await["visibility"].clone();
    assert_eq!(visibility["keywords"], 4);
    assert_eq!(visibility["in_top_10"], 2);
    assert_eq!(visibility["percent"], 50.0);
    assert!(visibility["reason"].is_null());
}

#[sqlx::test]
async fn visibility_needs_keywords_and_search_console(pool: PgPool) {
    let w = GoogleWorld::new(pool).await;
    w.add_keyword("blue widgets").await;
    let visibility = metrics(&w).await["visibility"].clone();
    assert_eq!(visibility["keywords"], 1);
    assert!(visibility["percent"].is_null() && visibility["in_top_10"].is_null());
    assert_eq!(
        visibility["reason"],
        "Connect Google Search Console to see visibility."
    );
    // Connected and synced, but Search Console has shown nothing for the site.
    w.connect(PROPERTY).await;
    mark_synced(&w).await;
    let visibility = metrics(&w).await["visibility"].clone();
    assert!(visibility["percent"].is_null());
    assert_eq!(
        visibility["reason"],
        "Search Console has not reported any searches for this site yet."
    );
}

#[sqlx::test]
async fn one_block_failing_does_not_fail_the_others(pool: PgPool) {
    let w = GoogleWorld::new(pool).await;
    w.crawl(&[("https://example.com/", true)]).await;
    w.connect(PROPERTY).await;
    mark_synced(&w).await;
    sqlx::query("DROP TABLE search_daily")
        .execute(&w.pool)
        .await
        .unwrap();
    let data = metrics(&w).await;
    assert_eq!(data["organic"]["reason"], FAILED);
    assert!(data["organic"]["clicks"].is_null());
    assert_eq!(data["organic"]["daily"], json!([]));
    assert!(data["site_health"]["score"].is_i64());
    assert!(data["site_health"]["reason"].is_null());
    assert!(data["ai_visibility"]["reason"].is_string());
    assert!(data["backlinks"]["reason"].is_string());
}

#[sqlx::test]
async fn another_workspaces_site_is_404(pool: PgPool) {
    let w = GoogleWorld::new(pool).await;
    let (status, _) = w
        .send(
            "other-token",
            reqwest::Method::GET,
            &format!("/api/v1/sites/{}/metrics", w.site_id),
            None,
        )
        .await;
    assert_eq!(status, 404);
    let (status, _) = w
        .get(&format!("/api/v1/sites/{}/metrics", Uuid::new_v4()))
        .await;
    assert_eq!(status, 404);
}

/// Everything connected: a crawl, Search Console with keywords, Bing and an AI run.
async fn fully_connected(pool: PgPool) -> GoogleWorld {
    use common::ai::Q1;
    let w = GoogleWorld::with_ai(pool).await;
    w.crawl(&[("https://example.com/", true)]).await;
    w.add_keyword("blue widgets").await;
    w.connect(PROPERTY).await;
    mark_synced(&w).await;
    w.seed_rows(&[("blue widgets", "https://example.com/a", 10, 100, 4.0)])
        .await;
    seed_daily(&w, &[(3, 10, 200)]).await;
    seed_previous(&w, 4, 100, 1).await;
    w.insert_bing("connected").await;
    sqlx::query("UPDATE bing_connections SET synced_at = now()")
        .execute(&w.pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO backlink_targets (workspace_id, site_id, target_url, count) VALUES ($1, $2, 'https://example.com/', 7)",
    )
    .bind(w.workspace_id)
    .bind(w.site_id)
    .execute(&w.pool)
    .await
    .unwrap();
    w.put_ai_settings("Acme", &[], &[]).await;
    w.add_prompt(Q1).await;
    w.mock_engines("Acme is the answer.", &[], None).await;
    w.run_to_end().await;
    w
}

#[sqlx::test]
async fn a_fully_connected_site_returns_every_number(pool: PgPool) {
    let w = fully_connected(pool).await;
    let data = metrics(&w).await;
    assert!(data["site_health"]["score"].is_i64());
    assert_eq!(data["ai_visibility"]["percent"], 100.0);
    assert_eq!(data["ai_visibility"]["answers"], 5);
    assert_eq!(data["visibility"]["percent"], 100.0);
    assert_eq!(data["organic"]["clicks"], 10);
    assert_eq!(data["backlinks"]["total"], 7);
    for block in [
        "site_health",
        "ai_visibility",
        "visibility",
        "organic",
        "backlinks",
    ] {
        assert!(data[block]["reason"].is_null(), "{block}: {}", data[block]);
    }
}

#[sqlx::test]
async fn the_ai_block_failing_does_not_fail_the_others(pool: PgPool) {
    let w = fully_connected(pool).await;
    sqlx::query("DROP TABLE ai_answers CASCADE")
        .execute(&w.pool)
        .await
        .unwrap();
    let data = metrics(&w).await;
    assert_eq!(data["ai_visibility"]["reason"], FAILED);
    assert!(data["ai_visibility"]["percent"].is_null());
    assert_eq!(data["ai_visibility"]["engines"], json!([]));
    assert_eq!(data["organic"]["clicks"], 10);
    assert_eq!(data["backlinks"]["total"], 7);
    assert!(data["site_health"]["score"].is_i64());
}
