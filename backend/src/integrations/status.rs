//! `GET /integrations`: what the server has been set up with.

use axum::extract::State;
use axum::routing::get;
use axum::{Json, Router};
use serde::Serialize;

use crate::auth::Principal;
use crate::envelope::Envelope;
use crate::error::AppError;
use crate::settings::resolve;
use crate::AppState;

pub fn routes() -> Router<AppState> {
    Router::new().route("/api/v1/integrations", get(integrations))
}

#[derive(Serialize)]
struct Google {
    configured: bool,
}

#[derive(Serialize)]
struct Engine {
    engine: &'static str,
    configured: bool,
    model: String,
}

#[derive(Serialize)]
struct Status {
    secret_key_set: bool,
    google: Google,
    engines: Vec<Engine>,
}

async fn integrations(
    State(state): State<AppState>,
    principal: Principal,
) -> Result<Json<Envelope<Status>>, AppError> {
    let config = &state.config;
    let saved = resolve::load(&state.pool, config, principal.workspace_id).await?;
    let settings = saved.engine_settings();
    let engines = settings
        .all()
        .into_iter()
        .map(|(engine, setting)| Engine {
            engine,
            configured: setting.configured(),
            model: setting.model.clone(),
        })
        .collect();
    Ok(Json(Envelope::ok(Status {
        secret_key_set: config.secret_key.is_some(),
        google: Google {
            configured: saved.google().0.is_some(),
        },
        engines,
    })))
}
