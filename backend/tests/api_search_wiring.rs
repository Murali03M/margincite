//! What Search Console data changes in the keyword, checks and summary endpoints.
mod common;

use common::google::GoogleWorld;
use serde_json::{json, Value};
use sqlx::PgPool;

const A: &str = "https://example.com/a";
const B: &str = "https://example.com/b";
const PROPERTY: &str = "sc-domain:example.com";

async fn keywords(w: &GoogleWorld) -> Vec<Value> {
    let (status, body) = w
        .get(&format!("/api/v1/sites/{}/keywords", w.site_id))
        .await;
    assert_eq!(status, 200, "{body}");
    body["data"].as_array().unwrap().clone()
}

#[sqlx::test]
async fn keywords_show_search_console_numbers_without_any_crawl(pool: PgPool) {
    let w = GoogleWorld::new(pool).await;
    w.add_keyword("blue widgets").await;
    w.add_keyword("Red Widgets!").await;
    w.add_keyword("never shown").await;
    w.connect(PROPERTY).await;
    w.seed_rows(&[
        ("blue widgets", A, 10, 100, 4.0),
        ("Blue  Widgets", B, 5, 300, 8.0),
        ("red widgets", A, 1, 50, 2.0),
        ("other", A, 1, 50, 2.0),
    ])
    .await;
    let found = keywords(&w).await;
    let by_name = |name: &str| found.iter().find(|k| k["keyword"] == name).unwrap().clone();
    let blue = by_name("blue widgets");
    assert_eq!(blue["clicks"], 15);
    assert_eq!(blue["impressions"], 400);
    assert!((blue["position"].as_f64().unwrap() - 7.0).abs() < 1e-9);
    let red = by_name("Red Widgets!");
    assert_eq!(
        (red["clicks"].as_i64(), red["impressions"].as_i64()),
        (Some(1), Some(50))
    );
    // Connected, but nothing shown for it: zero, with no position.
    let never = by_name("never shown");
    assert_eq!(never["clicks"], 0);
    assert_eq!(never["impressions"], 0);
    assert!(never["position"].is_null());
}

#[sqlx::test]
async fn keywords_have_null_numbers_when_search_console_is_not_connected(pool: PgPool) {
    let w = GoogleWorld::new(pool).await;
    w.add_keyword("blue widgets").await;
    for state in [None, Some("choose_property"), Some("needs_reconnecting")] {
        if let Some(status) = state {
            w.insert_connection(None, status).await;
        }
        let found = keywords(&w).await;
        for field in ["position", "clicks", "impressions"] {
            assert!(found[0][field].is_null(), "{state:?} {field}");
        }
    }
}

#[sqlx::test]
async fn a_keyword_lands_on_the_page_search_console_shows_most(pool: PgPool) {
    let w = GoogleWorld::new(pool).await;
    w.crawl(&[("https://example.com/", true), (A, true), (B, true)])
        .await;
    w.add_keyword("blue widgets").await;
    w.connect(PROPERTY).await;
    w.seed_rows(&[
        ("blue widgets", A, 1, 10, 4.0),
        ("blue widgets", B, 1, 90, 4.0),
    ])
    .await;
    let found = keywords(&w).await;
    assert_eq!(found[0]["landing_url"], B);
    assert_eq!(found[0]["landing_source"], "search_console");
}

#[sqlx::test]
async fn search_checks_are_not_run_until_search_console_has_data(pool: PgPool) {
    let w = GoogleWorld::new(pool).await;
    w.crawl(&[("https://example.com/", true), (A, true)]).await;
    let path = format!("/api/v1/sites/{}/checks", w.site_id);
    let search_checks = |body: &Value| -> Vec<Value> {
        body["data"]["checks"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|c| c["check_id"].as_str().unwrap().starts_with("search_"))
            .cloned()
            .collect()
    };
    let (_, body) = w.get(&path).await;
    let checks = search_checks(&body);
    assert_eq!(checks.len(), 3);
    for check in &checks {
        assert_eq!(check["status"], "not_run");
        assert_eq!(
            check["reason"],
            "Connect Google Search Console to run this check."
        );
    }
    // Connected but no rows yet: still not run.
    w.connect(PROPERTY).await;
    let (_, body) = w.get(&path).await;
    assert!(search_checks(&body)
        .iter()
        .all(|c| c["status"] == "not_run"));
    // Rows arrive and the checks run (a sync does this).
    w.seed_rows(&[("widgets", A, 0, 1000, 1.0)]).await;
    let crawl = margincite::crawl::repo::latest_completed(&w.pool, w.workspace_id, w.site_id)
        .await
        .unwrap()
        .unwrap();
    margincite::checks::store::run_for_crawl(&w.pool, w.workspace_id, crawl.id)
        .await
        .unwrap();
    let (_, body) = w.get(&path).await;
    let status_of = |id: &str| {
        let checks = search_checks(&body);
        checks.iter().find(|c| c["check_id"] == id).unwrap()["status"].clone()
    };
    assert_eq!(status_of("search_low_ctr"), json!("failed"));
    assert_eq!(status_of("search_striking_distance"), json!("passed"));
    assert_eq!(status_of("search_no_impressions"), json!("failed"));
}

#[sqlx::test]
async fn the_summary_says_what_reach_is_counted_in(pool: PgPool) {
    let w = GoogleWorld::new(pool).await;
    w.crawl(&[("https://example.com/", true)]).await;
    let path = format!("/api/v1/sites/{}/summary", w.site_id);
    let (_, body) = w.get(&path).await;
    assert_eq!(body["data"]["info"]["reach_basis"], "pages");
    w.connect(PROPERTY).await;
    w.seed_rows(&[("widgets", "https://example.com/", 1, 10, 3.0)])
        .await;
    let (_, body) = w.get(&path).await;
    assert_eq!(body["data"]["info"]["reach_basis"], "impressions");
}
