//! Bing connections and the backlinks they bring in. Every function takes
//! the workspace and filters by it.

use sqlx::PgPool;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::error::AppError;

fn internal(err: sqlx::Error) -> AppError {
    AppError::Internal(err.into())
}

/// One site's connection. Deliberately no `Debug`: it holds the sealed key.
#[derive(sqlx::FromRow)]
pub struct Connection {
    pub api_key_sealed: String,
    pub site_url: String,
    pub status: String,
    pub error: Option<String>,
    pub synced_at: Option<OffsetDateTime>,
}

pub async fn get_connection(
    pool: &PgPool,
    workspace_id: Uuid,
    site_id: Uuid,
) -> Result<Option<Connection>, AppError> {
    sqlx::query_as(
        "SELECT api_key_sealed, site_url, status, error, synced_at
         FROM bing_connections WHERE workspace_id = $1 AND site_id = $2",
    )
    .bind(workspace_id)
    .bind(site_id)
    .fetch_optional(pool)
    .await
    .map_err(internal)
}

/// Saves a checked key and site address. A different address than before
/// drops the old address's rows at once, so they are never shown as the new
/// one's.
pub async fn save_connection(
    pool: &PgPool,
    workspace_id: Uuid,
    site_id: Uuid,
    sealed_key: &str,
    site_url: &str,
) -> Result<(), AppError> {
    let mut tx = pool.begin().await.map_err(internal)?;
    let previous: Option<String> = sqlx::query_scalar(
        "SELECT site_url FROM bing_connections
         WHERE workspace_id = $1 AND site_id = $2 FOR UPDATE",
    )
    .bind(workspace_id)
    .bind(site_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(internal)?;
    if previous.as_deref() != Some(site_url) {
        delete_data(&mut tx, workspace_id, site_id).await?;
    }
    sqlx::query(
        "INSERT INTO bing_connections (site_id, workspace_id, api_key_sealed, site_url, status)
         VALUES ($1, $2, $3, $4, 'connected')
         ON CONFLICT (site_id) DO UPDATE SET
             api_key_sealed = EXCLUDED.api_key_sealed,
             site_url = EXCLUDED.site_url,
             status = 'connected',
             error = NULL,
             synced_at = CASE WHEN bing_connections.site_url = EXCLUDED.site_url
                              THEN bing_connections.synced_at ELSE NULL END",
    )
    .bind(site_id)
    .bind(workspace_id)
    .bind(sealed_key)
    .bind(site_url)
    .execute(&mut *tx)
    .await
    .map_err(internal)?;
    tx.commit().await.map_err(internal)
}

pub async fn mark_needs_reconnecting(
    pool: &PgPool,
    workspace_id: Uuid,
    site_id: Uuid,
    message: &str,
) -> Result<(), AppError> {
    sqlx::query(
        "UPDATE bing_connections SET status = 'needs_reconnecting', error = $3
         WHERE workspace_id = $1 AND site_id = $2",
    )
    .bind(workspace_id)
    .bind(site_id)
    .bind(message)
    .execute(pool)
    .await
    .map_err(internal)?;
    Ok(())
}

/// Keeps a plain sentence about the last failed sync without changing status.
pub async fn record_error(
    pool: &PgPool,
    workspace_id: Uuid,
    site_id: Uuid,
    message: &str,
) -> Result<(), AppError> {
    sqlx::query("UPDATE bing_connections SET error = $3 WHERE workspace_id = $1 AND site_id = $2")
        .bind(workspace_id)
        .bind(site_id)
        .bind(message)
        .execute(pool)
        .await
        .map_err(internal)?;
    Ok(())
}

async fn delete_data(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    workspace_id: Uuid,
    site_id: Uuid,
) -> Result<(), AppError> {
    sqlx::query("DELETE FROM backlink_sources WHERE workspace_id = $1 AND site_id = $2")
        .bind(workspace_id)
        .bind(site_id)
        .execute(&mut **tx)
        .await
        .map_err(internal)?;
    sqlx::query("DELETE FROM backlink_targets WHERE workspace_id = $1 AND site_id = $2")
        .bind(workspace_id)
        .bind(site_id)
        .execute(&mut **tx)
        .await
        .map_err(internal)?;
    Ok(())
}

/// Forgets the connection, its key and every row it brought in. Returns
/// whether there was a connection.
pub async fn delete_connection(
    pool: &PgPool,
    workspace_id: Uuid,
    site_id: Uuid,
) -> Result<bool, AppError> {
    let mut tx = pool.begin().await.map_err(internal)?;
    delete_data(&mut tx, workspace_id, site_id).await?;
    let removed =
        sqlx::query("DELETE FROM bing_connections WHERE workspace_id = $1 AND site_id = $2")
            .bind(workspace_id)
            .bind(site_id)
            .execute(&mut *tx)
            .await
            .map_err(internal)?
            .rows_affected();
    tx.commit().await.map_err(internal)?;
    Ok(removed > 0)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewTarget {
    pub url: String,
    pub count: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewSource {
    pub target_url: String,
    pub source_url: String,
    pub anchor_text: String,
}

/// Everything one sync fetched.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncData {
    pub targets: Vec<NewTarget>,
    pub sources: Vec<NewSource>,
}

/// Swaps in a sync's data, all or nothing. The connection row is locked and
/// must still be `connected`, so a disconnect during a sync is not undone by
/// it. Returns whether the data was stored.
pub async fn replace_data(
    pool: &PgPool,
    workspace_id: Uuid,
    site_id: Uuid,
    data: &SyncData,
) -> Result<bool, AppError> {
    let mut tx = pool.begin().await.map_err(internal)?;
    let live: Option<i32> = sqlx::query_scalar(
        "SELECT 1 FROM bing_connections
         WHERE workspace_id = $1 AND site_id = $2 AND status = 'connected' FOR UPDATE",
    )
    .bind(workspace_id)
    .bind(site_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(internal)?;
    if live.is_none() {
        return Ok(false);
    }
    delete_data(&mut tx, workspace_id, site_id).await?;
    let urls: Vec<&str> = data.targets.iter().map(|t| t.url.as_str()).collect();
    let counts: Vec<i64> = data.targets.iter().map(|t| t.count).collect();
    sqlx::query(
        "INSERT INTO backlink_targets (workspace_id, site_id, target_url, count)
         SELECT $1, $2, u, c FROM UNNEST($3::text[], $4::bigint[]) AS t(u, c)
         ON CONFLICT (site_id, target_url) DO NOTHING",
    )
    .bind(workspace_id)
    .bind(site_id)
    .bind(&urls)
    .bind(&counts)
    .execute(&mut *tx)
    .await
    .map_err(internal)?;
    let targets: Vec<&str> = data.sources.iter().map(|s| s.target_url.as_str()).collect();
    let sources: Vec<&str> = data.sources.iter().map(|s| s.source_url.as_str()).collect();
    let anchors: Vec<&str> = data
        .sources
        .iter()
        .map(|s| s.anchor_text.as_str())
        .collect();
    sqlx::query(
        "INSERT INTO backlink_sources (workspace_id, site_id, target_url, source_url, anchor_text)
         SELECT $1, $2, t, s, a FROM UNNEST($3::text[], $4::text[], $5::text[]) AS r(t, s, a)",
    )
    .bind(workspace_id)
    .bind(site_id)
    .bind(&targets)
    .bind(&sources)
    .bind(&anchors)
    .execute(&mut *tx)
    .await
    .map_err(internal)?;
    sqlx::query(
        "UPDATE bing_connections SET synced_at = now(), error = NULL
         WHERE workspace_id = $1 AND site_id = $2",
    )
    .bind(workspace_id)
    .bind(site_id)
    .execute(&mut *tx)
    .await
    .map_err(internal)?;
    tx.commit().await.map_err(internal)?;
    Ok(true)
}

#[derive(Debug, sqlx::FromRow)]
pub struct TargetRow {
    pub target_url: String,
    pub count: i64,
}

#[derive(Debug, sqlx::FromRow)]
pub struct SourceRow {
    pub source_url: String,
    pub anchor_text: String,
}

/// One page of targets, most linked first, with how many there are.
pub async fn list_targets(
    pool: &PgPool,
    workspace_id: Uuid,
    site_id: Uuid,
    page: i64,
    limit: i64,
) -> Result<(Vec<TargetRow>, i64), AppError> {
    let rows = sqlx::query_as(
        "SELECT target_url, count FROM backlink_targets
         WHERE workspace_id = $1 AND site_id = $2
         ORDER BY count DESC, target_url LIMIT $3 OFFSET $4",
    )
    .bind(workspace_id)
    .bind(site_id)
    .bind(limit)
    .bind((page - 1) * limit)
    .fetch_all(pool)
    .await
    .map_err(internal)?;
    let total = sqlx::query_scalar(
        "SELECT COUNT(*) FROM backlink_targets WHERE workspace_id = $1 AND site_id = $2",
    )
    .bind(workspace_id)
    .bind(site_id)
    .fetch_one(pool)
    .await
    .map_err(internal)?;
    Ok((rows, total))
}

pub async fn list_sources(
    pool: &PgPool,
    workspace_id: Uuid,
    site_id: Uuid,
    target_url: &str,
) -> Result<Vec<SourceRow>, AppError> {
    sqlx::query_as(
        "SELECT source_url, anchor_text FROM backlink_sources
         WHERE workspace_id = $1 AND site_id = $2 AND target_url = $3
         ORDER BY source_url, anchor_text",
    )
    .bind(workspace_id)
    .bind(site_id)
    .bind(target_url)
    .fetch_all(pool)
    .await
    .map_err(internal)
}

/// The totals the metrics block shows: the sum of counts, the number of
/// targets and the five most linked.
pub async fn summary(
    pool: &PgPool,
    workspace_id: Uuid,
    site_id: Uuid,
) -> Result<(i64, i64, Vec<TargetRow>), AppError> {
    let (total, targets): (i64, i64) = sqlx::query_as(
        "SELECT COALESCE(SUM(count), 0)::bigint, COUNT(*)
         FROM backlink_targets WHERE workspace_id = $1 AND site_id = $2",
    )
    .bind(workspace_id)
    .bind(site_id)
    .fetch_one(pool)
    .await
    .map_err(internal)?;
    let (top, _) = list_targets(pool, workspace_id, site_id, 1, 5).await?;
    Ok((total, targets, top))
}
