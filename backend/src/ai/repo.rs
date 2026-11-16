//! AI visibility settings, questions and runs. Every function takes the
//! workspace and filters by it.

use std::collections::HashSet;

use serde::Serialize;
use serde_json::Value;
use sqlx::{PgPool, Row};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::error::AppError;

/// Questions one site may hold.
pub const MAX_PROMPTS: i64 = 25;
/// Manual runs one site may start per UTC day.
pub const MAX_MANUAL_RUNS_PER_DAY: i64 = 3;
/// Manual runs one workspace may start per UTC day, across all its sites
/// (deleted ones included).
pub const MAX_WORKSPACE_RUNS_PER_DAY: i64 = 10;
/// Question-suggestion requests one site may make per UTC day.
pub const MAX_SUGGESTIONS_PER_DAY: i64 = 10;

const WORKSPACE_LIMIT: &str =
    "You have used today's 10 AI visibility checks for this workspace. Try again tomorrow.";
const SUGGESTION_LIMIT: &str =
    "You have asked for suggestions 10 times today for this site. Try again tomorrow.";

/// The start of today, UTC, as SQL.
macro_rules! today_utc {
    () => {
        "(date_trunc('day', now() AT TIME ZONE 'UTC') AT TIME ZONE 'UTC')"
    };
}

fn internal(err: sqlx::Error) -> AppError {
    AppError::Internal(err.into())
}

fn is_unique(err: &sqlx::Error) -> bool {
    err.as_database_error()
        .is_some_and(|db| db.is_unique_violation())
}

/// Locks the site row so that counting and inserting cannot interleave.
async fn lock_site(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    workspace_id: Uuid,
    site_id: Uuid,
) -> Result<(), AppError> {
    sqlx::query("SELECT id FROM sites WHERE id = $1 AND workspace_id = $2 FOR UPDATE")
        .bind(site_id)
        .bind(workspace_id)
        .fetch_optional(&mut **tx)
        .await
        .map_err(internal)?
        .map(|_| ())
        .ok_or(AppError::NotFound)
}

// ---------------------------------------------------------------- settings

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Settings {
    pub brand_name: String,
    pub aliases: Vec<String>,
    pub competitors: Vec<String>,
}

fn strings(value: Value) -> Vec<String> {
    serde_json::from_value(value).unwrap_or_default()
}

pub async fn get_settings(
    pool: &PgPool,
    workspace_id: Uuid,
    site_id: Uuid,
) -> Result<Settings, AppError> {
    let row: Option<(String, Value, Value)> = sqlx::query_as(
        "SELECT brand_name, aliases, competitors FROM ai_settings
         WHERE workspace_id = $1 AND site_id = $2",
    )
    .bind(workspace_id)
    .bind(site_id)
    .fetch_optional(pool)
    .await
    .map_err(internal)?;
    Ok(
        row.map_or_else(Settings::default, |(brand_name, aliases, competitors)| {
            Settings {
                brand_name,
                aliases: strings(aliases),
                competitors: strings(competitors),
            }
        }),
    )
}

pub async fn put_settings(
    pool: &PgPool,
    workspace_id: Uuid,
    site_id: Uuid,
    settings: &Settings,
) -> Result<(), AppError> {
    sqlx::query(
        "INSERT INTO ai_settings (site_id, workspace_id, brand_name, aliases, competitors)
         VALUES ($1, $2, $3, $4, $5)
         ON CONFLICT (site_id) DO UPDATE
         SET brand_name = EXCLUDED.brand_name, aliases = EXCLUDED.aliases,
             competitors = EXCLUDED.competitors",
    )
    .bind(site_id)
    .bind(workspace_id)
    .bind(&settings.brand_name)
    .bind(serde_json::json!(settings.aliases))
    .bind(serde_json::json!(settings.competitors))
    .execute(pool)
    .await
    .map_err(internal)?;
    Ok(())
}

// ----------------------------------------------------------------- prompts

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct Prompt {
    pub id: Uuid,
    pub text: String,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
}

pub async fn list_prompts(
    pool: &PgPool,
    workspace_id: Uuid,
    site_id: Uuid,
) -> Result<Vec<Prompt>, AppError> {
    sqlx::query_as(
        "SELECT id, text, created_at FROM ai_prompts
         WHERE workspace_id = $1 AND site_id = $2 ORDER BY created_at, id",
    )
    .bind(workspace_id)
    .bind(site_id)
    .fetch_all(pool)
    .await
    .map_err(internal)
}

pub async fn create_prompt(
    pool: &PgPool,
    workspace_id: Uuid,
    site_id: Uuid,
    text: &str,
) -> Result<Prompt, AppError> {
    let mut tx = pool.begin().await.map_err(internal)?;
    lock_site(&mut tx, workspace_id, site_id).await?;
    let count: i64 = sqlx::query("SELECT COUNT(*) FROM ai_prompts WHERE site_id = $1")
        .bind(site_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(internal)?
        .get(0);
    if count >= MAX_PROMPTS {
        return Err(AppError::Conflict(
            "A site can have at most 25 questions.".into(),
        ));
    }
    let inserted = sqlx::query_as::<_, Prompt>(
        "INSERT INTO ai_prompts (id, workspace_id, site_id, text) VALUES ($1, $2, $3, $4)
         RETURNING id, text, created_at",
    )
    .bind(Uuid::new_v4())
    .bind(workspace_id)
    .bind(site_id)
    .bind(text)
    .fetch_one(&mut *tx)
    .await;
    match inserted {
        Ok(row) => {
            tx.commit().await.map_err(internal)?;
            Ok(row)
        }
        Err(err) if is_unique(&err) => Err(AppError::Conflict(
            "This question is already on the list.".into(),
        )),
        Err(err) => Err(internal(err)),
    }
}

/// Deletes the question and returns the site it belonged to.
pub async fn delete_prompt(pool: &PgPool, workspace_id: Uuid, id: Uuid) -> Result<Uuid, AppError> {
    sqlx::query_scalar(
        "DELETE FROM ai_prompts WHERE id = $1 AND workspace_id = $2 RETURNING site_id",
    )
    .bind(id)
    .bind(workspace_id)
    .fetch_optional(pool)
    .await
    .map_err(internal)?
    .ok_or(AppError::NotFound)
}

// -------------------------------------------------------------------- runs

#[derive(Debug, Clone, Serialize)]
pub struct Run {
    pub id: Uuid,
    pub status: String,
    pub trigger: String,
    pub engines: Vec<String>,
    pub answers_total: i32,
    pub answers_done: i32,
    pub error: Option<String>,
    #[serde(with = "time::serde::rfc3339::option")]
    pub started_at: Option<OffsetDateTime>,
    #[serde(with = "time::serde::rfc3339::option")]
    pub finished_at: Option<OffsetDateTime>,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    #[serde(skip)]
    pub site_id: Uuid,
}

type RunRow = (
    Uuid,
    String,
    String,
    Value,
    i32,
    i32,
    Option<String>,
    Option<OffsetDateTime>,
    Option<OffsetDateTime>,
    OffsetDateTime,
    Uuid,
);

macro_rules! run_columns {
    () => {
        "id, status, trigger, engines, answers_total, answers_done, error, started_at, \
         finished_at, created_at, site_id"
    };
}

fn run_of(row: RunRow) -> Run {
    Run {
        id: row.0,
        status: row.1,
        trigger: row.2,
        engines: strings(row.3),
        answers_total: row.4,
        answers_done: row.5,
        error: row.6,
        started_at: row.7,
        finished_at: row.8,
        created_at: row.9,
        site_id: row.10,
    }
}

pub async fn get_run(pool: &PgPool, workspace_id: Uuid, id: Uuid) -> Result<Run, AppError> {
    sqlx::query_as::<_, RunRow>(concat!(
        "SELECT ",
        run_columns!(),
        " FROM ai_runs WHERE id = $1 AND workspace_id = $2"
    ))
    .bind(id)
    .bind(workspace_id)
    .fetch_optional(pool)
    .await
    .map_err(internal)?
    .map(run_of)
    .ok_or(AppError::NotFound)
}

pub async fn list_runs(
    pool: &PgPool,
    workspace_id: Uuid,
    site_id: Uuid,
    limit: i64,
    offset: i64,
) -> Result<(Vec<Run>, i64), AppError> {
    let rows = sqlx::query_as::<_, RunRow>(concat!(
        "SELECT ",
        run_columns!(),
        " FROM ai_runs WHERE workspace_id = $1 AND site_id = $2
         ORDER BY created_at DESC, id LIMIT $3 OFFSET $4"
    ))
    .bind(workspace_id)
    .bind(site_id)
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await
    .map_err(internal)?;
    let total: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM ai_runs WHERE workspace_id = $1 AND site_id = $2")
            .bind(workspace_id)
            .bind(site_id)
            .fetch_one(pool)
            .await
            .map_err(internal)?;
    Ok((rows.into_iter().map(run_of).collect(), total))
}

/// The run that is queued or running for this site, if any.
pub async fn active_run(
    pool: &PgPool,
    workspace_id: Uuid,
    site_id: Uuid,
) -> Result<Option<Run>, AppError> {
    sqlx::query_as::<_, RunRow>(concat!(
        "SELECT ",
        run_columns!(),
        " FROM ai_runs
         WHERE workspace_id = $1 AND site_id = $2 AND status IN ('queued', 'running')"
    ))
    .bind(workspace_id)
    .bind(site_id)
    .fetch_optional(pool)
    .await
    .map(|row| row.map(run_of))
    .map_err(internal)
}

const ACTIVE_RUN: &str = "An AI visibility check is already queued or running for this site.";

/// Starts a run in the `queued` state. One run is queued or running per site
/// at a time, enforced by the database, so two requests at once leave one.
/// A manual run also counts against the day's allowance; a scheduled one does
/// not.
pub async fn create_run(
    pool: &PgPool,
    workspace_id: Uuid,
    site_id: Uuid,
    trigger: &str,
    engines: &[&str],
    answers_total: i32,
) -> Result<Run, AppError> {
    let mut tx = pool.begin().await.map_err(internal)?;
    lock_site(&mut tx, workspace_id, site_id).await?;
    let active: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM ai_runs WHERE site_id = $1 AND status IN ('queued', 'running'))",
    )
    .bind(site_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(internal)?;
    if active {
        return Err(AppError::Conflict(ACTIVE_RUN.into()));
    }
    if trigger == "manual" {
        let used: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM ai_runs
             WHERE site_id = $1 AND trigger = 'manual'
               AND created_at >= (date_trunc('day', now() AT TIME ZONE 'UTC') AT TIME ZONE 'UTC')",
        )
        .bind(site_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(internal)?;
        if used >= MAX_MANUAL_RUNS_PER_DAY {
            return Err(AppError::DailyLimit(
                "You have used today's 3 AI visibility checks for this site. Try again tomorrow."
                    .into(),
            ));
        }
        ensure_workspace_allowance(&mut tx, workspace_id).await?;
    }
    let inserted = sqlx::query_as::<_, RunRow>(concat!(
        "INSERT INTO ai_runs (id, workspace_id, site_id, status, trigger, engines, answers_total)
         VALUES ($1, $2, $3, 'queued', $4, $5, $6) RETURNING ",
        run_columns!()
    ))
    .bind(Uuid::new_v4())
    .bind(workspace_id)
    .bind(site_id)
    .bind(trigger)
    .bind(serde_json::json!(engines))
    .bind(answers_total)
    .fetch_one(&mut *tx)
    .await;
    match inserted {
        Ok(row) => {
            if trigger == "manual" {
                record_usage(&mut tx, workspace_id, site_id, "manual_run").await?;
            }
            tx.commit().await.map_err(internal)?;
            Ok(run_of(row))
        }
        Err(err) if is_unique(&err) => Err(AppError::Conflict(ACTIVE_RUN.into())),
        Err(err) => Err(internal(err)),
    }
}

/// Serialises everyone counting and spending one allowance, so two requests
/// at once cannot both take the last place.
async fn lock_allowance(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    key: String,
) -> Result<(), AppError> {
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))")
        .bind(key)
        .execute(&mut **tx)
        .await
        .map_err(internal)?;
    Ok(())
}

/// The workspace's manual runs today, deleted sites included, because the
/// count comes from `ai_usage` and not from the runs.
async fn ensure_workspace_allowance(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    workspace_id: Uuid,
) -> Result<(), AppError> {
    lock_allowance(tx, format!("ai_usage:{workspace_id}")).await?;
    let used: i64 = sqlx::query_scalar(concat!(
        "SELECT COUNT(*) FROM ai_usage
         WHERE workspace_id = $1 AND kind = 'manual_run' AND created_at >= ",
        today_utc!()
    ))
    .bind(workspace_id)
    .fetch_one(&mut **tx)
    .await
    .map_err(internal)?;
    if used >= MAX_WORKSPACE_RUNS_PER_DAY {
        return Err(AppError::DailyLimit(WORKSPACE_LIMIT.into()));
    }
    Ok(())
}

async fn record_usage(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    workspace_id: Uuid,
    site_id: Uuid,
    kind: &str,
) -> Result<(), AppError> {
    sqlx::query("INSERT INTO ai_usage (workspace_id, site_id, kind) VALUES ($1, $2, $3)")
        .bind(workspace_id)
        .bind(site_id)
        .bind(kind)
        .execute(&mut **tx)
        .await
        .map_err(internal)?;
    Ok(())
}

/// Counts one question-suggestion request against the site's day, or refuses
/// it once the day's 10 are used. Asked before Claude is called, so a
/// request that fails after that still counts: it may have been paid for.
pub async fn spend_suggestion(
    pool: &PgPool,
    workspace_id: Uuid,
    site_id: Uuid,
) -> Result<(), AppError> {
    let mut tx = pool.begin().await.map_err(internal)?;
    lock_allowance(&mut tx, format!("ai_suggest:{site_id}")).await?;
    let used: i64 = sqlx::query_scalar(concat!(
        "SELECT COUNT(*) FROM ai_usage
         WHERE site_id = $1 AND kind = 'suggest' AND created_at >= ",
        today_utc!()
    ))
    .bind(site_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(internal)?;
    if used >= MAX_SUGGESTIONS_PER_DAY {
        return Err(AppError::DailyLimit(SUGGESTION_LIMIT.into()));
    }
    record_usage(&mut tx, workspace_id, site_id, "suggest").await?;
    tx.commit().await.map_err(internal)
}

/// Moves a run to `running` (it may already be, on a retry) and records how
/// many answers it will hold. Returns false when the run is gone or finished.
pub async fn begin_run(pool: &PgPool, id: Uuid, answers_total: i32) -> Result<bool, AppError> {
    let result = sqlx::query(
        "UPDATE ai_runs SET status = 'running', started_at = COALESCE(started_at, now()),
                answers_total = $2
         WHERE id = $1 AND status IN ('queued', 'running')",
    )
    .bind(id)
    .bind(answers_total)
    .execute(pool)
    .await
    .map_err(internal)?;
    Ok(result.rows_affected() == 1)
}

/// `answers_done` is always the number of stored answers, so a retried run
/// never counts twice.
pub async fn refresh_progress(pool: &PgPool, id: Uuid) -> Result<(), AppError> {
    sqlx::query(
        "UPDATE ai_runs SET answers_done = (SELECT COUNT(*) FROM ai_answers WHERE run_id = $1)
         WHERE id = $1",
    )
    .bind(id)
    .execute(pool)
    .await
    .map_err(internal)?;
    Ok(())
}

pub async fn finish_run(
    pool: &PgPool,
    id: Uuid,
    status: &str,
    error: Option<&str>,
) -> Result<(), AppError> {
    sqlx::query(
        "UPDATE ai_runs SET status = $2, error = $3, finished_at = now()
         WHERE id = $1 AND status IN ('queued', 'running')",
    )
    .bind(id)
    .bind(status)
    .bind(error)
    .execute(pool)
    .await
    .map_err(internal)?;
    Ok(())
}

/// The `(prompt, engine)` pairs a run already holds an answer for.
pub async fn stored_pairs(
    pool: &PgPool,
    run_id: Uuid,
) -> Result<HashSet<(Uuid, String)>, AppError> {
    let rows: Vec<(Uuid, String)> =
        sqlx::query_as("SELECT prompt_id, engine FROM ai_answers WHERE run_id = $1")
            .bind(run_id)
            .fetch_all(pool)
            .await
            .map_err(internal)?;
    Ok(rows.into_iter().collect())
}

/// Whether any engine has a completed run to look at.
pub async fn has_completed_run(
    pool: &PgPool,
    workspace_id: Uuid,
    site_id: Uuid,
) -> Result<bool, AppError> {
    sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM ai_runs
         WHERE workspace_id = $1 AND site_id = $2 AND status = 'completed')",
    )
    .bind(workspace_id)
    .bind(site_id)
    .fetch_one(pool)
    .await
    .map_err(internal)
}

/// The newest completed run.
pub async fn latest_completed(
    pool: &PgPool,
    workspace_id: Uuid,
    site_id: Uuid,
) -> Result<Option<Run>, AppError> {
    sqlx::query_as::<_, RunRow>(concat!(
        "SELECT ",
        run_columns!(),
        " FROM ai_runs
         WHERE workspace_id = $1 AND site_id = $2 AND status = 'completed'
         ORDER BY finished_at DESC, created_at DESC LIMIT 1"
    ))
    .bind(workspace_id)
    .bind(site_id)
    .fetch_optional(pool)
    .await
    .map(|row| row.map(run_of))
    .map_err(internal)
}

/// The status of the newest run of any kind.
pub async fn latest_status(
    pool: &PgPool,
    workspace_id: Uuid,
    site_id: Uuid,
) -> Result<Option<String>, AppError> {
    sqlx::query_scalar(
        "SELECT status FROM ai_runs WHERE workspace_id = $1 AND site_id = $2
         ORDER BY created_at DESC LIMIT 1",
    )
    .bind(workspace_id)
    .bind(site_id)
    .fetch_optional(pool)
    .await
    .map_err(internal)
}
