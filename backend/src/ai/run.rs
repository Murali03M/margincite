//! The `ai_run` job: asks every question of every engine, stores each answer
//! and finishes the run. An engine failing becomes an answer with an error;
//! only a database failure fails the job, and a retried job picks up where
//! the last one stopped.

use std::sync::Arc;

use async_trait::async_trait;
use futures::stream::{self, StreamExt};
use sqlx::PgPool;
use time::{Duration, OffsetDateTime};
use uuid::Uuid;

use super::answers::{self, NewAnswer};
use super::detect;
use super::engines::{Engine, EngineAnswer, EngineError, EngineFactory, Engines};
use super::repo::{self, Prompt, Run, Settings};
use crate::config::Config;
use crate::crawl::clean::strip_nul;
use crate::error::AppError;
use crate::jobs::queue::{self, Job};
use crate::jobs::worker::JobHandler;
use crate::sites::repo as sites;

pub const KIND: &str = "ai_run";
/// Requests in flight at once to one engine.
pub const PER_ENGINE_PARALLEL: usize = 2;
/// Stored answer text is cut at this many characters.
pub const MAX_ANSWER_CHARS: usize = 20_000;
const MAX_CITATIONS: usize = 100;
const MAX_TEXT_CHARS: usize = 2000;
const WEEK_DAYS: i64 = 7;

const ALL_FAILED: &str = "No engine could answer. Open the answers to see why.";
const STOPPED: &str = "The check stopped because of an internal error. Try again.";
const ENGINE_GONE: &str = "This engine is no longer set up. Add its key in Settings.";

pub struct AiRunJob {
    pool: PgPool,
    config: Arc<Config>,
    engines: Arc<EngineFactory>,
}

impl AiRunJob {
    pub fn new(pool: PgPool, config: Arc<Config>, engines: Arc<EngineFactory>) -> Self {
        Self {
            pool,
            config,
            engines,
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
impl JobHandler for AiRunJob {
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
        let engines = self
            .engines
            .for_workspace(&self.pool, &self.config, workspace_id)
            .await?;
        execute(&self.pool, &engines, workspace_id, &run).await?;
        Ok(())
    }

    async fn on_dead(&self, job: &Job, error: &str) {
        let Ok((workspace_id, site_id)) = site_ids(job) else {
            return;
        };
        tracing::error!(%site_id, %error, "ai run failed for good");
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

impl AiRunJob {
    /// Writes the new run's id into the job, so a retry resumes this run
    /// instead of meeting the one-active-run rule. When that cannot be done
    /// the run is failed, so it does not hold the site's place.
    async fn remember_run(&self, job: &Job, run: &Run) -> anyhow::Result<()> {
        if let Err(err) = queue::attach_run(&self.pool, job.id, run.id).await {
            let _ = repo::finish_run(&self.pool, run.id, "failed", Some(STOPPED)).await;
            return Err(err);
        }
        Ok(())
    }

    /// A weekly run: made when its job comes due, and only when there is
    /// something to ask and an engine to ask, and no run is under way.
    async fn start_scheduled(
        &self,
        workspace_id: Uuid,
        site_id: Uuid,
    ) -> Result<Option<Run>, AppError> {
        let prompts = repo::list_prompts(&self.pool, workspace_id, site_id).await?;
        let ids = self
            .engines
            .for_workspace(&self.pool, &self.config, workspace_id)
            .await?
            .ids();
        if ids.is_empty() {
            // Keys may be added later, so the weekly chain goes on.
            schedule_next(&self.pool, workspace_id, site_id).await;
            return Ok(None);
        }
        if prompts.is_empty() {
            return Ok(None);
        }
        let total = i32::try_from(prompts.len() * ids.len()).unwrap_or(i32::MAX);
        match repo::create_run(&self.pool, workspace_id, site_id, "scheduled", &ids, total).await {
            Ok(run) => Ok(Some(run)),
            Err(AppError::Conflict(_)) | Err(AppError::NotFound) => Ok(None),
            Err(err) => Err(err),
        }
    }
}

/// Queues the next weekly run, unless one is already waiting.
pub async fn schedule_next(pool: &PgPool, workspace_id: Uuid, site_id: Uuid) {
    let run_at = OffsetDateTime::now_utc() + Duration::days(WEEK_DAYS);
    if let Err(err) =
        queue::enqueue_site_job_unless_queued(pool, KIND, workspace_id, site_id, run_at).await
    {
        tracing::error!(%site_id, error = %err, "could not schedule the next AI visibility run");
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

/// Everything the answers of one run need to know.
struct Context {
    pool: PgPool,
    workspace_id: Uuid,
    run_id: Uuid,
    site_id: Uuid,
    site_host: String,
    settings: Settings,
}

pub async fn execute(
    pool: &PgPool,
    engines: &Engines,
    workspace_id: Uuid,
    run: &Run,
) -> Result<(), AppError> {
    if !matches!(run.status.as_str(), "queued" | "running") {
        return Ok(());
    }
    let site = sites::get(pool, workspace_id, run.site_id).await?;
    let prompts = repo::list_prompts(pool, workspace_id, run.site_id).await?;
    if prompts.is_empty() || run.engines.is_empty() {
        let reason = "There was nothing to ask: add questions and set up an engine.";
        repo::finish_run(pool, run.id, "failed", Some(reason)).await?;
        return Ok(());
    }
    let total = i32::try_from(prompts.len() * run.engines.len()).unwrap_or(i32::MAX);
    if !repo::begin_run(pool, run.id, total).await? {
        return Ok(());
    }
    let ctx = Arc::new(Context {
        pool: pool.clone(),
        workspace_id,
        run_id: run.id,
        site_id: run.site_id,
        site_host: site.host,
        settings: repo::get_settings(pool, workspace_id, run.site_id).await?,
    });
    let done = repo::stored_pairs(pool, run.id).await?;
    let work: Vec<_> = run
        .engines
        .iter()
        .map(|id| {
            let todo: Vec<Prompt> = prompts
                .iter()
                .filter(|p| !done.contains(&(p.id, id.clone())))
                .cloned()
                .collect();
            ask_engine(ctx.clone(), id.clone(), engines.get(id).cloned(), todo)
        })
        .collect();
    for result in futures::future::join_all(work).await {
        result?;
    }
    finish(pool, workspace_id, run.id, run.site_id).await
}

/// Asks one engine every question it has not answered yet, two at a time.
async fn ask_engine(
    ctx: Arc<Context>,
    engine_id: String,
    engine: Option<Arc<dyn Engine>>,
    prompts: Vec<Prompt>,
) -> Result<(), AppError> {
    let results: Vec<Result<(), AppError>> = stream::iter(prompts)
        .map(|prompt| {
            let (ctx, engine, engine_id) = (ctx.clone(), engine.clone(), engine_id.clone());
            async move {
                let outcome = match &engine {
                    Some(engine) => engine.ask(&prompt.text).await,
                    None => Err(EngineError::Transport),
                };
                if engine.is_some() {
                    crate::usage::record_quietly(
                        &ctx.pool,
                        ctx.workspace_id,
                        &engine_id,
                        "answer",
                        1,
                        None,
                    )
                    .await;
                }
                let model = engine.as_ref().map_or("", |engine| engine.model());
                let stored =
                    answer_row(&ctx, &prompt, &engine_id, model, outcome, engine.is_none());
                answers::store(&ctx.pool, &stored).await?;
                repo::refresh_progress(&ctx.pool, ctx.run_id).await
            }
        })
        .buffer_unordered(PER_ENGINE_PARALLEL)
        .collect()
        .await;
    results.into_iter().collect()
}

fn cut(text: &str, limit: usize) -> String {
    strip_nul(text).chars().take(limit).collect()
}

fn answer_row(
    ctx: &Context,
    prompt: &Prompt,
    engine_id: &str,
    model: &str,
    outcome: Result<EngineAnswer, EngineError>,
    engine_gone: bool,
) -> NewAnswer {
    let base = NewAnswer {
        workspace_id: ctx.workspace_id,
        run_id: ctx.run_id,
        site_id: ctx.site_id,
        prompt_id: prompt.id,
        prompt_text: prompt.text.clone(),
        engine: engine_id.to_string(),
        model: model.to_string(),
        answer_text: String::new(),
        citations: Vec::new(),
        mentioned: false,
        cited: false,
        citation_rank: None,
        competitor_mentions: Vec::new(),
        notes: Vec::new(),
        error: None,
        input_tokens: 0,
        output_tokens: 0,
        searches: 0,
    };
    match outcome {
        Err(_) if engine_gone => NewAnswer {
            error: Some(ENGINE_GONE.into()),
            ..base
        },
        Err(err) => NewAnswer {
            error: Some(err.to_string()),
            ..base
        },
        Ok(answer) => judged(ctx, base, answer),
    }
}

/// An answer with the verdicts on it: did it mention the site, cite it, and
/// name competitors.
fn judged(ctx: &Context, base: NewAnswer, answer: EngineAnswer) -> NewAnswer {
    let settings = &ctx.settings;
    let text = cut(&answer.text, MAX_ANSWER_CHARS);
    let citations: Vec<_> = answer
        .citations
        .into_iter()
        .take(MAX_CITATIONS)
        .map(|c| super::engines::Citation {
            url: cut(&c.url, MAX_TEXT_CHARS),
            title: c.title.map(|t| cut(&t, MAX_TEXT_CHARS)),
            domain: cut(&c.domain, MAX_TEXT_CHARS),
        })
        .collect();
    let (cited, citation_rank) = detect::cited(&citations, &ctx.site_host);
    NewAnswer {
        mentioned: detect::mentioned(
            &text,
            &settings.brand_name,
            &settings.aliases,
            &ctx.site_host,
        ),
        cited,
        citation_rank,
        competitor_mentions: detect::competitors(&text, &citations, &settings.competitors),
        notes: answer
            .notes
            .iter()
            .map(|n| cut(n, MAX_TEXT_CHARS))
            .collect(),
        input_tokens: answer.input_tokens,
        output_tokens: answer.output_tokens,
        searches: answer.searches,
        answer_text: text,
        citations,
        ..base
    }
}

/// Completed when at least one answer has no error; failed when every
/// answer failed. A finished run re-runs the checks and queues the next one.
async fn finish(
    pool: &PgPool,
    workspace_id: Uuid,
    run_id: Uuid,
    site_id: Uuid,
) -> Result<(), AppError> {
    let good: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM ai_answers WHERE run_id = $1 AND error IS NULL")
            .bind(run_id)
            .fetch_one(pool)
            .await
            .map_err(|err| AppError::Internal(err.into()))?;
    if good > 0 {
        repo::finish_run(pool, run_id, "completed", None).await?;
        crate::integrations::google::sync::refresh_checks(pool, workspace_id, site_id).await;
    } else {
        repo::finish_run(pool, run_id, "failed", Some(ALL_FAILED)).await?;
    }
    schedule_next(pool, workspace_id, site_id).await;
    Ok(())
}
