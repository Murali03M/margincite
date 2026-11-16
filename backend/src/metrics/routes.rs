//! `GET /sites/{id}/metrics`.

use axum::extract::{Path, State};
use axum::routing::get;
use axum::{Json, Router};
use uuid::Uuid;

use super::Metrics;
use crate::auth::Principal;
use crate::envelope::Envelope;
use crate::error::AppError;
use crate::sites::repo as sites;
use crate::AppState;

pub fn routes() -> Router<AppState> {
    Router::new().route("/api/v1/sites/{id}/metrics", get(metrics))
}

async fn metrics(
    State(state): State<AppState>,
    principal: Principal,
    Path(site_id): Path<Uuid>,
) -> Result<Json<Envelope<Metrics>>, AppError> {
    let ws = principal.workspace_id;
    sites::get(&state.pool, ws, site_id).await?;
    Ok(Json(Envelope::ok(
        super::load(&state.pool, ws, site_id).await,
    )))
}
