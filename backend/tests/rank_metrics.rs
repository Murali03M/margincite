//! The rankings and AI Overviews metric blocks, and visibility from rankings.

mod common;

use common::google::GoogleWorld;
use serde_json::{json, Value};
use sqlx::PgPool;

#[sqlx::test]
async fn a_new_site_says_why_there_are_no_rankings(pool: PgPool) {
    let w = GoogleWorld::new(pool).await;
    assert_eq!(
        w.metrics_block("rankings").await,
        json!({ "tracked": 0, "in_top_3": null, "in_top_10": null, "average_position": null,
                "checked_at": null, "reason": "Add target keywords to track rankings." })
    );
    assert_eq!(
        w.metrics_block("ai_overviews").await,
        json!({ "tracked": 0, "present": null, "cited": null, "checked_at": null,
                "reason": "Add target keywords to track rankings." })
    );
    w.add_keyword("a").await;
    let rankings = w.metrics_block("rankings").await;
    assert_eq!(
        rankings["reason"],
        "Rankings have not been checked yet. Add a DataForSEO login in Settings, then press Check now."
    );
    assert_eq!(rankings["tracked"], 0);
    assert_eq!(rankings["in_top_10"], Value::Null);
    assert!(w.metrics_block("ai_overviews").await["reason"].is_string());
}

#[sqlx::test]
async fn rankings_count_the_top_three_the_top_ten_and_average_the_ranking_ones(pool: PgPool) {
    let w = GoogleWorld::new(pool).await;
    for keyword in ["a", "b", "c", "d", "e"] {
        w.add_keyword(keyword).await;
    }
    w.seed_check("a", Some(2), "absent", &[], 0).await;
    w.seed_check("b", Some(3), "cited", &["example.com"], 0)
        .await;
    w.seed_check("c", Some(10), "present", &["x.com"], 0).await;
    w.seed_check("d", Some(40), "unknown", &[], 0).await;
    // Checked and not in the top 100: counts as tracked, not in the average.
    w.seed_check("e", None, "absent", &[], 0).await;
    let rankings = w.metrics_block("rankings").await;
    assert_eq!(rankings["tracked"], 5);
    assert_eq!(rankings["in_top_3"], 2);
    assert_eq!(rankings["in_top_10"], 3);
    assert_eq!(rankings["average_position"], 13.8);
    assert!(rankings["checked_at"].is_string());
    assert_eq!(rankings["reason"], Value::Null);
    let overviews = w.metrics_block("ai_overviews").await;
    assert_eq!(overviews["tracked"], 5);
    assert_eq!(overviews["present"], 2, "cited counts as present");
    assert_eq!(overviews["cited"], 1);
    assert_eq!(overviews["reason"], Value::Null);
}

#[sqlx::test]
async fn only_the_newest_result_of_a_keyword_counts(pool: PgPool) {
    let w = GoogleWorld::new(pool).await;
    w.add_keyword("a").await;
    w.seed_check("a", Some(30), "absent", &[], 5).await;
    w.seed_check("a", Some(4), "absent", &[], 0).await;
    let rankings = w.metrics_block("rankings").await;
    assert_eq!(rankings["tracked"], 1);
    assert_eq!(rankings["average_position"], 4.0);
}

#[sqlx::test]
async fn a_site_with_no_keyword_in_the_top_100_has_no_average_and_no_false_zero(pool: PgPool) {
    let w = GoogleWorld::new(pool).await;
    w.add_keyword("a").await;
    w.seed_check("a", None, "absent", &[], 0).await;
    let rankings = w.metrics_block("rankings").await;
    assert_eq!(rankings["tracked"], 1);
    assert_eq!(rankings["in_top_10"], 0);
    assert_eq!(rankings["average_position"], Value::Null);
}

#[sqlx::test]
async fn results_from_another_country_are_not_counted(pool: PgPool) {
    let w = GoogleWorld::new(pool).await;
    w.add_keyword("a").await;
    w.seed_check("a", Some(2), "absent", &[], 0).await;
    w.set_site(json!({ "country": "GB" })).await;
    let rankings = w.metrics_block("rankings").await;
    assert_eq!(rankings["tracked"], 0);
    assert_eq!(rankings["in_top_10"], Value::Null);
}

#[sqlx::test]
async fn visibility_uses_rankings_when_they_exist_and_says_so(pool: PgPool) {
    let w = GoogleWorld::new(pool).await;
    for keyword in ["a", "b", "c", "d"] {
        w.add_keyword(keyword).await;
    }
    // Without any source, visibility has no number and no source.
    let before = w.metrics_block("visibility").await;
    assert_eq!(before["source"], Value::Null);
    assert_eq!(before["percent"], Value::Null);
    // Rankings for three of the four keywords.
    w.seed_check("a", Some(1), "absent", &[], 0).await;
    w.seed_check("b", Some(9), "absent", &[], 0).await;
    w.seed_check("c", Some(11), "absent", &[], 0).await;
    let visibility = w.metrics_block("visibility").await;
    assert_eq!(visibility["source"], "rankings");
    assert_eq!(visibility["keywords"], 3);
    assert_eq!(visibility["in_top_10"], 2);
    assert_eq!(visibility["percent"], 66.7);
    assert_eq!(visibility["reason"], Value::Null);
}

#[sqlx::test]
async fn visibility_from_search_console_names_its_source(pool: PgPool) {
    let w = GoogleWorld::new(pool).await;
    w.add_keyword("blue widgets").await;
    w.connect("sc-domain:example.com").await;
    w.seed_rows(&[("blue widgets", "https://example.com/a", 10, 100, 3.0)])
        .await;
    sqlx::query("UPDATE google_connections SET synced_at = now() WHERE site_id = $1")
        .bind(w.site_id)
        .execute(&w.pool)
        .await
        .unwrap();
    let visibility = w.metrics_block("visibility").await;
    assert_eq!(visibility["source"], "search_console");
    assert_eq!(visibility["in_top_10"], 1);
}

#[sqlx::test]
async fn rankings_win_over_search_console_for_visibility(pool: PgPool) {
    let w = GoogleWorld::new(pool).await;
    w.add_keyword("blue widgets").await;
    w.connect("sc-domain:example.com").await;
    w.seed_rows(&[("blue widgets", "https://example.com/a", 10, 100, 3.0)])
        .await;
    sqlx::query("UPDATE google_connections SET synced_at = now() WHERE site_id = $1")
        .bind(w.site_id)
        .execute(&w.pool)
        .await
        .unwrap();
    w.seed_check("blue widgets", Some(30), "absent", &[], 0)
        .await;
    let visibility = w.metrics_block("visibility").await;
    assert_eq!(visibility["source"], "rankings");
    assert_eq!(visibility["in_top_10"], 0);
    assert_eq!(visibility["percent"], 0.0);
}

#[sqlx::test]
async fn another_workspaces_rankings_are_not_mine(pool: PgPool) {
    let w = GoogleWorld::new(pool).await;
    w.add_keyword("a").await;
    w.seed_check("a", Some(2), "absent", &[], 0).await;
    let (status, _) = w
        .send(
            "other-token",
            reqwest::Method::GET,
            &format!("/api/v1/sites/{}/metrics", w.site_id),
            None,
        )
        .await;
    assert_eq!(status, 404);
}
