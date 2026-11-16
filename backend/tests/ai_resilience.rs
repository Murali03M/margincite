//! A scheduled AI run that is retried, or whose job dies, never stays stuck.

mod common;

use common::ai::Q1;
use common::google::GoogleWorld;
use margincite::jobs::queue::{self, Job};
use margincite::jobs::worker::JobHandler;
use serde_json::json;
use sqlx::PgPool;
use time::OffsetDateTime;
use uuid::Uuid;

async fn sql(pool: &PgPool, text: &'static str) {
    sqlx::query(text).execute(pool).await.unwrap();
}

/// Makes every answer insert fail until `heal` is called.
async fn break_answers(pool: &PgPool) {
    sql(
        pool,
        "CREATE FUNCTION fail_answers() RETURNS trigger AS $$ BEGIN RAISE EXCEPTION 'disk full'; END $$ LANGUAGE plpgsql",
    )
    .await;
    sql(
        pool,
        "CREATE TRIGGER fail_answers BEFORE INSERT ON ai_answers FOR EACH ROW EXECUTE FUNCTION fail_answers()",
    )
    .await;
}

async fn heal(pool: &PgPool) {
    sql(pool, "DROP TRIGGER fail_answers ON ai_answers").await;
}

/// A scheduled job as the queue holds it, claimed by a worker.
async fn claimed_scheduled_job(w: &GoogleWorld) -> Job {
    let payload = json!({ "workspace_id": w.workspace_id, "site_id": w.site_id });
    queue::enqueue(&w.pool, "ai_run", &payload, OffsetDateTime::now_utc())
        .await
        .unwrap();
    queue::claim(&w.pool, "test").await.unwrap().unwrap()
}

/// The job as a retry would see it: read again from the queue.
async fn reloaded(w: &GoogleWorld, job: &Job) -> Job {
    let payload: serde_json::Value = sqlx::query_scalar("SELECT payload FROM jobs WHERE id = $1")
        .bind(job.id)
        .fetch_one(&w.pool)
        .await
        .unwrap();
    Job {
        payload,
        attempts: job.attempts + 1,
        ..job.clone()
    }
}

async fn runs(w: &GoogleWorld) -> Vec<(Uuid, String, String)> {
    sqlx::query_as("SELECT id, trigger, status FROM ai_runs")
        .fetch_all(&w.pool)
        .await
        .unwrap()
}

#[sqlx::test]
async fn a_scheduled_run_that_failed_part_way_is_resumed_by_the_retry(pool: PgPool) {
    let w = GoogleWorld::with_ai(pool).await;
    w.add_prompt(Q1).await;
    w.mock_engines("Acme is great.", &[], None).await;
    let job = claimed_scheduled_job(&w).await;
    break_answers(&w.pool).await;
    assert!(w.ai_handler().run(&job).await.is_err());
    heal(&w.pool).await;
    let retry = reloaded(&w, &job).await;
    assert!(
        retry.payload["run_id"].is_string(),
        "the job remembers its run"
    );
    w.ai_handler().run(&retry).await.unwrap();
    let all = runs(&w).await;
    assert_eq!(all.len(), 1, "the retry made no second run");
    assert_eq!(
        (all[0].1.as_str(), all[0].2.as_str()),
        ("scheduled", "completed")
    );
}

#[sqlx::test]
async fn a_dead_scheduled_job_fails_its_run_and_a_manual_run_is_then_accepted(pool: PgPool) {
    let w = GoogleWorld::with_ai(pool).await;
    w.add_prompt(Q1).await;
    w.mock_engines("Acme is great.", &[], None).await;
    let job = claimed_scheduled_job(&w).await;
    break_answers(&w.pool).await;
    assert!(w.ai_handler().run(&job).await.is_err());
    assert_eq!(
        w.start_run().await.0,
        409,
        "the stuck run blocks manual runs"
    );
    w.ai_handler()
        .on_dead(&reloaded(&w, &job).await, "boom")
        .await;
    let all = runs(&w).await;
    assert_eq!(all[0].2, "failed");
    heal(&w.pool).await;
    assert_eq!(w.start_run().await.0, 202);
}

#[sqlx::test]
async fn a_dead_job_that_made_no_run_leaves_a_manual_run_alone(pool: PgPool) {
    let w = GoogleWorld::with_ai(pool).await;
    w.add_prompt(Q1).await;
    assert_eq!(w.start_run().await.0, 202);
    let job = w.ai_job(None);
    w.ai_handler().on_dead(&job, "boom").await;
    assert_eq!(runs(&w).await[0].2, "queued", "not this job's run");
}

#[sqlx::test]
async fn a_weekly_run_that_finds_no_engine_keys_is_put_off_a_week_not_dropped(pool: PgPool) {
    use margincite::ai::engines::EngineFactory;
    use margincite::ai::run::AiRunJob;
    use std::sync::Arc;
    use std::time::Duration;

    let w = GoogleWorld::new(pool).await;
    w.add_prompt(Q1).await;
    let factory = EngineFactory::new(common::engine_bases(&w.mock.uri()), Duration::from_secs(5));
    let config = Arc::new(common::test_config(&[]));
    let none = factory
        .for_workspace(&w.pool, &config, w.workspace_id)
        .await
        .unwrap();
    assert!(none.ids().is_empty());
    let handler = AiRunJob::new(w.pool.clone(), config, Arc::new(factory));
    handler.run(&w.ai_job(None)).await.unwrap();
    assert!(runs(&w).await.is_empty(), "nothing was asked");
    let wait: f64 = sqlx::query_scalar(
        "SELECT EXTRACT(EPOCH FROM (run_at - now()))::float8 FROM jobs WHERE kind = 'ai_run' AND status = 'queued'",
    )
    .fetch_one(&w.pool)
    .await
    .unwrap();
    let days = wait / 86_400.0;
    assert!((6.9..7.1).contains(&days), "{days}");
}
