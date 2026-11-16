//! Losing a data source never marks its suggestions fixed, and traffic only
//! ever raises a suggestion's reach.

mod common;

use common::suggestions::*;
use margincite::checks::Finding;
use margincite::suggestions::repo as sugg;
use margincite::suggestions::repo::ListFilter;
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

async fn sql(pool: &PgPool, text: &'static str) {
    sqlx::query(text).execute(pool).await.unwrap();
}

async fn add_keyword(pool: &PgPool, s: &Site) {
    sqlx::query(
        "INSERT INTO site_keywords (id, workspace_id, site_id, keyword) VALUES ($1, $2, $3, 'widgets')",
    )
    .bind(Uuid::new_v4())
    .bind(s.workspace_id)
    .bind(s.site_id)
    .execute(pool)
    .await
    .unwrap();
}

async fn add_completed_run(pool: &PgPool, s: &Site) {
    sqlx::query(
        "INSERT INTO ai_runs (id, workspace_id, site_id, status, trigger) VALUES ($1, $2, $3, 'completed', 'manual')",
    )
    .bind(Uuid::new_v4())
    .bind(s.workspace_id)
    .bind(s.site_id)
    .execute(pool)
    .await
    .unwrap();
}

/// A site with Search Console data and three open items that need it.
async fn site_with_search_items(pool: &PgPool) -> (Site, String) {
    let s = site(pool).await;
    connect_with_rows(pool, &s, &[(A, 0)]).await;
    let target = format!("\"widgets\" on {A}");
    let found = [
        on_page("search_no_impressions", A),
        Finding::page("keyword_cannibalization", B, json!({"url": B})),
        Finding::page(
            "search_striking_distance",
            &target,
            json!({"query": "widgets", "url": A, "position": 8.0, "impressions": 300, "clicks": 2}),
        ),
    ];
    add_keyword(pool, &s).await;
    crawl(pool, &s, &[A, B], &found).await;
    (s, target)
}

async fn assert_search_items(pool: &PgPool, s: &Site, target: &str, wanted: &str, when: &str) {
    for (check, item) in [
        ("search_no_impressions", A),
        ("keyword_cannibalization", B),
        ("search_striking_distance", target),
    ] {
        assert_eq!(
            status_of(pool, s, check, item).await.as_deref(),
            Some(wanted),
            "{check} {when}"
        );
    }
}

#[sqlx::test]
async fn a_connection_that_needs_reconnecting_leaves_search_suggestions_open(pool: PgPool) {
    let (s, target) = site_with_search_items(&pool).await;
    sql(
        &pool,
        "UPDATE google_connections SET status = 'needs_reconnecting'",
    )
    .await;
    crawl(&pool, &s, &[A, B], &[]).await;
    assert_search_items(&pool, &s, &target, "open", "while reconnecting").await;
    // With the data back, a crawl that no longer finds them resolves them.
    sql(&pool, "UPDATE google_connections SET status = 'connected'").await;
    crawl(&pool, &s, &[A, B], &[]).await;
    assert_search_items(&pool, &s, &target, "done", "once connected again").await;
}

#[sqlx::test]
async fn a_disconnected_site_leaves_search_suggestions_open(pool: PgPool) {
    let (s, target) = site_with_search_items(&pool).await;
    sql(&pool, "DELETE FROM google_connections").await;
    crawl(&pool, &s, &[A, B], &[]).await;
    assert_search_items(&pool, &s, &target, "open", "after disconnecting").await;
}

#[sqlx::test]
async fn without_a_completed_ai_run_ai_suggestions_stay_open(pool: PgPool) {
    let s = site(&pool).await;
    add_completed_run(&pool, &s).await;
    let question = "Which widget shop ships fast?";
    let found = [Finding::site("ai_not_mentioned", question, json!({}))];
    crawl(&pool, &s, &[A], &found).await;
    sql(&pool, "DELETE FROM ai_runs").await;
    crawl(&pool, &s, &[A], &[]).await;
    assert_eq!(
        status_of(&pool, &s, "ai_not_mentioned", question)
            .await
            .as_deref(),
        Some("open")
    );
    add_completed_run(&pool, &s).await;
    crawl(&pool, &s, &[A], &[]).await;
    assert_eq!(
        status_of(&pool, &s, "ai_not_mentioned", question)
            .await
            .as_deref(),
        Some("done"),
        "a run that no longer finds it resolves it"
    );
}

#[sqlx::test]
async fn without_keywords_keyword_suggestions_stay_open(pool: PgPool) {
    let s = site(&pool).await;
    add_keyword(&pool, &s).await;
    let target = format!("\"widgets\" on {A}");
    let finding = Finding::page(
        "keyword_not_in_title",
        &target,
        json!({"keyword": "widgets", "url": A, "landing_source": "set"}),
    );
    crawl(&pool, &s, &[A], &[finding]).await;
    sql(&pool, "DELETE FROM site_keywords").await;
    crawl(&pool, &s, &[A], &[]).await;
    assert_eq!(
        status_of(&pool, &s, "keyword_not_in_title", &target)
            .await
            .as_deref(),
        Some("open")
    );
}

#[sqlx::test]
async fn items_waiting_on_a_missing_source_do_not_lower_the_score(pool: PgPool) {
    let s = site(&pool).await;
    add_completed_run(&pool, &s).await;
    let found = [Finding::site(
        "ai_not_mentioned",
        "A question here?",
        json!({}),
    )];
    let with_run = crawl(&pool, &s, &[A], &found).await;
    sql(&pool, "DELETE FROM ai_runs").await;
    let without = crawl(&pool, &s, &[A], &[]).await;
    assert_eq!(
        without.score,
        Some(100),
        "the open item is not in this site's score"
    );
    assert!(with_run.score < Some(100));
}

async fn reaches(pool: &PgPool, s: &Site) -> Vec<(String, f64)> {
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
    rows.into_iter().map(|r| (r.check_id, r.reach)).collect()
}

#[sqlx::test]
async fn a_broken_page_with_no_impressions_still_counts(pool: PgPool) {
    let s = site(&pool).await;
    let found = [on_page("broken_page", C)];
    let before = crawl(&pool, &s, &[A, B, C], &found).await;
    connect_with_rows(&pool, &s, &[(A, 700), (B, 300)]).await;
    let reach = reach_of(&pool, &s, "broken_page").await;
    assert!((reach - 1.0 / 3.0).abs() < 1e-9, "{reach}");
    // The score does not forget the page either.
    let after = crawl(&pool, &s, &[A, B, C], &found).await;
    assert_eq!(after.score, before.score);
}

#[sqlx::test]
async fn search_no_impressions_can_rank_because_its_pages_have_none(pool: PgPool) {
    let s = site(&pool).await;
    crawl(
        &pool,
        &s,
        &[A, B, C],
        &[on_page("search_no_impressions", C)],
    )
    .await;
    connect_with_rows(&pool, &s, &[(A, 700), (B, 300)]).await;
    assert!(reach_of(&pool, &s, "search_no_impressions").await > 0.0);
}

#[sqlx::test]
async fn the_page_with_most_of_the_impressions_still_ranks_first(pool: PgPool) {
    let s = site(&pool).await;
    // Same severity and effort, so reach alone decides the order.
    let found = [on_page("title_missing", A), on_page("title_duplicate", B)];
    crawl(&pool, &s, &[A, B, C, "https://example.com/d"], &found).await;
    connect_with_rows(&pool, &s, &[(A, 900), (B, 50), (C, 50)]).await;
    let order = reaches(&pool, &s).await;
    let first = order.iter().max_by(|a, b| a.1.total_cmp(&b.1)).unwrap();
    assert_eq!(first.0, "title_missing");
    assert!((first.1 - 0.9).abs() < 1e-9, "{first:?}");
}
