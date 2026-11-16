//! The `keyword_volumes` job: one request to DataForSEO refreshes the search
//! volume, competition and cost per click of all of a site's keywords. It is
//! started only by a person, and asks nothing without the workspace's own
//! DataForSEO login.
//!
//! A rate limit (the Google Ads endpoints allow 12 requests a minute per
//! account), a timeout or a provider error fails the job so the queue tries
//! again; a login that is refused or a balance that has run out ends it and
//! marks the setting, since retrying cannot help.
//!
//! The paid answer is saved on the job as soon as it arrives, so a retry after
//! a failure to store it stores that answer instead of paying again. Every end
//! without stored volumes is remembered as a fixed sentence (`volume_state`).

use std::sync::Arc;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::PgPool;
use time::OffsetDateTime;
use uuid::Uuid;

use super::repo;
use super::volume_state;
use crate::config::Config;
use crate::integrations::dataforseo::keywords::{fits, KeywordMetrics, MAX_VOLUME_KEYWORDS};
use crate::integrations::dataforseo::{
    locations, pricing, Credentials, DataForSeoClient, DataForSeoError,
};
use crate::jobs::queue::{self, Job};
use crate::jobs::worker::JobHandler;
use crate::settings::resolve;
use crate::sites::repo as sites;

pub const KIND: &str = "keyword_volumes";
pub const USAGE_SOURCE: &str = "dataforseo";
pub const USAGE_KIND: &str = "keyword_volume";

const NO_LOGIN: &str =
    "The DataForSEO login is no longer set up. Add it in Settings, then try again.";
const NO_LOCATION: &str =
    "This site's country cannot be searched. Change it in the site's settings.";
const NO_KEYWORDS: &str = "None of this site's keywords can be searched. A keyword can be at most 80 characters and 10 words.";
pub const STOPPED: &str = "The refresh stopped because of an internal error. Try again.";

pub struct KeywordVolumesJob {
    pool: PgPool,
    config: Arc<Config>,
    client: Arc<DataForSeoClient>,
}

impl KeywordVolumesJob {
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

#[async_trait]
impl JobHandler for KeywordVolumesJob {
    fn kind(&self) -> &'static str {
        KIND
    }

    async fn run(&self, job: &Job) -> anyhow::Result<()> {
        let (workspace_id, site_id) = site_ids(job)?;
        refresh(
            &self.pool,
            &self.config,
            &self.client,
            workspace_id,
            site_id,
            job,
        )
        .await
    }

    async fn on_dead(&self, job: &Job, error: &str) {
        let Ok((workspace_id, site_id)) = site_ids(job) else {
            return;
        };
        tracing::error!(%site_id, %error, "search volume refresh failed for good");
        // A provider's own sentence, recorded while the job was retried, is
        // more useful than this one, so it stays.
        if volume_state::status(&self.pool, workspace_id, site_id)
            .await
            .is_ok_and(|state| state.error.is_none())
        {
            volume_state::record_failure(&self.pool, workspace_id, site_id, STOPPED).await;
        }
    }
}

/// Queues a refresh for right now, unless one is already waiting.
pub async fn enqueue(pool: &PgPool, workspace_id: Uuid, site_id: Uuid) -> anyhow::Result<bool> {
    queue::enqueue_site_job_unless_queued(
        pool,
        KIND,
        workspace_id,
        site_id,
        OffsetDateTime::now_utc(),
    )
    .await
}

/// Says on the DataForSEO setting what is wrong, until it is tested again.
pub async fn mark_setting(pool: &PgPool, workspace_id: Uuid, err: DataForSeoError) {
    if let Err(failure) = crate::settings::repo::set_status(
        pool,
        workspace_id,
        crate::settings::DATAFORSEO,
        Some(&err.to_string()),
        None,
    )
    .await
    {
        tracing::warn!(error = %failure, "could not mark the DataForSEO setting");
    }
}

/// The keywords asked about and what DataForSEO said, as saved on the job.
#[derive(Serialize, Deserialize)]
struct Answer {
    asked: Vec<String>,
    found: Vec<KeywordMetrics>,
}

fn saved_answer(job: &Job) -> Option<Answer> {
    serde_json::from_value(job.payload.get("answer")?.clone()).ok()
}

/// Ends the job without volumes and says why on the site.
async fn give_up(
    pool: &PgPool,
    workspace_id: Uuid,
    site_id: Uuid,
    message: &str,
) -> anyhow::Result<()> {
    volume_state::record_failure(pool, workspace_id, site_id, message).await;
    Ok(())
}

async fn refresh(
    pool: &PgPool,
    config: &Config,
    client: &DataForSeoClient,
    workspace_id: Uuid,
    site_id: Uuid,
    job: &Job,
) -> anyhow::Result<()> {
    let place = match sites::get_settings(pool, workspace_id, site_id).await {
        Ok(settings) => settings,
        Err(crate::error::AppError::NotFound) => return Ok(()),
        Err(err) => return Err(err.into()),
    };
    let Some((login, password)) = resolve::dataforseo(pool, config, workspace_id).await? else {
        return give_up(pool, workspace_id, site_id, NO_LOGIN).await;
    };
    let Some(location_code) = locations::location_code(&place.country) else {
        return give_up(pool, workspace_id, site_id, NO_LOCATION).await;
    };
    let asked: Vec<String> = repo::list(pool, workspace_id, site_id)
        .await?
        .into_iter()
        .map(|row| row.keyword)
        .filter(|keyword| fits(keyword))
        .take(MAX_VOLUME_KEYWORDS)
        .collect();
    if asked.is_empty() {
        return give_up(pool, workspace_id, site_id, NO_KEYWORDS).await;
    }
    let answer = match saved_answer(job) {
        Some(answer) => Ok(answer),
        None => {
            let credentials = Credentials {
                login: &login,
                password: &password,
            };
            let language = locations::ads_language(&place.language);
            ask(
                pool,
                client,
                (&credentials, workspace_id),
                (location_code, language),
                job,
                &asked,
            )
            .await
        }
    };
    finish(pool, workspace_id, site_id, answer).await
}

/// One paid request. A good answer is counted and saved on the job at once.
async fn ask(
    pool: &PgPool,
    client: &DataForSeoClient,
    (credentials, workspace_id): (&Credentials<'_>, Uuid),
    (location_code, language): (u32, &str),
    job: &Job,
    asked: &[String],
) -> Result<Answer, DataForSeoError> {
    let found = client
        .search_volume(credentials, asked, location_code, language)
        .await?;
    crate::usage::record_quietly(
        pool,
        workspace_id,
        USAGE_SOURCE,
        USAGE_KIND,
        1,
        Some(pricing::ADS_REQUEST_USD),
    )
    .await;
    let answer = Answer {
        asked: asked.to_vec(),
        found,
    };
    if let Ok(fields) = serde_json::to_value(&answer).map(|value| json!({ "answer": value })) {
        if let Err(err) = queue::attach(pool, job.id, &fields).await {
            tracing::warn!(error = %err, "could not save a search volume answer on its job");
        }
    }
    Ok(answer)
}

/// Stores a good answer, or ends the job according to what went wrong.
async fn finish(
    pool: &PgPool,
    workspace_id: Uuid,
    site_id: Uuid,
    answer: Result<Answer, DataForSeoError>,
) -> anyhow::Result<()> {
    match answer {
        Ok(answer) => {
            repo::save_metrics(pool, workspace_id, site_id, &answer.asked, &answer.found).await?;
            volume_state::clear_failure(pool, workspace_id, site_id).await;
            Ok(())
        }
        Err(err) if err.is_fatal() => {
            mark_setting(pool, workspace_id, err).await;
            give_up(pool, workspace_id, site_id, &err.to_string()).await
        }
        Err(err) if err.is_temporary() => {
            volume_state::record_failure(pool, workspace_id, site_id, &err.to_string()).await;
            Err(anyhow::anyhow!(err.to_string()))
        }
        Err(err) => give_up(pool, workspace_id, site_id, &err.to_string()).await,
    }
}
