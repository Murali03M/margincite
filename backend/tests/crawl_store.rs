use margincite::crawl::repo::{self, Finish, NewPage};
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

async fn crawl(pool: &PgPool) -> (Uuid, Uuid) {
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
    let crawl_id = repo::create_crawl(pool, workspace_id, site_id, "manual")
        .await
        .unwrap();
    (workspace_id, crawl_id)
}

fn page_with_nul() -> NewPage {
    NewPage {
        url: "https://example.com/a".into(),
        depth: 0,
        in_sitemap: false,
        status_code: Some(200),
        content_type: Some("text/html\0".into()),
        fetch_error: Some("bad\0news".into()),
        redirects: json!(["https://example.com/\0x"]),
        response_ms: Some(1),
        bytes: Some(1),
        is_https: true,
        x_robots_tag: Some("no\0index".into()),
        needed_javascript: false,
        facts: Some(json!({ "title": "a\u{0}b", "nested": [{ "k\u{0}ey": "v\u{0}" }] })),
        text: Some("hello\0world".into()),
    }
}

#[sqlx::test]
async fn a_page_with_nul_characters_is_stored_without_them(pool: PgPool) {
    let (workspace_id, crawl_id) = crawl(&pool).await;
    repo::upsert_page(&pool, workspace_id, crawl_id, &page_with_nul())
        .await
        .unwrap();
    let pages = repo::list_pages(&pool, workspace_id, crawl_id)
        .await
        .unwrap();
    let stored = &pages[0];
    assert_eq!(stored.text.as_deref(), Some("helloworld"));
    assert_eq!(stored.fetch_error.as_deref(), Some("badnews"));
    assert_eq!(stored.x_robots_tag.as_deref(), Some("noindex"));
    assert_eq!(stored.facts.as_ref().unwrap()["title"], "ab");
    assert_eq!(stored.facts.as_ref().unwrap()["nested"][0]["key"], "v");
}

#[sqlx::test]
async fn robots_text_and_errors_with_nul_characters_are_stored_without_them(pool: PgPool) {
    let (workspace_id, crawl_id) = crawl(&pool).await;
    repo::note_discovery(
        &pool,
        workspace_id,
        crawl_id,
        Some("User\0-agent: *"),
        true,
        false,
    )
    .await
    .unwrap();
    let done = Finish {
        status: "failed".into(),
        stop_reason: "site_unreachable".into(),
        pages_crawled: 0,
        pages_failed: 0,
        error: Some("no\0pe".into()),
    };
    repo::finish(&pool, workspace_id, crawl_id, &done)
        .await
        .unwrap();
    let (robots, error): (Option<String>, Option<String>) =
        sqlx::query_as("SELECT robots_txt, error FROM crawls WHERE id = $1")
            .bind(crawl_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(robots.as_deref(), Some("User-agent: *"));
    assert_eq!(error.as_deref(), Some("nope"));
}

#[sqlx::test]
async fn robots_text_is_cut_at_500000_characters(pool: PgPool) {
    let (workspace_id, crawl_id) = crawl(&pool).await;
    let long = "a".repeat(600_000);
    repo::note_discovery(&pool, workspace_id, crawl_id, Some(&long), false, false)
        .await
        .unwrap();
    let stored: i32 = sqlx::query_scalar("SELECT length(robots_txt) FROM crawls WHERE id = $1")
        .bind(crawl_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(stored, 500_000);
}

#[sqlx::test]
async fn failing_a_crawl_records_a_plain_error_and_finish_time(pool: PgPool) {
    let (workspace_id, crawl_id) = crawl(&pool).await;
    repo::mark_running(&pool, workspace_id, crawl_id)
        .await
        .unwrap();
    repo::fail_crawl(&pool, workspace_id, crawl_id, "It broke.")
        .await
        .unwrap();
    let crawl = repo::get_crawl(&pool, workspace_id, crawl_id)
        .await
        .unwrap();
    assert_eq!(crawl.status, "failed");
    assert_eq!(crawl.error.as_deref(), Some("It broke."));
    let finished: bool =
        sqlx::query_scalar("SELECT finished_at IS NOT NULL FROM crawls WHERE id = $1")
            .bind(crawl_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(finished);
}

#[sqlx::test]
async fn failing_a_finished_crawl_changes_nothing(pool: PgPool) {
    let (workspace_id, crawl_id) = crawl(&pool).await;
    let done = Finish {
        status: "completed".into(),
        stop_reason: "finished".into(),
        pages_crawled: 1,
        pages_failed: 0,
        error: None,
    };
    repo::finish(&pool, workspace_id, crawl_id, &done)
        .await
        .unwrap();
    repo::fail_crawl(&pool, workspace_id, crawl_id, "late")
        .await
        .unwrap();
    let crawl = repo::get_crawl(&pool, workspace_id, crawl_id)
        .await
        .unwrap();
    assert_eq!(crawl.status, "completed");
    assert!(crawl.error.is_none());
}

#[sqlx::test]
async fn a_retried_crawl_starts_clean(pool: PgPool) {
    let (workspace_id, crawl_id) = crawl(&pool).await;
    repo::fail_crawl(&pool, workspace_id, crawl_id, "It broke.")
        .await
        .unwrap();
    repo::mark_running(&pool, workspace_id, crawl_id)
        .await
        .unwrap();
    let row: (String, Option<String>, bool) =
        sqlx::query_as("SELECT status, error, finished_at IS NULL FROM crawls WHERE id = $1")
            .bind(crawl_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(row, ("running".into(), None, true));
}
