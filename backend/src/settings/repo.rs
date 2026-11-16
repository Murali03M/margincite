//! Storage of `workspace_settings` and `provider_status`. Every query is
//! scoped to one workspace.

use std::collections::HashMap;

use sqlx::PgPool;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::error::AppError;

fn internal(err: sqlx::Error) -> AppError {
    AppError::Internal(err.into())
}

/// One stored value: sealed for a secret, plain for anything else.
#[derive(Debug, Clone)]
pub struct Row {
    pub sealed: Option<String>,
    pub plain: Option<String>,
}

/// What to store under a key.
pub enum Value {
    Plain(String),
    Sealed(String),
}

/// Every setting the workspace has saved.
pub async fn all(pool: &PgPool, workspace_id: Uuid) -> Result<HashMap<String, Row>, AppError> {
    let rows: Vec<(String, Option<String>, Option<String>)> = sqlx::query_as(
        "SELECT key, value_sealed, value_plain FROM workspace_settings WHERE workspace_id = $1",
    )
    .bind(workspace_id)
    .fetch_all(pool)
    .await
    .map_err(internal)?;
    Ok(rows
        .into_iter()
        .map(|(key, sealed, plain)| (key, Row { sealed, plain }))
        .collect())
}

/// Saves every value or none of them.
pub async fn set_many(
    pool: &PgPool,
    workspace_id: Uuid,
    values: &[(String, Value)],
) -> Result<(), AppError> {
    let mut tx = pool.begin().await.map_err(internal)?;
    for (key, value) in values {
        let (sealed, plain) = match value {
            Value::Sealed(text) => (Some(text.as_str()), None),
            Value::Plain(text) => (None, Some(text.as_str())),
        };
        sqlx::query(
            "INSERT INTO workspace_settings (workspace_id, key, value_sealed, value_plain)
             VALUES ($1, $2, $3, $4)
             ON CONFLICT (workspace_id, key) DO UPDATE
             SET value_sealed = EXCLUDED.value_sealed,
                 value_plain = EXCLUDED.value_plain,
                 updated_at = now()",
        )
        .bind(workspace_id)
        .bind(key)
        .bind(sealed)
        .bind(plain)
        .execute(&mut *tx)
        .await
        .map_err(internal)?;
    }
    tx.commit().await.map_err(internal)
}

pub async fn delete_keys(
    pool: &PgPool,
    workspace_id: Uuid,
    keys: &[String],
) -> Result<(), AppError> {
    sqlx::query("DELETE FROM workspace_settings WHERE workspace_id = $1 AND key = ANY($2)")
        .bind(workspace_id)
        .bind(keys)
        .execute(pool)
        .await
        .map_err(internal)?;
    Ok(())
}

/// The last test of a provider or source.
#[derive(Debug, Clone)]
pub struct Status {
    pub status: String,
    pub error: Option<String>,
    pub balance: Option<f64>,
    pub checked_at: OffsetDateTime,
}

#[derive(sqlx::FromRow)]
struct StatusRow {
    provider: String,
    status: String,
    error: Option<String>,
    balance: Option<f64>,
    checked_at: OffsetDateTime,
}

pub async fn statuses(
    pool: &PgPool,
    workspace_id: Uuid,
) -> Result<HashMap<String, Status>, AppError> {
    let rows: Vec<StatusRow> = sqlx::query_as(
        "SELECT provider, status, error, balance, checked_at
         FROM provider_status WHERE workspace_id = $1",
    )
    .bind(workspace_id)
    .fetch_all(pool)
    .await
    .map_err(internal)?;
    Ok(rows
        .into_iter()
        .map(|row| {
            (
                row.provider,
                Status {
                    status: row.status,
                    error: row.error,
                    balance: row.balance,
                    checked_at: row.checked_at,
                },
            )
        })
        .collect())
}

/// Records the outcome of a test. `error` is a plain sentence written here,
/// never text a provider sent.
pub async fn set_status(
    pool: &PgPool,
    workspace_id: Uuid,
    provider: &str,
    error: Option<&str>,
    balance: Option<f64>,
) -> Result<(), AppError> {
    let status = if error.is_some() { "error" } else { "ok" };
    sqlx::query(
        "INSERT INTO provider_status (workspace_id, provider, status, error, balance, checked_at)
         VALUES ($1, $2, $3, $4, $5, now())
         ON CONFLICT (workspace_id, provider) DO UPDATE
         SET status = EXCLUDED.status, error = EXCLUDED.error,
             balance = EXCLUDED.balance, checked_at = now()",
    )
    .bind(workspace_id)
    .bind(provider)
    .bind(status)
    .bind(error)
    .bind(balance)
    .execute(pool)
    .await
    .map_err(internal)?;
    Ok(())
}

pub async fn clear_status(
    pool: &PgPool,
    workspace_id: Uuid,
    provider: &str,
) -> Result<(), AppError> {
    sqlx::query("DELETE FROM provider_status WHERE workspace_id = $1 AND provider = $2")
        .bind(workspace_id)
        .bind(provider)
        .execute(pool)
        .await
        .map_err(internal)?;
    Ok(())
}
