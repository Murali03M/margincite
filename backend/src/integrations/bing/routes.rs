//! Bing Webmaster Tools endpoints. Everything needs a bearer token and a site
//! in the caller's own workspace.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use url::Url;
use uuid::Uuid;

use super::client::BingError;
use super::repo::{self, Connection};
use super::sync;
use crate::auth::Principal;
use crate::crawl::routes::paging;
use crate::envelope::{Envelope, Meta};
use crate::error::AppError;
use crate::request::{Body, Params};
use crate::secrets::Secrets;
use crate::sites::repo as sites;
use crate::AppState;

const MAX_KEY_CHARS: usize = 200;
const MAX_URL_CHARS: usize = 2048;

const NO_SECRET_KEY: &str =
    "Bing Webmaster Tools cannot be connected until the server has MARGINCITE_SECRET_KEY set.";
const BAD_KEY: &str = "Bing did not accept that API key. Copy it again from Bing Webmaster Tools, Settings, API access.";
const SITE_NOT_IN_ACCOUNT: &str =
    "That site is not in this Bing Webmaster Tools account. Add and verify it there first.";

pub fn routes() -> Router<AppState> {
    Router::new()
        .route(
            "/api/v1/sites/{id}/integrations/bing",
            get(status).put(connect).delete(disconnect),
        )
        .route("/api/v1/sites/{id}/integrations/bing/sync", post(sync_now))
        .route("/api/v1/sites/{id}/backlinks", get(backlinks))
        .route("/api/v1/sites/{id}/backlinks/sources", get(sources))
}

#[derive(Serialize)]
struct Status {
    status: &'static str,
    site_url: Option<String>,
    #[serde(with = "time::serde::rfc3339::option")]
    synced_at: Option<OffsetDateTime>,
    error: Option<String>,
}

fn present(connection: Option<Connection>) -> Status {
    match connection {
        None => Status {
            status: "not_connected",
            site_url: None,
            synced_at: None,
            error: None,
        },
        Some(c) => Status {
            status: if c.status == "needs_reconnecting" {
                "needs_reconnecting"
            } else {
                "connected"
            },
            site_url: Some(c.site_url),
            synced_at: c.synced_at,
            error: c.error,
        },
    }
}

async fn current_status(
    state: &AppState,
    workspace_id: Uuid,
    site_id: Uuid,
) -> Result<Status, AppError> {
    Ok(present(
        repo::get_connection(&state.pool, workspace_id, site_id).await?,
    ))
}

async fn status(
    State(state): State<AppState>,
    principal: Principal,
    Path(site_id): Path<Uuid>,
) -> Result<Json<Envelope<Status>>, AppError> {
    let ws = principal.workspace_id;
    sites::get(&state.pool, ws, site_id).await?;
    Ok(Json(Envelope::ok(
        current_status(&state, ws, site_id).await?,
    )))
}

fn secrets(state: &AppState) -> Result<&Secrets, AppError> {
    state
        .config
        .secret_key
        .as_ref()
        .ok_or_else(|| AppError::Invalid(NO_SECRET_KEY.into()))
}

/// The host without a leading `www.`, lower case.
fn bare_host(host: &str) -> String {
    host.to_ascii_lowercase()
        .trim_start_matches("www.")
        .to_string()
}

/// The address the person typed, if it is an http(s) address on this site's
/// host (a leading `www.` aside) and carries no credentials.
fn checked_site_url(input: &str, site_host: &str) -> Result<Url, AppError> {
    let invalid = || {
        AppError::Invalid(
            "Enter the address of this site as it appears in Bing Webmaster Tools, such as https://example.com/."
                .into(),
        )
    };
    let input = input.trim();
    if input.is_empty() || input.len() > MAX_URL_CHARS {
        return Err(invalid());
    }
    let url = Url::parse(input).map_err(|_| invalid())?;
    let on_this_site = url
        .host_str()
        .is_some_and(|host| bare_host(host) == bare_host(site_host));
    if !matches!(url.scheme(), "http" | "https")
        || !url.username().is_empty()
        || url.password().is_some()
        || !on_this_site
    {
        return Err(invalid());
    }
    Ok(url)
}

/// Two addresses for the same Bing site: equal apart from case and a
/// trailing slash.
fn same_listing(listed: &str, wanted: &str) -> bool {
    let normal = |text: &str| text.trim().trim_end_matches('/').to_ascii_lowercase();
    normal(listed) == normal(wanted)
}

#[derive(Deserialize)]
struct Connect {
    api_key: String,
    site_url: String,
}

/// Checks the key and the site against Bing, keeps the key sealed and starts
/// a sync.
async fn connect(
    State(state): State<AppState>,
    principal: Principal,
    Path(site_id): Path<Uuid>,
    Body(body): Body<Connect>,
) -> Result<Json<Envelope<Status>>, AppError> {
    let ws = principal.workspace_id;
    let site = sites::get(&state.pool, ws, site_id).await?;
    let secrets = secrets(&state)?;
    let api_key = body.api_key.trim();
    if api_key.is_empty() || api_key.len() > MAX_KEY_CHARS {
        return Err(AppError::Invalid(
            "Paste the API key from Bing Webmaster Tools, Settings, API access.".into(),
        ));
    }
    let wanted = checked_site_url(&body.site_url, &site.host)?;
    let listed = state
        .bing
        .user_sites(api_key)
        .await
        .map_err(|err| match &err {
            BingError::Rejected => AppError::Invalid(BAD_KEY.into()),
            BingError::Transient(message) => AppError::BadGateway(message.clone()),
            BingError::Unrecognised => AppError::BadGateway(err.to_string()),
        })?;
    let Some(site_url) = listed.iter().find(|l| same_listing(l, wanted.as_str())) else {
        return Err(AppError::Invalid(SITE_NOT_IN_ACCOUNT.into()));
    };
    repo::save_connection(&state.pool, ws, site_id, &secrets.seal(api_key), site_url).await?;
    queue_sync(&state, ws, site_id).await;
    Ok(Json(Envelope::ok(
        current_status(&state, ws, site_id).await?,
    )))
}

async fn queue_sync(state: &AppState, workspace_id: Uuid, site_id: Uuid) {
    if let Err(err) = sync::enqueue_now(&state.pool, workspace_id, site_id).await {
        tracing::error!(%site_id, error = %err, "could not queue a Bing sync");
    }
}

async fn sync_now(
    State(state): State<AppState>,
    principal: Principal,
    Path(site_id): Path<Uuid>,
) -> Result<(StatusCode, Json<Envelope<Status>>), AppError> {
    let ws = principal.workspace_id;
    sites::get(&state.pool, ws, site_id).await?;
    match repo::get_connection(&state.pool, ws, site_id).await? {
        Some(c) if c.status == "connected" => {}
        Some(_) => {
            return Err(AppError::Invalid(
                "Connect Bing Webmaster Tools again before syncing.".into(),
            ))
        }
        None => {
            return Err(AppError::Invalid(
                "Connect Bing Webmaster Tools first.".into(),
            ))
        }
    }
    sync::enqueue_now(&state.pool, ws, site_id)
        .await
        .map_err(AppError::Internal)?;
    Ok((
        StatusCode::ACCEPTED,
        Json(Envelope::ok(current_status(&state, ws, site_id).await?)),
    ))
}

async fn disconnect(
    State(state): State<AppState>,
    principal: Principal,
    Path(site_id): Path<Uuid>,
) -> Result<Json<Envelope<Status>>, AppError> {
    let ws = principal.workspace_id;
    sites::get(&state.pool, ws, site_id).await?;
    repo::delete_connection(&state.pool, ws, site_id).await?;
    Ok(Json(Envelope::ok(present(None))))
}

#[derive(Serialize)]
struct TargetOut {
    target_url: String,
    count: i64,
}

#[derive(Deserialize)]
struct PageQuery {
    page: Option<i64>,
    limit: Option<i64>,
}

async fn backlinks(
    State(state): State<AppState>,
    principal: Principal,
    Path(site_id): Path<Uuid>,
    Params(query): Params<PageQuery>,
) -> Result<Json<Envelope<Vec<TargetOut>>>, AppError> {
    let ws = principal.workspace_id;
    sites::get(&state.pool, ws, site_id).await?;
    let (page, limit) = paging(query.page, query.limit);
    let (rows, total) = repo::list_targets(&state.pool, ws, site_id, page, limit).await?;
    let rows = rows
        .into_iter()
        .map(|r| TargetOut {
            target_url: r.target_url,
            count: r.count,
        })
        .collect();
    Ok(Json(Envelope::page(rows, Meta { total, page, limit })))
}

#[derive(Deserialize)]
struct SourcesQuery {
    target_url: Option<String>,
}

#[derive(Serialize)]
struct SourceOut {
    source_url: String,
    anchor_text: String,
}

async fn sources(
    State(state): State<AppState>,
    principal: Principal,
    Path(site_id): Path<Uuid>,
    Params(query): Params<SourcesQuery>,
) -> Result<Json<Envelope<Vec<SourceOut>>>, AppError> {
    let ws = principal.workspace_id;
    sites::get(&state.pool, ws, site_id).await?;
    let Some(target) = query.target_url.filter(|t| !t.is_empty()) else {
        return Err(AppError::Invalid("Say which page with target_url.".into()));
    };
    let rows = repo::list_sources(&state.pool, ws, site_id, &target).await?;
    Ok(Json(Envelope::ok(
        rows.into_iter()
            .map(|r| SourceOut {
                source_url: r.source_url,
                anchor_text: r.anchor_text,
            })
            .collect(),
    )))
}
