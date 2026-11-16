use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use serde::Deserialize;
use uuid::Uuid;

use super::locale;
use super::repo;
use super::scope::SiteScope;
use crate::auth::Principal;
use crate::envelope::Envelope;
use crate::error::AppError;
use crate::request::Body;
use crate::AppState;

/// Sites one workspace may hold.
const MAX_SITES: i64 = 25;

#[derive(Deserialize)]
struct CreateSite {
    url: String,
    name: Option<String>,
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/api/v1/sites", get(list_sites).post(create_site))
        .route("/api/v1/sites/{id}", get(get_site).delete(delete_site))
        .route(
            "/api/v1/sites/{id}/settings",
            get(get_settings).put(put_settings),
        )
}

const MIN_PAGE_CAP: i32 = 10;
const MAX_PAGE_CAP: i32 = 5000;
const MAX_NAME_CHARS: usize = 100;

#[derive(serde::Serialize)]
struct Choice {
    code: &'static str,
    name: &'static str,
}

#[derive(serde::Serialize)]
struct SettingsOut {
    #[serde(flatten)]
    settings: repo::SiteSettings,
    countries: Vec<Choice>,
    languages: Vec<Choice>,
}

fn choices(list: &'static [(&'static str, &'static str)]) -> Vec<Choice> {
    list.iter()
        .map(|(code, name)| Choice { code, name })
        .collect()
}

fn settings_out(settings: repo::SiteSettings) -> SettingsOut {
    SettingsOut {
        settings,
        countries: choices(locale::COUNTRIES),
        languages: choices(locale::LANGUAGES),
    }
}

async fn get_settings(
    State(state): State<AppState>,
    principal: Principal,
    Path(id): Path<Uuid>,
) -> Result<Json<Envelope<SettingsOut>>, AppError> {
    let settings = repo::get_settings(&state.pool, principal.workspace_id, id).await?;
    Ok(Json(Envelope::ok(settings_out(settings))))
}

#[derive(Deserialize)]
struct SettingsBody {
    display_name: Option<String>,
    page_cap: Option<i64>,
    country: Option<String>,
    language: Option<String>,
    device: Option<String>,
    track_rankings: Option<bool>,
    ranking_frequency: Option<String>,
}

fn one_of(
    value: Option<String>,
    allowed: &[&str],
    message: &str,
) -> Result<Option<String>, AppError> {
    match value {
        None => Ok(None),
        Some(v) if allowed.contains(&v.as_str()) => Ok(Some(v)),
        Some(_) => Err(AppError::Invalid(message.to_string())),
    }
}

fn checked_change(body: SettingsBody) -> Result<repo::SiteSettingsChange, AppError> {
    let invalid = |message: &str| AppError::Invalid(message.to_string());
    let display_name = match body.display_name {
        None => None,
        Some(name) => {
            let name = name.split_whitespace().collect::<Vec<_>>().join(" ");
            if name.is_empty() || name.chars().count() > MAX_NAME_CHARS {
                return Err(invalid("The site name must be 1 to 100 characters."));
            }
            Some(name)
        }
    };
    let page_cap = match body.page_cap {
        None => None,
        Some(cap) if (i64::from(MIN_PAGE_CAP)..=i64::from(MAX_PAGE_CAP)).contains(&cap) => {
            Some(cap as i32)
        }
        Some(_) => return Err(invalid("The page cap must be between 10 and 5000.")),
    };
    let country = match body.country {
        Some(code) if !locale::is_country(&code) => {
            return Err(invalid(
                "That country is not available. Choose one from the list.",
            ))
        }
        other => other,
    };
    let language = match body.language {
        Some(code) if !locale::is_language(&code) => {
            return Err(invalid(
                "That language is not available. Choose one from the list.",
            ))
        }
        other => other,
    };
    Ok(repo::SiteSettingsChange {
        display_name,
        page_cap,
        country,
        language,
        device: one_of(
            body.device,
            &["desktop", "mobile"],
            "The device must be desktop or mobile.",
        )?,
        track_rankings: body.track_rankings,
        ranking_frequency: one_of(
            body.ranking_frequency,
            &["daily", "weekly"],
            "The ranking frequency must be daily or weekly.",
        )?,
    })
}

async fn put_settings(
    State(state): State<AppState>,
    principal: Principal,
    Path(id): Path<Uuid>,
    Body(body): Body<SettingsBody>,
) -> Result<Json<Envelope<SettingsOut>>, AppError> {
    let change = checked_change(body)?;
    let ws = principal.workspace_id;
    let settings = repo::update_settings(&state.pool, ws, id, &change).await?;
    crate::rankings::run::start_schedule(&state.pool, &state.config, ws, id).await;
    Ok(Json(Envelope::ok(settings_out(settings))))
}

async fn list_sites(
    State(state): State<AppState>,
    principal: Principal,
) -> Result<Json<Envelope<Vec<repo::Site>>>, AppError> {
    let sites = repo::list(&state.pool, principal.workspace_id).await?;
    Ok(Json(Envelope::ok(sites)))
}

async fn create_site(
    State(state): State<AppState>,
    principal: Principal,
    Body(body): Body<CreateSite>,
) -> Result<(StatusCode, Json<Envelope<CreatedSite>>), AppError> {
    let scope = SiteScope::parse(&body.url).map_err(|err| AppError::Invalid(err.to_string()))?;
    if repo::count(&state.pool, principal.workspace_id).await? >= MAX_SITES {
        return Err(AppError::Conflict(
            "You have reached the limit of 25 sites. Delete one to add another.".into(),
        ));
    }
    crate::settings::routes::require_contact(&state, principal.workspace_id).await?;
    crate::crawl::routes::ensure_daily_allowance(&state.pool, principal.workspace_id).await?;
    let site = repo::create(&state.pool, principal.workspace_id, &scope, body.name).await?;
    let first_crawl_id =
        crate::crawl::routes::start_for_site(&state, principal.workspace_id, site.id, "first")
            .await?;
    Ok((
        StatusCode::CREATED,
        Json(Envelope::ok(CreatedSite {
            site,
            first_crawl_id,
        })),
    ))
}

#[derive(serde::Serialize)]
struct CreatedSite {
    #[serde(flatten)]
    site: repo::Site,
    first_crawl_id: Uuid,
}

async fn get_site(
    State(state): State<AppState>,
    principal: Principal,
    Path(id): Path<Uuid>,
) -> Result<Json<Envelope<repo::Site>>, AppError> {
    let site = repo::get(&state.pool, principal.workspace_id, id).await?;
    Ok(Json(Envelope::ok(site)))
}

async fn delete_site(
    State(state): State<AppState>,
    principal: Principal,
    Path(id): Path<Uuid>,
) -> Result<Json<Envelope<serde_json::Value>>, AppError> {
    repo::delete(&state.pool, principal.workspace_id, id).await?;
    Ok(Json(Envelope::ok(serde_json::json!({ "deleted": true }))))
}
