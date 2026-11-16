use sqlx::PgPool;
use uuid::Uuid;

use super::scope::SiteScope;
use crate::error::AppError;

#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow)]
pub struct Site {
    pub id: Uuid,
    pub root_url: String,
    pub host: String,
    pub path_prefix: String,
    pub display_name: String,
    pub page_cap: i32,
}

pub async fn create(
    pool: &PgPool,
    workspace_id: Uuid,
    scope: &SiteScope,
    name: Option<String>,
) -> Result<Site, AppError> {
    let display_name = name
        .filter(|name| !name.trim().is_empty())
        .unwrap_or_else(|| scope.host.clone());
    let inserted = sqlx::query_as::<_, Site>(
        "INSERT INTO sites (id, workspace_id, root_url, host, path_prefix, display_name)
         VALUES ($1, $2, $3, $4, $5, $6)
         RETURNING id, root_url, host, path_prefix, display_name, page_cap",
    )
    .bind(Uuid::new_v4())
    .bind(workspace_id)
    .bind(scope.root.as_str())
    .bind(&scope.host)
    .bind(&scope.path_prefix)
    .bind(display_name)
    .fetch_one(pool)
    .await;
    match inserted {
        Ok(site) => Ok(site),
        Err(err) if unique(&err) => Err(AppError::Conflict(
            "This site is already in the workspace.".into(),
        )),
        Err(err) => Err(AppError::Internal(err.into())),
    }
}

pub async fn count(pool: &PgPool, workspace_id: Uuid) -> Result<i64, AppError> {
    sqlx::query_scalar("SELECT COUNT(*) FROM sites WHERE workspace_id = $1")
        .bind(workspace_id)
        .fetch_one(pool)
        .await
        .map_err(|err| AppError::Internal(err.into()))
}

pub async fn list(pool: &PgPool, workspace_id: Uuid) -> Result<Vec<Site>, AppError> {
    sqlx::query_as(
        "SELECT id, root_url, host, path_prefix, display_name, page_cap
         FROM sites WHERE workspace_id = $1 ORDER BY created_at",
    )
    .bind(workspace_id)
    .fetch_all(pool)
    .await
    .map_err(|err| AppError::Internal(err.into()))
}

pub async fn get(pool: &PgPool, workspace_id: Uuid, id: Uuid) -> Result<Site, AppError> {
    sqlx::query_as(
        "SELECT id, root_url, host, path_prefix, display_name, page_cap
         FROM sites WHERE id = $1 AND workspace_id = $2",
    )
    .bind(id)
    .bind(workspace_id)
    .fetch_optional(pool)
    .await
    .map_err(|err| AppError::Internal(err.into()))?
    .ok_or(AppError::NotFound)
}

pub async fn delete(pool: &PgPool, workspace_id: Uuid, id: Uuid) -> Result<(), AppError> {
    let result = sqlx::query("DELETE FROM sites WHERE id = $1 AND workspace_id = $2")
        .bind(id)
        .bind(workspace_id)
        .execute(pool)
        .await
        .map_err(|err| AppError::Internal(err.into()))?;
    if result.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }
    Ok(())
}

fn unique(err: &sqlx::Error) -> bool {
    err.as_database_error()
        .is_some_and(|db| db.is_unique_violation())
}

/// What a person can change about a site.
#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow)]
pub struct SiteSettings {
    pub display_name: String,
    pub page_cap: i32,
    pub country: String,
    pub language: String,
    pub device: String,
    pub track_rankings: bool,
    pub ranking_frequency: String,
}

/// A change to some of a site's settings; `None` leaves a field alone.
#[derive(Debug, Default)]
pub struct SiteSettingsChange {
    pub display_name: Option<String>,
    pub page_cap: Option<i32>,
    pub country: Option<String>,
    pub language: Option<String>,
    pub device: Option<String>,
    pub track_rankings: Option<bool>,
    pub ranking_frequency: Option<String>,
}

pub async fn get_settings(
    pool: &PgPool,
    workspace_id: Uuid,
    id: Uuid,
) -> Result<SiteSettings, AppError> {
    sqlx::query_as(
        "SELECT display_name, page_cap, country, language, device, track_rankings,
                ranking_frequency
         FROM sites WHERE id = $1 AND workspace_id = $2",
    )
    .bind(id)
    .bind(workspace_id)
    .fetch_optional(pool)
    .await
    .map_err(|err| AppError::Internal(err.into()))?
    .ok_or(AppError::NotFound)
}

pub async fn update_settings(
    pool: &PgPool,
    workspace_id: Uuid,
    id: Uuid,
    change: &SiteSettingsChange,
) -> Result<SiteSettings, AppError> {
    sqlx::query_as(
        "UPDATE sites SET
            display_name = COALESCE($3, display_name),
            page_cap = COALESCE($4, page_cap),
            country = COALESCE($5, country),
            language = COALESCE($6, language),
            device = COALESCE($7, device),
            track_rankings = COALESCE($8, track_rankings),
            ranking_frequency = COALESCE($9, ranking_frequency)
         WHERE id = $1 AND workspace_id = $2
         RETURNING display_name, page_cap, country, language, device, track_rankings,
                   ranking_frequency",
    )
    .bind(id)
    .bind(workspace_id)
    .bind(&change.display_name)
    .bind(change.page_cap)
    .bind(&change.country)
    .bind(&change.language)
    .bind(&change.device)
    .bind(change.track_rankings)
    .bind(&change.ranking_frequency)
    .fetch_optional(pool)
    .await
    .map_err(|err| AppError::Internal(err.into()))?
    .ok_or(AppError::NotFound)
}
