//! The two ranking checks, end to end: found, listed, and settled when the
//! results get better, or left alone when the page was not read.

mod common;

use common::dataforseo as df;
use common::google::GoogleWorld;
use common::rankings::overview;
use margincite::integrations::google::sync::refresh_checks;
use serde_json::{json, Value};
use sqlx::PgPool;

const HOME: &str = "https://example.com/";
const A: &str = "https://example.com/a";

async fn check(w: &GoogleWorld, id: &str) -> Value {
    let (status, body) = w.get(&format!("/api/v1/sites/{}/checks", w.site_id)).await;
    assert_eq!(status, 200, "{body}");
    body["data"]["checks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["check_id"] == id)
        .unwrap_or_else(|| panic!("no check {id}"))
        .clone()
}

/// The status of every item of a check.
async fn items(w: &GoogleWorld, check_id: &str) -> Vec<(String, String)> {
    sqlx::query_as(
        "SELECT i.target, i.status FROM suggestion_items i JOIN suggestions s ON s.id = i.suggestion_id
         WHERE s.site_id = $1 AND s.check_id = $2 ORDER BY i.target",
    )
    .bind(w.site_id)
    .bind(check_id)
    .fetch_all(&w.pool)
    .await
    .unwrap()
}

async fn world(pool: PgPool) -> GoogleWorld {
    let w = GoogleWorld::new(pool).await;
    w.crawl(&[(HOME, true), (A, true)]).await;
    w.add_keyword("blue widgets").await;
    w
}

#[sqlx::test]
async fn both_checks_are_not_run_until_something_has_been_checked(pool: PgPool) {
    let w = world(pool).await;
    for id in ["rank_dropped", "ai_overview_not_cited"] {
        let found = check(&w, id).await;
        assert_eq!(found["status"], "not_run", "{id}");
        assert_eq!(
            found["reason"],
            "Check your keyword rankings to see this. It needs a DataForSEO login in Settings."
        );
    }
    w.seed_check("blue widgets", Some(3), "absent", &[], 1)
        .await;
    refresh_checks(&w.pool, w.workspace_id, w.site_id).await;
    for id in ["rank_dropped", "ai_overview_not_cited"] {
        assert_eq!(check(&w, id).await["status"], "passed", "{id}");
    }
}

#[sqlx::test]
async fn a_drop_is_found_listed_and_settled_when_the_keyword_recovers(pool: PgPool) {
    let w = world(pool).await;
    w.seed_check("blue widgets", Some(3), "absent", &[], 7)
        .await;
    w.seed_check("blue widgets", Some(12), "absent", &[], 0)
        .await;
    refresh_checks(&w.pool, w.workspace_id, w.site_id).await;
    let found = check(&w, "rank_dropped").await;
    assert_eq!(found["status"], "failed");
    assert_eq!(found["open"], 1);
    assert_eq!(
        items(&w, "rank_dropped").await,
        vec![("\"blue widgets\" in Google results".into(), "open".into())]
    );
    // A later check puts it back.
    w.seed_check("blue widgets", Some(4), "absent", &[], 0)
        .await;
    sqlx::query(
        "UPDATE rank_checks SET checked_at = now() + interval '1 minute' WHERE position = 4",
    )
    .execute(&w.pool)
    .await
    .unwrap();
    refresh_checks(&w.pool, w.workspace_id, w.site_id).await;
    assert_eq!(check(&w, "rank_dropped").await["status"], "passed");
    assert_eq!(items(&w, "rank_dropped").await[0].1, "done");
}

#[sqlx::test]
async fn leaving_the_top_100_is_a_drop_and_the_item_names_a_crawled_page(pool: PgPool) {
    let w = world(pool).await;
    w.seed_check("blue widgets", Some(40), "absent", &[], 7)
        .await;
    w.seed_check("blue widgets", None, "absent", &[], 0).await;
    refresh_checks(&w.pool, w.workspace_id, w.site_id).await;
    assert_eq!(check(&w, "rank_dropped").await["status"], "failed");
    let url: String = sqlx::query_scalar(
        "SELECT i.evidence->>'url' FROM suggestion_items i JOIN suggestions s ON s.id = i.suggestion_id
         WHERE s.check_id = 'rank_dropped' AND s.site_id = $1",
    )
    .bind(w.site_id)
    .fetch_one(&w.pool)
    .await
    .unwrap();
    assert_eq!(url, HOME);
    // Back inside the top 100: settled, because the home page was read.
    w.seed_check("blue widgets", Some(60), "absent", &[], 0)
        .await;
    sqlx::query(
        "UPDATE rank_checks SET checked_at = now() + interval '1 minute' WHERE position = 60",
    )
    .execute(&w.pool)
    .await
    .unwrap();
    refresh_checks(&w.pool, w.workspace_id, w.site_id).await;
    assert_eq!(items(&w, "rank_dropped").await[0].1, "done");
}

#[sqlx::test]
async fn a_keyword_that_was_never_in_the_top_100_is_not_a_drop(pool: PgPool) {
    let w = world(pool).await;
    w.seed_check("blue widgets", None, "absent", &[], 7).await;
    w.seed_check("blue widgets", None, "absent", &[], 0).await;
    refresh_checks(&w.pool, w.workspace_id, w.site_id).await;
    assert_eq!(check(&w, "rank_dropped").await["status"], "passed");
}

#[sqlx::test]
async fn a_drop_in_another_country_is_not_compared_with_this_ones_history(pool: PgPool) {
    let w = world(pool).await;
    w.seed_check("blue widgets", Some(2), "absent", &[], 7)
        .await;
    w.set_site(json!({ "country": "GB" })).await;
    sqlx::query(
        "INSERT INTO rank_checks (id, workspace_id, site_id, keyword_id, keyword, country, language,
             device, position, ai_overview) VALUES (gen_random_uuid(), $1, $2, gen_random_uuid(),
             'blue widgets', 'GB', 'en', 'desktop', 30, 'absent')",
    )
    .bind(w.workspace_id)
    .bind(w.site_id)
    .execute(&w.pool)
    .await
    .unwrap();
    refresh_checks(&w.pool, w.workspace_id, w.site_id).await;
    assert_eq!(
        check(&w, "rank_dropped").await["status"],
        "passed",
        "2 in the US, 30 in the UK"
    );
}

#[sqlx::test]
async fn an_overview_that_cites_others_is_found_with_their_domains_and_settles_when_cited(
    pool: PgPool,
) {
    let w = world(pool).await;
    w.seed_check("blue widgets", Some(6), "present", &["a.com", "b.com"], 0)
        .await;
    refresh_checks(&w.pool, w.workspace_id, w.site_id).await;
    let found = check(&w, "ai_overview_not_cited").await;
    assert_eq!(found["status"], "failed");
    let evidence: Value = sqlx::query_scalar(
        "SELECT i.evidence FROM suggestion_items i JOIN suggestions s ON s.id = i.suggestion_id
         WHERE s.check_id = 'ai_overview_not_cited' AND s.site_id = $1",
    )
    .bind(w.site_id)
    .fetch_one(&w.pool)
    .await
    .unwrap();
    assert_eq!(evidence["cited_domains"], json!(["a.com", "b.com"]));
    assert_eq!(evidence["url"], A);
    w.seed_check(
        "blue widgets",
        Some(6),
        "cited",
        &["a.com", "example.com"],
        0,
    )
    .await;
    sqlx::query("UPDATE rank_checks SET checked_at = now() + interval '1 minute' WHERE ai_overview = 'cited'")
        .execute(&w.pool)
        .await
        .unwrap();
    refresh_checks(&w.pool, w.workspace_id, w.site_id).await;
    assert_eq!(items(&w, "ai_overview_not_cited").await[0].1, "done");
}

#[sqlx::test]
async fn an_item_whose_page_failed_in_the_crawl_is_left_open(pool: PgPool) {
    let w = world(pool).await;
    w.seed_check("blue widgets", Some(6), "present", &["a.com"], 0)
        .await;
    refresh_checks(&w.pool, w.workspace_id, w.site_id).await;
    // The ranking page could not be read in the latest crawl, and the result improves.
    sqlx::query("UPDATE pages SET fetch_error = 'timeout', status_code = NULL WHERE url = $1")
        .bind(A)
        .execute(&w.pool)
        .await
        .unwrap();
    sqlx::query("UPDATE rank_checks SET ai_overview = 'absent', ai_cited_domains = '[]'")
        .execute(&w.pool)
        .await
        .unwrap();
    refresh_checks(&w.pool, w.workspace_id, w.site_id).await;
    assert_eq!(items(&w, "ai_overview_not_cited").await[0].1, "open");
}

#[sqlx::test]
async fn a_rank_run_ends_by_running_the_checks(pool: PgPool) {
    let w = world(pool).await;
    w.save_dataforseo().await;
    w.seed_check("blue widgets", Some(2), "absent", &[], 7)
        .await;
    w.mock_serp(
        "blue widgets",
        200,
        df::serp(json!([
            df::organic(1, "a.com"),
            df::organic(9, "example.com"),
            overview(&[("a.com", "https://a.com/")])
        ])),
    )
    .await;
    w.run_ranks().await;
    assert_eq!(check(&w, "rank_dropped").await["status"], "failed");
    assert_eq!(check(&w, "ai_overview_not_cited").await["status"], "failed");
    assert_eq!(w.rankings().await["keywords"][0]["previous"], 2);
}
