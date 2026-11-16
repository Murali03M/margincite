//! The usage ledger: every paid or metered call a workspace makes, counted
//! per source and kind, for the usage screen.

use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppError;

/// Counts `calls` calls of `kind` made to `source`. `estimated_cost_usd` is
/// `None` when the cost is not known.
pub async fn record(
    pool: &PgPool,
    workspace_id: Uuid,
    source: &str,
    kind: &str,
    calls: i32,
    estimated_cost_usd: Option<f64>,
) -> Result<(), AppError> {
    sqlx::query(
        "INSERT INTO usage_events (workspace_id, source, kind, calls, estimated_cost_usd)
         VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(workspace_id)
    .bind(source)
    .bind(kind)
    .bind(calls.max(0))
    .bind(estimated_cost_usd)
    .execute(pool)
    .await
    .map_err(|err| AppError::Internal(err.into()))?;
    Ok(())
}

/// `record`, for the places where a ledger failure must not spoil the work:
/// it is logged and dropped.
pub async fn record_quietly(
    pool: &PgPool,
    workspace_id: Uuid,
    source: &str,
    kind: &str,
    calls: i32,
    estimated_cost_usd: Option<f64>,
) {
    if let Err(err) = record(pool, workspace_id, source, kind, calls, estimated_cost_usd).await {
        tracing::warn!(source, kind, error = %err, "could not record usage");
    }
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct Day {
    pub source: String,
    pub kind: String,
    pub calls: i64,
    pub estimated_cost_usd: Option<f64>,
    /// The UTC day, `YYYY-MM-DD`.
    pub day: String,
}

/// Usage per day, source and kind for the last `days` days, newest first.
pub async fn list(pool: &PgPool, workspace_id: Uuid, days: i64) -> Result<Vec<Day>, AppError> {
    sqlx::query_as(
        "SELECT source, kind, SUM(calls)::bigint AS calls,
                SUM(estimated_cost_usd) AS estimated_cost_usd,
                to_char(created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD') AS day
         FROM usage_events
         WHERE workspace_id = $1 AND created_at >= now() - make_interval(days => $2::int)
         GROUP BY day, source, kind
         ORDER BY day DESC, source, kind",
    )
    .bind(workspace_id)
    .bind(days as i32)
    .fetch_all(pool)
    .await
    .map_err(|err| AppError::Internal(err.into()))
}
