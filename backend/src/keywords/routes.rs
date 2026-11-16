use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::{get, patch};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use url::Url;
use uuid::Uuid;

use super::landing::{self, LandingSource};
use super::repo::{self, KeywordRow};
use crate::auth::Principal;
use crate::checks::context::CrawlContext;
use crate::checks::keywords::words;
use crate::crawl::repo as crawls;
use crate::crawl::urls::normalize;
use crate::envelope::Envelope;
use crate::error::AppError;
use crate::integrations::google::search::{self, SearchNumbers};
use crate::integrations::google::sync::refresh_checks;
use crate::request::Body;
use crate::sites::repo::{self as sites, Site};
use crate::sites::scope::SiteScope;
use crate::AppState;

const MAX_KEYWORD_CHARS: usize = 100;
const MAX_URL_CHARS: usize = 2048;

pub fn routes() -> Router<AppState> {
    Router::new()
        .merge(super::research::routes())
        .route(
            "/api/v1/sites/{id}/keywords",
            get(list_keywords).post(create_keyword),
        )
        .route(
            "/api/v1/keywords/{id}",
            patch(update_keyword).delete(delete_keyword),
        )
}

#[derive(Serialize)]
struct KeywordOut {
    id: Uuid,
    keyword: String,
    target_url: Option<String>,
    landing_url: Option<String>,
    landing_source: Option<LandingSource>,
    /// From Search Console, last 28 days. `null` until it is connected.
    position: Option<f64>,
    clicks: Option<i64>,
    impressions: Option<i64>,
    /// Monthly searches, from Google Ads data via DataForSEO. `null` until
    /// refreshed, and when Google has no data.
    search_volume: Option<i32>,
    competition: Option<String>,
    cpc: Option<f64>,
}

/// The keywords with the page each one lands on, judged against the latest
/// completed crawl. Without a crawl there is no page to point at.
async fn present(
    state: &AppState,
    workspace_id: Uuid,
    site_id: Uuid,
    rows: Vec<KeywordRow>,
) -> Result<Vec<KeywordOut>, AppError> {
    let ctx = latest_context(state, workspace_id, site_id).await?;
    let stats = search_numbers(state, workspace_id, site_id).await?;
    Ok(rows
        .into_iter()
        .map(|row| {
            let chosen = ctx.as_ref().and_then(|ctx| {
                landing::choose(
                    &row.keyword,
                    row.target_url.as_deref(),
                    &ctx.pages,
                    ctx.search.as_ref(),
                )
            });
            let (landing_url, landing_source) = match chosen {
                Some((url, source)) => (Some(url), Some(source)),
                None => (None, None),
            };
            let shown = stats.as_ref().map(|stats| stats.for_keyword(&row.keyword));
            KeywordOut {
                id: row.id,
                keyword: row.keyword,
                target_url: row.target_url,
                landing_url,
                landing_source,
                position: shown.as_ref().and_then(|s| s.position),
                clicks: shown.as_ref().map(|s| s.clicks),
                impressions: shown.as_ref().map(|s| s.impressions),
                search_volume: row.search_volume,
                competition: row.competition,
                cpc: row.cpc,
            }
        })
        .collect())
}

/// The site's Search Console numbers by query, or `None` when it has none.
async fn search_numbers(
    state: &AppState,
    workspace_id: Uuid,
    site_id: Uuid,
) -> Result<Option<SearchNumbers>, AppError> {
    if !search::has_data(&state.pool, workspace_id, site_id).await? {
        return Ok(None);
    }
    let stats = search::query_stats(&state.pool, workspace_id, site_id).await?;
    Ok(Some(SearchNumbers::new(stats)))
}

async fn latest_context(
    state: &AppState,
    workspace_id: Uuid,
    site_id: Uuid,
) -> Result<Option<CrawlContext>, AppError> {
    let Some(crawl) = crawls::latest_completed(&state.pool, workspace_id, site_id).await? else {
        return Ok(None);
    };
    crawls::load_context(&state.pool, workspace_id, crawl.id)
        .await
        .map(Some)
}

async fn list_keywords(
    State(state): State<AppState>,
    principal: Principal,
    Path(site_id): Path<Uuid>,
) -> Result<Json<Envelope<Vec<KeywordOut>>>, AppError> {
    let ws = principal.workspace_id;
    sites::get(&state.pool, ws, site_id).await?;
    let rows = repo::list(&state.pool, ws, site_id).await?;
    Ok(Json(Envelope::ok(
        present(&state, ws, site_id, rows).await?,
    )))
}

#[derive(Deserialize)]
struct CreateKeyword {
    keyword: String,
    target_url: Option<String>,
}

async fn create_keyword(
    State(state): State<AppState>,
    principal: Principal,
    Path(site_id): Path<Uuid>,
    Body(body): Body<CreateKeyword>,
) -> Result<(StatusCode, Json<Envelope<KeywordOut>>), AppError> {
    let ws = principal.workspace_id;
    let site = sites::get(&state.pool, ws, site_id).await?;
    let keyword = clean_keyword(&body.keyword)?;
    let target = clean_target(&site, body.target_url.as_deref())?;
    let row = repo::create(&state.pool, ws, site_id, &keyword, target.as_deref()).await?;
    // The checks list must not call a keyword's checks passed before they ran.
    refresh_checks(&state.pool, ws, site_id).await;
    crate::rankings::run::start_schedule(&state.pool, &state.config, ws, site_id).await;
    let out = single(present(&state, ws, site_id, vec![row]).await?)?;
    Ok((StatusCode::CREATED, Json(Envelope::ok(out))))
}

#[derive(Deserialize)]
struct UpdateKeyword {
    target_url: Option<String>,
}

async fn update_keyword(
    State(state): State<AppState>,
    principal: Principal,
    Path(id): Path<Uuid>,
    Body(body): Body<UpdateKeyword>,
) -> Result<Json<Envelope<KeywordOut>>, AppError> {
    let ws = principal.workspace_id;
    let existing = repo::get(&state.pool, ws, id).await?;
    let site = sites::get(&state.pool, ws, existing.site_id).await?;
    let target = clean_target(&site, body.target_url.as_deref())?;
    let row = repo::set_target(&state.pool, ws, id, target.as_deref()).await?;
    refresh_checks(&state.pool, ws, site.id).await;
    let out = single(present(&state, ws, site.id, vec![row]).await?)?;
    Ok(Json(Envelope::ok(out)))
}

async fn delete_keyword(
    State(state): State<AppState>,
    principal: Principal,
    Path(id): Path<Uuid>,
) -> Result<Json<Envelope<serde_json::Value>>, AppError> {
    let ws = principal.workspace_id;
    let site_id = repo::delete(&state.pool, ws, id).await?;
    refresh_checks(&state.pool, ws, site_id).await;
    Ok(Json(Envelope::ok(serde_json::json!({ "deleted": true }))))
}

fn single(mut rows: Vec<KeywordOut>) -> Result<KeywordOut, AppError> {
    rows.pop()
        .ok_or_else(|| AppError::Internal(anyhow::anyhow!("keyword vanished while presenting")))
}

/// Trims and collapses spaces. A keyword needs at least one letter or number
/// and at most 100 characters.
fn clean_keyword(raw: &str) -> Result<String, AppError> {
    let keyword = raw.split_whitespace().collect::<Vec<_>>().join(" ");
    if keyword.is_empty() {
        return Err(AppError::Invalid("Enter a keyword.".into()));
    }
    if keyword.chars().count() > MAX_KEYWORD_CHARS {
        return Err(AppError::Invalid(
            "A keyword can be at most 100 characters.".into(),
        ));
    }
    if words(&keyword).is_empty() {
        return Err(AppError::Invalid(
            "A keyword needs at least one letter or number.".into(),
        ));
    }
    Ok(keyword)
}

/// The page a person chose: blank means none; a path such as `/pricing` is
/// read against the site; anything else must be a web address on the site.
fn clean_target(site: &Site, raw: Option<&str>) -> Result<Option<String>, AppError> {
    let Some(raw) = raw.map(str::trim).filter(|raw| !raw.is_empty()) else {
        return Ok(None);
    };
    let outside = || AppError::Invalid("That page is not on this site.".into());
    if raw.len() > MAX_URL_CHARS {
        return Err(AppError::Invalid("That address is too long.".into()));
    }
    let scope = SiteScope::parse(&site.root_url).map_err(|_| outside())?;
    let url = if raw.starts_with('/') {
        scope.root.join(raw).map_err(|_| outside())?
    } else {
        Url::parse(raw).map_err(|_| AppError::Invalid("That address could not be read.".into()))?
    };
    if !matches!(url.scheme(), "http" | "https") || !scope.contains(&url) {
        return Err(outside());
    }
    Ok(Some(normalize(&url).to_string()))
}
