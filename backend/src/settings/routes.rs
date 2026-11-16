//! `/api/v1/settings` and `/api/v1/usage`. Secrets are write-only: no
//! response here carries one, whole or in part, and request bodies are read
//! by hand so a malformed body is never quoted back.

use axum::extract::{Path, State};
use axum::routing::{get, post, put};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use uuid::Uuid;

use super::keys;
use super::repo::{self, Value};
use super::resolve::{self, ResolvedProvider, Saved, Source};
use super::test_provider;
use crate::ai::engines::EngineId;
use crate::auth::Principal;
use crate::config::{provider_spec, PROVIDERS};
use crate::envelope::Envelope;
use crate::error::AppError;
use crate::integrations::google::oauth;
use crate::request::{Body, Params};
use crate::secrets::Secrets;
use crate::usage;
use crate::AppState;

const MAX_SECRET_CHARS: usize = 512;
const MAX_MODEL_CHARS: usize = 100;
const MAX_CONTACT_CHARS: usize = 200;
const NO_SECRET_KEY: &str =
    "Secrets cannot be saved because the server has no secret key yet. Restart the server to create one.";

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/api/v1/settings", get(show))
        .route(
            "/api/v1/settings/providers/{provider}",
            put(put_provider).delete(delete_provider),
        )
        .route(
            "/api/v1/settings/providers/{provider}/test",
            post(test_provider_route),
        )
        .route(
            "/api/v1/settings/google",
            put(put_google).delete(delete_google),
        )
        .route(
            "/api/v1/settings/dataforseo",
            put(put_dataforseo).delete(delete_dataforseo),
        )
        .route("/api/v1/settings/dataforseo/test", post(test_dataforseo))
        .route(
            "/api/v1/settings/pagespeed",
            put(put_pagespeed).delete(delete_pagespeed),
        )
        .route(
            "/api/v1/settings/crawler",
            put(put_crawler).delete(delete_crawler),
        )
        .route("/api/v1/usage", get(usage_list))
}

fn invalid(message: &str) -> AppError {
    AppError::Invalid(message.to_string())
}

// ------------------------------------------------------------- validation

/// A key or token: visible characters only.
fn checked_secret(label: &str, value: &str) -> Result<String, AppError> {
    let value = value.trim();
    if value.is_empty() {
        return Err(AppError::Invalid(format!("{label} is required.")));
    }
    if value.chars().count() > MAX_SECRET_CHARS || !value.chars().all(|c| c.is_ascii_graphic()) {
        return Err(AppError::Invalid(format!(
            "{label} is not valid. Paste it again without spaces."
        )));
    }
    Ok(value.to_string())
}

/// A password may hold spaces but nothing that is not printable.
fn checked_password(value: &str) -> Result<String, AppError> {
    if value.is_empty() {
        return Err(invalid("The password is required."));
    }
    if value.chars().count() > MAX_SECRET_CHARS || value.chars().any(char::is_control) {
        return Err(invalid("The password is not valid."));
    }
    Ok(value.to_string())
}

/// A model name goes into addresses, so only a safe set of characters passes.
fn checked_model(value: &str) -> Result<String, AppError> {
    let value = value.trim();
    let safe = value
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_' | ':'));
    if value.is_empty() || value.chars().count() > MAX_MODEL_CHARS || !safe {
        return Err(invalid(
            "The model name can use letters, digits and . - _ : only.",
        ));
    }
    Ok(value.to_string())
}

/// An email address or an http(s) address, since it is sent in a header.
pub fn checked_contact(value: &str) -> Result<String, AppError> {
    let value = value.trim();
    let message = "Enter an email address or an http(s) web address.";
    if value.is_empty()
        || value.chars().count() > MAX_CONTACT_CHARS
        || value
            .chars()
            .any(|c| c.is_control() || c.is_whitespace() || !c.is_ascii())
    {
        return Err(invalid(message));
    }
    if value.starts_with("http://") || value.starts_with("https://") {
        let url = url::Url::parse(value).map_err(|_| invalid(message))?;
        return if url.host_str().is_some() {
            Ok(value.to_string())
        } else {
            Err(invalid(message))
        };
    }
    let mut parts = value.split('@');
    let (Some(name), Some(domain), None) = (parts.next(), parts.next(), parts.next()) else {
        return Err(invalid(message));
    };
    let domain_ok = domain.contains('.')
        && !domain.starts_with('.')
        && !domain.ends_with('.')
        && !domain.contains("..");
    if name.is_empty() || !domain_ok || value.contains(['(', ')', '<', '>']) {
        return Err(invalid(message));
    }
    Ok(value.to_string())
}

fn secrets(state: &AppState) -> Result<&Secrets, AppError> {
    state
        .config
        .secret_key
        .as_ref()
        .ok_or_else(|| invalid(NO_SECRET_KEY))
}

fn engine_id(name: &str) -> Result<EngineId, AppError> {
    EngineId::from_name(name).ok_or(AppError::NotFound)
}

// -------------------------------------------------------------- responses

#[derive(Serialize)]
struct ProviderOut {
    provider: &'static str,
    key_set: bool,
    source: Source,
    model: String,
    default_model: &'static str,
    status: &'static str,
    error: Option<String>,
    #[serde(with = "time::serde::rfc3339::option")]
    checked_at: Option<OffsetDateTime>,
}

#[derive(Serialize)]
struct GoogleOut {
    client_id_set: bool,
    client_secret_set: bool,
    source: Source,
    /// The address Google sends people back to, exactly as this server sends
    /// it. Register this one with the OAuth client. Not a secret.
    redirect_uri: String,
}

#[derive(Serialize)]
struct DataForSeoOut {
    login_set: bool,
    password_set: bool,
    source: Source,
    status: &'static str,
    error: Option<String>,
    balance: Option<f64>,
    #[serde(with = "time::serde::rfc3339::option")]
    checked_at: Option<OffsetDateTime>,
}

#[derive(Serialize)]
struct PagespeedOut {
    key_set: bool,
    source: Source,
}

#[derive(Serialize)]
struct CrawlerOut {
    contact: Option<String>,
    contact_source: Source,
}

#[derive(Serialize)]
struct SettingsOut {
    providers: Vec<ProviderOut>,
    google: GoogleOut,
    dataforseo: DataForSeoOut,
    pagespeed: PagespeedOut,
    crawler: CrawlerOut,
    secret_key_ready: bool,
}

fn status_word(status: Option<&repo::Status>) -> &'static str {
    match status.map(|s| s.status.as_str()) {
        Some("ok") => "ok",
        Some("error") => "error",
        _ => "unchecked",
    }
}

fn provider_out(
    id: EngineId,
    resolved: ResolvedProvider,
    status: Option<&repo::Status>,
) -> ProviderOut {
    let spec = provider_spec(id.as_str()).expect("every engine has a spec");
    ProviderOut {
        provider: spec.name,
        key_set: resolved.api_key.is_some(),
        source: resolved.source,
        model: resolved.model,
        default_model: spec.default_model,
        status: status_word(status),
        error: status.and_then(|s| s.error.clone()),
        checked_at: status.map(|s| s.checked_at),
    }
}

fn google_out(saved: &Saved, state: &AppState) -> GoogleOut {
    let (credentials, source) = saved.google();
    GoogleOut {
        client_id_set: credentials.is_some(),
        client_secret_set: credentials.is_some(),
        source,
        redirect_uri: oauth::redirect_uri(&state.config.public_base_url),
    }
}

fn dataforseo_out(saved: &Saved, status: Option<&repo::Status>) -> DataForSeoOut {
    let (credentials, source) = saved.dataforseo();
    let set = credentials.is_some();
    DataForSeoOut {
        login_set: set,
        password_set: set,
        source,
        status: if set {
            status_word(status)
        } else {
            "unchecked"
        },
        error: status.filter(|_| set).and_then(|s| s.error.clone()),
        balance: status.filter(|_| set).and_then(|s| s.balance),
        checked_at: status.filter(|_| set).map(|s| s.checked_at),
    }
}

fn pagespeed_out(saved: &Saved) -> PagespeedOut {
    let (key, source) = saved.pagespeed_key();
    PagespeedOut {
        key_set: key.is_some(),
        source,
    }
}

fn crawler_out(saved: &Saved) -> CrawlerOut {
    let (contact, contact_source) = saved.crawler_contact();
    CrawlerOut {
        contact,
        contact_source,
    }
}

type Reply<T> = Result<Json<Envelope<T>>, AppError>;

async fn show(State(state): State<AppState>, principal: Principal) -> Reply<SettingsOut> {
    let ws = principal.workspace_id;
    let saved = resolve::load(&state.pool, &state.config, ws).await?;
    let statuses = repo::statuses(&state.pool, ws).await?;
    let providers = PROVIDERS
        .iter()
        .filter_map(|spec| EngineId::from_name(spec.name))
        .map(|id| provider_out(id, saved.provider(id), statuses.get(id.as_str())))
        .collect();
    Ok(Json(Envelope::ok(SettingsOut {
        providers,
        google: google_out(&saved, &state),
        dataforseo: dataforseo_out(&saved, statuses.get(super::DATAFORSEO)),
        pagespeed: pagespeed_out(&saved),
        crawler: crawler_out(&saved),
        secret_key_ready: state.config.secret_key.is_some(),
    })))
}

// -------------------------------------------------------------- providers

#[derive(Deserialize)]
struct ProviderBody {
    api_key: Option<String>,
    model: Option<String>,
}

async fn provider_state(state: &AppState, ws: Uuid, id: EngineId) -> Result<ProviderOut, AppError> {
    let saved = resolve::load(&state.pool, &state.config, ws).await?;
    let statuses = repo::statuses(&state.pool, ws).await?;
    Ok(provider_out(
        id,
        saved.provider(id),
        statuses.get(id.as_str()),
    ))
}

/// Tests the provider's current key and stores the outcome.
async fn probe(state: &AppState, ws: Uuid, id: EngineId) -> Result<(), AppError> {
    let resolved = resolve::provider(&state.pool, &state.config, ws, id).await?;
    let Some(key) = resolved.api_key else {
        return repo::clear_status(&state.pool, ws, id.as_str()).await;
    };
    let outcome = test_provider::provider(&state.engines.bases, id, &key, &resolved.model).await;
    repo::set_status(&state.pool, ws, id.as_str(), outcome.err().as_deref(), None).await
}

async fn put_provider(
    State(state): State<AppState>,
    principal: Principal,
    Path(name): Path<String>,
    Body(body): Body<ProviderBody>,
) -> Reply<ProviderOut> {
    let ws = principal.workspace_id;
    let id = engine_id(&name)?;
    if body.api_key.is_none() && body.model.is_none() {
        return Err(invalid("Send an API key, a model, or both."));
    }
    let mut values = Vec::new();
    let mut cleared = Vec::new();
    if let Some(key) = &body.api_key {
        let key = checked_secret("The API key", key)?;
        values.push((
            keys::provider_key(id.as_str()),
            Value::Sealed(secrets(&state)?.seal(&key)),
        ));
    }
    if let Some(model) = &body.model {
        if model.trim().is_empty() {
            cleared.push(keys::provider_model(id.as_str()));
        } else {
            values.push((
                keys::provider_model(id.as_str()),
                Value::Plain(checked_model(model)?),
            ));
        }
    }
    repo::set_many(&state.pool, ws, &values).await?;
    repo::delete_keys(&state.pool, ws, &cleared).await?;
    probe(&state, ws, id).await?;
    Ok(Json(Envelope::ok(provider_state(&state, ws, id).await?)))
}

async fn test_provider_route(
    State(state): State<AppState>,
    principal: Principal,
    Path(name): Path<String>,
) -> Reply<ProviderOut> {
    let ws = principal.workspace_id;
    let id = engine_id(&name)?;
    probe(&state, ws, id).await?;
    Ok(Json(Envelope::ok(provider_state(&state, ws, id).await?)))
}

async fn delete_provider(
    State(state): State<AppState>,
    principal: Principal,
    Path(name): Path<String>,
) -> Reply<ProviderOut> {
    let ws = principal.workspace_id;
    let id = engine_id(&name)?;
    let stored = [
        keys::provider_key(id.as_str()),
        keys::provider_model(id.as_str()),
    ];
    repo::delete_keys(&state.pool, ws, &stored).await?;
    repo::clear_status(&state.pool, ws, id.as_str()).await?;
    Ok(Json(Envelope::ok(provider_state(&state, ws, id).await?)))
}

// ----------------------------------------------------------------- google

#[derive(Deserialize)]
struct GoogleBody {
    client_id: String,
    client_secret: String,
}

async fn google_state(state: &AppState, ws: Uuid) -> Reply<GoogleOut> {
    let saved = resolve::load(&state.pool, &state.config, ws).await?;
    Ok(Json(Envelope::ok(google_out(&saved, state))))
}

async fn put_google(
    State(state): State<AppState>,
    principal: Principal,
    Body(body): Body<GoogleBody>,
) -> Reply<GoogleOut> {
    let ws = principal.workspace_id;
    let client_id = checked_secret("The client ID", &body.client_id)?;
    let client_secret = checked_secret("The client secret", &body.client_secret)?;
    let sealed = secrets(&state)?.seal(&client_secret);
    repo::set_many(
        &state.pool,
        ws,
        &[
            (keys::GOOGLE_CLIENT_ID.into(), Value::Plain(client_id)),
            (keys::GOOGLE_CLIENT_SECRET.into(), Value::Sealed(sealed)),
        ],
    )
    .await?;
    google_state(&state, ws).await
}

async fn delete_google(State(state): State<AppState>, principal: Principal) -> Reply<GoogleOut> {
    let ws = principal.workspace_id;
    let stored = [
        keys::GOOGLE_CLIENT_ID.into(),
        keys::GOOGLE_CLIENT_SECRET.into(),
    ];
    repo::delete_keys(&state.pool, ws, &stored).await?;
    google_state(&state, ws).await
}

// ------------------------------------------------------------- dataforseo

#[derive(Deserialize)]
struct DataForSeoBody {
    login: String,
    password: String,
}

async fn dataforseo_state(state: &AppState, ws: Uuid) -> Reply<DataForSeoOut> {
    let saved = resolve::load(&state.pool, &state.config, ws).await?;
    let statuses = repo::statuses(&state.pool, ws).await?;
    Ok(Json(Envelope::ok(dataforseo_out(
        &saved,
        statuses.get(super::DATAFORSEO),
    ))))
}

async fn probe_dataforseo(state: &AppState, ws: Uuid) -> Result<(), AppError> {
    let Some((login, password)) = resolve::dataforseo(&state.pool, &state.config, ws).await? else {
        return repo::clear_status(&state.pool, ws, super::DATAFORSEO).await;
    };
    match test_provider::dataforseo(&state.dataforseo_base, &login, &password).await {
        Ok(balance) => repo::set_status(&state.pool, ws, super::DATAFORSEO, None, balance).await,
        Err(message) => {
            repo::set_status(&state.pool, ws, super::DATAFORSEO, Some(&message), None).await
        }
    }
}

async fn put_dataforseo(
    State(state): State<AppState>,
    principal: Principal,
    Body(body): Body<DataForSeoBody>,
) -> Reply<DataForSeoOut> {
    let ws = principal.workspace_id;
    let login = checked_secret("The login", &body.login)?;
    let password = checked_password(&body.password)?;
    let sealed = secrets(&state)?.seal(&password);
    repo::set_many(
        &state.pool,
        ws,
        &[
            (keys::DATAFORSEO_LOGIN.into(), Value::Plain(login)),
            (keys::DATAFORSEO_PASSWORD.into(), Value::Sealed(sealed)),
        ],
    )
    .await?;
    probe_dataforseo(&state, ws).await?;
    crate::rankings::run::start_schedules(&state.pool, &state.config, ws).await;
    dataforseo_state(&state, ws).await
}

async fn test_dataforseo(
    State(state): State<AppState>,
    principal: Principal,
) -> Reply<DataForSeoOut> {
    probe_dataforseo(&state, principal.workspace_id).await?;
    dataforseo_state(&state, principal.workspace_id).await
}

async fn delete_dataforseo(
    State(state): State<AppState>,
    principal: Principal,
) -> Reply<DataForSeoOut> {
    let ws = principal.workspace_id;
    let stored = [
        keys::DATAFORSEO_LOGIN.into(),
        keys::DATAFORSEO_PASSWORD.into(),
    ];
    repo::delete_keys(&state.pool, ws, &stored).await?;
    repo::clear_status(&state.pool, ws, super::DATAFORSEO).await?;
    dataforseo_state(&state, ws).await
}

// -------------------------------------------------------------- pagespeed

#[derive(Deserialize)]
struct PagespeedBody {
    api_key: String,
}

async fn pagespeed_state(state: &AppState, ws: Uuid) -> Reply<PagespeedOut> {
    let saved = resolve::load(&state.pool, &state.config, ws).await?;
    Ok(Json(Envelope::ok(pagespeed_out(&saved))))
}

async fn put_pagespeed(
    State(state): State<AppState>,
    principal: Principal,
    Body(body): Body<PagespeedBody>,
) -> Reply<PagespeedOut> {
    let ws = principal.workspace_id;
    let key = checked_secret("The API key", &body.api_key)?;
    let sealed = secrets(&state)?.seal(&key);
    repo::set_many(
        &state.pool,
        ws,
        &[(keys::PAGESPEED_KEY.into(), Value::Sealed(sealed))],
    )
    .await?;
    pagespeed_state(&state, ws).await
}

async fn delete_pagespeed(
    State(state): State<AppState>,
    principal: Principal,
) -> Reply<PagespeedOut> {
    let ws = principal.workspace_id;
    repo::delete_keys(&state.pool, ws, &[keys::PAGESPEED_KEY.into()]).await?;
    pagespeed_state(&state, ws).await
}

// ---------------------------------------------------------------- crawler

#[derive(Deserialize)]
struct CrawlerBody {
    contact: String,
}

async fn crawler_state(state: &AppState, ws: Uuid) -> Reply<CrawlerOut> {
    let saved = resolve::load(&state.pool, &state.config, ws).await?;
    Ok(Json(Envelope::ok(crawler_out(&saved))))
}

async fn put_crawler(
    State(state): State<AppState>,
    principal: Principal,
    Body(body): Body<CrawlerBody>,
) -> Reply<CrawlerOut> {
    let ws = principal.workspace_id;
    let contact = checked_contact(&body.contact)?;
    repo::set_many(
        &state.pool,
        ws,
        &[(keys::CRAWLER_CONTACT.into(), Value::Plain(contact))],
    )
    .await?;
    crawler_state(&state, ws).await
}

async fn delete_crawler(State(state): State<AppState>, principal: Principal) -> Reply<CrawlerOut> {
    let ws = principal.workspace_id;
    repo::delete_keys(&state.pool, ws, &[keys::CRAWLER_CONTACT.into()]).await?;
    crawler_state(&state, ws).await
}

/// The contact the crawler would send, or the 422 that stops a crawl.
pub async fn require_contact(state: &AppState, ws: Uuid) -> Result<String, AppError> {
    resolve::crawler_contact(&state.pool, &state.config, ws)
        .await?
        .ok_or_else(|| invalid(NO_CONTACT))
}

pub const NO_CONTACT: &str =
    "Add a contact address in Settings before crawling. It is sent with every request the crawler makes.";

// ------------------------------------------------------------------ usage

#[derive(Deserialize)]
struct UsageQuery {
    days: Option<i64>,
}

async fn usage_list(
    State(state): State<AppState>,
    principal: Principal,
    Params(query): Params<UsageQuery>,
) -> Reply<Vec<usage::Day>> {
    let days = query.days.unwrap_or(30).clamp(1, 365);
    Ok(Json(Envelope::ok(
        usage::list(&state.pool, principal.workspace_id, days).await?,
    )))
}
