//! Search Console endpoints. Everything needs a bearer token except the OAuth
//! callback, which is the browser coming back from Google and treats every
//! query parameter as hostile.

use axum::extract::{Path, Query, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use uuid::Uuid;

use super::client::{GoogleError, Property};
use super::oauth::{self, Outcome, Reason};
use super::repo::{self, Connection};
use super::search::{self, PageRowOut, QueryRowOut, QuerySort};
use super::sync;
use crate::auth::Principal;
use crate::config::GoogleCredentials;
use crate::crawl::routes::paging;
use crate::envelope::{Envelope, Meta};
use crate::error::AppError;
use crate::request::{Body, Params};
use crate::secrets::Secrets;
use crate::sites::repo as sites;
use crate::AppState;

const MAX_PROPERTY_CHARS: usize = 2048;
const MAX_CODE_CHARS: usize = 2048;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route(
            "/api/v1/sites/{id}/integrations/google",
            get(status).put(choose_property).delete(disconnect),
        )
        .route("/api/v1/sites/{id}/integrations/google/start", post(start))
        .route(
            "/api/v1/sites/{id}/integrations/google/properties",
            get(properties),
        )
        .route(
            "/api/v1/sites/{id}/integrations/google/sync",
            post(sync_now),
        )
        .route("/api/v1/oauth/google/callback", get(callback))
        .route("/api/v1/sites/{id}/search/queries", get(queries))
        .route("/api/v1/sites/{id}/search/pages", get(pages))
}

#[derive(Serialize)]
struct Status {
    status: &'static str,
    property: Option<String>,
    #[serde(with = "time::serde::rfc3339::option")]
    synced_at: Option<OffsetDateTime>,
    error: Option<String>,
}

fn present(connection: Option<Connection>) -> Status {
    match connection {
        None => Status {
            status: "not_connected",
            property: None,
            synced_at: None,
            error: None,
        },
        Some(c) => Status {
            status: match c.status.as_str() {
                "connected" => "connected",
                "needs_reconnecting" => "needs_reconnecting",
                _ => "choose_property",
            },
            property: c.property,
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

/// Google sign-in works only when the server has both the Google client and
/// the key that seals the token.
async fn setup(
    state: &AppState,
    workspace_id: Uuid,
) -> Result<(GoogleCredentials, &Secrets), AppError> {
    let credentials =
        crate::settings::resolve::google(&state.pool, &state.config, workspace_id).await?;
    match (credentials, state.config.secret_key.as_ref()) {
        (Some(credentials), Some(secrets)) => Ok((credentials, secrets)),
        _ => Err(AppError::Invalid(
            "Google sign-in is not set up. Add the Google client ID and secret in Settings.".into(),
        )),
    }
}

#[derive(Serialize)]
struct Start {
    auth_url: String,
}

async fn start(
    State(state): State<AppState>,
    principal: Principal,
    Path(site_id): Path<Uuid>,
) -> Result<Response, AppError> {
    let ws = principal.workspace_id;
    sites::get(&state.pool, ws, site_id).await?;
    let (credentials, _) = setup(&state, ws).await?;
    let nonce = oauth::new_nonce();
    let state_id = repo::create_state(&state.pool, ws, site_id, &oauth::hash_nonce(&nonce)).await?;
    let redirect_uri = oauth::redirect_uri(&state.config.public_base_url);
    let auth_url = oauth::authorization_url(
        state.google.hosts(),
        &credentials.client_id,
        &redirect_uri,
        state_id,
    )
    .map_err(|err| AppError::Internal(err.into()))?;
    let cookie = oauth::nonce_cookie(&nonce, &state.config.public_base_url);
    Ok((
        [(header::SET_COOKIE, cookie)],
        Json(Envelope::ok(Start { auth_url })),
    )
        .into_response())
}

#[derive(Deserialize)]
struct CallbackParams {
    code: Option<String>,
    state: Option<String>,
    error: Option<String>,
}

/// The browser comes back from Google. Whatever happens, it is sent on to the
/// web app; the address is built from `PUBLIC_BASE_URL` and a site id that
/// came from our own table.
async fn callback(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<CallbackParams>,
) -> Response {
    let browser_nonce = oauth::nonce_from_cookies(&headers);
    let (site_id, outcome) = finish_sign_in(&state, params, browser_nonce.as_deref()).await;
    let base = &state.config.public_base_url;
    let location = oauth::app_redirect(base, site_id, outcome);
    (
        StatusCode::FOUND,
        [
            (header::LOCATION, location),
            (header::SET_COOKIE, oauth::clear_cookie(base)),
        ],
    )
        .into_response()
}

async fn finish_sign_in(
    state: &AppState,
    params: CallbackParams,
    browser_nonce: Option<&str>,
) -> (Option<Uuid>, Outcome) {
    let fail = |site: Option<Uuid>, reason| (site, Outcome::Error(reason));
    let Some(state_id) = params.state.as_deref().and_then(|s| s.parse::<Uuid>().ok()) else {
        return fail(None, Reason::InvalidState);
    };
    let pending = match repo::peek_state(&state.pool, state_id).await {
        Ok(Some(pending)) => pending,
        Ok(None) => return fail(None, Reason::InvalidState),
        Err(err) => {
            tracing::error!(error = %err, "could not look up a sign-in state");
            return fail(None, Reason::ServerError);
        }
    };
    // The state alone proves nothing: a link can be passed to someone else.
    // Only the browser that started the sign-in holds the matching nonce.
    if !oauth::nonce_matches(pending.nonce_hash.as_deref(), browser_nonce) {
        return fail(None, Reason::InvalidState);
    }
    let site = Some(pending.site_id);
    if !pending.fresh {
        return fail(site, Reason::Expired);
    }
    if params.error.is_some() {
        return fail(site, Reason::Denied);
    }
    let Some(code) = params.code.as_deref().filter(|code| !code.is_empty()) else {
        return fail(site, Reason::MissingCode);
    };
    if code.len() > MAX_CODE_CHARS {
        return fail(site, Reason::TokenRejected);
    }
    let Ok((credentials, secrets)) = setup(state, pending.workspace_id).await else {
        return fail(site, Reason::NotConfigured);
    };
    let redirect_uri = oauth::redirect_uri(&state.config.public_base_url);
    let tokens = match state
        .google
        .exchange_code(&credentials, &redirect_uri, code)
        .await
    {
        Ok(tokens) => tokens,
        Err(GoogleError::Rejected) => return fail(site, Reason::TokenRejected),
        Err(GoogleError::Transient(_)) => return fail(site, Reason::GoogleUnavailable),
        Err(GoogleError::ServerCredentials) => return fail(site, Reason::ServerError),
    };
    let Some(refresh_token) = tokens.refresh_token else {
        return fail(site, Reason::NoRefreshToken);
    };
    let sealed = secrets.seal(&refresh_token);
    match repo::store_connection(&state.pool, state_id, &sealed).await {
        Ok(Some(has_property)) => {
            if has_property {
                queue_sync(state, pending.workspace_id, pending.site_id).await;
            }
            (site, Outcome::Connected)
        }
        Ok(None) => fail(site, Reason::InvalidState),
        Err(err) => {
            tracing::error!(error = %err, "could not store the Google connection");
            fail(site, Reason::ServerError)
        }
    }
}

async fn queue_sync(state: &AppState, workspace_id: Uuid, site_id: Uuid) {
    if let Err(err) = sync::enqueue_now(&state.pool, workspace_id, site_id).await {
        tracing::error!(%site_id, error = %err, "could not queue a Search Console sync");
    }
}

/// A short-lived access token for the site's connection. A refused token
/// marks the connection `needs_reconnecting`.
async fn access_token(
    state: &AppState,
    workspace_id: Uuid,
    site_id: Uuid,
) -> Result<(Connection, String), AppError> {
    let (credentials, secrets) = setup(state, workspace_id).await?;
    let connection = repo::get_connection(&state.pool, workspace_id, site_id)
        .await?
        .ok_or_else(|| AppError::Invalid("Connect Google Search Console first.".into()))?;
    if connection.status == "needs_reconnecting" {
        return Err(AppError::Invalid(
            "Google no longer accepts this connection. Connect Google Search Console again.".into(),
        ));
    }
    let refresh_token = secrets
        .open(&connection.refresh_token_sealed)
        .map_err(|_| {
            AppError::Invalid(
                "The saved Google connection could not be read. Connect again.".into(),
            )
        })?;
    match state
        .google
        .refresh_access_token(&credentials, &refresh_token)
        .await
    {
        Ok(token) => Ok((connection, token)),
        Err(GoogleError::Rejected) => {
            repo::mark_needs_reconnecting(
                &state.pool,
                workspace_id,
                site_id,
                "Google no longer accepts this connection. Connect Google Search Console again.",
            )
            .await?;
            Err(AppError::Invalid(
                "Google no longer accepts this connection. Connect Google Search Console again."
                    .into(),
            ))
        }
        Err(GoogleError::Transient(message)) => Err(AppError::BadGateway(message)),
        Err(GoogleError::ServerCredentials) => {
            Err(AppError::BadGateway(sync::SERVER_CREDENTIALS.into()))
        }
    }
}

#[derive(Serialize)]
struct PropertyOut {
    property: String,
    permission: String,
}

/// The properties the person can query: those they may read, the ones that
/// look like this site first.
async fn readable_properties(
    state: &AppState,
    workspace_id: Uuid,
    site_id: Uuid,
    host: &str,
) -> Result<Vec<Property>, AppError> {
    let (_, token) = access_token(state, workspace_id, site_id).await?;
    let mut found = state
        .google
        .list_properties(&token)
        .await
        .map_err(|err| match err {
            GoogleError::Rejected => AppError::Invalid(
                "Google would not list the properties. Connect Google Search Console again.".into(),
            ),
            GoogleError::Transient(message) => AppError::BadGateway(message),
            GoogleError::ServerCredentials => AppError::BadGateway(sync::SERVER_CREDENTIALS.into()),
        })?;
    found.retain(|p| p.permission != "siteUnverifiedUser");
    found.sort_by_key(|p| (!mentions_host(&p.property, host), p.property.clone()));
    Ok(found)
}

fn mentions_host(property: &str, host: &str) -> bool {
    let bare = host.trim_start_matches("www.");
    property
        .to_ascii_lowercase()
        .contains(&bare.to_ascii_lowercase())
}

async fn properties(
    State(state): State<AppState>,
    principal: Principal,
    Path(site_id): Path<Uuid>,
) -> Result<Json<Envelope<Vec<PropertyOut>>>, AppError> {
    let ws = principal.workspace_id;
    let site = sites::get(&state.pool, ws, site_id).await?;
    let found = readable_properties(&state, ws, site_id, &site.host).await?;
    Ok(Json(Envelope::ok(
        found
            .into_iter()
            .map(|p| PropertyOut {
                property: p.property,
                permission: p.permission,
            })
            .collect(),
    )))
}

#[derive(Deserialize)]
struct ChooseProperty {
    property: String,
}

async fn choose_property(
    State(state): State<AppState>,
    principal: Principal,
    Path(site_id): Path<Uuid>,
    Body(body): Body<ChooseProperty>,
) -> Result<Json<Envelope<Status>>, AppError> {
    let ws = principal.workspace_id;
    let site = sites::get(&state.pool, ws, site_id).await?;
    let property = body.property.trim();
    if property.is_empty() || property.len() > MAX_PROPERTY_CHARS {
        return Err(AppError::Invalid(
            "Choose a Search Console property.".into(),
        ));
    }
    let allowed = readable_properties(&state, ws, site_id, &site.host).await?;
    if !allowed.iter().any(|p| p.property == property) {
        return Err(AppError::Invalid(
            "That property is not one you can read in Search Console.".into(),
        ));
    }
    repo::set_property(&state.pool, ws, site_id, property).await?;
    queue_sync(&state, ws, site_id).await;
    Ok(Json(Envelope::ok(
        current_status(&state, ws, site_id).await?,
    )))
}

async fn sync_now(
    State(state): State<AppState>,
    principal: Principal,
    Path(site_id): Path<Uuid>,
) -> Result<(StatusCode, Json<Envelope<Status>>), AppError> {
    let ws = principal.workspace_id;
    sites::get(&state.pool, ws, site_id).await?;
    let connection = repo::get_connection(&state.pool, ws, site_id).await?;
    match connection {
        Some(c) if c.status == "connected" && c.property.is_some() => {}
        Some(c) if c.status == "needs_reconnecting" => {
            return Err(AppError::Invalid(
                "Connect Google Search Console again before syncing.".into(),
            ))
        }
        _ => {
            return Err(AppError::Invalid(
                "Connect Google Search Console and choose a property first.".into(),
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
    if repo::delete_connection(&state.pool, ws, site_id).await? {
        sync::refresh_checks(&state.pool, ws, site_id).await;
    }
    Ok(Json(Envelope::ok(present(None))))
}

#[derive(Deserialize)]
struct QueriesQuery {
    page: Option<i64>,
    limit: Option<i64>,
    sort: Option<String>,
}

async fn queries(
    State(state): State<AppState>,
    principal: Principal,
    Path(site_id): Path<Uuid>,
    Params(query): Params<QueriesQuery>,
) -> Result<Json<Envelope<Vec<QueryRowOut>>>, AppError> {
    let ws = principal.workspace_id;
    sites::get(&state.pool, ws, site_id).await?;
    let sort = QuerySort::parse(query.sort.as_deref())?;
    let (page, limit) = paging(query.page, query.limit);
    let (rows, total) = search::list_queries(&state.pool, ws, site_id, sort, page, limit).await?;
    Ok(Json(Envelope::page(rows, Meta { total, page, limit })))
}

#[derive(Deserialize)]
struct PagesQuery {
    page: Option<i64>,
    limit: Option<i64>,
}

async fn pages(
    State(state): State<AppState>,
    principal: Principal,
    Path(site_id): Path<Uuid>,
    Params(query): Params<PagesQuery>,
) -> Result<Json<Envelope<Vec<PageRowOut>>>, AppError> {
    let ws = principal.workspace_id;
    sites::get(&state.pool, ws, site_id).await?;
    let (page, limit) = paging(query.page, query.limit);
    let (rows, total) = search::list_pages(&state.pool, ws, site_id, page, limit).await?;
    Ok(Json(Envelope::page(rows, Meta { total, page, limit })))
}
