//! Stored rank runs and results, and the daily allowance for manual checks.
//! Every function takes the workspace and filters by it.

use std::collections::{HashMap, HashSet};

use serde::Serialize;
use serde_json::Value;
use sqlx::types::Json;
use sqlx::PgPool;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::checks::context::{RankedKeyword, RankingData};
use crate::error::AppError;
use crate::integrations::dataforseo::serp::Serp;
use crate::keywords::repo as keywords;
use crate::sites::repo as sites;

/// Manual checks a site may start in a UTC day.
pub const MAX_MANUAL_PER_DAY: i64 = 5;
/// Points kept in a keyword's trend on screen.
pub const HISTORY_POINTS: i64 = 30;

fn internal(err: sqlx::Error) -> AppError {
    AppError::Internal(err.into())
}

/// Where results are tracked: results from different places are never mixed
/// in one trend.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Location {
    pub country: String,
    pub language: String,
    pub device: String,
}

pub async fn location(
    pool: &PgPool,
    workspace_id: Uuid,
    site_id: Uuid,
) -> Result<Location, AppError> {
    let settings = sites::get_settings(pool, workspace_id, site_id).await?;
    Ok(Location {
        country: settings.country,
        language: settings.language,
        device: settings.device,
    })
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Run {
    pub id: Uuid,
    pub site_id: Uuid,
    pub status: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Trigger {
    Manual,
    Scheduled,
}

impl Trigger {
    fn as_str(self) -> &'static str {
        match self {
            Trigger::Manual => "manual",
            Trigger::Scheduled => "scheduled",
        }
    }
}

pub const ALREADY_RUNNING: &str = "A ranking check is already queued or running for this site.";
const DAILY_LIMIT: &str =
    "You have used today's 5 ranking checks for this site. Try again tomorrow.";

/// Makes a queued run. One run per site is queued or running at a time, and a
/// manual one counts against the site's allowance for today (UTC). A refusal
/// uses up nothing. The site row is locked while counting.
pub async fn create_run(
    pool: &PgPool,
    workspace_id: Uuid,
    site_id: Uuid,
    trigger: Trigger,
    total: i32,
) -> Result<Run, AppError> {
    let mut tx = pool.begin().await.map_err(internal)?;
    sqlx::query("SELECT id FROM sites WHERE id = $1 AND workspace_id = $2 FOR UPDATE")
        .bind(site_id)
        .bind(workspace_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(internal)?
        .ok_or(AppError::NotFound)?;
    let active: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM rank_runs WHERE site_id = $1 AND status IN ('queued', 'running'))",
    )
    .bind(site_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(internal)?;
    if active {
        return Err(AppError::Conflict(ALREADY_RUNNING.into()));
    }
    if trigger == Trigger::Manual {
        let used: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM rank_runs
             WHERE site_id = $1 AND trigger = 'manual'
               AND created_at >= (date_trunc('day', now() AT TIME ZONE 'UTC') AT TIME ZONE 'UTC')",
        )
        .bind(site_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(internal)?;
        if used >= MAX_MANUAL_PER_DAY {
            return Err(AppError::DailyLimit(DAILY_LIMIT.into()));
        }
    }
    let run = sqlx::query_as::<_, Run>(
        "INSERT INTO rank_runs (id, workspace_id, site_id, status, trigger, keywords_total)
         VALUES ($1, $2, $3, 'queued', $4, $5) RETURNING id, site_id, status",
    )
    .bind(Uuid::new_v4())
    .bind(workspace_id)
    .bind(site_id)
    .bind(trigger.as_str())
    .bind(total)
    .fetch_one(&mut *tx)
    .await
    .map_err(internal)?;
    tx.commit().await.map_err(internal)?;
    Ok(run)
}

pub async fn get_run(pool: &PgPool, workspace_id: Uuid, id: Uuid) -> Result<Run, AppError> {
    sqlx::query_as("SELECT id, site_id, status FROM rank_runs WHERE id = $1 AND workspace_id = $2")
        .bind(id)
        .bind(workspace_id)
        .fetch_optional(pool)
        .await
        .map_err(internal)?
        .ok_or(AppError::NotFound)
}

/// Marks the run as running. False when it is already over. A resumed run
/// starts its failure count again, since its failed keywords are asked again.
pub async fn begin_run(pool: &PgPool, id: Uuid, total: i32) -> Result<bool, AppError> {
    let result = sqlx::query(
        "UPDATE rank_runs SET status = 'running', keywords_total = $2, keywords_failed = 0,
             started_at = COALESCE(started_at, now())
         WHERE id = $1 AND status IN ('queued', 'running')",
    )
    .bind(id)
    .bind(total)
    .execute(pool)
    .await
    .map_err(internal)?;
    Ok(result.rows_affected() == 1)
}

/// Brings the run's progress up to date: keywords stored plus keywords failed.
pub async fn refresh_progress(pool: &PgPool, id: Uuid) -> Result<(), AppError> {
    sqlx::query(
        "UPDATE rank_runs SET keywords_done =
             (SELECT COUNT(*) FROM rank_checks WHERE run_id = $1)::int + keywords_failed
         WHERE id = $1",
    )
    .bind(id)
    .execute(pool)
    .await
    .map_err(internal)?;
    Ok(())
}

pub async fn record_failure(pool: &PgPool, id: Uuid) -> Result<(), AppError> {
    sqlx::query("UPDATE rank_runs SET keywords_failed = keywords_failed + 1 WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await
        .map_err(internal)?;
    refresh_progress(pool, id).await
}

/// Ends a queued or running run. A run already over is left as it is.
pub async fn finish_run(
    pool: &PgPool,
    id: Uuid,
    status: &str,
    error: Option<&str>,
) -> Result<(), AppError> {
    sqlx::query(
        "UPDATE rank_runs SET status = $2, error = $3, finished_at = now()
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

/// Whether a run that did not fail was made in the last `hours` hours.
pub async fn recent_run(pool: &PgPool, site_id: Uuid, hours: i32) -> Result<bool, AppError> {
    sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM rank_runs WHERE site_id = $1 AND status <> 'failed'
             AND created_at >= now() - make_interval(hours => $2))",
    )
    .bind(site_id)
    .bind(hours)
    .fetch_one(pool)
    .await
    .map_err(internal)
}

/// The keywords (lower case) this run has already stored.
pub async fn stored_keywords(pool: &PgPool, run_id: Uuid) -> Result<HashSet<String>, AppError> {
    let rows: Vec<String> =
        sqlx::query_scalar("SELECT lower(keyword) FROM rank_checks WHERE run_id = $1")
            .bind(run_id)
            .fetch_all(pool)
            .await
            .map_err(internal)?;
    Ok(rows.into_iter().collect())
}

pub async fn stored_count(pool: &PgPool, run_id: Uuid) -> Result<i64, AppError> {
    sqlx::query_scalar("SELECT COUNT(*) FROM rank_checks WHERE run_id = $1")
        .bind(run_id)
        .fetch_one(pool)
        .await
        .map_err(internal)
}

/// Stores one keyword's result. A keyword the run already stored is left
/// alone, so a retry never doubles it.
pub async fn save_check(
    pool: &PgPool,
    workspace_id: Uuid,
    site_id: Uuid,
    run_id: Uuid,
    keyword: &keywords::KeywordRow,
    location: &Location,
    serp: &Serp,
) -> Result<(), AppError> {
    sqlx::query(
        "INSERT INTO rank_checks
             (id, workspace_id, site_id, run_id, keyword_id, keyword, country, language, device,
              position, url, ai_overview, ai_cited_domains, ai_your_urls, top)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15)
         ON CONFLICT (run_id, lower(keyword)) WHERE run_id IS NOT NULL DO NOTHING",
    )
    .bind(Uuid::new_v4())
    .bind(workspace_id)
    .bind(site_id)
    .bind(run_id)
    .bind(keyword.id)
    .bind(&keyword.keyword)
    .bind(&location.country)
    .bind(&location.language)
    .bind(&location.device)
    .bind(serp.position)
    .bind(&serp.url)
    .bind(serp.ai.status.as_str())
    .bind(Json(&serp.ai.cited_domains))
    .bind(Json(&serp.ai.your_urls))
    .bind(Json(&serp.top))
    .execute(pool)
    .await
    .map_err(internal)?;
    Ok(())
}

/// One stored result.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct StoredCheck {
    pub keyword: String,
    pub position: Option<i32>,
    pub url: Option<String>,
    pub ai_overview: String,
    pub ai_cited_domains: Json<Vec<String>>,
    pub ai_your_urls: Json<Vec<String>>,
    pub top: Value,
    pub checked_at: OffsetDateTime,
}

/// The newest `per_keyword` results of each keyword at this location, newest
/// first, grouped by the keyword in lower case.
pub async fn recent(
    pool: &PgPool,
    workspace_id: Uuid,
    site_id: Uuid,
    location: &Location,
    per_keyword: i64,
) -> Result<HashMap<String, Vec<StoredCheck>>, AppError> {
    let rows: Vec<StoredCheck> = sqlx::query_as(
        "SELECT keyword, position, url, ai_overview, ai_cited_domains, ai_your_urls, top, checked_at
         FROM (SELECT *, row_number() OVER (PARTITION BY lower(keyword)
                                           ORDER BY checked_at DESC, id DESC) AS n
               FROM rank_checks
               WHERE workspace_id = $1 AND site_id = $2
                 AND country = $3 AND language = $4 AND device = $5) t
         WHERE n <= $6 ORDER BY lower(keyword), n",
    )
    .bind(workspace_id)
    .bind(site_id)
    .bind(&location.country)
    .bind(&location.language)
    .bind(&location.device)
    .bind(per_keyword)
    .fetch_all(pool)
    .await
    .map_err(internal)?;
    let mut grouped: HashMap<String, Vec<StoredCheck>> = HashMap::new();
    for row in rows {
        grouped
            .entry(row.keyword.to_lowercase())
            .or_default()
            .push(row);
    }
    Ok(grouped)
}

/// How the site's newest run of any outcome stands. `error` is a plain
/// sentence stored by the run itself, never a provider's own words.
#[derive(Debug, serde::Serialize, sqlx::FromRow)]
pub struct LastRun {
    pub id: Uuid,
    pub status: String,
    pub error: Option<String>,
    #[serde(with = "time::serde::rfc3339::option")]
    pub finished_at: Option<OffsetDateTime>,
}

/// The newest run of the site, including one that stored nothing.
pub async fn last_run(
    pool: &PgPool,
    workspace_id: Uuid,
    site_id: Uuid,
) -> Result<Option<LastRun>, AppError> {
    sqlx::query_as(
        "SELECT id, status, error, finished_at FROM rank_runs
         WHERE workspace_id = $1 AND site_id = $2
         ORDER BY created_at DESC, id LIMIT 1",
    )
    .bind(workspace_id)
    .bind(site_id)
    .fetch_optional(pool)
    .await
    .map_err(internal)
}

/// When the newest finished run that stored anything here finished. A run
/// still going has not finished, so a screen waiting for this to change waits
/// for the whole run.
pub async fn finished_at(
    pool: &PgPool,
    workspace_id: Uuid,
    site_id: Uuid,
    location: &Location,
) -> Result<Option<OffsetDateTime>, AppError> {
    sqlx::query_scalar(
        "SELECT MAX(r.finished_at) FROM rank_runs r
         WHERE r.workspace_id = $1 AND r.site_id = $2 AND r.status IN ('completed', 'failed')
           AND EXISTS (SELECT 1 FROM rank_checks c
                       WHERE c.run_id = r.id AND c.country = $3 AND c.language = $4
                         AND c.device = $5)",
    )
    .bind(workspace_id)
    .bind(site_id)
    .bind(&location.country)
    .bind(&location.language)
    .bind(&location.device)
    .fetch_one(pool)
    .await
    .map_err(internal)
}

/// What the ranking checks read: each current keyword's latest result and
/// the one before it, at the site's location. `None` until something has
/// been checked.
pub async fn load_ranking_data(
    pool: &PgPool,
    workspace_id: Uuid,
    site_id: Uuid,
) -> Result<Option<RankingData>, AppError> {
    let place = location(pool, workspace_id, site_id).await?;
    let mut stored = recent(pool, workspace_id, site_id, &place, 2).await?;
    let mut found = Vec::new();
    for row in keywords::list(pool, workspace_id, site_id).await? {
        let Some(mut checks) = stored.remove(&row.keyword.to_lowercase()) else {
            continue;
        };
        let latest = checks.remove(0);
        found.push(RankedKeyword {
            keyword: row.keyword,
            url: latest.url,
            position: latest.position,
            previous: checks.first().map(|earlier| earlier.position),
            ai_overview: latest.ai_overview,
            cited_domains: latest.ai_cited_domains.0,
        });
    }
    Ok((!found.is_empty()).then_some(RankingData { keywords: found }))
}

/// The latest result of each of the site's current keywords at its location,
/// and when the newest finished run was done (or, with none, when the newest
/// result was stored).
pub async fn current(
    pool: &PgPool,
    workspace_id: Uuid,
    site_id: Uuid,
) -> Result<(Vec<StoredCheck>, Option<OffsetDateTime>), AppError> {
    let place = location(pool, workspace_id, site_id).await?;
    let mut stored = recent(pool, workspace_id, site_id, &place, 1).await?;
    let mut latest = Vec::new();
    for row in keywords::list(pool, workspace_id, site_id).await? {
        if let Some(mut checks) = stored.remove(&row.keyword.to_lowercase()) {
            latest.push(checks.remove(0));
        }
    }
    let when = match finished_at(pool, workspace_id, site_id, &place).await? {
        Some(done) => Some(done),
        None => latest.iter().map(|check| check.checked_at).max(),
    };
    Ok((latest, when))
}
