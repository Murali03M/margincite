mod common;

use common::google::GoogleWorld;
use common::speed::{lab, PSI_KEY};
use margincite::jobs::worker::JobHandler;
use serde_json::{json, Value};
use sqlx::PgPool;
use uuid::Uuid;

const HOME: &str = "https://example.com/";

fn page(n: u32) -> String {
    format!("https://example.com/p{n}")
}

async fn run(w: &GoogleWorld) {
    w.speed_handler().run(&w.speed_job()).await.unwrap();
}

async fn rows(w: &GoogleWorld) -> Vec<(String, Option<i32>, Option<String>)> {
    sqlx::query_as(
        "SELECT url, performance, error FROM speed_results WHERE site_id = $1 ORDER BY url",
    )
    .bind(w.site_id)
    .fetch_all(&w.pool)
    .await
    .unwrap()
}

async fn set_depth(w: &GoogleWorld, url: &str, depth: i32) {
    sqlx::query("UPDATE pages SET depth = $2 WHERE url = $1 AND workspace_id = $3")
        .bind(url)
        .bind(depth)
        .bind(w.workspace_id)
        .execute(&w.pool)
        .await
        .unwrap();
}

async fn many_pages(w: &GoogleWorld, count: u32) {
    let urls: Vec<String> = std::iter::once(HOME.to_string())
        .chain((1..=count).map(page))
        .collect();
    let pairs: Vec<(&str, bool)> = urls.iter().map(|u| (u.as_str(), true)).collect();
    w.crawl(&pairs).await;
}

#[sqlx::test]
async fn the_home_page_and_the_four_shallowest_pages_are_tested(pool: PgPool) {
    let w = GoogleWorld::new(pool).await;
    many_pages(&w, 6).await;
    for (n, depth) in [(1, 3), (2, 1), (3, 2), (4, 1), (5, 4), (6, 1)] {
        set_depth(&w, &page(n), depth).await;
    }
    w.mock_speed_for_all(lab(0.8, 1800.0, 0.02, 120.0)).await;
    run(&w).await;
    let asked: Vec<String> = w.speed_requests().await.into_iter().map(|r| r.0).collect();
    assert_eq!(asked, vec![HOME, &page(2), &page(4), &page(6), &page(3)]);
    assert_eq!(rows(&w).await.len(), 5);
}

/// Pages listed in the sitemap are stored at depth 0, like the home page.
#[sqlx::test]
async fn pages_found_through_the_sitemap_are_tested_too(pool: PgPool) {
    let w = GoogleWorld::new(pool).await;
    many_pages(&w, 3).await;
    for n in 1..=3 {
        set_depth(&w, &page(n), 0).await;
    }
    w.mock_speed_for_all(lab(0.8, 1800.0, 0.02, 120.0)).await;
    run(&w).await;
    let asked: Vec<String> = w.speed_requests().await.into_iter().map(|r| r.0).collect();
    assert_eq!(asked, vec![HOME, &page(1), &page(2), &page(3)]);
}

#[sqlx::test]
async fn pages_with_the_most_impressions_come_first_when_search_console_has_data(pool: PgPool) {
    let w = GoogleWorld::new(pool).await;
    many_pages(&w, 6).await;
    w.connect("sc-domain:example.com").await;
    w.seed_rows(&[
        ("q1", &page(5), 10, 900, 3.0),
        ("q2", &page(5), 1, 100, 4.0),
        ("q3", &page(3), 1, 500, 5.0),
        ("q4", &page(6), 1, 40, 5.0),
        ("q5", &page(2), 1, 30, 5.0),
        ("q6", &page(1), 1, 20, 5.0),
    ])
    .await;
    w.mock_speed_for_all(lab(0.8, 1800.0, 0.02, 120.0)).await;
    run(&w).await;
    let asked: Vec<String> = w.speed_requests().await.into_iter().map(|r| r.0).collect();
    assert_eq!(asked, vec![HOME, &page(5), &page(3), &page(6), &page(2)]);
}

#[sqlx::test]
async fn a_site_never_crawled_tests_only_its_home_page(pool: PgPool) {
    let w = GoogleWorld::new(pool).await;
    w.mock_speed_for_all(lab(0.5, 3000.0, 0.2, 400.0)).await;
    run(&w).await;
    let asked: Vec<String> = w.speed_requests().await.into_iter().map(|r| r.0).collect();
    assert_eq!(asked, vec![HOME]);
}

#[sqlx::test]
async fn lab_and_field_results_are_stored_with_the_field_flag(pool: PgPool) {
    let w = GoogleWorld::new(pool).await;
    many_pages(&w, 1).await;
    w.mock_speed(&page(1), 200, {
        let mut body = lab(0.9, 1500.0, 0.01, 100.0);
        body["loadingExperience"] = json!({ "metrics": {
            "LARGEST_CONTENTFUL_PAINT_MS": { "percentile": 2100 },
            "CUMULATIVE_LAYOUT_SHIFT_SCORE": { "percentile": 7 },
            "INTERACTION_TO_NEXT_PAINT": { "percentile": 190 }
        }});
        body
    })
    .await;
    w.mock_speed(HOME, 200, lab(0.45, 4200.0, 0.3, 700.0)).await;
    run(&w).await;
    let (_, body) = w.get(&w.speed_path("")).await;
    let pages = body["data"]["pages"].as_array().unwrap();
    assert_eq!(pages[0]["url"], HOME);
    assert_eq!(
        pages[0],
        json!({ "url": HOME, "strategy": "mobile", "performance": 45, "lcp_ms": 4200,
                "cls": 0.3, "inp_ms": null, "tbt_ms": 700, "field_data": false, "error": null })
    );
    assert_eq!(
        pages[1],
        json!({ "url": page(1), "strategy": "mobile", "performance": 90, "lcp_ms": 2100,
                "cls": 0.07, "inp_ms": 190, "tbt_ms": 100, "field_data": true, "error": null })
    );
    assert!(body["data"]["checked_at"].is_string());
}

#[sqlx::test]
async fn a_429_or_500_stores_an_error_for_that_page_and_the_run_goes_on(pool: PgPool) {
    let w = GoogleWorld::new(pool).await;
    many_pages(&w, 3).await;
    w.mock_speed(HOME, 429, json!({})).await;
    w.mock_speed(&page(1), 500, json!({ "error": { "message": PSI_KEY } }))
        .await;
    w.mock_speed(&page(2), 200, lab(0.7, 2000.0, 0.05, 200.0))
        .await;
    w.mock_speed(&page(3), 200, json!({ "surprise": true }))
        .await;
    run(&w).await;
    let stored = rows(&w).await;
    assert_eq!(stored.len(), 4);
    let error_of = |url: &str| stored.iter().find(|r| r.0 == url).unwrap().2.clone();
    // No key is set here, so the limit is the shared no-key allowance.
    assert!(error_of(HOME).unwrap().contains("without an API key"));
    assert_eq!(
        error_of(&page(1)).unwrap(),
        "PageSpeed could not test this page."
    );
    assert_eq!(error_of(&page(2)), None);
    assert!(error_of(&page(3)).unwrap().contains("format"));
    let everything = serde_json::to_string(&stored).unwrap();
    assert!(!everything.contains(PSI_KEY));
}

#[sqlx::test]
async fn a_failed_test_keeps_the_last_good_numbers_and_never_shows_a_zero(pool: PgPool) {
    let w = GoogleWorld::new(pool).await;
    w.mock_speed(HOME, 200, lab(0.66, 2200.0, 0.04, 150.0))
        .await;
    run(&w).await;
    let before: Value = w.get(&w.speed_path("")).await.1;
    w.mock.reset().await;
    w.mock_speed(HOME, 503, json!({})).await;
    run(&w).await;
    let after: Value = w.get(&w.speed_path("")).await.1;
    let (old, new) = (&before["data"]["pages"][0], &after["data"]["pages"][0]);
    assert_eq!(new["performance"], 66);
    assert_eq!(new["lcp_ms"], 2200);
    assert_eq!(new["error"], "PageSpeed could not test this page.");
    assert!(old["error"].is_null());
    // The date moves to the newest attempt, even though it failed.
    let at = |body: &Value| {
        time::OffsetDateTime::parse(
            body["data"]["checked_at"].as_str().unwrap(),
            &time::format_description::well_known::Rfc3339,
        )
        .unwrap()
    };
    assert!(at(&after) > at(&before));
}

#[sqlx::test]
async fn a_run_where_every_page_fails_still_moves_the_date(pool: PgPool) {
    let w = GoogleWorld::new(pool).await;
    sqlx::query(
        "INSERT INTO speed_results (workspace_id, site_id, url, strategy, performance, error)
         VALUES ($1, $2, $3, 'mobile', 70, NULL)",
    )
    .bind(w.workspace_id)
    .bind(w.site_id)
    .bind(HOME)
    .execute(&w.pool)
    .await
    .unwrap();
    sqlx::query("UPDATE speed_results SET checked_at = now() - interval '3 days'")
        .execute(&w.pool)
        .await
        .unwrap();
    w.mock_speed(HOME, 500, json!({})).await;
    run(&w).await;
    let (_, body) = w.get(&w.speed_path("")).await;
    let checked: String = body["data"]["checked_at"].as_str().unwrap().to_string();
    let newest: bool = sqlx::query_scalar(
        "SELECT bool_and(attempted_at > now() - interval '1 minute') FROM speed_results",
    )
    .fetch_one(&w.pool)
    .await
    .unwrap();
    assert!(newest, "{checked}");
    assert!(checked.starts_with(&time::OffsetDateTime::now_utc().date().to_string()));
    assert_eq!(body["data"]["pages"][0]["performance"], 70);
}

#[sqlx::test]
async fn pages_that_drop_out_of_the_tested_set_are_removed_after_the_run(pool: PgPool) {
    let w = GoogleWorld::new(pool).await;
    many_pages(&w, 2).await;
    for url in ["https://example.com/gone", "https://example.com/also-gone"] {
        sqlx::query(
            "INSERT INTO speed_results (workspace_id, site_id, url, strategy, performance)
             VALUES ($1, $2, $3, 'mobile', 20)",
        )
        .bind(w.workspace_id)
        .bind(w.site_id)
        .bind(url)
        .execute(&w.pool)
        .await
        .unwrap();
    }
    w.mock_speed_for_all(lab(0.8, 1800.0, 0.02, 120.0)).await;
    run(&w).await;
    let urls: Vec<String> = rows(&w).await.into_iter().map(|r| r.0).collect();
    assert_eq!(urls, vec![HOME.to_string(), page(1), page(2)]);
}

#[sqlx::test]
async fn another_sites_results_are_left_alone_by_the_cleanup(pool: PgPool) {
    let w = GoogleWorld::new(pool.clone()).await;
    let other = common::google::add_site(&pool, w.workspace_id, "https://other.example/").await;
    sqlx::query(
        "INSERT INTO speed_results (workspace_id, site_id, url, strategy, performance)
         VALUES ($1, $2, 'https://other.example/', 'mobile', 50)",
    )
    .bind(w.workspace_id)
    .bind(other)
    .execute(&pool)
    .await
    .unwrap();
    w.mock_speed_for_all(lab(0.8, 1800.0, 0.02, 120.0)).await;
    run(&w).await;
    let left: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM speed_results WHERE site_id = $1")
        .bind(other)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(left, 1);
}

#[sqlx::test]
async fn a_good_test_clears_the_error(pool: PgPool) {
    let w = GoogleWorld::new(pool).await;
    w.mock_speed(HOME, 500, json!({})).await;
    run(&w).await;
    assert!(rows(&w).await[0].2.is_some());
    w.mock.reset().await;
    w.mock_speed(HOME, 200, lab(0.9, 1000.0, 0.0, 10.0)).await;
    run(&w).await;
    assert_eq!(rows(&w).await, vec![(HOME.to_string(), Some(90), None)]);
}

#[sqlx::test]
async fn only_the_sites_own_pages_are_ever_sent(pool: PgPool) {
    let w = GoogleWorld::new(pool.clone()).await;
    many_pages(&w, 2).await;
    // A stray address from another host, another site's page, and a page of
    // the same host outside nothing: only example.com pages may go out.
    let crawl_id: Uuid = sqlx::query_scalar("SELECT id FROM crawls WHERE site_id = $1")
        .bind(w.site_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    for url in [
        "https://evil.example/steal",
        "http://169.254.169.254/latest",
        "https://example.com.evil.example/x",
    ] {
        sqlx::query(
            "INSERT INTO pages (id, workspace_id, crawl_id, url, depth, status_code, content_type, is_https)
             VALUES ($1, $2, $3, $4, 1, 200, 'text/html', true)",
        )
        .bind(Uuid::new_v4())
        .bind(w.workspace_id)
        .bind(crawl_id)
        .bind(url)
        .execute(&pool)
        .await
        .unwrap();
    }
    let other =
        common::google::add_site(&pool, w.workspace_id, "https://other-site.example/").await;
    let _ = other;
    w.mock_speed_for_all(lab(0.8, 1800.0, 0.02, 120.0)).await;
    run(&w).await;
    let asked: Vec<String> = w.speed_requests().await.into_iter().map(|r| r.0).collect();
    assert!(!asked.is_empty());
    assert!(
        asked
            .iter()
            .all(|url| url == HOME || url.starts_with("https://example.com/")),
        "{asked:?}"
    );
}

#[sqlx::test]
async fn a_site_with_a_path_prefix_only_sends_pages_under_it(pool: PgPool) {
    let w = GoogleWorld::new(pool.clone()).await;
    sqlx::query("UPDATE sites SET root_url = 'https://example.com/blog/', path_prefix = '/blog' WHERE id = $1")
        .bind(w.site_id)
        .execute(&pool)
        .await
        .unwrap();
    let crawl_id = w
        .crawl(&[
            ("https://example.com/blog/", true),
            ("https://example.com/blog/a", true),
            ("https://example.com/shop/b", true),
        ])
        .await;
    let _ = crawl_id;
    w.mock_speed_for_all(lab(0.8, 1800.0, 0.02, 120.0)).await;
    run(&w).await;
    let asked: Vec<String> = w.speed_requests().await.into_iter().map(|r| r.0).collect();
    assert!(asked.contains(&"https://example.com/blog/".to_string()));
    assert!(!asked.iter().any(|url| url.contains("/shop/")), "{asked:?}");
}

#[sqlx::test]
async fn the_key_is_sent_only_when_the_workspace_has_one_and_never_logged_or_stored(pool: PgPool) {
    let w = GoogleWorld::new(pool).await;
    w.mock_speed_for_all(lab(0.8, 1800.0, 0.02, 120.0)).await;
    run(&w).await;
    assert_eq!(w.speed_requests().await, vec![(HOME.to_string(), false)]);
    let (status, _) = w
        .send(
            "test-token",
            reqwest::Method::PUT,
            "/api/v1/settings/pagespeed",
            Some(json!({ "api_key": PSI_KEY })),
        )
        .await;
    assert_eq!(status, 200);
    run(&w).await;
    assert_eq!(
        w.speed_requests().await.last().unwrap(),
        &(HOME.to_string(), true)
    );
    let (_, body) = w.get(&w.speed_path("")).await;
    assert!(!body.to_string().contains(PSI_KEY));
}

#[sqlx::test]
async fn every_page_test_is_counted_in_usage(pool: PgPool) {
    let w = GoogleWorld::new(pool).await;
    many_pages(&w, 2).await;
    w.mock_speed(&page(1), 500, json!({})).await;
    w.mock_speed_for_all(lab(0.8, 1800.0, 0.02, 120.0)).await;
    run(&w).await;
    let (calls, cost): (i64, Option<f64>) = sqlx::query_as(
        "SELECT COALESCE(SUM(calls), 0), SUM(estimated_cost_usd) FROM usage_events
         WHERE workspace_id = $1 AND source = 'pagespeed'",
    )
    .bind(w.workspace_id)
    .fetch_one(&w.pool)
    .await
    .unwrap();
    assert_eq!(calls, 3);
    assert_eq!(cost, Some(0.0));
}

#[sqlx::test]
async fn a_deleted_site_ends_the_run_quietly(pool: PgPool) {
    let w = GoogleWorld::new(pool.clone()).await;
    sqlx::query("DELETE FROM sites WHERE id = $1")
        .bind(w.site_id)
        .execute(&pool)
        .await
        .unwrap();
    run(&w).await;
    assert!(w.speed_requests().await.is_empty());
}

#[sqlx::test]
async fn a_run_re_runs_the_checks_on_the_latest_crawl(pool: PgPool) {
    let w = GoogleWorld::new(pool.clone()).await;
    many_pages(&w, 1).await;
    w.mock_speed(&page(1), 200, lab(0.3, 6000.0, 0.4, 900.0))
        .await;
    w.mock_speed(HOME, 200, lab(0.9, 1000.0, 0.0, 10.0)).await;
    run(&w).await;
    let found: Vec<(String, String)> = sqlx::query_as(
        "SELECT check_id, target FROM findings WHERE workspace_id = $1 AND check_id LIKE 'speed_%' ORDER BY 1",
    )
    .bind(w.workspace_id)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(
        found,
        vec![
            ("speed_layout_shift".to_string(), page(1)),
            ("speed_slow_interaction".to_string(), page(1)),
            ("speed_slow_lcp".to_string(), page(1)),
        ]
    );
    // A later, faster test resolves them.
    w.mock.reset().await;
    w.mock_speed_for_all(lab(0.95, 900.0, 0.0, 20.0)).await;
    run(&w).await;
    let left: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM findings WHERE check_id LIKE 'speed_%'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(left, 0);
}
