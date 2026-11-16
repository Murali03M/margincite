//! The `bing_sync` job: pulls a site's backlinks from Bing Webmaster Tools
//! and swaps them in. 429s, 5xx and anything unreadable fail the job so the
//! queue retries; a refused key marks the connection `needs_reconnecting`.

use std::collections::HashSet;
use std::sync::Arc;

use async_trait::async_trait;
use sqlx::PgPool;
use time::{Duration, OffsetDateTime};
use uuid::Uuid;

use super::client::{BingClient, BingError, LinkCount};
use super::repo::{self, NewSource, NewTarget, SyncData};
use crate::config::Config;
use crate::crawl::clean::strip_nul;
use crate::jobs::queue::{self, Job};
use crate::jobs::worker::JobHandler;

pub const KIND: &str = "bing_sync";
/// Bing's link counts come in pages; this many pages are read at most.
pub const MAX_COUNT_PAGES: u32 = 20;
/// Links are listed for this many of the most linked pages, first page only.
pub const SOURCE_TARGETS: usize = 10;
/// Rows kept per page of links, whatever Bing sends.
const MAX_SOURCES_PER_TARGET: usize = 100;
const MAX_TEXT_CHARS: usize = 2000;
const WEEK_DAYS: i64 = 7;

pub const NEEDS_RECONNECTING: &str =
    "Bing no longer accepts this API key. Connect Bing Webmaster Tools again.";
const UNREADABLE_KEY: &str = "The saved Bing API key could not be read. Connect again.";
const SYNC_FAILED: &str = "The last Bing sync failed. It will be tried again, or press Sync now.";

pub struct BingSyncJob {
    pool: PgPool,
    config: Arc<Config>,
    client: Arc<BingClient>,
}

impl BingSyncJob {
    pub fn new(pool: PgPool, config: Arc<Config>, client: Arc<BingClient>) -> Self {
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
impl JobHandler for BingSyncJob {
    fn kind(&self) -> &'static str {
        KIND
    }

    async fn run(&self, job: &Job) -> anyhow::Result<()> {
        let (workspace_id, site_id) = site_ids(job)?;
        sync_site(
            &self.pool,
            &self.config,
            &self.client,
            workspace_id,
            site_id,
        )
        .await
    }

    async fn on_dead(&self, job: &Job, error: &str) {
        let Ok((workspace_id, site_id)) = site_ids(job) else {
            return;
        };
        tracing::error!(%site_id, %error, "bing sync failed for good");
        // A changed format has its own sentence, and it should stay.
        let message = if error == BingError::Unrecognised.to_string() {
            error
        } else {
            SYNC_FAILED
        };
        if let Err(err) = repo::record_error(&self.pool, workspace_id, site_id, message).await {
            tracing::error!(%site_id, error = %err, "could not record the failed sync");
        }
        schedule_next(&self.pool, workspace_id, site_id).await;
    }
}

/// Queues a sync for right now, unless one is waiting (then that one is made
/// due now).
pub async fn enqueue_now(pool: &PgPool, workspace_id: Uuid, site_id: Uuid) -> anyhow::Result<bool> {
    queue::enqueue_site_job_unless_queued(
        pool,
        KIND,
        workspace_id,
        site_id,
        OffsetDateTime::now_utc(),
    )
    .await
}

async fn schedule_next(pool: &PgPool, workspace_id: Uuid, site_id: Uuid) {
    let run_at = OffsetDateTime::now_utc() + Duration::days(WEEK_DAYS);
    if let Err(err) =
        queue::enqueue_site_job_unless_queued(pool, KIND, workspace_id, site_id, run_at).await
    {
        tracing::error!(%site_id, error = %err, "could not schedule the next Bing sync");
    }
}

/// What a failed Bing call means for the sync.
enum Stop {
    /// The key is dead; say so and finish the job.
    Reconnect,
    /// Try again later.
    Retry(anyhow::Error),
    /// Bing's answer changed shape. Say so on the connection, keep the stored
    /// rows, and try again later.
    Unrecognised,
}

impl From<BingError> for Stop {
    fn from(err: BingError) -> Self {
        match err {
            BingError::Rejected => Stop::Reconnect,
            BingError::Transient(message) => Stop::Retry(anyhow::anyhow!(message)),
            BingError::Unrecognised => Stop::Unrecognised,
        }
    }
}

pub async fn sync_site(
    pool: &PgPool,
    config: &Config,
    client: &BingClient,
    workspace_id: Uuid,
    site_id: Uuid,
) -> anyhow::Result<()> {
    let Some(secrets) = config.secret_key.as_ref() else {
        // The key may be set later, so the weekly chain goes on.
        schedule_next(pool, workspace_id, site_id).await;
        return Ok(());
    };
    let Some(connection) = repo::get_connection(pool, workspace_id, site_id).await? else {
        return Ok(());
    };
    if connection.status != "connected" {
        return Ok(());
    }
    let Ok(api_key) = secrets.open(&connection.api_key_sealed) else {
        repo::mark_needs_reconnecting(pool, workspace_id, site_id, UNREADABLE_KEY).await?;
        return Ok(());
    };
    let data = match fetch(client, &api_key, &connection.site_url).await {
        Ok(data) => data,
        Err(Stop::Reconnect) => {
            repo::mark_needs_reconnecting(pool, workspace_id, site_id, NEEDS_RECONNECTING).await?;
            return Ok(());
        }
        Err(Stop::Retry(err)) => return Err(err),
        Err(Stop::Unrecognised) => {
            let message = BingError::Unrecognised.to_string();
            repo::record_error(pool, workspace_id, site_id, &message).await?;
            anyhow::bail!(message);
        }
    };
    if !repo::replace_data(pool, workspace_id, site_id, &data).await? {
        return Ok(());
    }
    schedule_next(pool, workspace_id, site_id).await;
    Ok(())
}

async fn fetch(client: &BingClient, api_key: &str, site_url: &str) -> Result<SyncData, Stop> {
    let targets = all_targets(client, api_key, site_url).await?;
    let mut sources = Vec::new();
    for target in targets.iter().take(SOURCE_TARGETS) {
        let links = client.url_links(api_key, site_url, &target.url, 0).await?;
        sources.extend(
            links
                .into_iter()
                .take(MAX_SOURCES_PER_TARGET)
                .map(|link| NewSource {
                    target_url: target.url.clone(),
                    source_url: clean(&link.source_url),
                    anchor_text: clean(&link.anchor_text),
                }),
        );
    }
    Ok(SyncData { targets, sources })
}

/// Link counts across pages, most linked first. A page that is empty, or
/// that brings no address not seen before, ends the walk, and so does the
/// page limit. An address seen twice counts once.
async fn all_targets(
    client: &BingClient,
    api_key: &str,
    site_url: &str,
) -> Result<Vec<NewTarget>, Stop> {
    let mut seen: HashSet<String> = HashSet::new();
    let mut targets: Vec<NewTarget> = Vec::new();
    for page in 0..MAX_COUNT_PAGES {
        let rows: Vec<LinkCount> = client.link_counts(api_key, site_url, page).await?;
        let before = targets.len();
        for row in rows {
            let url = clean(&row.url);
            if seen.insert(url.clone()) {
                targets.push(NewTarget {
                    url,
                    count: row.count,
                });
            }
        }
        if targets.len() == before {
            break;
        }
    }
    targets.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.url.cmp(&b.url)));
    Ok(targets)
}

/// Text from outside, safe to store: no NUL bytes, a sane length.
fn clean(text: &str) -> String {
    strip_nul(text).chars().take(MAX_TEXT_CHARS).collect()
}
