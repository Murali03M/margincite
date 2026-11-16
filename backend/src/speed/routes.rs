//! Page speed endpoints. Everything needs a bearer token and a site in the
//! caller's own workspace.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Serialize;
use time::OffsetDateTime;
use uuid::Uuid;

use super::repo::{self, PageResult};
use super::run;
use crate::auth::Principal;
use crate::envelope::Envelope;
use crate::error::AppError;
use crate::sites::repo as sites;
use crate::AppState;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/api/v1/sites/{id}/speed", get(speed))
        .route("/api/v1/sites/{id}/speed/check", post(check))
}

#[derive(Serialize)]
struct Speed {
    /// When the newest test was attempted, worked or not; `null` before the
    /// first one.
    #[serde(with = "time::serde::rfc3339::option")]
    checked_at: Option<OffsetDateTime>,
    pages: Vec<PageResult>,
}

async fn speed(
    State(state): State<AppState>,
    principal: Principal,
    Path(site_id): Path<Uuid>,
) -> Result<Json<Envelope<Speed>>, AppError> {
    let ws = principal.workspace_id;
    let site = sites::get(&state.pool, ws, site_id).await?;
    let pages = repo::list(&state.pool, ws, site_id, &site.root_url).await?;
    let checked_at = pages.iter().map(|page| page.attempted_at).max();
    Ok(Json(Envelope::ok(Speed { checked_at, pages })))
}

#[derive(Serialize)]
struct Queued {
    queued: bool,
}

async fn check(
    State(state): State<AppState>,
    principal: Principal,
    Path(site_id): Path<Uuid>,
) -> Result<(StatusCode, Json<Envelope<Queued>>), AppError> {
    let ws = principal.workspace_id;
    sites::get(&state.pool, ws, site_id).await?;
    repo::reserve_manual_check(&state.pool, ws, site_id).await?;
    run::enqueue_now(&state.pool, ws, site_id)
        .await
        .map_err(AppError::Internal)?;
    Ok((
        StatusCode::ACCEPTED,
        Json(Envelope::ok(Queued { queued: true })),
    ))
}
