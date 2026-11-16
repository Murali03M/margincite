//! Search Console connections, sign-in states and the synced rows. Every
//! function takes the workspace and filters by it, except the callback's
//! `peek_state`, which learns the workspace from the state itself.

use sqlx::PgPool;
use time::{Date, OffsetDateTime};
use uuid::Uuid;

use super::oauth::{PROVIDER, STATE_MINUTES};
use crate::error::AppError;

fn internal(err: sqlx::Error) -> AppError {
    AppError::Internal(err.into())
}

/// One site's connection. Deliberately no `Debug`: it holds the sealed token.
#[derive(sqlx::FromRow)]
pub struct Connection {
    pub refresh_token_sealed: String,
    pub property: Option<String>,
    pub status: String,
    pub error: Option<String>,
    pub synced_at: Option<OffsetDateTime>,
}

/// A sign-in that left and may come back.
#[derive(Debug, sqlx::FromRow)]
pub struct PendingState {
    pub workspace_id: Uuid,
    pub site_id: Uuid,
    pub fresh: bool,
    /// SHA-256 of the nonce held by the browser that started the sign-in.
    pub nonce_hash: Option<String>,
}

/// Records that this site's owner is about to sign in. Also forgets attempts
/// that can never come back, so the table stays small.
pub async fn create_state(
    pool: &PgPool,
    workspace_id: Uuid,
    site_id: Uuid,
    nonce_hash: &str,
) -> Result<Uuid, AppError> {
    sqlx::query("DELETE FROM oauth_states WHERE created_at < now() - interval '1 hour'")
        .execute(pool)
        .await
        .map_err(internal)?;
    let id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO oauth_states (id, workspace_id, site_id, provider, nonce_hash)
         VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(id)
    .bind(workspace_id)
    .bind(site_id)
    .bind(PROVIDER)
    .bind(nonce_hash)
    .execute(pool)
    .await
    .map_err(internal)?;
    Ok(id)
}

/// Looks at a state without using it up. Only a state whose site still
/// exists in the state's own workspace is found.
pub async fn peek_state(pool: &PgPool, id: Uuid) -> Result<Option<PendingState>, AppError> {
    sqlx::query_as(
        "SELECT o.workspace_id, o.site_id,
                o.created_at > now() - ($2 * interval '1 minute') AS fresh,
                o.nonce_hash
         FROM oauth_states o
         JOIN sites s ON s.id = o.site_id AND s.workspace_id = o.workspace_id
         WHERE o.id = $1 AND o.provider = $3",
    )
    .bind(id)
    .bind(STATE_MINUTES)
    .bind(PROVIDER)
    .fetch_optional(pool)
    .await
    .map_err(internal)
}

/// Uses the state up and keeps the sealed token, in one transaction. `None`
/// when the state was already used, has expired or no longer matches a site,
/// and then nothing is stored. Otherwise whether the site already had a
/// property, which means a sync can start straight away.
pub async fn store_connection(
    pool: &PgPool,
    state_id: Uuid,
    sealed_token: &str,
) -> Result<Option<bool>, AppError> {
    let mut tx = pool.begin().await.map_err(internal)?;
    let claimed: Option<(Uuid, Uuid)> = sqlx::query_as(
        "DELETE FROM oauth_states o
         USING sites s
         WHERE o.id = $1 AND o.provider = $3
           AND o.created_at > now() - ($2 * interval '1 minute')
           AND s.id = o.site_id AND s.workspace_id = o.workspace_id
         RETURNING o.workspace_id, o.site_id",
    )
    .bind(state_id)
    .bind(STATE_MINUTES)
    .bind(PROVIDER)
    .fetch_optional(&mut *tx)
    .await
    .map_err(internal)?;
    let Some((workspace_id, site_id)) = claimed else {
        return Ok(None);
    };
    let property: Option<String> = sqlx::query_scalar(
        "INSERT INTO google_connections (site_id, workspace_id, refresh_token_sealed, status)
         VALUES ($1, $2, $3, 'choose_property')
         ON CONFLICT (site_id) DO UPDATE SET
             refresh_token_sealed = EXCLUDED.refresh_token_sealed,
             status = CASE WHEN google_connections.property IS NULL
                           THEN 'choose_property' ELSE 'connected' END,
             error = NULL
         RETURNING property",
    )
    .bind(site_id)
    .bind(workspace_id)
    .bind(sealed_token)
    .fetch_one(&mut *tx)
    .await
    .map_err(internal)?;
    tx.commit().await.map_err(internal)?;
    Ok(Some(property.is_some()))
}

pub async fn get_connection(
    pool: &PgPool,
    workspace_id: Uuid,
    site_id: Uuid,
) -> Result<Option<Connection>, AppError> {
    sqlx::query_as(
        "SELECT refresh_token_sealed, property, status, error, synced_at
         FROM google_connections WHERE workspace_id = $1 AND site_id = $2",
    )
    .bind(workspace_id)
    .bind(site_id)
    .fetch_optional(pool)
    .await
    .map_err(internal)
}

/// Chooses the property. A different property than before drops the old
/// property's rows at once, so they are never shown as the new one's.
pub async fn set_property(
    pool: &PgPool,
    workspace_id: Uuid,
    site_id: Uuid,
    property: &str,
) -> Result<(), AppError> {
    let mut tx = pool.begin().await.map_err(internal)?;
    let previous: Option<Option<String>> = sqlx::query_scalar(
        "SELECT property FROM google_connections
         WHERE workspace_id = $1 AND site_id = $2 FOR UPDATE",
    )
    .bind(workspace_id)
    .bind(site_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(internal)?;
    let Some(previous) = previous else {
        return Err(AppError::NotFound);
    };
    if previous.as_deref() != Some(property) {
        delete_data(&mut tx, workspace_id, site_id).await?;
    }
    sqlx::query(
        "UPDATE google_connections
         SET property = $3, status = 'connected', error = NULL,
             synced_at = CASE WHEN property IS DISTINCT FROM $3 THEN NULL ELSE synced_at END
         WHERE workspace_id = $1 AND site_id = $2",
    )
    .bind(workspace_id)
    .bind(site_id)
    .bind(property)
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
        "UPDATE google_connections SET status = 'needs_reconnecting', error = $3
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
    sqlx::query(
        "UPDATE google_connections SET error = $3 WHERE workspace_id = $1 AND site_id = $2",
    )
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
    sqlx::query("DELETE FROM search_rows WHERE workspace_id = $1 AND site_id = $2")
        .bind(workspace_id)
        .bind(site_id)
        .execute(&mut **tx)
        .await
        .map_err(internal)?;
    sqlx::query("DELETE FROM search_daily WHERE workspace_id = $1 AND site_id = $2")
        .bind(workspace_id)
        .bind(site_id)
        .execute(&mut **tx)
        .await
        .map_err(internal)?;
    sqlx::query("DELETE FROM search_previous WHERE workspace_id = $1 AND site_id = $2")
        .bind(workspace_id)
        .bind(site_id)
        .execute(&mut **tx)
        .await
        .map_err(internal)?;
    Ok(())
}

/// Forgets the connection, its token and every row it brought in. Returns
/// whether there was a connection.
pub async fn delete_connection(
    pool: &PgPool,
    workspace_id: Uuid,
    site_id: Uuid,
) -> Result<bool, AppError> {
    let mut tx = pool.begin().await.map_err(internal)?;
    delete_data(&mut tx, workspace_id, site_id).await?;
    sqlx::query("DELETE FROM oauth_states WHERE workspace_id = $1 AND site_id = $2")
        .bind(workspace_id)
        .bind(site_id)
        .execute(&mut *tx)
        .await
        .map_err(internal)?;
    let removed =
        sqlx::query("DELETE FROM google_connections WHERE workspace_id = $1 AND site_id = $2")
            .bind(workspace_id)
            .bind(site_id)
            .execute(&mut *tx)
            .await
            .map_err(internal)?
            .rows_affected();
    tx.commit().await.map_err(internal)?;
    Ok(removed > 0)
}

#[derive(Debug, Clone, PartialEq)]
pub struct NewRow {
    pub query: String,
    pub page: String,
    pub clicks: i64,
    pub impressions: i64,
    pub ctr: f64,
    pub position: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DailyTotal {
    pub date: Date,
    pub clicks: i64,
    pub impressions: i64,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Previous {
    pub clicks: i64,
    pub impressions: i64,
    pub keywords: i64,
}

/// Everything one sync fetched.
#[derive(Debug, Clone, PartialEq)]
pub struct SyncData {
    pub rows: Vec<NewRow>,
    pub daily: Vec<DailyTotal>,
    pub previous: Previous,
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
        "SELECT 1 FROM google_connections
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
    insert_rows(&mut tx, workspace_id, site_id, &data.rows).await?;
    let dates: Vec<Date> = data.daily.iter().map(|d| d.date).collect();
    let clicks: Vec<i64> = data.daily.iter().map(|d| d.clicks).collect();
    let impressions: Vec<i64> = data.daily.iter().map(|d| d.impressions).collect();
    sqlx::query(
        "INSERT INTO search_daily (workspace_id, site_id, date, clicks, impressions)
         SELECT $1, $2, d, c, i FROM UNNEST($3::date[], $4::bigint[], $5::bigint[]) AS t(d, c, i)
         ON CONFLICT (site_id, date) DO NOTHING",
    )
    .bind(workspace_id)
    .bind(site_id)
    .bind(&dates)
    .bind(&clicks)
    .bind(&impressions)
    .execute(&mut *tx)
    .await
    .map_err(internal)?;
    sqlx::query(
        "INSERT INTO search_previous (site_id, workspace_id, clicks, impressions, keywords)
         VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(site_id)
    .bind(workspace_id)
    .bind(data.previous.clicks)
    .bind(data.previous.impressions)
    .bind(data.previous.keywords)
    .execute(&mut *tx)
    .await
    .map_err(internal)?;
    sqlx::query(
        "UPDATE google_connections SET synced_at = now(), error = NULL
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

async fn insert_rows(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    workspace_id: Uuid,
    site_id: Uuid,
    rows: &[NewRow],
) -> Result<(), AppError> {
    let queries: Vec<&str> = rows.iter().map(|r| r.query.as_str()).collect();
    let pages: Vec<&str> = rows.iter().map(|r| r.page.as_str()).collect();
    let clicks: Vec<i64> = rows.iter().map(|r| r.clicks).collect();
    let impressions: Vec<i64> = rows.iter().map(|r| r.impressions).collect();
    let ctrs: Vec<f64> = rows.iter().map(|r| r.ctr).collect();
    let positions: Vec<f64> = rows.iter().map(|r| r.position).collect();
    sqlx::query(
        "INSERT INTO search_rows (workspace_id, site_id, query, page, clicks, impressions, ctr, position)
         SELECT $1, $2, q, p, c, i, t, o
         FROM UNNEST($3::text[], $4::text[], $5::bigint[], $6::bigint[], $7::float8[], $8::float8[])
              AS r(q, p, c, i, t, o)",
    )
    .bind(workspace_id)
    .bind(site_id)
    .bind(&queries)
    .bind(&pages)
    .bind(&clicks)
    .bind(&impressions)
    .bind(&ctrs)
    .bind(&positions)
    .execute(&mut **tx)
    .await
    .map_err(internal)?;
    Ok(())
}
