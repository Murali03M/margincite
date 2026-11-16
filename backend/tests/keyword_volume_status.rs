//! The daily allowance for refreshing search volume, what a refresh reports
//! back, and the rule that a retried job does not pay twice.

mod common;

use common::dataforseo::{self as df, ads_item, VOLUME_PATH};
use common::google::GoogleWorld;
use margincite::integrations::dataforseo::DataForSeoClient;
use margincite::jobs::queue::{self, Job};
use margincite::jobs::worker::JobHandler;
use margincite::keywords::volumes::KeywordVolumesJob;
use serde_json::{json, Value};
use sqlx::PgPool;
use std::sync::Arc;
use time::OffsetDateTime;
use wiremock::matchers::{method, path};
use wiremock::{Mock, ResponseTemplate};

const SECRET_MARKER: &str = "body-secret-marker-31337";

async fn world(pool: PgPool) -> GoogleWorld {
    let w = GoogleWorld::new(pool).await;
    w.save_dataforseo().await;
    w.add_keyword("blue widgets").await;
    w
}

fn volumes(w: &GoogleWorld) -> String {
    format!("/api/v1/sites/{}/keywords/volumes", w.site_id)
}

async fn status(w: &GoogleWorld) -> Value {
    let (code, body) = w.get(&volumes(w)).await;
    assert_eq!(code, 200, "{body}");
    body["data"].clone()
}

async fn mock(w: &GoogleWorld, code: u16, body: Value) {
    w.mock.reset().await;
    Mock::given(method("POST"))
        .and(path(VOLUME_PATH))
        .respond_with(ResponseTemplate::new(code).set_body_json(body))
        .mount(&w.mock)
        .await;
}

fn job_row(w: &GoogleWorld, id: uuid::Uuid, payload: Value) -> (KeywordVolumesJob, Job) {
    let config = Arc::new(common::test_config(common::GOOGLE_SETTINGS));
    let client = Arc::new(DataForSeoClient::new(&common::rankings::dataforseo_base(
        &w.mock,
    )));
    let job = Job {
        id,
        kind: "keyword_volumes".into(),
        payload,
        attempts: 1,
    };
    (KeywordVolumesJob::new(w.pool.clone(), config, client), job)
}

fn plain_job(w: &GoogleWorld) -> (KeywordVolumesJob, Job) {
    job_row(
        w,
        uuid::Uuid::new_v4(),
        json!({ "workspace_id": w.workspace_id, "site_id": w.site_id }),
    )
}

async fn finish_jobs(w: &GoogleWorld) {
    sqlx::query("UPDATE jobs SET status = 'done' WHERE kind = 'keyword_volumes'")
        .execute(&w.pool)
        .await
        .unwrap();
}

async fn volume_requests(w: &GoogleWorld) -> usize {
    w.requests_to(VOLUME_PATH).await.len()
}

// --------------------------------------------------------------- allowance

#[sqlx::test]
async fn the_sixth_refresh_of_a_day_is_429_and_queues_nothing(pool: PgPool) {
    let w = world(pool).await;
    for n in 1..=5 {
        let (code, body) = w.post(&volumes(&w)).await;
        assert_eq!(code, 202, "refresh {n}: {body}");
        finish_jobs(&w).await;
    }
    let (code, body) = w.post(&volumes(&w)).await;
    assert_eq!(code, 429, "{body}");
    assert_eq!(body["success"], false);
    assert_eq!(
        body["error"],
        "You can refresh search volume 5 times a day for each site. Try again tomorrow."
    );
    let queued: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM jobs WHERE kind = 'keyword_volumes' AND status = 'queued'",
    )
    .fetch_one(&w.pool)
    .await
    .unwrap();
    assert_eq!(queued, 0);
}

#[sqlx::test]
async fn asking_again_while_a_refresh_waits_uses_no_allowance(pool: PgPool) {
    let w = world(pool).await;
    for _ in 0..8 {
        let (code, _) = w.post(&volumes(&w)).await;
        assert_eq!(code, 202);
    }
    let used: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM keyword_volume_requests")
        .fetch_one(&w.pool)
        .await
        .unwrap();
    assert_eq!(used, 1);
}

#[sqlx::test]
async fn the_allowance_is_per_site_and_counts_only_today(pool: PgPool) {
    let w = world(pool).await;
    for _ in 0..5 {
        assert_eq!(w.post(&volumes(&w)).await.0, 202);
        finish_jobs(&w).await;
    }
    assert_eq!(w.post(&volumes(&w)).await.0, 429);
    // Another site of the same workspace has its own five.
    let other = common::google::add_site(&w.pool, w.workspace_id, "https://other.example/").await;
    sqlx::query("INSERT INTO site_keywords (id, workspace_id, site_id, keyword) VALUES (gen_random_uuid(), $1, $2, 'k')")
        .bind(w.workspace_id)
        .bind(other)
        .execute(&w.pool)
        .await
        .unwrap();
    let (code, _) = w
        .post(&format!("/api/v1/sites/{other}/keywords/volumes"))
        .await;
    assert_eq!(code, 202);
    // Yesterday's refreshes do not count.
    sqlx::query("UPDATE keyword_volume_requests SET created_at = now() - interval '2 days'")
        .execute(&w.pool)
        .await
        .unwrap();
    assert_eq!(w.post(&volumes(&w)).await.0, 202);
}

// ------------------------------------------------------------------ status

#[sqlx::test]
async fn a_site_never_refreshed_is_idle_with_no_date(pool: PgPool) {
    let w = world(pool).await;
    assert_eq!(
        status(&w).await,
        json!({ "status": "idle", "checked_at": null, "error": null })
    );
}

#[sqlx::test]
async fn a_waiting_refresh_is_queued_and_a_started_one_is_running(pool: PgPool) {
    let w = world(pool).await;
    assert_eq!(w.post(&volumes(&w)).await.0, 202);
    assert_eq!(status(&w).await["status"], "queued");
    sqlx::query("UPDATE jobs SET status = 'running' WHERE kind = 'keyword_volumes'")
        .execute(&w.pool)
        .await
        .unwrap();
    let running = status(&w).await;
    assert_eq!(running["status"], "running");
    assert!(running["error"].is_null());
}

#[sqlx::test]
async fn a_stored_refresh_is_idle_with_the_date_it_was_stored(pool: PgPool) {
    let w = world(pool).await;
    mock(
        &w,
        200,
        df::answer(json!([ads_item("blue widgets", Some(10), None, None)])),
    )
    .await;
    let (job, queued) = plain_job(&w);
    job.run(&queued).await.unwrap();
    let done = status(&w).await;
    assert_eq!(done["status"], "idle");
    assert!(done["error"].is_null());
    let at = OffsetDateTime::parse(
        done["checked_at"].as_str().unwrap(),
        &time::format_description::well_known::Rfc3339,
    )
    .unwrap();
    assert!((OffsetDateTime::now_utc() - at).whole_seconds().abs() < 60);
}

#[sqlx::test]
async fn a_refused_login_or_used_up_balance_is_failed_in_fixed_words(pool: PgPool) {
    let w = world(pool).await;
    for (code, sentence) in [
        (
            401,
            "DataForSEO rejected the login or password. Check them in Settings.",
        ),
        (402, "Your DataForSEO balance has run out."),
    ] {
        mock(&w, code, json!({ "status_message": SECRET_MARKER })).await;
        let (job, queued) = plain_job(&w);
        job.run(&queued).await.unwrap();
        let failed = status(&w).await;
        assert_eq!(failed["status"], "failed", "{code}");
        assert_eq!(failed["error"], sentence);
        assert!(failed["checked_at"].is_null());
        assert!(!failed.to_string().contains(SECRET_MARKER));
    }
}

#[sqlx::test]
async fn a_failure_keeps_the_date_of_the_last_good_refresh(pool: PgPool) {
    let w = world(pool).await;
    mock(
        &w,
        200,
        df::answer(json!([ads_item("blue widgets", Some(10), None, None)])),
    )
    .await;
    let (job, queued) = plain_job(&w);
    job.run(&queued).await.unwrap();
    let good = status(&w).await["checked_at"].clone();
    mock(&w, 402, json!({})).await;
    job.run(&queued).await.unwrap();
    let failed = status(&w).await;
    assert_eq!(failed["status"], "failed");
    assert_eq!(failed["checked_at"], good);
}

#[sqlx::test]
async fn queueing_again_clears_the_failure_and_a_good_run_keeps_it_cleared(pool: PgPool) {
    let w = world(pool).await;
    mock(&w, 402, json!({})).await;
    let (job, queued) = plain_job(&w);
    job.run(&queued).await.unwrap();
    assert_eq!(status(&w).await["status"], "failed");
    assert_eq!(w.post(&volumes(&w)).await.0, 202);
    let waiting = status(&w).await;
    assert_eq!(waiting["status"], "queued");
    assert!(waiting["error"].is_null());
    finish_jobs(&w).await;
    mock(
        &w,
        200,
        df::answer(json!([ads_item("blue widgets", Some(1), None, None)])),
    )
    .await;
    job.run(&queued).await.unwrap();
    assert_eq!(status(&w).await["status"], "idle");
}

#[sqlx::test]
async fn a_job_that_ends_without_asking_is_failed_too(pool: PgPool) {
    let w = GoogleWorld::new(pool).await;
    w.add_keyword("a").await;
    let (job, queued) = plain_job(&w);
    job.run(&queued).await.unwrap();
    let failed = status(&w).await;
    assert_eq!(failed["status"], "failed");
    assert_eq!(
        failed["error"],
        "The DataForSEO login is no longer set up. Add it in Settings, then try again."
    );
}

#[sqlx::test]
async fn a_job_that_failed_for_good_says_so(pool: PgPool) {
    let w = world(pool).await;
    let (job, queued) = plain_job(&w);
    job.on_dead(&queued, "relation x does not exist (internal detail)")
        .await;
    let failed = status(&w).await;
    assert_eq!(failed["status"], "failed");
    assert_eq!(
        failed["error"],
        "The refresh stopped because of an internal error. Try again."
    );
}

#[sqlx::test]
async fn another_workspace_gets_not_found(pool: PgPool) {
    let w = world(pool).await;
    let (code, _) = w
        .send("other-token", reqwest::Method::GET, &volumes(&w), None)
        .await;
    assert_eq!(code, 404);
}

// ------------------------------------------------------- no paying twice

#[sqlx::test]
async fn a_retry_after_dataforseo_answered_stores_the_answer_without_asking_again(pool: PgPool) {
    let w = world(pool).await;
    let id = queue::enqueue(
        &w.pool,
        "keyword_volumes",
        &json!({ "workspace_id": w.workspace_id, "site_id": w.site_id }),
        OffsetDateTime::now_utc(),
    )
    .await
    .unwrap();
    mock(
        &w,
        200,
        df::answer(json!([ads_item("blue widgets", Some(42), None, None)])),
    )
    .await;
    let (job, first) = job_row(
        &w,
        id,
        json!({ "workspace_id": w.workspace_id, "site_id": w.site_id }),
    );
    job.run(&first).await.unwrap();
    assert_eq!(volume_requests(&w).await, 1);
    // The queue hands a retry the stored payload, which now holds the answer.
    let payload: Value = sqlx::query_scalar("SELECT payload FROM jobs WHERE id = $1")
        .bind(id)
        .fetch_one(&w.pool)
        .await
        .unwrap();
    assert!(payload.get("answer").is_some(), "{payload}");
    sqlx::query("UPDATE site_keywords SET search_volume = NULL, metrics_checked_at = NULL")
        .execute(&w.pool)
        .await
        .unwrap();
    let (job, retry) = job_row(&w, id, payload);
    job.run(&retry).await.unwrap();
    assert_eq!(volume_requests(&w).await, 1, "no second paid request");
    let stored: Option<i32> = sqlx::query_scalar("SELECT search_volume FROM site_keywords")
        .fetch_one(&w.pool)
        .await
        .unwrap();
    assert_eq!(stored, Some(42));
    assert_eq!(w.usage_rows().await.len(), 1, "and no second charge");
}
