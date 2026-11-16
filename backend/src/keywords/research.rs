//! Keyword research endpoints, paid through DataForSEO: refreshing the search
//! volume of a site's keywords, and keyword ideas for a seed. Neither asks
//! anything without the workspace's own login, and a refusal costs nothing.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::repo;
use super::volume_state;
use super::volumes::{self, USAGE_SOURCE};
use crate::auth::Principal;
use crate::envelope::Envelope;
use crate::error::AppError;
use crate::integrations::dataforseo::keywords::{fits, KeywordMetrics};
use crate::integrations::dataforseo::{locations, pricing, Credentials};
use crate::request::Body;
use crate::settings::resolve;
use crate::sites::repo as sites;
use crate::AppState;

/// Ideas returned for one search.
const MAX_IDEAS: usize = 50;
const IDEAS_KIND: &str = "keyword_ideas";

pub fn routes() -> Router<AppState> {
    Router::new()
        .route(
            "/api/v1/sites/{id}/keywords/volumes",
            get(volume_status).post(refresh_volumes),
        )
        .route("/api/v1/sites/{id}/keywords/ideas", post(find_ideas))
}

#[derive(Serialize)]
struct Queued {
    queued: bool,
}

async fn refresh_volumes(
    State(state): State<AppState>,
    principal: Principal,
    Path(site_id): Path<Uuid>,
) -> Result<(StatusCode, Json<Envelope<Queued>>), AppError> {
    let ws = principal.workspace_id;
    sites::get(&state.pool, ws, site_id).await?;
    if resolve::dataforseo(&state.pool, &state.config, ws)
        .await?
        .is_none()
    {
        return Err(AppError::Invalid(
            "Search volume needs a DataForSEO login. Add it in Settings, then try again.".into(),
        ));
    }
    if repo::list(&state.pool, ws, site_id).await?.is_empty() {
        return Err(AppError::Invalid(
            "Add at least one keyword before refreshing search volume.".into(),
        ));
    }
    start_refresh(&state, ws, site_id).await?;
    Ok((
        StatusCode::ACCEPTED,
        Json(Envelope::ok(Queued { queued: true })),
    ))
}

/// Reserves one of the site's refreshes for today and queues it. When one was
/// already waiting nothing new is paid for, so the reservation is given back.
async fn start_refresh(state: &AppState, ws: Uuid, site_id: Uuid) -> Result<(), AppError> {
    let reservation = volume_state::reserve(&state.pool, ws, site_id).await?;
    match volumes::enqueue(&state.pool, ws, site_id).await {
        Ok(true) => volume_state::clear_failure(&state.pool, ws, site_id).await,
        Ok(false) => volume_state::give_back(&state.pool, reservation).await,
        Err(err) => {
            volume_state::give_back(&state.pool, reservation).await;
            return Err(AppError::Internal(err));
        }
    }
    Ok(())
}

async fn volume_status(
    State(state): State<AppState>,
    principal: Principal,
    Path(site_id): Path<Uuid>,
) -> Result<Json<Envelope<volume_state::Status>>, AppError> {
    let ws = principal.workspace_id;
    sites::get(&state.pool, ws, site_id).await?;
    Ok(Json(Envelope::ok(
        volume_state::status(&state.pool, ws, site_id).await?,
    )))
}

#[derive(Deserialize)]
struct IdeasBody {
    #[serde(default)]
    seed: String,
}

#[derive(Serialize)]
struct Ideas {
    ideas: Vec<KeywordMetrics>,
}

/// The seed as typed, with spaces collapsed, when DataForSEO would take it and
/// it has a letter or a number in it.
fn clean_seed(raw: &str) -> Result<String, AppError> {
    let seed = raw.split_whitespace().collect::<Vec<_>>().join(" ");
    if seed.is_empty() {
        return Err(AppError::Invalid(
            "Enter a word or phrase to search for.".into(),
        ));
    }
    if !fits(&seed) {
        return Err(AppError::Invalid(
            "A search can be at most 80 characters and 10 words.".into(),
        ));
    }
    if !seed.chars().any(char::is_alphanumeric) {
        return Err(AppError::Invalid(
            "A search needs at least one letter or number.".into(),
        ));
    }
    Ok(seed)
}

/// Biggest volume first, ideas without a volume last, each keyword once.
fn ranked(found: Vec<KeywordMetrics>) -> Vec<KeywordMetrics> {
    let mut seen = std::collections::HashSet::new();
    let mut ideas: Vec<KeywordMetrics> = found
        .into_iter()
        .filter(|idea| seen.insert(idea.keyword.to_lowercase()))
        .collect();
    ideas.sort_by(|a, b| {
        b.search_volume
            .cmp(&a.search_volume)
            .then_with(|| a.keyword.cmp(&b.keyword))
    });
    ideas.truncate(MAX_IDEAS);
    ideas
}

async fn find_ideas(
    State(state): State<AppState>,
    principal: Principal,
    Path(site_id): Path<Uuid>,
    Body(body): Body<IdeasBody>,
) -> Result<Json<Envelope<Ideas>>, AppError> {
    let ws = principal.workspace_id;
    let place = sites::get_settings(&state.pool, ws, site_id).await?;
    let seed = clean_seed(&body.seed)?;
    let Some((login, password)) = resolve::dataforseo(&state.pool, &state.config, ws).await? else {
        return Err(AppError::Invalid(
            "Keyword ideas need a DataForSEO login. Add it in Settings, then try again.".into(),
        ));
    };
    let location_code = locations::location_code(&place.country).ok_or_else(|| {
        AppError::Invalid(
            "This site's country cannot be searched. Change it in the site's settings.".into(),
        )
    })?;
    repo::reserve_idea_search(&state.pool, ws).await?;
    let credentials = Credentials {
        login: &login,
        password: &password,
    };
    let language = locations::ads_language(&place.language);
    match state
        .dataforseo
        .keyword_ideas(&credentials, &seed, location_code, language)
        .await
    {
        Ok(found) => {
            crate::usage::record_quietly(
                &state.pool,
                ws,
                USAGE_SOURCE,
                IDEAS_KIND,
                1,
                Some(pricing::ADS_REQUEST_USD),
            )
            .await;
            Ok(Json(Envelope::ok(Ideas {
                ideas: ranked(found),
            })))
        }
        Err(err) => {
            if err.is_fatal() {
                volumes::mark_setting(&state.pool, ws, err).await;
            }
            Err(AppError::BadGateway(err.to_string()))
        }
    }
}
