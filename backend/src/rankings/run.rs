//! The `rank_run` job: asks DataForSEO for Google's results for every keyword
//! of a site, four at a time, and stores each answer. One keyword failing
//! leaves that keyword's old result and the run goes on; only a database
//! failure fails the job, and a retried job picks up where the last one
//! stopped. Nothing is asked without the workspace's own DataForSEO login,
//! whoever started the run.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use futures::stream::{self, StreamExt};
use sqlx::PgPool;
use time::{Duration, OffsetDateTime};
use uuid::Uuid;

use super::repo::{self, Location, Run, Trigger};
use crate::config::Config;
use crate::error::AppError;
use crate::integrations::dataforseo::client::SerpQuery;
use crate::integrations::dataforseo::{
    locations, pricing, serp, Credentials, DataForSeoClient, DataForSeoError,
};
use crate::jobs::queue::{self, Job};
use crate::jobs::worker::JobHandler;
use crate::keywords::repo::{self as keywords, KeywordRow};
use crate::settings::resolve;
use crate::sites::repo as sites;

pub const KIND: &str = "rank_run";
/// Requests in flight at once.
pub const PARALLEL: usize = 4;
/// A schedule that is being started waits this long, so a person adding
/// several keywords makes one run.
const FIRST_RUN_DELAY: Duration = Duration::minutes(10);
const USAGE_SOURCE: &str = "dataforseo";
const USAGE_KIND: &str = "serp";

pub const NO_LOGIN: &str =
    "Rank tracking needs a DataForSEO login. Add it in Settings, then try again.";
const NO_KEYWORDS: &str = "Add at least one keyword before checking rankings.";
const NO_LOCATION: &str =
    "This site's country or language cannot be tracked. Change it in the site's settings.";
const STOPPED: &str = "The check stopped because of an internal error. Try again.";
const GONE: &str = "The DataForSEO login is no longer set up. Add it in Settings, then try again.";

pub struct RankRunJob {
    pool: PgPool,
    config: Arc<Config>,
    client: Arc<DataForSeoClient>,
}

impl RankRunJob {
    pub fn new(pool: PgPool, config: Arc<Config>, client: Arc<DataForSeoClient>) -> Self {
        Self {
            pool,
            config,
            client,
        }
    }
}

fn site_ids(job: &Job) -> anyhow::Result<(Uuid, Uuid)> {
    let id = |key: &str| -> anyhow::Result<Uuid> {
        job.payload
            .get(key)
            .and_then(|value| value.as_str())
            .unwrap_or("")
            .parse()
            .map_err(|err| anyhow::anyhow!("{key}: {err}"))
    };
    Ok((id("workspace_id")?, id("site_id")?))
}

fn run_id(job: &Job) -> Option<Uuid> {
    job.payload
        .get("run_id")
        .and_then(|value| value.as_str())
        .and_then(|text| text.parse().ok())
}

#[async_trait]
impl JobHandler for RankRunJob {
    fn kind(&self) -> &'static str {
        KIND
    }

    async fn run(&self, job: &Job) -> anyhow::Result<()> {
        let (workspace_id, site_id) = site_ids(job)?;
        let run = match run_id(job) {
            Some(id) => match repo::get_run(&self.pool, workspace_id, id).await {
                Ok(run) => run,
                Err(AppError::NotFound) => return Ok(()),
                Err(err) => return Err(err.into()),
            },
            None => match self.start_scheduled(workspace_id, site_id).await? {
                Some(run) => {
                    self.remember_run(job, &run).await?;
                    run
                }
                None => return Ok(()),
            },
        };
        let Some((login, password)) =
            resolve::dataforseo(&self.pool, &self.config, workspace_id).await?
        else {
            repo::finish_run(&self.pool, run.id, "failed", Some(GONE)).await?;
            return Ok(());
        };
        let credentials = Credentials {
            login: &login,
            password: &password,
        };
        execute(&self.pool, &self.client, &credentials, workspace_id, &run).await?;
        Ok(())
    }

    async fn on_dead(&self, job: &Job, error: &str) {
        let Ok((workspace_id, site_id)) = site_ids(job) else {
            return;
        };
        tracing::error!(%site_id, %error, "rank run failed for good");
        // A run left queued or running would block the site's next one. Only
        // the run this job carries is touched: a job without one made none.
        if let Some(id) = run_id(job) {
            if let Err(err) = repo::finish_run(&self.pool, id, "failed", Some(STOPPED)).await {
                tracing::error!(%site_id, error = %err, "could not mark the run failed");
            }
        }
        schedule_next(&self.pool, workspace_id, site_id).await;
    }
}

impl RankRunJob {
    /// Writes the new run's id into the job, so a retry resumes this run
    /// instead of meeting the one-active-run rule.
    async fn remember_run(&self, job: &Job, run: &Run) -> anyhow::Result<()> {
        if let Err(err) = queue::attach_run(&self.pool, job.id, run.id).await {
            let _ = repo::finish_run(&self.pool, run.id, "failed", Some(STOPPED)).await;
            return Err(err);
        }
        Ok(())
    }

    /// A scheduled run: made when its job comes due, and only when the site
    /// still tracks rankings, has keywords and the workspace has a login. When
    /// not, nothing is asked and the schedule stops until one of those changes.
    async fn start_scheduled(
        &self,
        workspace_id: Uuid,
        site_id: Uuid,
    ) -> Result<Option<Run>, AppError> {
        let settings = match sites::get_settings(&self.pool, workspace_id, site_id).await {
            Ok(settings) => settings,
            Err(AppError::NotFound) => return Ok(None),
            Err(err) => return Err(err),
        };
        if !settings.track_rankings || !has_login(&self.pool, &self.config, workspace_id).await? {
            return Ok(None);
        }
        let total = keywords::list(&self.pool, workspace_id, site_id)
            .await?
            .len();
        if total == 0 {
            return Ok(None);
        }
        // A check a person just made counts: the schedule goes on from there.
        let hours = interval(&settings.ranking_frequency).whole_hours() as i32 / 2;
        if repo::recent_run(&self.pool, site_id, hours).await? {
            schedule_next(&self.pool, workspace_id, site_id).await;
            return Ok(None);
        }
        let total = i32::try_from(total).unwrap_or(i32::MAX);
        match repo::create_run(&self.pool, workspace_id, site_id, Trigger::Scheduled, total).await {
            Ok(run) => Ok(Some(run)),
            // A run is under way; its end queues the next one.
            Err(AppError::Conflict(_)) | Err(AppError::NotFound) => Ok(None),
            Err(err) => Err(err),
        }
    }
}

pub async fn has_login(
    pool: &PgPool,
    config: &Config,
    workspace_id: Uuid,
) -> Result<bool, AppError> {
    Ok(resolve::dataforseo(pool, config, workspace_id)
        .await?
        .is_some())
}

fn interval(frequency: &str) -> Duration {
    if frequency == "daily" {
        Duration::days(1)
    } else {
        Duration::days(7)
    }
}

/// Queues the next scheduled run, unless one is already waiting. It does
/// nothing when it comes due unless the site still qualifies.
pub async fn schedule_next(pool: &PgPool, workspace_id: Uuid, site_id: Uuid) {
    let frequency = match sites::get_settings(pool, workspace_id, site_id).await {
        Ok(settings) => settings.ranking_frequency,
        Err(_) => return,
    };
    let run_at = OffsetDateTime::now_utc() + interval(&frequency);
    if let Err(err) =
        queue::enqueue_site_job_unless_queued(pool, KIND, workspace_id, site_id, run_at).await
    {
        tracing::error!(%site_id, error = %err, "could not schedule the next ranking check");
    }
}

/// Starts a site's schedule when everything it needs is in place: tracking is
/// on, there are keywords and the workspace has a login. The first run comes
/// a few minutes later and a schedule already waiting is left as it is, so
/// changing a setting never brings a paid run closer. Never fails.
pub async fn start_schedule(pool: &PgPool, config: &Config, workspace_id: Uuid, site_id: Uuid) {
    let ready = async {
        let settings = sites::get_settings(pool, workspace_id, site_id).await?;
        let keywords = keywords::list(pool, workspace_id, site_id).await?;
        let login = has_login(pool, config, workspace_id).await?;
        Ok::<bool, AppError>(settings.track_rankings && login && !keywords.is_empty())
    };
    match ready.await {
        Ok(true) => {
            let run_at = OffsetDateTime::now_utc() + FIRST_RUN_DELAY;
            if let Err(err) =
                queue::enqueue_site_job_if_none(pool, KIND, workspace_id, site_id, run_at).await
            {
                tracing::error!(%site_id, error = %err, "could not start the ranking schedule");
            }
        }
        Ok(false) => {}
        Err(err) => {
            tracing::error!(%site_id, error = %err, "could not look up the ranking schedule")
        }
    }
}

/// `start_schedule` for every site of the workspace, for when a login is saved.
pub async fn start_schedules(pool: &PgPool, config: &Config, workspace_id: Uuid) {
    match sites::list(pool, workspace_id).await {
        Ok(all) => {
            for site in all {
                start_schedule(pool, config, workspace_id, site.id).await;
            }
        }
        Err(err) => tracing::error!(error = %err, "could not list sites to start rank schedules"),
    }
}

/// Queues a run a person asked for, so it starts as soon as the worker is free.
pub async fn enqueue_manual(
    pool: &PgPool,
    workspace_id: Uuid,
    site_id: Uuid,
    run_id: Uuid,
) -> anyhow::Result<()> {
    let payload = serde_json::json!({
        "workspace_id": workspace_id,
        "site_id": site_id,
        "run_id": run_id,
    });
    queue::enqueue(pool, KIND, &payload, OffsetDateTime::now_utc()).await?;
    Ok(())
}

enum Outcome {
    Stored,
    Failed,
}

/// What the keywords of one run found out, shared between them.
struct Shared {
    halt: AtomicBool,
    fatal: Mutex<Option<DataForSeoError>>,
    last: Mutex<Option<DataForSeoError>>,
}

pub async fn execute(
    pool: &PgPool,
    client: &DataForSeoClient,
    credentials: &Credentials<'_>,
    workspace_id: Uuid,
    run: &Run,
) -> Result<(), AppError> {
    if !matches!(run.status.as_str(), "queued" | "running") {
        return Ok(());
    }
    let site = sites::get(pool, workspace_id, run.site_id).await?;
    let place = repo::location(pool, workspace_id, run.site_id).await?;
    let all = keywords::list(pool, workspace_id, run.site_id).await?;
    let Some(location_code) = locations::location_code(&place.country) else {
        return end(pool, run, "failed", Some(NO_LOCATION)).await;
    };
    if all.is_empty() {
        return end(pool, run, "failed", Some(NO_KEYWORDS)).await;
    }
    let total = i32::try_from(all.len()).unwrap_or(i32::MAX);
    if !repo::begin_run(pool, run.id, total).await? {
        return Ok(());
    }
    let done = repo::stored_keywords(pool, run.id).await?;
    let todo: Vec<KeywordRow> = all
        .into_iter()
        .filter(|row| !done.contains(&row.keyword.to_lowercase()))
        .collect();
    let shared = Shared {
        halt: AtomicBool::new(false),
        fatal: Mutex::new(None),
        last: Mutex::new(None),
    };
    let language = locations::serp_language(&place.language);
    let outcomes: Vec<Result<Outcome, AppError>> = stream::iter(todo)
        .map(|row| {
            let ctx = KeywordCtx {
                pool,
                client,
                credentials,
                workspace_id,
                run,
                place: &place,
                host: &site.host,
                location_code,
                language,
                shared: &shared,
            };
            async move { check_keyword(&ctx, &row).await }
        })
        .buffer_unordered(PARALLEL)
        .collect()
        .await;
    for outcome in outcomes {
        outcome?;
    }
    finish(pool, workspace_id, run, total, &shared).await
}

async fn end(pool: &PgPool, run: &Run, status: &str, error: Option<&str>) -> Result<(), AppError> {
    repo::finish_run(pool, run.id, status, error).await
}

struct KeywordCtx<'a> {
    pool: &'a PgPool,
    client: &'a DataForSeoClient,
    credentials: &'a Credentials<'a>,
    workspace_id: Uuid,
    run: &'a Run,
    place: &'a Location,
    host: &'a str,
    location_code: u32,
    language: &'a str,
    shared: &'a Shared,
}

/// Asks about one keyword and stores the answer. Once the login is refused or
/// the balance is gone, the keywords still waiting are not asked.
async fn check_keyword(ctx: &KeywordCtx<'_>, row: &KeywordRow) -> Result<Outcome, AppError> {
    if ctx.shared.halt.load(Ordering::SeqCst) {
        return Ok(Outcome::Failed);
    }
    let query = SerpQuery {
        keyword: &row.keyword,
        location_code: ctx.location_code,
        language_code: ctx.language,
        device: &ctx.place.device,
    };
    match ctx.client.serp(ctx.credentials, &query).await {
        Ok(items) => {
            crate::usage::record_quietly(
                ctx.pool,
                ctx.workspace_id,
                USAGE_SOURCE,
                USAGE_KIND,
                1,
                Some(pricing::SERP_REQUEST_USD),
            )
            .await;
            let read = serp::read(&items, ctx.host);
            repo::save_check(
                ctx.pool,
                ctx.workspace_id,
                ctx.run.site_id,
                ctx.run.id,
                row,
                ctx.place,
                &read,
            )
            .await?;
            repo::refresh_progress(ctx.pool, ctx.run.id).await?;
            Ok(Outcome::Stored)
        }
        Err(err) => {
            tracing::warn!(site_id = %ctx.run.site_id, "a keyword could not be checked");
            if err.is_fatal() {
                ctx.shared.halt.store(true, Ordering::SeqCst);
                *ctx.shared.fatal.lock().expect("lock") = Some(err);
            }
            *ctx.shared.last.lock().expect("lock") = Some(err);
            repo::record_failure(ctx.pool, ctx.run.id).await?;
            Ok(Outcome::Failed)
        }
    }
}

/// Completed when at least one keyword was stored, failed when none was. A
/// finished run re-runs the checks and queues the next one.
async fn finish(
    pool: &PgPool,
    workspace_id: Uuid,
    run: &Run,
    total: i32,
    shared: &Shared,
) -> Result<(), AppError> {
    let fatal = *shared.fatal.lock().expect("lock");
    if let Some(err) = fatal {
        // The setting says what is wrong, until it is tested again.
        let message = err.to_string();
        if let Err(err) = crate::settings::repo::set_status(
            pool,
            workspace_id,
            crate::settings::DATAFORSEO,
            Some(&message),
            None,
        )
        .await
        {
            tracing::warn!(error = %err, "could not mark the DataForSEO setting");
        }
    }
    let stored = repo::stored_count(pool, run.id).await?;
    if stored > 0 {
        let missed = i64::from(total) - stored;
        let note = match fatal {
            Some(err) => Some(err.to_string()),
            None if missed > 0 => Some(format!(
                "{missed} of {total} keywords could not be checked."
            )),
            None => None,
        };
        repo::finish_run(pool, run.id, "completed", note.as_deref()).await?;
        crate::integrations::google::sync::refresh_checks(pool, workspace_id, run.site_id).await;
    } else {
        let last = *shared.last.lock().expect("lock");
        let reason = fatal
            .or(last)
            .map_or(STOPPED.to_string(), |err| err.to_string());
        repo::finish_run(pool, run.id, "failed", Some(&reason)).await?;
    }
    schedule_next(pool, workspace_id, run.site_id).await;
    Ok(())
}
