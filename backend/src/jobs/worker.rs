use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use tokio_util::sync::CancellationToken;

use super::queue::{self, Job, Lane};

#[async_trait]
pub trait JobHandler: Send + Sync {
    fn kind(&self) -> &'static str;
    async fn run(&self, job: &Job) -> anyhow::Result<()>;

    /// Called once when a job of this kind has failed for good.
    async fn on_dead(&self, _job: &Job, _error: &str) {}
}

/// How often a worker loop looks for work, tells the queue its job is alive,
/// and treats a silent job as dead.
#[derive(Debug, Clone, Copy)]
pub struct Timing {
    pub poll: Duration,
    pub heartbeat: Duration,
    pub stale_after: Duration,
}

impl Default for Timing {
    fn default() -> Self {
        Self {
            poll: Duration::from_secs(2),
            heartbeat: Duration::from_secs(60),
            stale_after: Duration::from_secs(15 * 60),
        }
    }
}

/// One worker loop that takes every kind of job.
pub async fn run(
    pool: sqlx::PgPool,
    handlers: Vec<Arc<dyn JobHandler>>,
    shutdown: CancellationToken,
) {
    run_lane(pool, handlers, Lane::Any, Timing::default(), shutdown).await;
}

/// A worker loop that takes only the jobs its lane allows, so a long job of
/// one kind does not hold up the others.
pub async fn run_lane(
    pool: sqlx::PgPool,
    handlers: Vec<Arc<dyn JobHandler>>,
    lane: Lane,
    timing: Timing,
    shutdown: CancellationToken,
) {
    let mut tick = tokio::time::interval(timing.poll);
    loop {
        tokio::select! {
            _ = shutdown.cancelled() => break,
            _ = tick.tick() => {
                if shutdown.is_cancelled() {
                    break;
                }
                let _ = queue::release_stale(&pool, timing.stale_after).await;
                match queue::claim_in(&pool, "margincite", lane).await {
                    Ok(Some(job)) => dispatch(&pool, &handlers, job, timing.heartbeat).await,
                    Ok(None) => {}
                    Err(err) => tracing::error!(error = %err, "could not claim a job"),
                }
            }
        }
    }
}

/// Refreshes the job's lock until dropped.
struct Heartbeat(tokio::task::JoinHandle<()>);

impl Heartbeat {
    fn start(pool: sqlx::PgPool, id: uuid::Uuid, every: Duration) -> Self {
        Self(tokio::spawn(async move {
            let mut tick = tokio::time::interval(every);
            tick.tick().await;
            loop {
                tick.tick().await;
                if let Err(err) = queue::heartbeat(&pool, id).await {
                    tracing::warn!(error = %err, job_id = %id, "could not refresh a job's lock");
                }
            }
        }))
    }
}

impl Drop for Heartbeat {
    fn drop(&mut self) {
        self.0.abort();
    }
}

async fn dispatch(
    pool: &sqlx::PgPool,
    handlers: &[Arc<dyn JobHandler>],
    job: Job,
    heartbeat: Duration,
) {
    let Some(handler) = handlers.iter().find(|handler| handler.kind() == job.kind) else {
        let _ = queue::dead(pool, job.id, "No handler is registered for this job.").await;
        return;
    };
    let outcome = {
        let _alive = Heartbeat::start(pool.clone(), job.id, heartbeat);
        handler.run(&job).await
    };
    match outcome {
        Ok(()) => {
            let _ = queue::complete(pool, job.id).await;
        }
        Err(err) => {
            tracing::warn!(error = %err, job_id = %job.id, "job failed");
            let message = err.to_string();
            if matches!(queue::fail(pool, job.id, &message).await, Ok(true)) {
                handler.on_dead(&job, &message).await;
            }
        }
    }
}
