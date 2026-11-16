use serde_json::Value;
use time::OffsetDateTime;
use uuid::Uuid;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Job {
    pub id: Uuid,
    pub kind: String,
    pub payload: Value,
    pub attempts: i32,
}

pub async fn enqueue(
    pool: &sqlx::PgPool,
    kind: &str,
    payload: &Value,
    run_at: OffsetDateTime,
) -> anyhow::Result<Uuid> {
    let id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO jobs (id, kind, payload, status, run_at) VALUES ($1, $2, $3, 'queued', $4)",
    )
    .bind(id)
    .bind(kind)
    .bind(payload)
    .bind(run_at)
    .execute(pool)
    .await?;
    Ok(id)
}

/// Queues a site's next scheduled crawl unless a crawl job for that site is
/// already waiting, whether it came from a schedule or a person. One statement,
/// so two crawls finishing together still queue one job. Returns whether a job
/// was queued.
pub async fn enqueue_crawl_unless_queued(
    pool: &sqlx::PgPool,
    workspace_id: Uuid,
    site_id: Uuid,
    run_at: OffsetDateTime,
) -> anyhow::Result<bool> {
    let payload = serde_json::json!({
        "workspace_id": workspace_id,
        "site_id": site_id,
        "trigger": "scheduled"
    });
    let result = sqlx::query(
        "INSERT INTO jobs (id, kind, payload, status, run_at)
         SELECT $1, 'crawl', $2, 'queued', $3
         WHERE NOT EXISTS (
             SELECT 1 FROM jobs j
             WHERE j.kind = 'crawl' AND j.status = 'queued'
               AND (j.payload->>'site_id' = $4
                    OR j.payload->>'crawl_id' IN (
                        SELECT c.id::text FROM crawls c WHERE c.site_id = $5)))",
    )
    .bind(Uuid::new_v4())
    .bind(&payload)
    .bind(run_at)
    .bind(site_id.to_string())
    .bind(site_id)
    .execute(pool)
    .await?;
    Ok(result.rows_affected() == 1)
}

/// Queues a job of this kind for one site unless one is already waiting. A job
/// that is already waiting is pulled forward when this one is due sooner, so
/// "Sync now" is not held up by tomorrow's scheduled run. Jobs of one kind and
/// site queue one at a time even when two callers arrive together: the
/// statement runs under a lock named for the kind and the site. The payload is
/// `{ workspace_id, site_id }`. Returns whether a new job was queued.
pub async fn enqueue_site_job_unless_queued(
    pool: &sqlx::PgPool,
    kind: &str,
    workspace_id: Uuid,
    site_id: Uuid,
    run_at: OffsetDateTime,
) -> anyhow::Result<bool> {
    let payload = serde_json::json!({ "workspace_id": workspace_id, "site_id": site_id });
    let mut tx = pool.begin().await?;
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))")
        .bind(format!("{kind}:{site_id}"))
        .execute(&mut *tx)
        .await?;
    let result = sqlx::query(
        "WITH pulled AS (
             UPDATE jobs SET run_at = LEAST(run_at, $3)
             WHERE kind = $1 AND status = 'queued' AND payload->>'site_id' = $2
             RETURNING id)
         INSERT INTO jobs (id, kind, payload, status, run_at)
         SELECT $4, $1, $5, 'queued', $3
         WHERE NOT EXISTS (SELECT 1 FROM pulled)",
    )
    .bind(kind)
    .bind(site_id.to_string())
    .bind(run_at)
    .bind(Uuid::new_v4())
    .bind(&payload)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(result.rows_affected() == 1)
}

/// Queues a job of this kind for one site only when none is waiting, and
/// leaves a waiting one exactly as it is: unlike `enqueue_site_job_unless_queued`
/// it never pulls a later job forward. For starting a schedule, where setting
/// something up must not bring a paid run closer. Returns whether a job was
/// queued.
pub async fn enqueue_site_job_if_none(
    pool: &sqlx::PgPool,
    kind: &str,
    workspace_id: Uuid,
    site_id: Uuid,
    run_at: OffsetDateTime,
) -> anyhow::Result<bool> {
    let payload = serde_json::json!({ "workspace_id": workspace_id, "site_id": site_id });
    let mut tx = pool.begin().await?;
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))")
        .bind(format!("{kind}:{site_id}"))
        .execute(&mut *tx)
        .await?;
    let result = sqlx::query(
        "INSERT INTO jobs (id, kind, payload, status, run_at)
         SELECT $3, $1, $4, 'queued', $5
         WHERE NOT EXISTS (
             SELECT 1 FROM jobs WHERE kind = $1 AND status = 'queued' AND payload->>'site_id' = $2)",
    )
    .bind(kind)
    .bind(site_id.to_string())
    .bind(Uuid::new_v4())
    .bind(&payload)
    .bind(run_at)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(result.rows_affected() == 1)
}

/// Adds fields to a job's payload, so a retry of the job starts from them.
pub async fn attach(
    pool: &sqlx::PgPool,
    id: Uuid,
    fields: &serde_json::Value,
) -> anyhow::Result<()> {
    sqlx::query("UPDATE jobs SET payload = payload || $2 WHERE id = $1")
        .bind(id)
        .bind(fields)
        .execute(pool)
        .await?;
    Ok(())
}

/// Remembers which crawl a job is running, so a retry of the job reuses it.
pub async fn attach_crawl(pool: &sqlx::PgPool, id: Uuid, crawl_id: Uuid) -> anyhow::Result<()> {
    sqlx::query("UPDATE jobs SET payload = payload || $2 WHERE id = $1")
        .bind(id)
        .bind(serde_json::json!({ "crawl_id": crawl_id }))
        .execute(pool)
        .await?;
    Ok(())
}

/// Remembers which AI run a scheduled job made, so a retry resumes that run
/// instead of meeting the one-active-run rule.
pub async fn attach_run(pool: &sqlx::PgPool, id: Uuid, run_id: Uuid) -> anyhow::Result<()> {
    sqlx::query("UPDATE jobs SET payload = payload || $2 WHERE id = $1")
        .bind(id)
        .bind(serde_json::json!({ "run_id": run_id }))
        .execute(pool)
        .await?;
    Ok(())
}

/// Which kinds of job a worker loop takes.
#[derive(Debug, Clone, Copy)]
pub enum Lane {
    Any,
    Only(&'static [&'static str]),
    Except(&'static [&'static str]),
}

impl Lane {
    fn parts(self) -> (bool, Vec<String>) {
        let own = |kinds: &[&str]| kinds.iter().map(|k| (*k).to_string()).collect();
        match self {
            Lane::Any => (false, Vec::new()),
            Lane::Only(kinds) => (true, own(kinds)),
            Lane::Except(kinds) => (false, own(kinds)),
        }
    }
}

pub async fn claim(pool: &sqlx::PgPool, worker: &str) -> anyhow::Result<Option<Job>> {
    claim_in(pool, worker, Lane::Any).await
}

/// Claims the next due job of the kinds this lane takes.
pub async fn claim_in(
    pool: &sqlx::PgPool,
    worker: &str,
    lane: Lane,
) -> anyhow::Result<Option<Job>> {
    let (only, kinds) = lane.parts();
    let job = sqlx::query_as::<_, Job>(
        "UPDATE jobs
         SET status = 'running', attempts = attempts + 1, locked_at = now(), locked_by = $1
         WHERE id = (
             SELECT id FROM jobs
             WHERE status = 'queued' AND run_at <= now()
               AND CASE WHEN $2 THEN kind = ANY($3) ELSE kind <> ALL($3) END
             ORDER BY run_at, created_at
             FOR UPDATE SKIP LOCKED
             LIMIT 1
         )
         RETURNING id, kind, payload, attempts",
    )
    .bind(worker)
    .bind(only)
    .bind(&kinds)
    .fetch_optional(pool)
    .await?;
    Ok(job)
}

/// Says a running job's worker is alive, so `release_stale` leaves it alone.
pub async fn heartbeat(pool: &sqlx::PgPool, id: Uuid) -> anyhow::Result<()> {
    sqlx::query("UPDATE jobs SET locked_at = now() WHERE id = $1 AND status = 'running'")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn complete(pool: &sqlx::PgPool, id: Uuid) -> anyhow::Result<()> {
    sqlx::query(
        "UPDATE jobs SET status = 'done', locked_at = NULL, locked_by = NULL WHERE id = $1",
    )
    .bind(id)
    .execute(pool)
    .await?;
    Ok(())
}

/// Records a failed attempt. Returns true when the job has failed for good.
pub async fn fail(pool: &sqlx::PgPool, id: Uuid, error: &str) -> anyhow::Result<bool> {
    let attempts: i32 = sqlx::query_scalar("SELECT attempts FROM jobs WHERE id = $1")
        .bind(id)
        .fetch_one(pool)
        .await?;
    if attempts >= 3 {
        dead(pool, id, error).await?;
        Ok(true)
    } else {
        let delay_secs: i64 = if attempts <= 1 { 60 } else { 300 };
        sqlx::query(
            "UPDATE jobs
             SET status = 'queued', run_at = now() + ($2 * interval '1 second'),
                 last_error = $3, locked_at = NULL, locked_by = NULL
             WHERE id = $1",
        )
        .bind(id)
        .bind(delay_secs)
        .bind(error)
        .execute(pool)
        .await?;
        Ok(false)
    }
}

pub async fn dead(pool: &sqlx::PgPool, id: Uuid, error: &str) -> anyhow::Result<()> {
    sqlx::query(
        "UPDATE jobs SET status = 'failed', last_error = $2, locked_at = NULL, locked_by = NULL WHERE id = $1",
    )
    .bind(id)
    .bind(error)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn release_stale(
    pool: &sqlx::PgPool,
    older_than: std::time::Duration,
) -> anyhow::Result<u64> {
    let secs = i64::try_from(older_than.as_secs()).unwrap_or(i64::MAX);
    let result = sqlx::query(
        "UPDATE jobs
         SET status = 'queued', locked_at = NULL, locked_by = NULL
         WHERE status = 'running' AND locked_at < now() - ($1 * interval '1 second')",
    )
    .bind(secs)
    .execute(pool)
    .await?;
    Ok(result.rows_affected())
}
