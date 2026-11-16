use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use serde::Deserialize;
use time::OffsetDateTime;
use uuid::Uuid;

use super::repo;
use crate::auth::Principal;
use crate::envelope::{Envelope, Meta};
use crate::error::AppError;
use crate::jobs::queue;
use crate::request::Params;
use crate::AppState;

const DAILY_CRAWLS: i64 = 10;

#[derive(Deserialize)]
struct PageQuery {
    page: Option<i64>,
    limit: Option<i64>,
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route(
            "/api/v1/sites/{id}/crawls",
            get(list_crawls).post(start_crawl),
        )
        .route("/api/v1/crawls/{id}", get(get_crawl))
        .route("/api/v1/crawls/{id}/pages", get(list_pages))
        .route("/api/v1/pages/{id}", get(get_page))
}

pub async fn start_for_site(
    state: &AppState,
    workspace_id: Uuid,
    site_id: Uuid,
    trigger: &str,
) -> Result<Uuid, AppError> {
    let crawl_id = repo::create_crawl(&state.pool, workspace_id, site_id, trigger).await?;
    queue::enqueue(
        &state.pool,
        "crawl",
        &serde_json::json!({
            "workspace_id": workspace_id,
            "crawl_id": crawl_id
        }),
        OffsetDateTime::now_utc(),
    )
    .await
    .map_err(AppError::Internal)?;
    Ok(crawl_id)
}

/// A workspace may start 10 crawls a day, first crawls included.
pub async fn ensure_daily_allowance(
    pool: &sqlx::PgPool,
    workspace_id: Uuid,
) -> Result<(), AppError> {
    let used = repo::limited_crawls_today(pool, workspace_id).await?;
    if used >= DAILY_CRAWLS {
        return Err(AppError::DailyLimit(
            "You have used today's 10 crawls. Try again tomorrow.".into(),
        ));
    }
    Ok(())
}

async fn start_crawl(
    State(state): State<AppState>,
    principal: Principal,
    Path(site_id): Path<Uuid>,
) -> Result<(StatusCode, Json<Envelope<repo::Crawl>>), AppError> {
    crate::sites::repo::get(&state.pool, principal.workspace_id, site_id).await?;
    if repo::crawl_in_progress(&state.pool, principal.workspace_id, site_id).await? {
        return Err(AppError::Conflict(
            "A crawl of this site is already running.".into(),
        ));
    }
    crate::settings::routes::require_contact(&state, principal.workspace_id).await?;
    ensure_daily_allowance(&state.pool, principal.workspace_id).await?;
    let crawl_id = start_for_site(&state, principal.workspace_id, site_id, "manual").await?;
    let crawl = repo::get_crawl(&state.pool, principal.workspace_id, crawl_id).await?;
    Ok((StatusCode::ACCEPTED, Json(Envelope::ok(crawl))))
}

async fn list_crawls(
    State(state): State<AppState>,
    principal: Principal,
    Path(site_id): Path<Uuid>,
    Params(query): Params<PageQuery>,
) -> Result<Json<Envelope<Vec<repo::Crawl>>>, AppError> {
    crate::sites::repo::get(&state.pool, principal.workspace_id, site_id).await?;
    let (page, limit) = page_of(&query);
    let (crawls, total) = repo::list_crawls_page(
        &state.pool,
        principal.workspace_id,
        site_id,
        limit,
        (page - 1) * limit,
    )
    .await?;
    Ok(Json(Envelope::page(crawls, Meta { total, page, limit })))
}

async fn get_crawl(
    State(state): State<AppState>,
    principal: Principal,
    Path(id): Path<Uuid>,
) -> Result<Json<Envelope<repo::Crawl>>, AppError> {
    let crawl = repo::get_crawl(&state.pool, principal.workspace_id, id).await?;
    Ok(Json(Envelope::ok(crawl)))
}

async fn list_pages(
    State(state): State<AppState>,
    principal: Principal,
    Path(id): Path<Uuid>,
    Params(query): Params<PageQuery>,
) -> Result<Json<Envelope<Vec<repo::PageSummary>>>, AppError> {
    repo::get_crawl(&state.pool, principal.workspace_id, id).await?;
    let (page, limit) = page_of(&query);
    let (pages, total) = repo::list_pages_page(
        &state.pool,
        principal.workspace_id,
        id,
        limit,
        (page - 1) * limit,
    )
    .await?;
    Ok(Json(Envelope::page(pages, Meta { total, page, limit })))
}

async fn get_page(
    State(state): State<AppState>,
    principal: Principal,
    Path(id): Path<Uuid>,
) -> Result<Json<Envelope<repo::Page>>, AppError> {
    let page = repo::get_page(&state.pool, principal.workspace_id, id).await?;
    Ok(Json(Envelope::ok(page)))
}

fn page_of(query: &PageQuery) -> (i64, i64) {
    paging(query.page, query.limit)
}

/// Page number from 1 and a limit from 1 to 100 (20 when not given).
pub(crate) fn paging(page: Option<i64>, limit: Option<i64>) -> (i64, i64) {
    (page.unwrap_or(1).max(1), limit.unwrap_or(20).clamp(1, 100))
}
