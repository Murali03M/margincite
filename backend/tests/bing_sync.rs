mod common;

use common::bing::BING_KEY;
use common::google::GoogleWorld;
use margincite::jobs::worker::JobHandler;
use serde_json::json;
use sqlx::PgPool;
use time::{Duration, OffsetDateTime};

async fn targets(w: &GoogleWorld) -> Vec<(String, i64)> {
    sqlx::query_as(
        "SELECT target_url, count FROM backlink_targets WHERE site_id = $1 ORDER BY target_url",
    )
    .bind(w.site_id)
    .fetch_all(&w.pool)
    .await
    .unwrap()
}

async fn sources(w: &GoogleWorld) -> Vec<(String, String, String)> {
    sqlx::query_as(
        "SELECT target_url, source_url, anchor_text FROM backlink_sources
         WHERE site_id = $1 ORDER BY target_url, source_url",
    )
    .bind(w.site_id)
    .fetch_all(&w.pool)
    .await
    .unwrap()
}

async fn synced_at(w: &GoogleWorld) -> Option<OffsetDateTime> {
    sqlx::query_scalar("SELECT synced_at FROM bing_connections WHERE site_id = $1")
        .bind(w.site_id)
        .fetch_one(&w.pool)
        .await
        .unwrap()
}

#[sqlx::test]
async fn counts_across_pages_are_stored_and_the_next_sync_is_a_week_away(pool: PgPool) {
    let w = GoogleWorld::new(pool).await;
    w.insert_bing("connected").await;
    w.mock_bing_counts(
        0,
        &[("https://example.com/a", 5), ("https://example.com/b", 2)],
    )
    .await;
    w.mock_bing_counts(1, &[("https://example.com/c", 7)]).await;
    w.mock_bing_counts(2, &[]).await;
    for (target, rows) in [
        (
            "https://example.com/a",
            vec![("https://blog.example/x", "widgets")],
        ),
        ("https://example.com/b", vec![]),
        (
            "https://example.com/c",
            vec![("https://news.example/y", "")],
        ),
    ] {
        w.mock_bing_sources(target, &rows).await;
    }
    w.run_bing_sync().await.unwrap();

    assert_eq!(
        targets(&w).await,
        vec![
            ("https://example.com/a".to_string(), 5),
            ("https://example.com/b".to_string(), 2),
            ("https://example.com/c".to_string(), 7),
        ]
    );
    assert!(synced_at(&w).await.is_some());
    let key_requests = w.bing_requests().await;
    assert!(key_requests.iter().all(|r| r
        .url
        .query_pairs()
        .any(|(k, v)| k == "apikey" && v == BING_KEY)));

    let (run_at, payload): (OffsetDateTime, serde_json::Value) = sqlx::query_as(
        "SELECT run_at, payload FROM jobs WHERE kind = 'bing_sync' AND status = 'queued'",
    )
    .fetch_one(&w.pool)
    .await
    .unwrap();
    let wait = run_at - OffsetDateTime::now_utc();
    assert!(
        wait > Duration::days(6) && wait <= Duration::days(7),
        "{wait}"
    );
    assert_eq!(payload["site_id"], json!(w.site_id));
}

#[sqlx::test]
async fn sources_are_stored_for_the_ten_most_linked_targets_first_page_only(pool: PgPool) {
    let w = GoogleWorld::new(pool).await;
    w.insert_bing("connected").await;
    let rows: Vec<(String, i64)> = (1..=12)
        .map(|n| (format!("https://example.com/p{n:02}"), n))
        .collect();
    let borrowed: Vec<(&str, i64)> = rows.iter().map(|(u, c)| (u.as_str(), *c)).collect();
    w.mock_bing_counts(0, &borrowed).await;
    w.mock_bing_counts(1, &[]).await;
    for (url, _) in &rows {
        w.mock_bing_sources(url, &[("https://blog.example/from", "a link")])
            .await;
    }
    w.run_bing_sync().await.unwrap();

    assert_eq!(targets(&w).await.len(), 12);
    let stored = sources(&w).await;
    assert_eq!(stored.len(), 10);
    assert!(stored
        .iter()
        .all(|(t, _, _)| t.as_str() >= "https://example.com/p03"));
    let asked: Vec<_> = w
        .bing_requests()
        .await
        .into_iter()
        .filter(|r| r.url.path() == "/bing/GetUrlLinks")
        .collect();
    assert_eq!(asked.len(), 10);
    assert!(asked
        .iter()
        .all(|r| r.url.query_pairs().any(|(k, v)| k == "page" && v == "0")));
}

#[sqlx::test]
async fn a_second_sync_replaces_the_first(pool: PgPool) {
    let w = GoogleWorld::new(pool).await;
    w.insert_bing("connected").await;
    sqlx::query(
        "INSERT INTO backlink_targets (workspace_id, site_id, target_url, count) VALUES ($1, $2, 'https://example.com/gone', 99)",
    )
    .bind(w.workspace_id)
    .bind(w.site_id)
    .execute(&w.pool)
    .await
    .unwrap();
    w.mock_bing_counts(0, &[("https://example.com/a", 1)]).await;
    w.mock_bing_counts(1, &[]).await;
    w.mock_bing_sources("https://example.com/a", &[]).await;
    w.run_bing_sync().await.unwrap();
    assert_eq!(
        targets(&w).await,
        vec![("https://example.com/a".to_string(), 1)]
    );
}

#[sqlx::test]
async fn field_names_in_any_case_and_counts_as_text_are_understood(pool: PgPool) {
    let w = GoogleWorld::new(pool).await;
    w.insert_bing("connected").await;
    w.mock_bing(
        "GetLinkCounts",
        &[("page", "0")],
        200,
        json!({ "d": { "links": [
            { "url": "https://example.com/a", "count": "12" },
            { "URL": "https://example.com/b", "COUNT": 3.0 },
            { "Url": "https://example.com/no-count" },
            { "Count": 4 },
            "junk"
        ] } }),
    )
    .await;
    w.mock_bing("GetLinkCounts", &[("page", "1")], 200, json!({ "d": [] }))
        .await;
    w.mock_bing(
        "GetUrlLinks",
        &[],
        200,
        json!({ "d": [ { "ANCHORTEXT": "hi", "url": "https://blog.example/x" }, { "AnchorText": "no url" } ] }),
    )
    .await;
    w.run_bing_sync().await.unwrap();
    assert_eq!(
        targets(&w).await,
        vec![
            ("https://example.com/a".to_string(), 12),
            ("https://example.com/b".to_string(), 3),
            ("https://example.com/no-count".to_string(), 0),
        ]
    );
    let stored = sources(&w).await;
    assert_eq!(stored.len(), 3, "one source row per target that has it");
    assert!(stored
        .iter()
        .all(|(_, s, a)| s == "https://blog.example/x" && a == "hi"));
}

const FORMAT_ERROR: &str =
    "Bing replied in a format Margincite does not recognise. The last good data is kept.";

async fn connection_error(w: &GoogleWorld) -> Option<String> {
    sqlx::query_scalar("SELECT error FROM bing_connections WHERE site_id = $1")
        .bind(w.site_id)
        .fetch_one(&w.pool)
        .await
        .unwrap()
}

/// A world that has synced one good backlink before Bing's answer changes.
async fn world_with_stored_backlinks(pool: PgPool) -> GoogleWorld {
    let w = GoogleWorld::new(pool).await;
    w.insert_bing("connected").await;
    w.mock_bing_counts(0, &[("https://example.com/a", 5)]).await;
    w.mock_bing_counts(1, &[]).await;
    w.mock_bing_sources("https://example.com/a", &[("https://blog.example/x", "w")])
        .await;
    w.run_bing_sync().await.unwrap();
    assert_eq!(targets(&w).await.len(), 1);
    w.mock.reset().await;
    w
}

#[sqlx::test]
async fn an_unrecognised_answer_keeps_the_old_backlinks_and_fails_the_job(pool: PgPool) {
    let w = world_with_stored_backlinks(pool).await;
    let before = synced_at(&w).await;
    for body in [
        json!({ "d": "nothing here" }),
        json!({ "something": "else" }),
        json!(["plain", "strings"]),
        json!({ "d": { "Links": "not a list" } }),
        json!({ "d": [ { "Count": 4 }, { "Other": "x" } ] }),
    ] {
        w.mock.reset().await;
        w.mock_bing("GetLinkCounts", &[], 200, body.clone()).await;
        let err = w.run_bing_sync().await.unwrap_err().to_string();
        assert_eq!(err, FORMAT_ERROR, "{body}");
        assert_eq!(targets(&w).await.len(), 1, "old data kept for {body}");
        assert_eq!(sources(&w).await.len(), 1);
        assert_eq!(connection_error(&w).await.as_deref(), Some(FORMAT_ERROR));
        assert_eq!(w.bing_status().await.as_deref(), Some("connected"));
        assert_eq!(synced_at(&w).await, before, "not marked as synced");
    }
}

#[sqlx::test]
async fn an_answer_that_is_not_json_keeps_the_old_backlinks_and_fails_the_job(pool: PgPool) {
    let w = world_with_stored_backlinks(pool).await;
    wiremock::Mock::given(wiremock::matchers::method("GET"))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_string("<html>oops</html>"))
        .mount(&w.mock)
        .await;
    assert!(w.run_bing_sync().await.is_err());
    assert_eq!(targets(&w).await.len(), 1);
    assert_eq!(connection_error(&w).await.as_deref(), Some(FORMAT_ERROR));
}

#[sqlx::test]
async fn an_unrecognised_list_of_links_for_a_page_fails_the_job_too(pool: PgPool) {
    let w = world_with_stored_backlinks(pool).await;
    w.mock_bing_counts(0, &[("https://example.com/a", 5)]).await;
    w.mock_bing_counts(1, &[]).await;
    w.mock_bing(
        "GetUrlLinks",
        &[],
        200,
        json!({ "d": [ { "AnchorText": "no url" } ] }),
    )
    .await;
    assert!(w.run_bing_sync().await.is_err());
    assert_eq!(sources(&w).await.len(), 1, "the old links are kept");
    assert_eq!(connection_error(&w).await.as_deref(), Some(FORMAT_ERROR));
}

#[sqlx::test]
async fn a_genuinely_empty_answer_clears_the_backlinks_and_succeeds(pool: PgPool) {
    for body in [json!({ "d": [] }), json!({ "d": null }), json!([])] {
        let w = world_with_stored_backlinks(pool.clone()).await;
        w.mock_bing("GetLinkCounts", &[], 200, body.clone()).await;
        w.run_bing_sync().await.unwrap();
        assert_eq!(targets(&w).await, vec![], "{body}");
        assert_eq!(connection_error(&w).await, None);
        assert!(synced_at(&w).await.is_some());
        sqlx::query("DELETE FROM sites WHERE id = $1")
            .bind(w.site_id)
            .execute(&pool)
            .await
            .unwrap();
    }
}

#[sqlx::test]
async fn a_job_that_fails_for_good_on_a_changed_format_keeps_saying_so(pool: PgPool) {
    let w = GoogleWorld::new(pool).await;
    w.insert_bing("connected").await;
    w.bing_handler().on_dead(&w.bing_job(), FORMAT_ERROR).await;
    assert_eq!(connection_error(&w).await.as_deref(), Some(FORMAT_ERROR));
}

#[sqlx::test]
async fn a_server_that_ignores_the_page_number_is_not_counted_twice(pool: PgPool) {
    let w = GoogleWorld::new(pool).await;
    w.insert_bing("connected").await;
    w.mock_bing(
        "GetLinkCounts",
        &[],
        200,
        json!({ "d": [ { "Url": "https://example.com/a", "Count": 5 } ] }),
    )
    .await;
    w.mock_bing_sources("https://example.com/a", &[]).await;
    w.run_bing_sync().await.unwrap();
    assert_eq!(
        targets(&w).await,
        vec![("https://example.com/a".to_string(), 5)]
    );
    let pages = w
        .bing_requests()
        .await
        .iter()
        .filter(|r| r.url.path() == "/bing/GetLinkCounts")
        .count();
    assert_eq!(pages, 2, "it stops when a page adds nothing new");
}

#[sqlx::test]
async fn it_never_asks_for_more_than_twenty_pages(pool: PgPool) {
    let w = GoogleWorld::new(pool).await;
    w.insert_bing("connected").await;
    for page in 0..30 {
        let url = format!("https://example.com/p{page}");
        w.mock_bing_counts(page, &[(url.as_str(), 1)]).await;
        w.mock_bing_sources(&url, &[]).await;
    }
    w.run_bing_sync().await.unwrap();
    assert_eq!(targets(&w).await.len(), 20);
    let pages = w
        .bing_requests()
        .await
        .iter()
        .filter(|r| r.url.path() == "/bing/GetLinkCounts")
        .count();
    assert_eq!(pages, 20);
}

#[sqlx::test]
async fn a_refused_key_marks_needs_reconnecting_and_keeps_old_rows_out_of_sight(pool: PgPool) {
    for status in [401, 403] {
        let w = GoogleWorld::new(pool.clone()).await;
        w.insert_bing("connected").await;
        w.mock_bing("GetLinkCounts", &[], status, json!({})).await;
        w.run_bing_sync().await.unwrap();
        assert_eq!(w.bing_status().await.as_deref(), Some("needs_reconnecting"));
        let error: Option<String> =
            sqlx::query_scalar("SELECT error FROM bing_connections WHERE site_id = $1")
                .bind(w.site_id)
                .fetch_one(&w.pool)
                .await
                .unwrap();
        assert_eq!(
            error.as_deref(),
            Some("Bing no longer accepts this API key. Connect Bing Webmaster Tools again.")
        );
        assert_eq!(
            w.bing_count("jobs").await,
            0,
            "nothing is scheduled for a dead key"
        );
        sqlx::query("DELETE FROM sites WHERE id = $1")
            .bind(w.site_id)
            .execute(&pool)
            .await
            .unwrap();
    }
}

#[sqlx::test]
async fn throttling_and_server_errors_fail_the_job_so_the_queue_retries(pool: PgPool) {
    for status in [429, 500, 503] {
        let w = GoogleWorld::new(pool.clone()).await;
        w.insert_bing("connected").await;
        w.mock_bing("GetLinkCounts", &[], status, json!({})).await;
        let err = w.run_bing_sync().await.unwrap_err().to_string();
        assert!(err.contains(&status.to_string()), "{err}");
        assert!(!err.contains(BING_KEY));
        assert!(!err.contains("apikey"), "no address in the message: {err}");
        assert_eq!(w.bing_status().await.as_deref(), Some("connected"));
        assert!(synced_at(&w).await.is_none());
        sqlx::query("DELETE FROM sites WHERE id = $1")
            .bind(w.site_id)
            .execute(&pool)
            .await
            .unwrap();
    }
}

#[sqlx::test]
async fn a_failed_sources_call_fails_the_job_and_stores_nothing(pool: PgPool) {
    let w = GoogleWorld::new(pool).await;
    w.insert_bing("connected").await;
    w.mock_bing_counts(0, &[("https://example.com/a", 5)]).await;
    w.mock_bing_counts(1, &[]).await;
    w.mock_bing("GetUrlLinks", &[], 500, json!({})).await;
    assert!(w.run_bing_sync().await.is_err());
    assert_eq!(targets(&w).await, vec![]);
    assert!(synced_at(&w).await.is_none());
}

#[sqlx::test]
async fn a_job_for_a_missing_or_dead_connection_does_nothing(pool: PgPool) {
    let w = GoogleWorld::new(pool).await;
    w.run_bing_sync().await.unwrap();
    w.insert_bing("needs_reconnecting").await;
    w.run_bing_sync().await.unwrap();
    assert!(w.bing_requests().await.is_empty());
    assert_eq!(w.bing_count("jobs").await, 0);
}

#[sqlx::test]
async fn a_key_that_cannot_be_opened_marks_needs_reconnecting(pool: PgPool) {
    let w = GoogleWorld::new(pool).await;
    w.insert_bing("connected").await;
    sqlx::query("UPDATE bing_connections SET api_key_sealed = 'not sealed'")
        .execute(&w.pool)
        .await
        .unwrap();
    w.run_bing_sync().await.unwrap();
    assert_eq!(w.bing_status().await.as_deref(), Some("needs_reconnecting"));
    assert!(w.bing_requests().await.is_empty());
}

#[sqlx::test]
async fn a_sync_for_a_disconnected_site_is_not_undone_by_the_sync(pool: PgPool) {
    let w = GoogleWorld::new(pool).await;
    w.insert_bing("connected").await;
    w.mock_bing_counts(0, &[("https://example.com/a", 1)]).await;
    w.mock_bing_counts(1, &[]).await;
    w.mock_bing_sources("https://example.com/a", &[]).await;
    sqlx::query("DELETE FROM bing_connections")
        .execute(&w.pool)
        .await
        .unwrap();
    w.run_bing_sync().await.unwrap();
    assert_eq!(w.bing_count("backlink_targets").await, 0);
    assert_eq!(w.bing_count("jobs").await, 0);
}

#[sqlx::test]
async fn a_job_that_fails_for_good_records_a_plain_error_and_schedules_the_next(pool: PgPool) {
    let w = GoogleWorld::new(pool).await;
    w.insert_bing("connected").await;
    w.bing_handler()
        .on_dead(&w.bing_job(), "Bing answered 500.")
        .await;
    let error: Option<String> =
        sqlx::query_scalar("SELECT error FROM bing_connections WHERE site_id = $1")
            .bind(w.site_id)
            .fetch_one(&w.pool)
            .await
            .unwrap();
    assert_eq!(
        error.as_deref(),
        Some("The last Bing sync failed. It will be tried again, or press Sync now.")
    );
    assert_eq!(w.bing_count("jobs").await, 1);
}

#[sqlx::test]
async fn a_job_with_a_bad_payload_fails(pool: PgPool) {
    let w = GoogleWorld::new(pool).await;
    let mut job = w.bing_job();
    job.payload = json!({ "site_id": "nope" });
    assert!(w.bing_handler().run(&job).await.is_err());
    assert_eq!(w.bing_handler().kind(), "bing_sync");
}

#[sqlx::test]
async fn a_sync_that_finds_no_secret_key_is_put_off_a_week_not_dropped(pool: PgPool) {
    use std::sync::Arc;

    let w = GoogleWorld::new(pool).await;
    w.insert_bing("connected").await;
    let config = Arc::new(common::test_config(&[]));
    assert!(config.secret_key.is_none());
    let client = Arc::new(margincite::integrations::bing::BingClient::new(
        &common::google::bing_base(&w.mock),
    ));
    let handler =
        margincite::integrations::bing::sync::BingSyncJob::new(w.pool.clone(), config, client);
    handler.run(&w.bing_job()).await.unwrap();
    assert!(w.bing_requests().await.is_empty());
    let wait: f64 = sqlx::query_scalar(
        "SELECT EXTRACT(EPOCH FROM (run_at - now()))::float8 FROM jobs WHERE kind = 'bing_sync' AND status = 'queued'",
    )
    .fetch_one(&w.pool)
    .await
    .unwrap();
    let days = wait / 86_400.0;
    assert!((6.9..7.1).contains(&days), "{days}");
}
