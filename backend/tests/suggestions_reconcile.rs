mod common;

use common::suggestions::*;
use margincite::checks::catalog::{self, Available};
use margincite::checks::context::Category;
use margincite::checks::Finding;
use margincite::crawl::repo::NewPage;
use margincite::suggestions::repo as sugg;
use margincite::suggestions::repo::ListFilter;
use margincite::suggestions::{rank, Status};
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

fn with_status(url: &str, status: i32) -> NewPage {
    NewPage {
        status_code: Some(status),
        ..page(url)
    }
}

async fn mark(pool: &PgPool, s: &Site, check: &str, status: Status, targets: Option<&[String]>) {
    let id = suggestion_id(pool, s, check).await;
    sugg::set_status(pool, s.workspace_id, id, status, targets)
        .await
        .unwrap();
}

async fn suggestion_id(pool: &PgPool, s: &Site, check: &str) -> Uuid {
    sqlx::query_scalar("SELECT id FROM suggestions WHERE site_id = $1 AND check_id = $2")
        .bind(s.site_id)
        .bind(check)
        .fetch_one(pool)
        .await
        .unwrap()
}

fn all(status: Option<Status>) -> ListFilter {
    ListFilter {
        status,
        category: None,
    }
}

#[sqlx::test]
async fn a_new_finding_becomes_an_open_item(pool: PgPool) {
    let s = site(&pool).await;
    crawl(&pool, &s, &[A], &[on_page("title_missing", A)]).await;
    assert_eq!(
        status_of(&pool, &s, "title_missing", A).await.as_deref(),
        Some("open")
    );
    let counts = sugg::counts(&pool, s.workspace_id, s.site_id)
        .await
        .unwrap();
    assert_eq!((counts.open, counts.done, counts.dismissed), (1, 0, 0));
}

#[sqlx::test]
async fn an_open_item_found_again_updates_its_evidence(pool: PgPool) {
    let s = site(&pool).await;
    crawl(&pool, &s, &[A], &[on_page("title_missing", A)]).await;
    let newer = Finding::page("title_missing", A, json!({"seen": "again"}));
    crawl(&pool, &s, &[A], &[newer]).await;
    let evidence: serde_json::Value =
        sqlx::query_scalar("SELECT evidence FROM suggestion_items WHERE target = $1")
            .bind(A)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(evidence["seen"], "again");
}

#[sqlx::test]
async fn a_done_item_that_is_still_found_reopens(pool: PgPool) {
    let s = site(&pool).await;
    crawl(&pool, &s, &[A], &[on_page("title_missing", A)]).await;
    mark(&pool, &s, "title_missing", Status::Done, None).await;
    assert_eq!(
        status_of(&pool, &s, "title_missing", A).await.as_deref(),
        Some("done")
    );
    crawl(&pool, &s, &[A], &[on_page("title_missing", A)]).await;
    assert_eq!(
        status_of(&pool, &s, "title_missing", A).await.as_deref(),
        Some("open")
    );
    let resolved: Option<time::OffsetDateTime> =
        sqlx::query_scalar("SELECT resolved_at FROM suggestion_items WHERE target = $1")
            .bind(A)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(resolved.is_none());
}

#[sqlx::test]
async fn a_dismissed_item_stays_dismissed_when_found_again(pool: PgPool) {
    let s = site(&pool).await;
    crawl(&pool, &s, &[A], &[on_page("title_missing", A)]).await;
    mark(&pool, &s, "title_missing", Status::Dismissed, None).await;
    crawl(&pool, &s, &[A], &[on_page("title_missing", A)]).await;
    assert_eq!(
        status_of(&pool, &s, "title_missing", A).await.as_deref(),
        Some("dismissed")
    );
}

#[sqlx::test]
async fn a_new_page_failing_a_dismissed_check_is_a_new_open_item(pool: PgPool) {
    let s = site(&pool).await;
    crawl(&pool, &s, &[A, B], &[on_page("title_missing", A)]).await;
    mark(&pool, &s, "title_missing", Status::Dismissed, None).await;
    let found = [on_page("title_missing", A), on_page("title_missing", B)];
    crawl(&pool, &s, &[A, B], &found).await;
    assert_eq!(
        status_of(&pool, &s, "title_missing", A).await.as_deref(),
        Some("dismissed")
    );
    assert_eq!(
        status_of(&pool, &s, "title_missing", B).await.as_deref(),
        Some("open")
    );
}

#[sqlx::test]
async fn an_open_item_no_longer_found_becomes_done(pool: PgPool) {
    let s = site(&pool).await;
    crawl(&pool, &s, &[A], &[on_page("title_missing", A)]).await;
    crawl(&pool, &s, &[A], &[]).await;
    assert_eq!(
        status_of(&pool, &s, "title_missing", A).await.as_deref(),
        Some("done")
    );
    let resolved: Option<time::OffsetDateTime> =
        sqlx::query_scalar("SELECT resolved_at FROM suggestion_items WHERE target = $1")
            .bind(A)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(resolved.is_some());
}

#[sqlx::test]
async fn a_site_level_item_no_longer_found_becomes_done(pool: PgPool) {
    let s = site(&pool).await;
    let missing = Finding::site("sitemap_missing", "https://example.com/", json!({}));
    crawl(&pool, &s, &[A], &[missing]).await;
    crawl(&pool, &s, &[A], &[]).await;
    assert_eq!(
        status_of(&pool, &s, "sitemap_missing", "https://example.com/")
            .await
            .as_deref(),
        Some("done")
    );
}

#[sqlx::test]
async fn an_item_for_a_page_not_crawled_this_time_keeps_its_status(pool: PgPool) {
    let s = site(&pool).await;
    let found = [on_page("title_missing", A), on_page("title_missing", B)];
    crawl(&pool, &s, &[A, B], &found).await;
    // The next crawl stopped at its cap before reaching B.
    crawl(&pool, &s, &[A, C], &[on_page("title_missing", A)]).await;
    assert_eq!(
        status_of(&pool, &s, "title_missing", B).await.as_deref(),
        Some("open")
    );
}

#[sqlx::test]
async fn a_group_is_listed_under_done_only_when_no_item_is_open(pool: PgPool) {
    let s = site(&pool).await;
    let found = [on_page("title_missing", A), on_page("title_missing", B)];
    crawl(&pool, &s, &[A, B], &found).await;
    let list = |status: Status| {
        let pool = pool.clone();
        let (w, si) = (s.workspace_id, s.site_id);
        async move {
            sugg::list(&pool, w, si, all(Some(status)), 1, 50)
                .await
                .unwrap()
                .1
        }
    };
    mark(
        &pool,
        &s,
        "title_missing",
        Status::Done,
        Some(&[A.to_string()]),
    )
    .await;
    assert_eq!((list(Status::Open).await, list(Status::Done).await), (1, 0));
    mark(
        &pool,
        &s,
        "title_missing",
        Status::Dismissed,
        Some(&[B.to_string()]),
    )
    .await;
    assert_eq!((list(Status::Open).await, list(Status::Done).await), (0, 1));
}

#[sqlx::test]
async fn suggestions_are_ranked_by_severity_reach_and_effort(pool: PgPool) {
    let s = site(&pool).await;
    let found = [
        on_page("image_alt_missing", A),
        on_page("image_alt_missing", B),
        on_page("broken_page", A),
        on_page("title_missing", A),
    ];
    crawl(&pool, &s, &[A, B], &found).await;
    let (rows, total) = sugg::list(&pool, s.workspace_id, s.site_id, all(None), 1, 50)
        .await
        .unwrap();
    let order: Vec<_> = rows.iter().map(|row| row.check_id.as_str()).collect();
    // title: 3*0.5/1 = 1.5, broken: 3*0.5/2 = 0.75, alt: 1*1/2 = 0.5
    assert_eq!(order, ["title_missing", "broken_page", "image_alt_missing"]);
    assert_eq!(total, 3);
    assert!((rows[0].priority - 1.5).abs() < 1e-9);
    let (second, _) = sugg::list(&pool, s.workspace_id, s.site_id, all(None), 2, 2)
        .await
        .unwrap();
    assert_eq!(second.len(), 1);
    let (technical, _) = sugg::list(
        &pool,
        s.workspace_id,
        s.site_id,
        ListFilter {
            status: None,
            category: Some(Category::Content),
        },
        1,
        50,
    )
    .await
    .unwrap();
    assert!(technical.is_empty());
}

#[sqlx::test]
async fn marking_chosen_targets_changes_only_those(pool: PgPool) {
    let s = site(&pool).await;
    let found = [on_page("title_missing", A), on_page("title_missing", B)];
    crawl(&pool, &s, &[A, B], &found).await;
    let id = suggestion_id(&pool, &s, "title_missing").await;
    let changed = sugg::set_status(
        &pool,
        s.workspace_id,
        id,
        Status::Done,
        Some(&[A.to_string()]),
    )
    .await
    .unwrap();
    assert_eq!(changed, 1);
    assert_eq!(
        status_of(&pool, &s, "title_missing", B).await.as_deref(),
        Some("open")
    );
    let everything = sugg::set_status(&pool, s.workspace_id, id, Status::Dismissed, None)
        .await
        .unwrap();
    assert_eq!(everything, 2);
    let (items, total) = sugg::items(&pool, s.workspace_id, id, Some(Status::Dismissed), 1, 10)
        .await
        .unwrap();
    assert_eq!((items.len(), total), (2, 2));
}

#[sqlx::test]
async fn another_workspace_cannot_read_or_change_my_suggestions(pool: PgPool) {
    let mine = site(&pool).await;
    let theirs = site(&pool).await;
    crawl(&pool, &mine, &[A], &[on_page("title_missing", A)]).await;
    let id = suggestion_id(&pool, &mine, "title_missing").await;
    let other = theirs.workspace_id;
    assert!(sugg::get(&pool, other, id).await.is_err());
    assert!(sugg::items(&pool, other, id, None, 1, 10).await.is_err());
    assert!(sugg::set_status(&pool, other, id, Status::Done, None)
        .await
        .is_err());
    let (rows, _) = sugg::list(&pool, other, mine.site_id, all(None), 1, 10)
        .await
        .unwrap();
    assert!(rows.is_empty());
    assert_eq!(
        status_of(&pool, &mine, "title_missing", A).await.as_deref(),
        Some("open")
    );
}

#[sqlx::test]
async fn score_is_100_with_no_open_items_and_lower_with_more(pool: PgPool) {
    let s = site(&pool).await;
    let clean = crawl(&pool, &s, &[A, B], &[]).await;
    assert_eq!((clean.score, clean.html_pages), (Some(100), 2));
    let found = [on_page("title_missing", A), on_page("title_missing", B)];
    let worse = crawl(&pool, &s, &[A, B], &found).await;
    let total: u32 = catalog::all()
        .iter()
        .filter(|c| c.needs.is_empty())
        .map(|c| c.severity as u32)
        .sum();
    let expected = (100.0 * (1.0 - 3.0 / f64::from(total))).round() as i32;
    assert_eq!(worse.score, Some(expected));
    assert!(worse.score < clean.score);
    assert_eq!(catalog::severity_total(Available::default()), total);
    assert!(rank::catalog_severity_total() > total);
}

/// The severities of the checks that need nothing but the crawl: 52 before
/// the keyword checks were added, plus 9 for the seven answer-readiness
/// checks (1+1+1+1+2+2+1).
const SEVERITY_BEFORE_KEYWORD_CHECKS: u32 = 52 + 9;

#[sqlx::test]
async fn checks_that_cannot_run_are_left_out_of_the_score(pool: PgPool) {
    let s = site(&pool).await;
    let found = [on_page("title_missing", A), on_page("title_missing", B)];
    let scored = crawl(&pool, &s, &[A, B], &found).await;
    // The same score a site had before keyword checks existed.
    assert_eq!(
        catalog::severity_total(Available::default()),
        SEVERITY_BEFORE_KEYWORD_CHECKS
    );
    let before = (100.0 * (1.0 - 3.0 / f64::from(SEVERITY_BEFORE_KEYWORD_CHECKS))).round();
    assert_eq!(scored.score, Some(before as i32));
}

#[sqlx::test]
async fn keyword_checks_join_the_score_once_the_site_has_keywords(pool: PgPool) {
    let s = site(&pool).await;
    sqlx::query(
        "INSERT INTO site_keywords (id, workspace_id, site_id, keyword) VALUES ($1, $2, $3, 'widgets')",
    )
    .bind(Uuid::new_v4())
    .bind(s.workspace_id)
    .bind(s.site_id)
    .execute(&pool)
    .await
    .unwrap();
    let found = [on_page("title_missing", A), on_page("title_missing", B)];
    let scored = crawl(&pool, &s, &[A, B], &found).await;
    // Five keyword checks run (severities 2+2+1+2+2); cannibalization needs
    // Search Console and stays out.
    let total = SEVERITY_BEFORE_KEYWORD_CHECKS + 9;
    let with_keywords = Available {
        keywords: true,
        ..Available::default()
    };
    assert_eq!(catalog::severity_total(with_keywords), total);
    let expected = (100.0 * (1.0 - 3.0 / f64::from(total))).round();
    assert_eq!(scored.score, Some(expected as i32));
}

#[sqlx::test]
async fn a_fixed_keyword_item_is_resolved_by_the_next_crawl(pool: PgPool) {
    let s = site(&pool).await;
    sqlx::query(
        "INSERT INTO site_keywords (id, workspace_id, site_id, keyword) VALUES ($1, $2, $3, 'widgets')",
    )
    .bind(Uuid::new_v4())
    .bind(s.workspace_id)
    .bind(s.site_id)
    .execute(&pool)
    .await
    .unwrap();
    let target = format!("\"widgets\" on {A}");
    let finding = Finding::page(
        "keyword_not_in_title",
        &target,
        json!({"keyword": "widgets", "url": A, "landing_source": "set"}),
    );
    crawl(&pool, &s, &[A], &[finding]).await;
    assert_eq!(
        status_of(&pool, &s, "keyword_not_in_title", &target)
            .await
            .as_deref(),
        Some("open")
    );
    crawl(&pool, &s, &[A], &[]).await;
    assert_eq!(
        status_of(&pool, &s, "keyword_not_in_title", &target)
            .await
            .as_deref(),
        Some("done")
    );
    // A page that could not be read proves nothing.
    let finding = Finding::page(
        "keyword_not_in_title",
        &target,
        json!({"keyword": "widgets", "url": A, "landing_source": "set"}),
    );
    crawl(&pool, &s, &[A], &[finding]).await;
    crawl_rows(&pool, &s, &[timed_out(A)], &[]).await;
    assert_eq!(
        status_of(&pool, &s, "keyword_not_in_title", &target)
            .await
            .as_deref(),
        Some("open")
    );
}

#[sqlx::test]
async fn score_is_none_when_there_are_no_working_pages(pool: PgPool) {
    let s = site(&pool).await;
    let sitemap = Finding::site("sitemap_missing", "https://example.com/", json!({}));
    let empty = crawl(&pool, &s, &[], &[sitemap]).await;
    assert_eq!((empty.score, empty.html_pages), (None, 0));
}

#[sqlx::test]
async fn an_item_for_a_page_that_timed_out_keeps_its_status(pool: PgPool) {
    let s = site(&pool).await;
    crawl(&pool, &s, &[A], &[on_page("title_missing", A)]).await;
    crawl_rows(&pool, &s, &[timed_out(A)], &[]).await;
    assert_eq!(
        status_of(&pool, &s, "title_missing", A).await.as_deref(),
        Some("open")
    );
}

#[sqlx::test]
async fn an_item_for_a_page_returning_500_keeps_its_status(pool: PgPool) {
    let s = site(&pool).await;
    crawl(&pool, &s, &[A], &[on_page("title_missing", A)]).await;
    crawl_rows(&pool, &s, &[with_status(A, 500)], &[]).await;
    assert_eq!(
        status_of(&pool, &s, "title_missing", A).await.as_deref(),
        Some("open")
    );
}

#[sqlx::test]
async fn an_item_for_a_page_returning_404_keeps_its_status(pool: PgPool) {
    let s = site(&pool).await;
    crawl(&pool, &s, &[A], &[on_page("title_missing", A)]).await;
    crawl_rows(&pool, &s, &[with_status(A, 404)], &[]).await;
    assert_eq!(
        status_of(&pool, &s, "title_missing", A).await.as_deref(),
        Some("open")
    );
}

#[sqlx::test]
async fn a_broken_page_that_now_loads_becomes_done(pool: PgPool) {
    let s = site(&pool).await;
    crawl_rows(
        &pool,
        &s,
        &[with_status(A, 500)],
        &[on_page("broken_page", A)],
    )
    .await;
    assert_eq!(
        status_of(&pool, &s, "broken_page", A).await.as_deref(),
        Some("open")
    );
    crawl(&pool, &s, &[A], &[]).await;
    assert_eq!(
        status_of(&pool, &s, "broken_page", A).await.as_deref(),
        Some("done")
    );
}

#[sqlx::test]
async fn a_broken_page_that_timed_out_this_time_stays_open(pool: PgPool) {
    let s = site(&pool).await;
    crawl_rows(
        &pool,
        &s,
        &[with_status(A, 500)],
        &[on_page("broken_page", A)],
    )
    .await;
    crawl_rows(&pool, &s, &[timed_out(A)], &[]).await;
    assert_eq!(
        status_of(&pool, &s, "broken_page", A).await.as_deref(),
        Some("open")
    );
}

#[sqlx::test]
async fn a_fixed_striking_distance_item_is_resolved_by_the_next_crawl(pool: PgPool) {
    let s = site(&pool).await;
    connect_with_rows(&pool, &s, &[(A, 300)]).await;
    let target = format!("\"widgets\" on {A}");
    let finding = Finding::page(
        "search_striking_distance",
        &target,
        json!({"query": "widgets", "url": A, "position": 8.0, "impressions": 300, "clicks": 2}),
    );
    crawl(&pool, &s, &[A], std::slice::from_ref(&finding)).await;
    // The crawl could not read the page: nothing is proven.
    crawl_rows(&pool, &s, &[timed_out(A)], &[]).await;
    assert_eq!(
        status_of(&pool, &s, "search_striking_distance", &target)
            .await
            .as_deref(),
        Some("open")
    );
    // The page reads fine and no longer qualifies: the item is done.
    crawl(&pool, &s, &[A], &[]).await;
    assert_eq!(
        status_of(&pool, &s, "search_striking_distance", &target)
            .await
            .as_deref(),
        Some("done")
    );
}

#[sqlx::test]
async fn with_search_console_a_suggestions_reach_is_its_share_of_impressions(pool: PgPool) {
    let s = site(&pool).await;
    let found = [on_page("title_missing", A)];
    crawl(&pool, &s, &[A, B, C], &found).await;
    // Without Search Console: one page of three.
    assert!((reach_of(&pool, &s, "title_missing").await - 1.0 / 3.0).abs() < 1e-9);
    // With it: page A gets 600 of the site's 1000 impressions.
    connect_with_rows(&pool, &s, &[(A, 600), (B, 300), (C, 100)]).await;
    assert!((reach_of(&pool, &s, "title_missing").await - 0.6).abs() < 1e-9);
}

#[sqlx::test]
async fn keyword_style_items_count_the_page_in_their_evidence(pool: PgPool) {
    let s = site(&pool).await;
    let finding = Finding::page(
        "keyword_not_in_title",
        &format!("\"widgets\" on {A}"),
        json!({"keyword": "widgets", "url": A, "landing_source": "set"}),
    );
    crawl(&pool, &s, &[A, B], &[finding]).await;
    connect_with_rows(&pool, &s, &[(A, 750), (B, 250)]).await;
    assert!((reach_of(&pool, &s, "keyword_not_in_title").await - 0.75).abs() < 1e-9);
}

#[sqlx::test]
async fn reach_falls_back_to_pages_when_the_site_has_no_impressions(pool: PgPool) {
    let s = site(&pool).await;
    crawl(&pool, &s, &[A, B], &[on_page("title_missing", A)]).await;
    connect_with_rows(&pool, &s, &[(A, 0)]).await;
    assert!((reach_of(&pool, &s, "title_missing").await - 0.5).abs() < 1e-9);
}

#[sqlx::test]
async fn a_site_level_suggestion_still_reaches_the_whole_site(pool: PgPool) {
    let s = site(&pool).await;
    let sitemap = Finding::site("sitemap_missing", "https://example.com/", json!({}));
    crawl(&pool, &s, &[A], &[sitemap]).await;
    connect_with_rows(&pool, &s, &[(A, 10)]).await;
    assert_eq!(reach_of(&pool, &s, "sitemap_missing").await, 1.0);
}
