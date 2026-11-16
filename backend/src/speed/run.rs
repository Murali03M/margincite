//! The `speed_run` job: tests a site's home page and a few key pages with
//! PageSpeed Insights, one at a time, and stores each result. A page that
//! cannot be tested keeps its last good numbers and shows why; the rest of
//! the run goes on.

use std::sync::Arc;

use async_trait::async_trait;
use sqlx::PgPool;
use time::OffsetDateTime;
use url::Url;
use uuid::Uuid;

use super::repo::{self, Outcome};
use crate::config::Config;
use crate::error::AppError;
use crate::integrations::pagespeed::PageSpeedClient;
use crate::jobs::queue::{self, Job};
use crate::jobs::worker::JobHandler;
use crate::sites::repo::{self as sites, Site};
use crate::sites::scope::SiteScope;

pub const KIND: &str = "speed_run";
const USAGE_SOURCE: &str = "pagespeed";
const USAGE_KIND: &str = "page_test";

pub struct SpeedRunJob {
    pool: PgPool,
    config: Arc<Config>,
    client: Arc<PageSpeedClient>,
}

impl SpeedRunJob {
    pub fn new(pool: PgPool, config: Arc<Config>, client: Arc<PageSpeedClient>) -> Self {
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
impl JobHandler for SpeedRunJob {
    fn kind(&self) -> &'static str {
        KIND
    }

    async fn run(&self, job: &Job) -> anyhow::Result<()> {
        let (workspace_id, site_id) = site_ids(job)?;
        run_site(
            &self.pool,
            &self.config,
            &self.client,
            workspace_id,
            site_id,
        )
        .await?;
        Ok(())
    }
}

/// Queues a speed run for right now, unless one is waiting (then that one is
/// made due now).
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

/// After a completed crawl the pages may have changed, so they are tested again.
pub async fn enqueue_after_crawl(pool: &PgPool, workspace_id: Uuid, site_id: Uuid) {
    if let Err(err) = enqueue_now(pool, workspace_id, site_id).await {
        tracing::error!(%site_id, error = %err, "could not queue a page speed run");
    }
}

/// Whether `address` is a page of this site. Only such pages are ever sent
/// to PageSpeed.
fn belongs_to(site: &Site, address: &str) -> bool {
    let (Ok(root), Ok(url)) = (Url::parse(&site.root_url), Url::parse(address)) else {
        return false;
    };
    let scope = SiteScope {
        root,
        host: site.host.clone(),
        path_prefix: site.path_prefix.clone(),
    };
    scope.contains(&url)
}

/// Tests the site's pages, stores every result together with the removal of
/// pages no longer tested, and re-runs the checks on the latest crawl.
pub async fn run_site(
    pool: &PgPool,
    config: &Config,
    client: &PageSpeedClient,
    workspace_id: Uuid,
    site_id: Uuid,
) -> Result<(), AppError> {
    let site = match sites::get(pool, workspace_id, site_id).await {
        Ok(site) => site,
        Err(AppError::NotFound) => return Ok(()),
        Err(err) => return Err(err),
    };
    let key = crate::settings::resolve::pagespeed_key(pool, config, workspace_id).await?;
    let pages = repo::pages_to_test(pool, workspace_id, site_id, &site.root_url).await?;
    let mut results = Vec::new();
    for page in pages.iter().filter(|page| belongs_to(&site, page)) {
        let outcome = client.measure(page, key.as_deref()).await;
        crate::usage::record_quietly(pool, workspace_id, USAGE_SOURCE, USAGE_KIND, 1, Some(0.0))
            .await;
        results.push((
            page.clone(),
            match outcome {
                Ok(found) => Outcome::Measured(found),
                Err(err) => {
                    tracing::warn!(%site_id, "a page could not be tested by PageSpeed");
                    Outcome::Failed(err.to_string())
                }
            },
        ));
    }
    repo::save_run(pool, workspace_id, site_id, &results).await?;
    crate::integrations::google::sync::refresh_checks(pool, workspace_id, site_id).await;
    Ok(())
}
