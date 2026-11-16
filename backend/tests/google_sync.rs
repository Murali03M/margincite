mod common;

use std::sync::Arc;

use common::google::{GoogleWorld, ACCESS_TOKEN, REFRESH_TOKEN};
use margincite::integrations::google::sync::{self, GoogleSyncJob};
use margincite::integrations::google::GoogleClient;
use margincite::jobs::queue::{self, Job};
use margincite::jobs::worker::JobHandler;
use serde_json::{json, Value};
use sqlx::PgPool;
use time::{Duration, OffsetDateTime};
use wiremock::matchers::{method, path};
use wiremock::{Mock, ResponseTemplate};

const PROPERTY: &str = "sc-domain:example.com";

fn handler(w: &GoogleWorld) -> GoogleSyncJob {
    let config = Arc::new(common::test_config(common::GOOGLE_SETTINGS));
    let hosts = common::google::hosts(&w.mock);
    GoogleSyncJob::new(w.pool.clone(), config, Arc::new(GoogleClient::new(hosts)))
}

fn job(w: &GoogleWorld) -> Job {
    Job {
        id: uuid::Uuid::new_v4(),
        kind: "google_sync".into(),
        payload: json!({ "workspace_id": w.workspace_id, "site_id": w.site_id }),
        attempts: 1,
    }
}

async fn run(w: &GoogleWorld) -> anyhow::Result<()> {
    handler(w).run(&job(w)).await
}

fn dates_back(days: &[i64]) -> Vec<String> {
    let end = OffsetDateTime::now_utc().date() - Duration::days(3);
    let fmt = time::macros::format_description!("[year]-[month]-[day]");
    days.iter()
        .map(|d| (end - Duration::days(*d)).format(&fmt).unwrap())
        .collect()
}

async fn mock_happy(w: &GoogleWorld) {
    w.mock_tokens().await;
    // 0 and 1 days before the end are in the current window; 30 days back is
    // in the previous window.
    let days = dates_back(&[0, 1, 30]);
    w.mock_reports(
        json!([
            {"keys": ["blue widgets", "https://Example.com/a?utm_source=x"], "clicks": 12.0, "impressions": 400.0, "ctr": 0.03, "position": 4.5},
            {"keys": ["red widgets", "https://example.com/b"], "clicks": 3.0, "impressions": 60.0, "ctr": 0.05, "position": 9.0}
        ]),
        json!([
            {"keys": [days[0]], "clicks": 10.0, "impressions": 300.0, "ctr": 0.03, "position": 5.0},
            {"keys": [days[1]], "clicks": 5.0, "impressions": 160.0, "ctr": 0.03, "position": 5.0},
            {"keys": [days[2]], "clicks": 7.0, "impressions": 70.0, "ctr": 0.1, "position": 5.0}
        ]),
        json!([
            {"keys": ["blue widgets"], "clicks": 7.0, "impressions": 70.0, "ctr": 0.1, "position": 5.0},
            {"keys": ["old query"], "clicks": 0.0, "impressions": 5.0, "ctr": 0.0, "position": 30.0}
        ]),
    )
    .await;
}

#[sqlx::test]
async fn a_sync_stores_rows_normalised_and_replaces_the_previous_sync(pool: PgPool) {
    let w = GoogleWorld::new(pool).await;
    w.connect(PROPERTY).await;
    w.seed_rows(&[("stale query", "https://example.com/stale", 9, 99, 1.0)])
        .await;
    mock_happy(&w).await;
    run(&w).await.unwrap();

    let rows: Vec<(String, String, i64, i64, f64, f64)> = sqlx::query_as(
        "SELECT query, page, clicks, impressions, ctr, position FROM search_rows WHERE site_id = $1 ORDER BY query",
    )
    .bind(w.site_id)
    .fetch_all(&w.pool)
    .await
    .unwrap();
    assert_eq!(
        rows,
        vec![
            (
                "blue widgets".into(),
                "https://example.com/a".into(),
                12,
                400,
                0.03,
                4.5
            ),
            (
                "red widgets".into(),
                "https://example.com/b".into(),
                3,
                60,
                0.05,
                9.0
            ),
        ]
    );
    assert_eq!(w.count("search_daily").await, 3);
    let previous: (i64, i64, i64) = sqlx::query_as(
        "SELECT clicks, impressions, keywords FROM search_previous WHERE site_id = $1",
    )
    .bind(w.site_id)
    .fetch_one(&w.pool)
    .await
    .unwrap();
    assert_eq!(previous, (7, 70, 2));
    let (synced, error): (Option<OffsetDateTime>, Option<String>) =
        sqlx::query_as("SELECT synced_at, error FROM google_connections WHERE site_id = $1")
            .bind(w.site_id)
            .fetch_one(&w.pool)
            .await
            .unwrap();
    assert!(synced.is_some() && error.is_none());

    // A second sync replaces rather than adds.
    run(&w).await.unwrap();
    assert_eq!(w.count("search_rows").await, 2);
    assert_eq!(w.count("search_daily").await, 3);
    assert_eq!(w.count("search_previous").await, 1);
}

#[sqlx::test]
async fn a_sync_asks_google_for_final_data_with_a_row_limit(pool: PgPool) {
    let w = GoogleWorld::new(pool).await;
    w.connect("https://example.com/").await;
    mock_happy(&w).await;
    run(&w).await.unwrap();

    let reports = w.report_requests().await;
    assert_eq!(reports.len(), 3);
    let dims: Vec<Value> = reports.iter().map(|r| r["dimensions"].clone()).collect();
    assert_eq!(
        dims,
        [json!(["query", "page"]), json!(["date"]), json!(["query"])]
    );
    for report in &reports {
        assert_eq!(report["rowLimit"], 25_000);
        assert_eq!(report["dataState"], "final");
    }
    // The property is one encoded path segment.
    let asked = w.mock.received_requests().await.unwrap();
    let report = asked
        .iter()
        .find(|r| r.url.path().ends_with("/searchAnalytics/query"))
        .unwrap();
    assert_eq!(
        report.url.path(),
        "/webmasters/v3/sites/https%3A%2F%2Fexample.com%2F/searchAnalytics/query"
    );
    assert_eq!(
        report
            .headers
            .get("authorization")
            .unwrap()
            .to_str()
            .unwrap(),
        format!("Bearer {ACCESS_TOKEN}")
    );
    // The window ends three days ago and spans 28 days.
    let end = dates_back(&[0])[0].clone();
    let start = dates_back(&[27])[0].clone();
    assert_eq!(reports[0]["endDate"], end);
    assert_eq!(reports[0]["startDate"], start);
    assert_eq!(reports[1]["startDate"], dates_back(&[89])[0]);
}

#[sqlx::test]
async fn a_sync_with_no_data_stores_zero_rows_and_succeeds(pool: PgPool) {
    let w = GoogleWorld::new(pool).await;
    w.connect(PROPERTY).await;
    w.mock_tokens().await;
    // Google leaves `rows` out when there is nothing to report.
    Mock::given(method("POST"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({"responseAggregationType": "auto"})),
        )
        .mount(&w.mock)
        .await;
    run(&w).await.unwrap();
    assert_eq!(w.count("search_rows").await, 0);
    let synced: Option<OffsetDateTime> =
        sqlx::query_scalar("SELECT synced_at FROM google_connections WHERE site_id = $1")
            .bind(w.site_id)
            .fetch_one(&w.pool)
            .await
            .unwrap();
    assert!(synced.is_some());
}

#[sqlx::test]
async fn a_sync_queues_the_next_daily_one(pool: PgPool) {
    let w = GoogleWorld::new(pool).await;
    w.connect(PROPERTY).await;
    mock_happy(&w).await;
    run(&w).await.unwrap();
    let run_at: OffsetDateTime = sqlx::query_scalar(
        "SELECT run_at FROM jobs WHERE kind = 'google_sync' AND status = 'queued'",
    )
    .fetch_one(&w.pool)
    .await
    .unwrap();
    let wait = run_at - OffsetDateTime::now_utc();
    assert!(
        wait > Duration::hours(23) && wait <= Duration::hours(24),
        "{wait}"
    );
    // Sync now pulls that queued run forward instead of adding another.
    let (status, _) = w.post(&w.google_path("/sync")).await;
    assert_eq!(status, 202);
    assert_eq!(w.count("jobs").await, 1);
    let run_at: OffsetDateTime = sqlx::query_scalar(
        "SELECT run_at FROM jobs WHERE kind = 'google_sync' AND status = 'queued'",
    )
    .fetch_one(&w.pool)
    .await
    .unwrap();
    assert!(run_at <= OffsetDateTime::now_utc());
}

#[sqlx::test]
async fn throttling_and_server_errors_fail_the_job_so_the_queue_retries(pool: PgPool) {
    for answer in [429, 500, 503] {
        let w = GoogleWorld::new(pool.clone()).await;
        w.connect(PROPERTY).await;
        w.mock_tokens().await;
        Mock::given(method("POST"))
            .and(path_regex_report())
            .respond_with(ResponseTemplate::new(answer).set_body_string("secret-looking body"))
            .mount(&w.mock)
            .await;
        let err = run(&w).await.unwrap_err();
        assert!(err.to_string().contains(&answer.to_string()), "{err}");
        assert!(!err.to_string().contains("secret-looking"));
        assert_eq!(
            w.connection_status().await.as_deref(),
            Some("connected"),
            "{answer}"
        );
        assert_eq!(w.count("search_rows").await, 0);
        sqlx::query("DELETE FROM sites WHERE id = $1")
            .bind(w.site_id)
            .execute(&pool)
            .await
            .unwrap();
    }
}

fn path_regex_report() -> wiremock::matchers::PathRegexMatcher {
    wiremock::matchers::path_regex(r"/searchAnalytics/query$")
}

#[sqlx::test]
async fn an_unreadable_answer_fails_the_job_for_a_retry(pool: PgPool) {
    let w = GoogleWorld::new(pool).await;
    w.connect(PROPERTY).await;
    w.mock_tokens().await;
    Mock::given(method("POST"))
        .and(path_regex_report())
        .respond_with(ResponseTemplate::new(200).set_body_string("<html>not json</html>"))
        .mount(&w.mock)
        .await;
    assert!(run(&w).await.is_err());
    assert_eq!(w.connection_status().await.as_deref(), Some("connected"));
}

#[sqlx::test]
async fn a_token_answer_of_the_wrong_shape_fails_the_job(pool: PgPool) {
    let w = GoogleWorld::new(pool).await;
    w.connect(PROPERTY).await;
    Mock::given(method("POST"))
        .and(path("/token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"unexpected": true})))
        .mount(&w.mock)
        .await;
    assert!(run(&w).await.is_err());
    assert_eq!(w.connection_status().await.as_deref(), Some("connected"));
}

#[sqlx::test]
async fn google_being_unreachable_fails_the_job(pool: PgPool) {
    let w = GoogleWorld::new(pool).await;
    w.connect(PROPERTY).await;
    let config = Arc::new(common::test_config(common::GOOGLE_SETTINGS));
    let dead = margincite::integrations::google::GoogleHosts {
        auth_url: "http://127.0.0.1:9/auth".into(),
        token_url: "http://127.0.0.1:9/token".into(),
        api_base: "http://127.0.0.1:9/v3".into(),
    };
    let job_handler = GoogleSyncJob::new(w.pool.clone(), config, Arc::new(GoogleClient::new(dead)));
    assert!(job_handler.run(&job(&w)).await.is_err());
    assert_eq!(w.connection_status().await.as_deref(), Some("connected"));
}

#[sqlx::test]
async fn invalid_grant_marks_the_connection_needs_reconnecting_and_ends_the_job(pool: PgPool) {
    let w = GoogleWorld::new(pool).await;
    w.connect(PROPERTY).await;
    w.seed_rows(&[("kept", "https://example.com/a", 1, 10, 3.0)])
        .await;
    Mock::given(method("POST"))
        .and(path("/token"))
        .respond_with(ResponseTemplate::new(400).set_body_json(json!({
            "error": "invalid_grant", "error_description": "Token has been expired or revoked."
        })))
        .mount(&w.mock)
        .await;
    run(&w).await.unwrap();
    assert_eq!(
        w.connection_status().await.as_deref(),
        Some("needs_reconnecting")
    );
    let error: String =
        sqlx::query_scalar("SELECT error FROM google_connections WHERE site_id = $1")
            .bind(w.site_id)
            .fetch_one(&w.pool)
            .await
            .unwrap();
    assert!(error.contains("Connect Google Search Console again"));
    assert!(!error.contains("invalid_grant"));
    assert_eq!(
        w.count("search_rows").await,
        1,
        "old rows stay until the person reconnects"
    );
    assert!(w.report_requests().await.is_empty());
}

#[sqlx::test]
async fn invalid_client_blames_the_server_credentials_not_the_connection(pool: PgPool) {
    let w = GoogleWorld::new(pool).await;
    w.connect(PROPERTY).await;
    w.seed_rows(&[("kept", "https://example.com/a", 1, 10, 3.0)])
        .await;
    Mock::given(method("POST"))
        .and(path("/token"))
        .respond_with(ResponseTemplate::new(401).set_body_json(json!({
            "error": "invalid_client", "error_description": "The OAuth client was not found."
        })))
        .mount(&w.mock)
        .await;
    run(&w).await.unwrap();
    assert_eq!(w.connection_status().await.as_deref(), Some("connected"));
    let error: String =
        sqlx::query_scalar("SELECT error FROM google_connections WHERE site_id = $1")
            .bind(w.site_id)
            .fetch_one(&w.pool)
            .await
            .unwrap();
    assert!(error.contains("rejected"), "{error}");
    assert!(error.contains("GOOGLE_CLIENT_ID") && error.contains("GOOGLE_CLIENT_SECRET"));
    assert!(!error.contains("invalid_client"));
    assert_eq!(w.count("search_rows").await, 1);
    assert_eq!(
        w.count("jobs").await,
        1,
        "the next daily sync is still queued"
    );
}

#[sqlx::test]
async fn a_refused_property_marks_the_connection_needs_reconnecting(pool: PgPool) {
    for answer in [401, 403] {
        let w = GoogleWorld::new(pool.clone()).await;
        w.connect(PROPERTY).await;
        w.mock_tokens().await;
        Mock::given(method("POST"))
            .and(path_regex_report())
            .respond_with(
                ResponseTemplate::new(answer).set_body_json(json!({"error": {"message": "nope"}})),
            )
            .mount(&w.mock)
            .await;
        run(&w).await.unwrap();
        assert_eq!(
            w.connection_status().await.as_deref(),
            Some("needs_reconnecting"),
            "{answer}"
        );
        sqlx::query("DELETE FROM sites WHERE id = $1")
            .bind(w.site_id)
            .execute(&pool)
            .await
            .unwrap();
    }
}

#[sqlx::test]
async fn a_sync_with_nothing_to_do_is_a_quiet_success(pool: PgPool) {
    let w = GoogleWorld::new(pool).await;
    // No connection at all, then one still choosing a property, then a dead one.
    run(&w).await.unwrap();
    w.insert_connection(None, "choose_property").await;
    run(&w).await.unwrap();
    w.insert_connection(Some(PROPERTY), "needs_reconnecting")
        .await;
    run(&w).await.unwrap();
    assert!(w.mock.received_requests().await.unwrap().is_empty());
}

#[sqlx::test]
async fn a_sync_without_google_set_up_does_nothing(pool: PgPool) {
    let w = GoogleWorld::new(pool).await;
    w.connect(PROPERTY).await;
    let config = Arc::new(common::test_config(&[]));
    let hosts = common::google::hosts(&w.mock);
    let job_handler =
        GoogleSyncJob::new(w.pool.clone(), config, Arc::new(GoogleClient::new(hosts)));
    job_handler.run(&job(&w)).await.unwrap();
    assert!(w.mock.received_requests().await.unwrap().is_empty());
}

#[sqlx::test]
async fn a_token_sealed_with_another_key_needs_reconnecting(pool: PgPool) {
    let w = GoogleWorld::new(pool).await;
    w.connect(PROPERTY).await;
    let other_key = "CAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAg=";
    let config = Arc::new(common::test_config(&[
        ("MARGINCITE_SECRET_KEY", other_key),
        ("GOOGLE_CLIENT_ID", "id"),
        ("GOOGLE_CLIENT_SECRET", "secret"),
    ]));
    let hosts = common::google::hosts(&w.mock);
    let job_handler =
        GoogleSyncJob::new(w.pool.clone(), config, Arc::new(GoogleClient::new(hosts)));
    job_handler.run(&job(&w)).await.unwrap();
    assert_eq!(
        w.connection_status().await.as_deref(),
        Some("needs_reconnecting")
    );
}

#[sqlx::test]
async fn a_disconnect_during_a_sync_is_not_undone_by_it(pool: PgPool) {
    let w = GoogleWorld::new(pool).await;
    w.connect(PROPERTY).await;
    mock_happy(&w).await;
    // The connection disappears between the fetch and the store.
    let data = margincite::integrations::google::repo::SyncData {
        rows: vec![],
        daily: vec![],
        previous: Default::default(),
    };
    sqlx::query("DELETE FROM google_connections WHERE site_id = $1")
        .bind(w.site_id)
        .execute(&w.pool)
        .await
        .unwrap();
    let stored = margincite::integrations::google::repo::replace_data(
        &w.pool,
        w.workspace_id,
        w.site_id,
        &data,
    )
    .await
    .unwrap();
    assert!(!stored);
    assert_eq!(w.count("search_previous").await, 0);
}

#[sqlx::test]
async fn a_dead_job_records_a_plain_sentence_and_schedules_the_next_one(pool: PgPool) {
    let w = GoogleWorld::new(pool).await;
    w.connect(PROPERTY).await;
    handler(&w).on_dead(&job(&w), "Google answered 503.").await;
    let error: String =
        sqlx::query_scalar("SELECT error FROM google_connections WHERE site_id = $1")
            .bind(w.site_id)
            .fetch_one(&w.pool)
            .await
            .unwrap();
    assert!(error.contains("last sync failed"));
    assert_eq!(w.count("jobs").await, 1);
}

#[sqlx::test]
async fn a_sync_runs_the_search_checks_on_the_latest_crawl(pool: PgPool) {
    let w = GoogleWorld::new(pool).await;
    w.crawl(&[
        ("https://example.com/a", true),
        ("https://example.com/b", true),
    ])
    .await;
    w.add_keyword("blue widgets").await;
    w.connect(PROPERTY).await;
    w.mock_tokens().await;
    // Two pages share a keyword's impressions, one of them far down.
    w.mock_reports(
        json!([
            {"keys": ["blue widgets", "https://example.com/a"], "clicks": 1.0, "impressions": 500.0, "ctr": 0.002, "position": 9.0},
            {"keys": ["blue widgets", "https://example.com/b"], "clicks": 1.0, "impressions": 400.0, "ctr": 0.002, "position": 12.0}
        ]),
        json!([]),
        json!([]),
    )
    .await;
    run(&w).await.unwrap();
    let found: Vec<(String, String)> =
        sqlx::query_as("SELECT s.check_id, i.target FROM suggestions s JOIN suggestion_items i ON i.suggestion_id = s.id WHERE s.site_id = $1 AND i.status = 'open' AND (s.check_id LIKE 'search\\_%' OR s.check_id = 'keyword_cannibalization') ORDER BY 1, 2")
            .bind(w.site_id)
            .fetch_all(&w.pool)
            .await
            .unwrap();
    let ids: Vec<&str> = found.iter().map(|(id, _)| id.as_str()).collect();
    assert!(ids.contains(&"keyword_cannibalization"), "{found:?}");
    assert!(ids.contains(&"search_low_ctr"), "{found:?}");
    assert_eq!(
        ids.iter()
            .filter(|id| **id == "search_striking_distance")
            .count(),
        2,
        "{found:?}"
    );
}

// ---------------------------------------------------------------- the queue helper

#[sqlx::test]
async fn site_jobs_queue_one_at_a_time_and_a_sooner_request_pulls_the_job_forward(pool: PgPool) {
    let w = GoogleWorld::new(pool).await;
    let later = OffsetDateTime::now_utc() + Duration::hours(5);
    let sooner = OffsetDateTime::now_utc() + Duration::hours(1);
    let queue_at = |kind: &'static str, at| {
        let pool = w.pool.clone();
        let (ws, site) = (w.workspace_id, w.site_id);
        async move {
            queue::enqueue_site_job_unless_queued(&pool, kind, ws, site, at)
                .await
                .unwrap()
        }
    };
    assert!(queue_at("google_sync", later).await);
    assert!(!queue_at("google_sync", later).await);
    assert!(!queue_at("google_sync", sooner).await);
    let run_at: OffsetDateTime =
        sqlx::query_scalar("SELECT run_at FROM jobs WHERE kind = 'google_sync'")
            .fetch_one(&w.pool)
            .await
            .unwrap();
    assert!((run_at - sooner).abs() < Duration::seconds(1));
    // A later request never pushes it back.
    assert!(!queue_at("google_sync", later).await);
    let run_at2: OffsetDateTime =
        sqlx::query_scalar("SELECT run_at FROM jobs WHERE kind = 'google_sync'")
            .fetch_one(&w.pool)
            .await
            .unwrap();
    assert_eq!(run_at, run_at2);
    // Another kind for the same site is its own job.
    assert!(queue_at("bing_sync", later).await);
    // A job that is running does not block queueing the next one.
    sqlx::query("UPDATE jobs SET status = 'running' WHERE kind = 'google_sync'")
        .execute(&w.pool)
        .await
        .unwrap();
    assert!(queue_at("google_sync", later).await);
}

#[sqlx::test]
async fn two_callers_at_once_still_queue_one_job(pool: PgPool) {
    let w = GoogleWorld::new(pool).await;
    let now = OffsetDateTime::now_utc();
    let attempts = (0..8).map(|_| {
        let pool = w.pool.clone();
        let (ws, site) = (w.workspace_id, w.site_id);
        tokio::spawn(async move {
            queue::enqueue_site_job_unless_queued(&pool, "google_sync", ws, site, now)
                .await
                .unwrap()
        })
    });
    let mut queued = 0;
    for attempt in attempts {
        if attempt.await.unwrap() {
            queued += 1;
        }
    }
    assert_eq!(queued, 1);
    assert_eq!(w.count("jobs").await, 1);
}

#[sqlx::test]
async fn after_a_crawl_only_connected_sites_sync(pool: PgPool) {
    let w = GoogleWorld::new(pool).await;
    sync::enqueue_after_crawl(&w.pool, w.workspace_id, w.site_id).await;
    assert_eq!(w.count("jobs").await, 0);
    w.insert_connection(None, "choose_property").await;
    sync::enqueue_after_crawl(&w.pool, w.workspace_id, w.site_id).await;
    assert_eq!(w.count("jobs").await, 0);
    w.connect(PROPERTY).await;
    sync::enqueue_after_crawl(&w.pool, w.workspace_id, w.site_id).await;
    sync::enqueue_after_crawl(&w.pool, w.workspace_id, w.site_id).await;
    assert_eq!(w.count("jobs").await, 1);
}

#[test]
fn the_token_is_not_logged_by_debug_output() {
    let hosts = margincite::integrations::google::GoogleHosts::production();
    let text = format!("{hosts:?}");
    assert!(!text.contains(REFRESH_TOKEN));
    assert!(hosts
        .token_url
        .starts_with("https://oauth2.googleapis.com/"));
    assert!(hosts.auth_url.starts_with("https://accounts.google.com/"));
    assert!(hosts
        .api_base
        .starts_with("https://www.googleapis.com/webmasters/v3"));
}
