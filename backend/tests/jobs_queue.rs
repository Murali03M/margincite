use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use margincite::jobs::queue::{self, Job, Lane};
use margincite::jobs::worker::{self, JobHandler, Timing};
use serde_json::json;
use sqlx::PgPool;
use time::OffsetDateTime;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

struct Record {
    seen: Mutex<Vec<serde_json::Value>>,
    dead: Mutex<Vec<String>>,
    fail: bool,
}

fn record(fail: bool) -> Arc<Record> {
    Arc::new(Record {
        seen: Mutex::new(Vec::new()),
        dead: Mutex::new(Vec::new()),
        fail,
    })
}

#[async_trait]
impl JobHandler for Record {
    fn kind(&self) -> &'static str {
        "record"
    }

    async fn run(&self, job: &Job) -> anyhow::Result<()> {
        if self.fail && job.payload["fail"] == true {
            anyhow::bail!("handler failed");
        }
        self.seen.lock().unwrap().push(job.payload.clone());
        Ok(())
    }

    async fn on_dead(&self, _job: &Job, error: &str) {
        self.dead.lock().unwrap().push(error.to_string());
    }
}

async fn due(pool: &PgPool, kind: &str) -> Uuid {
    queue::enqueue(pool, kind, &json!({}), OffsetDateTime::now_utc())
        .await
        .unwrap()
}

#[sqlx::test]
async fn a_due_job_is_claimed_once(pool: PgPool) {
    let id = due(&pool, "crawl").await;
    let first = queue::claim(&pool, "a").await.unwrap().unwrap();
    assert_eq!(first.id, id);
    assert!(queue::claim(&pool, "b").await.unwrap().is_none());
}

#[sqlx::test]
async fn a_future_job_is_not_claimed(pool: PgPool) {
    queue::enqueue(
        &pool,
        "crawl",
        &json!({}),
        OffsetDateTime::now_utc() + time::Duration::hours(1),
    )
    .await
    .unwrap();
    assert!(queue::claim(&pool, "a").await.unwrap().is_none());
}

#[sqlx::test]
async fn two_workers_never_get_the_same_job(pool: PgPool) {
    for _ in 0..20 {
        due(&pool, "crawl").await;
    }
    let mut tasks = Vec::new();
    for worker in 0..4 {
        let pool = pool.clone();
        tasks.push(tokio::spawn(async move {
            let mut ids = Vec::new();
            for _ in 0..20 {
                if let Some(job) = queue::claim(&pool, &format!("w{worker}")).await.unwrap() {
                    ids.push(job.id);
                }
            }
            ids
        }));
    }
    let mut all = Vec::new();
    for task in tasks {
        all.extend(task.await.unwrap());
    }
    all.sort();
    all.dedup();
    assert_eq!(all.len(), 20);
}

#[sqlx::test]
async fn a_failed_job_is_retried_later(pool: PgPool) {
    let id = due(&pool, "crawl").await;
    queue::claim(&pool, "a").await.unwrap().unwrap();
    queue::fail(&pool, id, "boom").await.unwrap();
    let (status, attempts, wait): (String, i32, f64) = sqlx::query_as(
        "SELECT status, attempts, EXTRACT(EPOCH FROM (run_at - now()))::float8 FROM jobs WHERE id = $1",
    )
    .bind(id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(status, "queued");
    assert_eq!(attempts, 1);
    assert!((50.0..80.0).contains(&wait), "{wait}");
}

#[sqlx::test]
async fn a_job_is_failed_for_good_after_three_tries(pool: PgPool) {
    let id = due(&pool, "crawl").await;
    for _ in 0..3 {
        sqlx::query("UPDATE jobs SET status = 'queued', run_at = now() WHERE id = $1")
            .bind(id)
            .execute(&pool)
            .await
            .unwrap();
        queue::claim(&pool, "a").await.unwrap().unwrap();
        queue::fail(&pool, id, "boom").await.unwrap();
    }
    let status: String = sqlx::query_scalar("SELECT status FROM jobs WHERE id = $1")
        .bind(id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(status, "failed");
}

#[sqlx::test]
async fn failing_says_whether_the_job_is_dead(pool: PgPool) {
    let id = due(&pool, "crawl").await;
    let mut outcomes = Vec::new();
    for _ in 0..3 {
        sqlx::query("UPDATE jobs SET status = 'queued', run_at = now() WHERE id = $1")
            .bind(id)
            .execute(&pool)
            .await
            .unwrap();
        queue::claim(&pool, "a").await.unwrap().unwrap();
        outcomes.push(queue::fail(&pool, id, "boom").await.unwrap());
    }
    assert_eq!(outcomes, vec![false, false, true]);
}

#[sqlx::test]
async fn the_worker_tells_the_handler_when_a_job_fails_for_good(pool: PgPool) {
    let handler = record(true);
    let shutdown = CancellationToken::new();
    let running = tokio::spawn(worker::run(
        pool.clone(),
        vec![handler.clone()],
        shutdown.clone(),
    ));
    let id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO jobs (id, kind, payload, status, run_at, attempts)
         VALUES ($1, 'record', '{\"fail\": true}', 'queued', now(), 2)",
    )
    .bind(id)
    .execute(&pool)
    .await
    .unwrap();
    wait_for_status(&pool, id, "failed").await;
    shutdown.cancel();
    running.await.unwrap();
    let dead = handler.dead.lock().unwrap();
    assert_eq!(dead.len(), 1);
    assert!(dead[0].contains("handler failed"));
}

#[sqlx::test]
async fn a_job_that_will_be_retried_is_not_reported_dead(pool: PgPool) {
    let handler = record(true);
    let shutdown = CancellationToken::new();
    let running = tokio::spawn(worker::run(
        pool.clone(),
        vec![handler.clone()],
        shutdown.clone(),
    ));
    let id = queue::enqueue(
        &pool,
        "record",
        &json!({"fail": true}),
        OffsetDateTime::now_utc(),
    )
    .await
    .unwrap();
    for _ in 0..30 {
        let attempts: i32 = sqlx::query_scalar("SELECT attempts FROM jobs WHERE id = $1")
            .bind(id)
            .fetch_one(&pool)
            .await
            .unwrap();
        if attempts == 1 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    tokio::time::sleep(Duration::from_millis(300)).await;
    shutdown.cancel();
    running.await.unwrap();
    assert!(handler.dead.lock().unwrap().is_empty());
}

#[sqlx::test]
async fn a_stale_running_job_is_released(pool: PgPool) {
    let id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO jobs (id, kind, payload, status, run_at, attempts, locked_at, locked_by)
         VALUES ($1, 'crawl', '{}', 'running', now(), 1, now() - interval '16 minutes', 'dead')",
    )
    .bind(id)
    .execute(&pool)
    .await
    .unwrap();
    let released = queue::release_stale(&pool, Duration::from_secs(15 * 60))
        .await
        .unwrap();
    assert_eq!(released, 1);
    let job = queue::claim(&pool, "a").await.unwrap().unwrap();
    assert_eq!(job.id, id);
}

#[sqlx::test]
async fn the_worker_runs_a_handler_and_marks_it_done(pool: PgPool) {
    let handler = record(false);
    let shutdown = CancellationToken::new();
    let running = tokio::spawn(worker::run(
        pool.clone(),
        vec![handler.clone()],
        shutdown.clone(),
    ));
    let id = queue::enqueue(&pool, "record", &json!({"n": 1}), OffsetDateTime::now_utc())
        .await
        .unwrap();
    wait_for_status(&pool, id, "done").await;
    shutdown.cancel();
    running.await.unwrap();
    assert_eq!(handler.seen.lock().unwrap().len(), 1);
}

#[sqlx::test]
async fn a_handler_error_is_recorded_without_stopping_the_worker(pool: PgPool) {
    let handler = record(true);
    let shutdown = CancellationToken::new();
    let running = tokio::spawn(worker::run(pool.clone(), vec![handler], shutdown.clone()));
    let bad = queue::enqueue(
        &pool,
        "record",
        &json!({"fail": true}),
        OffsetDateTime::now_utc(),
    )
    .await
    .unwrap();
    let good = queue::enqueue(
        &pool,
        "record",
        &json!({"fail": false}),
        OffsetDateTime::now_utc(),
    )
    .await
    .unwrap();
    wait_for_status(&pool, good, "done").await;
    let error: String = sqlx::query_scalar("SELECT last_error FROM jobs WHERE id = $1")
        .bind(bad)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(error.contains("handler failed"));
    shutdown.cancel();
    running.await.unwrap();
}

#[sqlx::test]
async fn a_job_of_unknown_kind_is_failed_with_a_reason(pool: PgPool) {
    let shutdown = CancellationToken::new();
    let running = tokio::spawn(worker::run(pool.clone(), vec![], shutdown.clone()));
    let id = due(&pool, "missing").await;
    wait_for_status(&pool, id, "failed").await;
    let error: String = sqlx::query_scalar("SELECT last_error FROM jobs WHERE id = $1")
        .bind(id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(error.contains("No handler"));
    shutdown.cancel();
    running.await.unwrap();
}

const AI: &[&str] = &["ai_run"];

#[sqlx::test]
async fn each_lane_takes_only_its_own_kinds(pool: PgPool) {
    let ai = due(&pool, "ai_run").await;
    let crawl = due(&pool, "crawl").await;
    let took = queue::claim_in(&pool, "a", Lane::Only(AI))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(took.id, ai);
    assert!(queue::claim_in(&pool, "a", Lane::Only(AI))
        .await
        .unwrap()
        .is_none());
    let took = queue::claim_in(&pool, "g", Lane::Except(AI))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(took.id, crawl);
    assert!(queue::claim_in(&pool, "g", Lane::Except(AI))
        .await
        .unwrap()
        .is_none());
}

#[sqlx::test]
async fn a_crawl_is_claimed_while_an_ai_job_is_running(pool: PgPool) {
    due(&pool, "ai_run").await;
    queue::claim_in(&pool, "a", Lane::Only(AI))
        .await
        .unwrap()
        .unwrap();
    let crawl = due(&pool, "crawl").await;
    let took = queue::claim_in(&pool, "g", Lane::Except(AI))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(took.id, crawl);
}

struct Kind(&'static str, Mutex<Vec<Uuid>>);

#[async_trait]
impl JobHandler for Kind {
    fn kind(&self) -> &'static str {
        self.0
    }
    async fn run(&self, job: &Job) -> anyhow::Result<()> {
        self.1.lock().unwrap().push(job.id);
        Ok(())
    }
}

#[sqlx::test]
async fn two_lanes_never_take_the_same_job(pool: PgPool) {
    let ai = Arc::new(Kind("ai_run", Mutex::new(Vec::new())));
    let other = Arc::new(Kind("crawl", Mutex::new(Vec::new())));
    let timing = Timing {
        poll: Duration::from_millis(20),
        ..Timing::default()
    };
    let shutdown = CancellationToken::new();
    let lanes = [
        (Lane::Only(AI), ai.clone() as Arc<dyn JobHandler>),
        (Lane::Except(AI), other.clone()),
    ];
    let loops: Vec<_> = lanes
        .into_iter()
        .map(|(lane, handler)| {
            tokio::spawn(worker::run_lane(
                pool.clone(),
                vec![handler],
                lane,
                timing,
                shutdown.clone(),
            ))
        })
        .collect();
    let mut ids = Vec::new();
    for _ in 0..6 {
        ids.push(due(&pool, "ai_run").await);
        ids.push(due(&pool, "crawl").await);
    }
    for id in &ids {
        wait_for_status(&pool, *id, "done").await;
    }
    shutdown.cancel();
    for l in loops {
        l.await.unwrap();
    }
    let (ai, other) = (
        ai.1.lock().unwrap().clone(),
        other.1.lock().unwrap().clone(),
    );
    assert_eq!(
        (ai.len(), other.len()),
        (6, 6),
        "each job ran once, in its own lane"
    );
}

#[sqlx::test]
async fn a_job_with_a_current_heartbeat_is_not_released(pool: PgPool) {
    let id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO jobs (id, kind, payload, status, run_at, attempts, locked_at, locked_by)
         VALUES ($1, 'ai_run', '{}', 'running', now(), 1, now() - interval '16 minutes', 'live')",
    )
    .bind(id)
    .execute(&pool)
    .await
    .unwrap();
    queue::heartbeat(&pool, id).await.unwrap();
    let released = queue::release_stale(&pool, Duration::from_secs(15 * 60))
        .await
        .unwrap();
    assert_eq!(released, 0);
}

#[sqlx::test]
async fn a_job_whose_worker_stopped_beating_fifteen_minutes_ago_is_released(pool: PgPool) {
    let id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO jobs (id, kind, payload, status, run_at, attempts, locked_at, locked_by)
         VALUES ($1, 'ai_run', '{}', 'running', now(), 1, now(), 'dead')",
    )
    .bind(id)
    .execute(&pool)
    .await
    .unwrap();
    queue::heartbeat(&pool, id).await.unwrap();
    sqlx::query(
        "UPDATE jobs SET locked_at = now() - interval '15 minutes 5 seconds' WHERE id = $1",
    )
    .bind(id)
    .execute(&pool)
    .await
    .unwrap();
    let released = queue::release_stale(&pool, Duration::from_secs(15 * 60))
        .await
        .unwrap();
    assert_eq!(released, 1);
}

struct Slow(Mutex<u32>);

#[async_trait]
impl JobHandler for Slow {
    fn kind(&self) -> &'static str {
        "slow"
    }
    async fn run(&self, _job: &Job) -> anyhow::Result<()> {
        *self.0.lock().unwrap() += 1;
        tokio::time::sleep(Duration::from_millis(2500)).await;
        Ok(())
    }
}

#[sqlx::test]
async fn the_worker_keeps_a_long_job_alive_so_it_is_not_run_twice(pool: PgPool) {
    let slow = Arc::new(Slow(Mutex::new(0)));
    let timing = Timing {
        poll: Duration::from_millis(100),
        heartbeat: Duration::from_millis(100),
        stale_after: Duration::from_secs(1),
    };
    let shutdown = CancellationToken::new();
    // The second loop would take the job again if it were released as stale.
    let busy = tokio::spawn(worker::run_lane(
        pool.clone(),
        vec![slow.clone()],
        Lane::Only(&["slow"]),
        timing,
        shutdown.clone(),
    ));
    let checker = tokio::spawn(worker::run_lane(
        pool.clone(),
        vec![slow.clone()],
        Lane::Only(&["slow"]),
        timing,
        shutdown.clone(),
    ));
    let id = due(&pool, "slow").await;
    wait_for_status(&pool, id, "running").await;
    for _ in 0..60 {
        tokio::time::sleep(Duration::from_millis(100)).await;
        let status: String = sqlx::query_scalar("SELECT status FROM jobs WHERE id = $1")
            .bind(id)
            .fetch_one(&pool)
            .await
            .unwrap();
        if status == "done" {
            break;
        }
    }
    shutdown.cancel();
    busy.await.unwrap();
    checker.await.unwrap();
    assert_eq!(
        *slow.0.lock().unwrap(),
        1,
        "the job was never released and run again"
    );
}

async fn wait_for_status(pool: &PgPool, id: Uuid, wanted: &str) {
    for _ in 0..30 {
        let status: Option<String> = sqlx::query_scalar("SELECT status FROM jobs WHERE id = $1")
            .bind(id)
            .fetch_optional(pool)
            .await
            .unwrap();
        if status.as_deref() == Some(wanted) {
            return;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    panic!("job {id} did not become {wanted}");
}

#[allow(dead_code)]
fn _job_type(job: Job) -> Uuid {
    job.id
}
