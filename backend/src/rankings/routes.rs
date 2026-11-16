//! Ranking endpoints. Everything needs a bearer token and a site in the
//! caller's own workspace.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Serialize;
use serde_json::Value;
use time::OffsetDateTime;
use uuid::Uuid;

use super::repo::{self, Location, StoredCheck, HISTORY_POINTS};
use super::run;
use crate::auth::Principal;
use crate::envelope::Envelope;
use crate::error::AppError;
use crate::keywords::repo as keywords;
use crate::sites::repo as sites;
use crate::AppState;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/api/v1/sites/{id}/rankings", get(rankings))
        .route("/api/v1/sites/{id}/rankings/check", post(check))
        .route("/api/v1/sites/{id}/ai-overviews", get(ai_overviews))
}

#[derive(Serialize)]
struct Point {
    #[serde(with = "time::serde::rfc3339")]
    at: OffsetDateTime,
    position: Option<i32>,
}

#[derive(Serialize)]
struct KeywordRanking {
    keyword_id: Uuid,
    keyword: String,
    /// `null` with a history means not in the top 100.
    position: Option<i32>,
    previous: Option<i32>,
    url: Option<String>,
    ai_overview: String,
    top: Value,
    history: Vec<Point>,
}

#[derive(Serialize)]
struct Rankings {
    location: Location,
    /// When the newest finished check at this location finished.
    #[serde(with = "time::serde::rfc3339::option")]
    checked_at: Option<OffsetDateTime>,
    /// The site's newest run of any outcome, `null` when there never was one.
    /// Its status is `queued`, `running`, `completed` or `failed`.
    last_run: Option<repo::LastRun>,
    keywords: Vec<KeywordRanking>,
}

async fn rankings(
    State(state): State<AppState>,
    principal: Principal,
    Path(site_id): Path<Uuid>,
) -> Result<Json<Envelope<Rankings>>, AppError> {
    let ws = principal.workspace_id;
    sites::get(&state.pool, ws, site_id).await?;
    let place = repo::location(&state.pool, ws, site_id).await?;
    let mut stored = repo::recent(&state.pool, ws, site_id, &place, HISTORY_POINTS).await?;
    let rows = keywords::list(&state.pool, ws, site_id).await?;
    let keywords = rows
        .into_iter()
        .map(|row| {
            let checks = stored
                .remove(&row.keyword.to_lowercase())
                .unwrap_or_default();
            present(row.id, row.keyword, checks)
        })
        .collect();
    Ok(Json(Envelope::ok(Rankings {
        checked_at: repo::finished_at(&state.pool, ws, site_id, &place).await?,
        location: place,
        last_run: repo::last_run(&state.pool, ws, site_id).await?,
        keywords,
    })))
}

/// One keyword's row from its stored results, newest first.
fn present(keyword_id: Uuid, keyword: String, checks: Vec<StoredCheck>) -> KeywordRanking {
    let previous = checks.get(1).and_then(|earlier| earlier.position);
    let history = checks
        .iter()
        .rev()
        .map(|check| Point {
            at: check.checked_at,
            position: check.position,
        })
        .collect();
    match checks.into_iter().next() {
        Some(latest) => KeywordRanking {
            keyword_id,
            keyword,
            position: latest.position,
            previous,
            url: latest.url,
            ai_overview: latest.ai_overview,
            top: latest.top,
            history,
        },
        None => KeywordRanking {
            keyword_id,
            keyword,
            position: None,
            previous: None,
            url: None,
            ai_overview: "unknown".into(),
            top: Value::Array(Vec::new()),
            history,
        },
    }
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
    if !run::has_login(&state.pool, &state.config, ws).await? {
        return Err(AppError::Invalid(run::NO_LOGIN.into()));
    }
    let count = keywords::list(&state.pool, ws, site_id).await?.len();
    if count == 0 {
        return Err(AppError::Invalid(
            "Add at least one keyword before checking rankings.".into(),
        ));
    }
    let total = i32::try_from(count).unwrap_or(i32::MAX);
    let created = repo::create_run(&state.pool, ws, site_id, repo::Trigger::Manual, total).await?;
    if let Err(err) = run::enqueue_manual(&state.pool, ws, site_id, created.id).await {
        // Without a job the run would hold the site's one place forever.
        let _ = repo::finish_run(
            &state.pool,
            created.id,
            "failed",
            Some("Could not be queued."),
        )
        .await;
        return Err(AppError::Internal(err));
    }
    Ok((
        StatusCode::ACCEPTED,
        Json(Envelope::ok(Queued { queued: true })),
    ))
}

#[derive(Serialize)]
struct Overview {
    keyword_id: Uuid,
    keyword: String,
    status: String,
    cited_domains: Vec<String>,
    your_urls: Vec<String>,
    #[serde(with = "time::serde::rfc3339")]
    checked_at: OffsetDateTime,
}

/// What Google's AI Overview did for each keyword that has been checked.
async fn ai_overviews(
    State(state): State<AppState>,
    principal: Principal,
    Path(site_id): Path<Uuid>,
) -> Result<Json<Envelope<Vec<Overview>>>, AppError> {
    let ws = principal.workspace_id;
    sites::get(&state.pool, ws, site_id).await?;
    let place = repo::location(&state.pool, ws, site_id).await?;
    let mut stored = repo::recent(&state.pool, ws, site_id, &place, 1).await?;
    let rows = keywords::list(&state.pool, ws, site_id).await?;
    let found = rows
        .into_iter()
        .filter_map(|row| {
            let latest = stored
                .remove(&row.keyword.to_lowercase())?
                .into_iter()
                .next()?;
            Some(Overview {
                keyword_id: row.id,
                keyword: row.keyword,
                status: latest.ai_overview,
                cited_domains: latest.ai_cited_domains.0,
                your_urls: latest.ai_your_urls.0,
                checked_at: latest.checked_at,
            })
        })
        .collect();
    Ok(Json(Envelope::ok(found)))
}
