//! AI visibility endpoints. Everything needs a bearer token and a site (or a
//! run, answer or question) in the caller's own workspace.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::answers::{self, AnswerDetail, AnswerOut, DomainOut};
use super::engines::bare_host;
use super::repo::{self, Prompt, Run, Settings};
use super::run;
use super::suggest::{suggest_prompts, MAX_PROMPT_CHARS, MIN_PROMPT_CHARS};
use crate::auth::Principal;
use crate::crawl::clean::strip_nul;
use crate::crawl::routes::paging;
use crate::envelope::{Envelope, Meta};
use crate::error::AppError;
use crate::keywords::repo as keywords;
use crate::request::{Body, Params};
use crate::sites::repo as sites;
use crate::AppState;

const MAX_BRAND_CHARS: usize = 100;
const MAX_LIST: usize = 10;

const NO_ENGINE: &str =
    "No AI engine is set up. Add an API key for Claude, ChatGPT, Grok, Gemini or Perplexity in Settings.";
const NO_PROMPTS: &str = "Add at least one question before running a check.";
const NO_CLAUDE: &str =
    "Question suggestions use Claude. Add a Claude API key in Settings to get them.";
const SUGGEST_FAILED: &str = "Claude could not suggest questions right now. Try again in a minute.";

pub fn routes() -> Router<AppState> {
    Router::new()
        .route(
            "/api/v1/sites/{id}/ai/settings",
            get(get_settings).put(put_settings),
        )
        .route(
            "/api/v1/sites/{id}/ai/prompts",
            get(list_prompts).post(create_prompt),
        )
        .route("/api/v1/sites/{id}/ai/prompts/suggest", post(suggest))
        .route("/api/v1/ai/prompts/{id}", delete(delete_prompt))
        .route("/api/v1/sites/{id}/ai/runs", get(list_runs).post(start_run))
        .route("/api/v1/ai/runs/{id}", get(get_run))
        .route("/api/v1/ai/runs/{id}/answers", get(run_answers))
        .route("/api/v1/ai/answers/{id}", get(get_answer))
        .route("/api/v1/sites/{id}/ai/domains", get(domains))
}

fn internal(err: anyhow::Error) -> AppError {
    AppError::Internal(err)
}

// ---------------------------------------------------------------- settings

async fn get_settings(
    State(state): State<AppState>,
    principal: Principal,
    Path(site_id): Path<Uuid>,
) -> Result<Json<Envelope<Settings>>, AppError> {
    let ws = principal.workspace_id;
    sites::get(&state.pool, ws, site_id).await?;
    Ok(Json(Envelope::ok(
        repo::get_settings(&state.pool, ws, site_id).await?,
    )))
}

#[derive(Deserialize)]
struct SettingsBody {
    #[serde(default)]
    brand_name: String,
    #[serde(default)]
    aliases: Vec<String>,
    #[serde(default)]
    competitors: Vec<String>,
}

/// Trimmed, no NUL, spaces collapsed.
fn tidy(text: &str) -> String {
    strip_nul(text)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// A competitor as a bare host: the person may paste a whole address.
fn competitor_host(input: &str) -> Option<String> {
    let input = input.trim().to_ascii_lowercase();
    let input = input
        .strip_prefix("https://")
        .or_else(|| input.strip_prefix("http://"))
        .unwrap_or(&input);
    let host = input.split(['/', '?', '#']).next().unwrap_or("");
    let host = bare_host(host);
    let valid = host.contains('.')
        && host.len() <= 253
        && !host.starts_with('.')
        && !host.starts_with('-')
        && host
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-')
        && host.split('.').all(|label| !label.is_empty());
    valid.then_some(host)
}

fn unique_in_order(items: Vec<String>) -> Vec<String> {
    let mut seen: Vec<String> = Vec::new();
    for item in items {
        if !seen.iter().any(|s| s.to_lowercase() == item.to_lowercase()) {
            seen.push(item);
        }
    }
    seen
}

fn checked_settings(body: SettingsBody) -> Result<Settings, AppError> {
    let invalid = |message: &str| AppError::Invalid(message.to_string());
    let brand_name = tidy(&body.brand_name);
    if brand_name.chars().count() > MAX_BRAND_CHARS {
        return Err(invalid("The brand name can be at most 100 characters."));
    }
    let aliases: Vec<String> = body
        .aliases
        .iter()
        .map(|a| tidy(a))
        .filter(|a| !a.is_empty())
        .collect();
    let aliases = unique_in_order(aliases);
    if aliases.len() > MAX_LIST {
        return Err(invalid("A site can have at most 10 aliases."));
    }
    if aliases.iter().any(|a| a.chars().count() > MAX_BRAND_CHARS) {
        return Err(invalid("An alias can be at most 100 characters."));
    }
    let mut competitors = Vec::new();
    for entry in body.competitors.iter().filter(|c| !c.trim().is_empty()) {
        competitors.push(competitor_host(entry).ok_or_else(|| {
            AppError::Invalid(format!(
                "\"{}\" is not a website address. Enter a domain such as rival.com.",
                tidy(entry).chars().take(60).collect::<String>()
            ))
        })?);
    }
    let competitors = unique_in_order(competitors);
    if competitors.len() > MAX_LIST {
        return Err(invalid("A site can have at most 10 competitors."));
    }
    Ok(Settings {
        brand_name,
        aliases,
        competitors,
    })
}

async fn put_settings(
    State(state): State<AppState>,
    principal: Principal,
    Path(site_id): Path<Uuid>,
    Body(body): Body<SettingsBody>,
) -> Result<Json<Envelope<Settings>>, AppError> {
    let ws = principal.workspace_id;
    sites::get(&state.pool, ws, site_id).await?;
    let settings = checked_settings(body)?;
    repo::put_settings(&state.pool, ws, site_id, &settings).await?;
    Ok(Json(Envelope::ok(settings)))
}

// ----------------------------------------------------------------- prompts

async fn list_prompts(
    State(state): State<AppState>,
    principal: Principal,
    Path(site_id): Path<Uuid>,
) -> Result<Json<Envelope<Vec<Prompt>>>, AppError> {
    let ws = principal.workspace_id;
    sites::get(&state.pool, ws, site_id).await?;
    Ok(Json(Envelope::ok(
        repo::list_prompts(&state.pool, ws, site_id).await?,
    )))
}

#[derive(Deserialize)]
struct PromptBody {
    text: String,
}

async fn create_prompt(
    State(state): State<AppState>,
    principal: Principal,
    Path(site_id): Path<Uuid>,
    Body(body): Body<PromptBody>,
) -> Result<(StatusCode, Json<Envelope<Prompt>>), AppError> {
    let ws = principal.workspace_id;
    sites::get(&state.pool, ws, site_id).await?;
    let text = tidy(&body.text);
    let length = text.chars().count();
    if !(MIN_PROMPT_CHARS..=MAX_PROMPT_CHARS).contains(&length) {
        return Err(AppError::Invalid(
            "A question must be between 10 and 300 characters.".into(),
        ));
    }
    let prompt = repo::create_prompt(&state.pool, ws, site_id, &text).await?;
    let engines = state
        .engines
        .for_workspace(&state.pool, &state.config, ws)
        .await?;
    if !engines.configured().is_empty() {
        run::schedule_next(&state.pool, ws, site_id).await;
    }
    Ok((StatusCode::CREATED, Json(Envelope::ok(prompt))))
}

async fn delete_prompt(
    State(state): State<AppState>,
    principal: Principal,
    Path(id): Path<Uuid>,
) -> Result<Json<Envelope<serde_json::Value>>, AppError> {
    let ws = principal.workspace_id;
    let site_id = repo::delete_prompt(&state.pool, ws, id).await?;
    // The question's finding is settled by the checks, so they run again.
    crate::integrations::google::sync::refresh_checks(&state.pool, ws, site_id).await;
    Ok(Json(Envelope::ok(serde_json::json!({ "deleted": true }))))
}

#[derive(Serialize)]
struct Suggestions {
    suggestions: Vec<String>,
}

async fn suggest(
    State(state): State<AppState>,
    principal: Principal,
    Path(site_id): Path<Uuid>,
) -> Result<Json<Envelope<Suggestions>>, AppError> {
    let ws = principal.workspace_id;
    let site = sites::get(&state.pool, ws, site_id).await?;
    let engines = state
        .engines
        .for_workspace(&state.pool, &state.config, ws)
        .await?;
    let claude = engines
        .claude()
        .ok_or_else(|| AppError::Invalid(NO_CLAUDE.into()))?;
    repo::spend_suggestion(&state.pool, ws, site_id).await?;
    let settings = repo::get_settings(&state.pool, ws, site_id).await?;
    let words: Vec<String> = keywords::targets(&state.pool, ws, site_id)
        .await?
        .into_iter()
        .map(|k| k.keyword)
        .collect();
    let description = home_description(&state.pool, ws, site_id).await?;
    let suggestions = suggest_prompts(
        claude,
        &site.display_name,
        &description,
        &words,
        &settings.brand_name,
    )
    .await
    .map_err(|err| {
        tracing::warn!(%site_id, error = %err, "question suggestions failed");
        AppError::BadGateway(SUGGEST_FAILED.into())
    })?;
    Ok(Json(Envelope::ok(Suggestions { suggestions })))
}

/// The meta description of the home page in the latest completed crawl.
async fn home_description(
    pool: &sqlx::PgPool,
    workspace_id: Uuid,
    site_id: Uuid,
) -> Result<String, AppError> {
    let text: Option<String> = sqlx::query_scalar(
        "SELECT p.facts->>'meta_description' FROM pages p
         JOIN crawls c ON c.id = p.crawl_id
         WHERE p.workspace_id = $1 AND c.site_id = $2 AND c.status = 'completed'
         ORDER BY c.finished_at DESC, p.depth, p.created_at LIMIT 1",
    )
    .bind(workspace_id)
    .bind(site_id)
    .fetch_optional(pool)
    .await
    .map_err(|err| AppError::Internal(err.into()))?
    .flatten();
    Ok(text.unwrap_or_default().chars().take(500).collect())
}

// -------------------------------------------------------------------- runs

async fn start_run(
    State(state): State<AppState>,
    principal: Principal,
    Path(site_id): Path<Uuid>,
) -> Result<(StatusCode, Json<Envelope<Run>>), AppError> {
    let ws = principal.workspace_id;
    sites::get(&state.pool, ws, site_id).await?;
    let engines = state
        .engines
        .for_workspace(&state.pool, &state.config, ws)
        .await?
        .ids();
    if engines.is_empty() {
        return Err(AppError::Invalid(NO_ENGINE.into()));
    }
    let prompts = repo::list_prompts(&state.pool, ws, site_id).await?;
    if prompts.is_empty() {
        return Err(AppError::Invalid(NO_PROMPTS.into()));
    }
    let total = i32::try_from(prompts.len() * engines.len()).unwrap_or(i32::MAX);
    let created = repo::create_run(&state.pool, ws, site_id, "manual", &engines, total).await?;
    if let Err(err) = run::enqueue_manual(&state.pool, ws, site_id, created.id).await {
        // Without a job the run would hold the site's one place forever.
        let _ = repo::finish_run(
            &state.pool,
            created.id,
            "failed",
            Some("Could not be queued."),
        )
        .await;
        return Err(internal(err));
    }
    Ok((StatusCode::ACCEPTED, Json(Envelope::ok(created))))
}

#[derive(Deserialize)]
struct PageQuery {
    page: Option<i64>,
    limit: Option<i64>,
}

async fn list_runs(
    State(state): State<AppState>,
    principal: Principal,
    Path(site_id): Path<Uuid>,
    Params(query): Params<PageQuery>,
) -> Result<Json<Envelope<Vec<Run>>>, AppError> {
    let ws = principal.workspace_id;
    sites::get(&state.pool, ws, site_id).await?;
    let (page, limit) = paging(query.page, query.limit);
    let (runs, total) =
        repo::list_runs(&state.pool, ws, site_id, limit, (page - 1) * limit).await?;
    Ok(Json(Envelope::page(runs, Meta { total, page, limit })))
}

async fn get_run(
    State(state): State<AppState>,
    principal: Principal,
    Path(id): Path<Uuid>,
) -> Result<Json<Envelope<Run>>, AppError> {
    Ok(Json(Envelope::ok(
        repo::get_run(&state.pool, principal.workspace_id, id).await?,
    )))
}

async fn run_answers(
    State(state): State<AppState>,
    principal: Principal,
    Path(id): Path<Uuid>,
) -> Result<Json<Envelope<Vec<AnswerOut>>>, AppError> {
    let ws = principal.workspace_id;
    repo::get_run(&state.pool, ws, id).await?;
    Ok(Json(Envelope::ok(
        answers::for_run(&state.pool, ws, id).await?,
    )))
}

async fn get_answer(
    State(state): State<AppState>,
    principal: Principal,
    Path(id): Path<Uuid>,
) -> Result<Json<Envelope<AnswerDetail>>, AppError> {
    Ok(Json(Envelope::ok(
        answers::get(&state.pool, principal.workspace_id, id).await?,
    )))
}

async fn domains(
    State(state): State<AppState>,
    principal: Principal,
    Path(site_id): Path<Uuid>,
) -> Result<Json<Envelope<Vec<DomainOut>>>, AppError> {
    let ws = principal.workspace_id;
    let site = sites::get(&state.pool, ws, site_id).await?;
    let Some(latest) = repo::latest_completed(&state.pool, ws, site_id).await? else {
        return Ok(Json(Envelope::ok(Vec::new())));
    };
    let settings = repo::get_settings(&state.pool, ws, site_id).await?;
    let rows = answers::top_domains(
        &state.pool,
        ws,
        latest.id,
        &site.host,
        &settings.competitors,
    )
    .await?;
    Ok(Json(Envelope::ok(rows)))
}
