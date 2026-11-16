//! What a site's search volume refreshes leave behind: the daily allowance
//! for starting them, and why the last one stored nothing.

use serde::Serialize;
use sqlx::PgPool;
use time::OffsetDateTime;
use uuid::Uuid;

use super::volumes::KIND;
use crate::error::AppError;

/// Refreshes a site may start in a UTC day. Each is a paid request.
pub const MAX_REFRESHES_PER_DAY: i64 = 5;

fn internal(err: sqlx::Error) -> AppError {
    AppError::Internal(err.into())
}

/// Counts a refresh against the site's allowance for today (UTC), or says it
/// is used up, and returns the reservation so it can be given back. Counting
/// and recording happen under one lock on the site.
pub async fn reserve(pool: &PgPool, workspace_id: Uuid, site_id: Uuid) -> Result<Uuid, AppError> {
    let mut tx = pool.begin().await.map_err(internal)?;
    sqlx::query("SELECT id FROM sites WHERE id = $1 AND workspace_id = $2 FOR UPDATE")
        .bind(site_id)
        .bind(workspace_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(internal)?
        .ok_or(AppError::NotFound)?;
    let used: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM keyword_volume_requests
         WHERE site_id = $1
           AND created_at >= (date_trunc('day', now() AT TIME ZONE 'UTC') AT TIME ZONE 'UTC')",
    )
    .bind(site_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(internal)?;
    if used >= MAX_REFRESHES_PER_DAY {
        return Err(AppError::DailyLimit(format!(
            "You can refresh search volume {MAX_REFRESHES_PER_DAY} times a day for each site. \
             Try again tomorrow."
        )));
    }
    let id: Uuid = sqlx::query_scalar(
        "INSERT INTO keyword_volume_requests (workspace_id, site_id) VALUES ($1, $2) RETURNING id",
    )
    .bind(workspace_id)
    .bind(site_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(internal)?;
    tx.commit().await.map_err(internal)?;
    Ok(id)
}

/// Gives a reservation back, for a refresh that turned out not to need a new
/// paid request.
pub async fn give_back(pool: &PgPool, reservation: Uuid) {
    if let Err(err) = sqlx::query("DELETE FROM keyword_volume_requests WHERE id = $1")
        .bind(reservation)
        .execute(pool)
        .await
    {
        tracing::warn!(error = %err, "could not give back a search volume reservation");
    }
}

/// Remembers why the site's latest refresh stored nothing.
pub async fn record_failure(pool: &PgPool, workspace_id: Uuid, site_id: Uuid, message: &str) {
    let saved = sqlx::query(
        "INSERT INTO keyword_volume_failures (site_id, workspace_id, error) VALUES ($1, $2, $3)
         ON CONFLICT (site_id) DO UPDATE SET error = excluded.error, failed_at = now()",
    )
    .bind(site_id)
    .bind(workspace_id)
    .bind(message)
    .execute(pool)
    .await;
    if let Err(err) = saved {
        tracing::warn!(%site_id, error = %err, "could not record a search volume failure");
    }
}

/// Forgets the site's last failure: a new refresh is on its way, or one has
/// stored volumes.
pub async fn clear_failure(pool: &PgPool, workspace_id: Uuid, site_id: Uuid) {
    let cleared =
        sqlx::query("DELETE FROM keyword_volume_failures WHERE workspace_id = $1 AND site_id = $2")
            .bind(workspace_id)
            .bind(site_id)
            .execute(pool)
            .await;
    if let Err(err) = cleared {
        tracing::warn!(%site_id, error = %err, "could not clear a search volume failure");
    }
}

#[derive(Debug, Serialize)]
pub struct Status {
    /// `idle`, `queued`, `running` or `failed`.
    pub status: &'static str,
    /// When volumes were last stored, `null` when never.
    #[serde(with = "time::serde::rfc3339::option")]
    pub checked_at: Option<OffsetDateTime>,
    /// A fixed plain sentence, only when `status` is `failed`.
    pub error: Option<String>,
}

pub async fn status(pool: &PgPool, workspace_id: Uuid, site_id: Uuid) -> Result<Status, AppError> {
    let waiting: Option<String> = sqlx::query_scalar(
        "SELECT status FROM jobs
         WHERE kind = $1 AND status IN ('queued', 'running')
           AND payload->>'workspace_id' = $2 AND payload->>'site_id' = $3
         ORDER BY (status = 'running') DESC LIMIT 1",
    )
    .bind(KIND)
    .bind(workspace_id.to_string())
    .bind(site_id.to_string())
    .fetch_optional(pool)
    .await
    .map_err(internal)?;
    let checked_at: Option<OffsetDateTime> = sqlx::query_scalar(
        "SELECT MAX(metrics_checked_at) FROM site_keywords WHERE workspace_id = $1 AND site_id = $2",
    )
    .bind(workspace_id)
    .bind(site_id)
    .fetch_one(pool)
    .await
    .map_err(internal)?;
    let failure: Option<String> = sqlx::query_scalar(
        "SELECT error FROM keyword_volume_failures WHERE workspace_id = $1 AND site_id = $2",
    )
    .bind(workspace_id)
    .bind(site_id)
    .fetch_optional(pool)
    .await
    .map_err(internal)?;
    let (status, error) = match (waiting.as_deref(), failure) {
        (Some("running"), _) => ("running", None),
        (Some(_), _) => ("queued", None),
        (None, Some(error)) => ("failed", Some(error)),
        (None, None) => ("idle", None),
    };
    Ok(Status {
        status,
        checked_at,
        error,
    })
}
