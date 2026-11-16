use std::collections::{HashSet, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use serde_json::{json, Value};
use url::Url;
use uuid::Uuid;

use super::client::CrawlClient;
use super::extract::{self, PageFacts};
use super::render::Renderer;
use super::repo::{self, Finish, NewPage};
use super::robots::{self, Robots};
use super::sitemap;
use super::urls;
use crate::error::AppError;
use crate::jobs::queue::{self, Job};
use crate::jobs::worker::JobHandler;
use crate::sites::scope::SiteScope;

#[derive(Debug, Clone, Copy)]
pub struct CrawlSettings {
    pub concurrency: usize,
    pub delay: Duration,
    /// A crawl stops here and keeps the pages it has.
    pub time_limit: Duration,
}

impl Default for CrawlSettings {
    fn default() -> Self {
        Self {
            concurrency: 2,
            delay: Duration::from_millis(500),
            time_limit: Duration::from_secs(30 * 60),
        }
    }
}

#[derive(Clone)]
pub struct CrawlDeps {
    pub pool: sqlx::PgPool,
    pub client: CrawlClient,
    pub renderer: Arc<dyn Renderer>,
    pub settings: CrawlSettings,
    /// When set, each crawl announces the contact address of its own
    /// workspace (saved in Settings, else the environment) instead of the
    /// one `client` was made with.
    pub config: Option<Arc<crate::config::Config>>,
}

impl CrawlDeps {
    /// These dependencies with the workspace's contact on the client.
    /// `Ok(None)` when no contact is set anywhere.
    async fn for_workspace(&self, workspace_id: Uuid) -> anyhow::Result<Option<CrawlDeps>> {
        let Some(config) = &self.config else {
            return Ok(Some(self.clone()));
        };
        let contact =
            crate::settings::resolve::crawler_contact(&self.pool, config, workspace_id).await?;
        Ok(contact.map(|contact| CrawlDeps {
            client: self.client.with_contact(&contact),
            ..self.clone()
        }))
    }
}

struct Item {
    url: Url,
    depth: i32,
    in_sitemap: bool,
}

/// Addresses held per page of the cap. A page may link to thousands of
/// addresses; only this many are ever remembered.
const FRONTIER_PER_PAGE: usize = 20;

/// The addresses still to fetch. Every address ever accepted, crawled or
/// queued, is remembered in `seen`, and `seen` never grows past
/// `FRONTIER_PER_PAGE` times the page cap, which also bounds the queue.
struct Frontier {
    queue: VecDeque<Item>,
    seen: HashSet<String>,
    /// Queued addresses that a sitemap also lists.
    from_sitemap: HashSet<String>,
    limit: usize,
}

impl Frontier {
    fn new(page_cap: usize) -> Self {
        Self {
            queue: VecDeque::new(),
            seen: HashSet::new(),
            from_sitemap: HashSet::new(),
            limit: page_cap.saturating_mul(FRONTIER_PER_PAGE),
        }
    }

    /// Notes an address that was already crawled.
    fn mark_seen(&mut self, url: &Url) {
        if self.seen.len() < self.limit {
            self.seen.insert(urls::normalize(url).to_string());
        }
    }

    fn push(&mut self, url: Url, depth: i32, in_sitemap: bool) {
        let url = urls::normalize(&url);
        let key = url.as_str().to_string();
        if !self.seen.contains(&key) {
            if self.seen.len() >= self.limit {
                return;
            }
            self.seen.insert(key.clone());
            self.queue.push_back(Item {
                url,
                depth,
                in_sitemap: false,
            });
        }
        if in_sitemap {
            self.from_sitemap.insert(key);
        }
    }

    fn pop_n(&mut self, n: usize) -> Vec<Item> {
        let mut batch = Vec::new();
        while batch.len() < n {
            let Some(mut item) = self.queue.pop_front() else {
                break;
            };
            item.in_sitemap = self.from_sitemap.remove(item.url.as_str());
            batch.push(item);
        }
        batch
    }
}

/// What a person sees when a crawl broke before it could finish.
const CRAWL_FAILED: &str = "The crawl stopped because of a problem on our side. Try again.";

/// Runs one crawl. When it returns an error the crawl is marked `failed`, so
/// it never stays `running`; the job may still be retried and reuses the row.
pub async fn run_crawl(deps: &CrawlDeps, workspace_id: Uuid, crawl_id: Uuid) -> anyhow::Result<()> {
    let own = match deps.for_workspace(workspace_id).await {
        Ok(Some(own)) => own,
        Ok(None) => {
            let message = crate::settings::routes::NO_CONTACT;
            repo::fail_crawl(&deps.pool, workspace_id, crawl_id, message).await?;
            return Ok(());
        }
        Err(err) => return Err(err),
    };
    let deps = &own;
    let result = execute(deps, workspace_id, crawl_id).await;
    if let Err(err) = &result {
        tracing::error!(%crawl_id, error = %err, "crawl failed");
        if let Err(mark) = repo::fail_crawl(&deps.pool, workspace_id, crawl_id, CRAWL_FAILED).await
        {
            tracing::error!(%crawl_id, error = %mark, "could not mark the crawl as failed");
        }
    }
    result
}

async fn execute(deps: &CrawlDeps, workspace_id: Uuid, crawl_id: Uuid) -> anyhow::Result<()> {
    let deadline = Instant::now() + deps.settings.time_limit;
    let Some(site) = load_site(deps, workspace_id, crawl_id).await? else {
        return Ok(());
    };
    repo::mark_running(&deps.pool, workspace_id, crawl_id).await?;
    let scope = scope_from_site(&site)?;
    let (outcome, sitemap_urls) = discover(deps, workspace_id, crawl_id, &scope).await?;
    let early = if outcome.unreachable {
        let error = Some("The site could not be reached.");
        Some(done("failed", "site_unreachable", 0, 0, error))
    } else if !outcome.rules.allows(&scope.root) {
        Some(done("completed", "robots_blocked", 0, 0, None))
    } else {
        None
    };
    if let Some(early) = early {
        return finish_now(deps, workspace_id, crawl_id, &early).await;
    }
    let mut frontier = seed_frontier(
        deps,
        workspace_id,
        crawl_id,
        &scope,
        &sitemap_urls,
        site.page_cap,
    )
    .await?;
    let delay = polite_delay(deps.settings, &outcome.rules);
    let walk = Walk {
        deps,
        workspace_id,
        crawl_id,
        scope: &scope,
        rules: &outcome.rules,
        deadline,
    };
    let (crawled, failed, reason) = crawl_pages(&walk, &mut frontier, site.page_cap, delay).await?;
    let unreachable = reason == "site_unreachable";
    let status = if unreachable { "failed" } else { "completed" };
    let error = unreachable.then_some("The site could not be reached.");
    let finish = done(status, reason, crawled, failed, error);
    finish_now(deps, workspace_id, crawl_id, &finish).await
}

/// The crawl's site, or `None` when the crawl or its site no longer exists.
async fn load_site(
    deps: &CrawlDeps,
    workspace_id: Uuid,
    crawl_id: Uuid,
) -> anyhow::Result<Option<crate::sites::repo::Site>> {
    let crawl = match repo::get_crawl(&deps.pool, workspace_id, crawl_id).await {
        Ok(crawl) => crawl,
        Err(AppError::NotFound) => return Ok(None),
        Err(err) => return Err(err.into()),
    };
    match crate::sites::repo::get(&deps.pool, workspace_id, crawl.site_id).await {
        Ok(site) => Ok(Some(site)),
        Err(AppError::NotFound) => Ok(None),
        Err(err) => Err(err.into()),
    }
}

/// Reads robots.txt, llms.txt and the sitemaps, and records what was found.
async fn discover(
    deps: &CrawlDeps,
    workspace_id: Uuid,
    crawl_id: Uuid,
    scope: &SiteScope,
) -> anyhow::Result<(robots::RobotsOutcome, Vec<Url>)> {
    let outcome = robots::load_robots(&deps.client, scope).await;
    let llms = llms_exists(&deps.client, scope).await;
    let map = sitemap::load_sitemap_urls(&deps.client, scope, &outcome.rules).await;
    let raw = outcome.raw.as_deref();
    repo::note_discovery(&deps.pool, workspace_id, crawl_id, raw, map.found, llms).await?;
    Ok((outcome, map.urls))
}

struct Walk<'a> {
    deps: &'a CrawlDeps,
    workspace_id: Uuid,
    crawl_id: Uuid,
    scope: &'a SiteScope,
    rules: &'a Robots,
    deadline: Instant,
}

fn done(
    status: &str,
    stop_reason: &str,
    pages_crawled: i32,
    pages_failed: i32,
    error: Option<&str>,
) -> Finish {
    Finish {
        status: status.to_string(),
        stop_reason: stop_reason.to_string(),
        pages_crawled,
        pages_failed,
        error: error.map(str::to_string),
    }
}

fn scope_from_site(site: &crate::sites::repo::Site) -> anyhow::Result<SiteScope> {
    Ok(SiteScope {
        root: Url::parse(&site.root_url)?,
        host: site.host.clone(),
        path_prefix: site.path_prefix.clone(),
    })
}

async fn seed_frontier(
    deps: &CrawlDeps,
    workspace_id: Uuid,
    crawl_id: Uuid,
    scope: &SiteScope,
    sitemap_urls: &[Url],
    page_cap: i32,
) -> Result<Frontier, AppError> {
    let mut frontier = Frontier::new(page_cap.max(0) as usize);
    for url in repo::crawled_urls(&deps.pool, workspace_id, crawl_id).await? {
        if let Ok(url) = Url::parse(&url) {
            frontier.mark_seen(&url);
        }
    }
    frontier.push(scope.root.clone(), 0, false);
    for url in sitemap_urls {
        frontier.push(url.clone(), 0, true);
    }
    for (href, depth, in_sitemap) in repo::stored_links(&deps.pool, workspace_id, crawl_id).await? {
        if let Ok(url) = Url::parse(&href) {
            if scope.contains(&url) {
                frontier.push(url, depth, in_sitemap);
            }
        }
    }
    Ok(frontier)
}

async fn crawl_pages(
    walk: &Walk<'_>,
    frontier: &mut Frontier,
    page_cap: i32,
    delay: Duration,
) -> anyhow::Result<(i32, i32, &'static str)> {
    let mut crawled = repo::crawled_urls(&walk.deps.pool, walk.workspace_id, walk.crawl_id)
        .await?
        .len() as i32;
    let mut failed = 0;
    let mut first_failed_hard = crawled == 0;
    while crawled < page_cap {
        if !frontier.queue.is_empty() && Instant::now() >= walk.deadline {
            return Ok((crawled, failed, "time_limit"));
        }
        let room = (page_cap - crawled) as usize;
        let batch = frontier.pop_n(room.min(walk.deps.settings.concurrency.max(1)));
        if batch.is_empty() {
            return Ok((crawled, failed, "finished"));
        }
        let pages = fetch_batch(walk.deps, batch).await;
        for page in pages {
            let http_failed = page.row.status_code.unwrap_or(0) >= 500;
            if page.fetch_error.is_some() || http_failed {
                failed += 1;
            }
            if !page.hard_failure {
                first_failed_hard = false;
            }
            for (link, depth) in &page.links {
                if walk.scope.contains(link) && walk.rules.allows(link) {
                    frontier.push(link.clone(), *depth, false);
                }
            }
            repo::upsert_page(&walk.deps.pool, walk.workspace_id, walk.crawl_id, &page.row).await?;
            crawled += 1;
        }
        if first_failed_hard {
            return Ok((crawled, failed, "site_unreachable"));
        }
        if delay > Duration::ZERO && crawled < page_cap && !frontier.queue.is_empty() {
            tokio::time::sleep(delay).await;
        }
    }
    let reason = if frontier.queue.is_empty() {
        "finished"
    } else {
        "page_cap"
    };
    Ok((crawled, failed, reason))
}

struct FetchedPage {
    row: NewPage,
    links: Vec<(Url, i32)>,
    fetch_error: Option<String>,
    hard_failure: bool,
}

async fn fetch_batch(deps: &CrawlDeps, batch: Vec<Item>) -> Vec<FetchedPage> {
    let mut tasks = Vec::new();
    for item in batch {
        let client = deps.client.clone();
        let renderer = deps.renderer.clone();
        tasks.push(async move { fetch_one(&client, renderer, item).await });
    }
    futures::future::join_all(tasks).await
}

async fn fetch_one(client: &CrawlClient, renderer: Arc<dyn Renderer>, item: Item) -> FetchedPage {
    let fetched = client.fetch(&item.url).await;
    let hard_failure = fetched.error.is_some() && fetched.status.is_none();
    let fetch_error = fetched.error.as_ref().map(|err| err.to_string());
    let (facts, needed) = page_facts(&fetched, &renderer).await;
    let links = facts
        .as_ref()
        .map(|facts| {
            facts
                .links
                .iter()
                .map(|link| (link.url.clone(), item.depth + 1))
                .collect()
        })
        .unwrap_or_default();
    let (facts_json, text) = split_text(facts);
    FetchedPage {
        row: NewPage {
            url: item.url.to_string(),
            depth: item.depth,
            in_sitemap: item.in_sitemap,
            status_code: fetched.status.map(i32::from),
            content_type: fetched.content_type,
            fetch_error: fetch_error.clone(),
            redirects: json!(fetched
                .redirects
                .iter()
                .map(|url| url.as_str())
                .collect::<Vec<_>>()),
            response_ms: Some(fetched.elapsed_ms as i32),
            bytes: Some(fetched.bytes as i32),
            is_https: item.url.scheme() == "https",
            x_robots_tag: fetched
                .headers
                .get("x-robots-tag")
                .and_then(|value| value.to_str().ok())
                .map(str::to_string),
            needed_javascript: needed,
            facts: facts_json,
            text,
        },
        links,
        fetch_error,
        hard_failure,
    }
}

async fn page_facts(
    fetched: &super::client::Fetched,
    renderer: &Arc<dyn Renderer>,
) -> (Option<PageFacts>, bool) {
    let Some(body) = fetched.body.clone() else {
        return (None, false);
    };
    let Some(mut facts) = extract_blocking(body, fetched.final_url.clone()).await else {
        return (None, false);
    };
    if !extract::looks_javascript_only(&facts) {
        return (Some(facts), false);
    }
    if let Ok(html) = renderer.render(&fetched.final_url).await {
        let rendered = extract_blocking(html, fetched.final_url.clone()).await;
        if let Some(rendered) = rendered.filter(|r| r.word_count > facts.word_count) {
            facts = rendered;
        }
    }
    (Some(facts), true)
}

/// Parsing a page is CPU work and a crafted page can make it long, so it runs
/// off the async threads. `None` means the extraction itself failed.
async fn extract_blocking(html: String, url: Url) -> Option<PageFacts> {
    match tokio::task::spawn_blocking(move || extract::extract(&html, &url)).await {
        Ok(facts) => Some(facts),
        Err(err) => {
            tracing::error!(error = %err, "page extraction failed");
            None
        }
    }
}

fn split_text(facts: Option<PageFacts>) -> (Option<Value>, Option<String>) {
    let Some(facts) = facts else {
        return (None, None);
    };
    let text = facts.text.clone();
    let mut value = serde_json::to_value(&facts).unwrap_or(Value::Null);
    if let Some(object) = value.as_object_mut() {
        object.remove("text");
    }
    (Some(value), Some(text))
}

async fn llms_exists(client: &CrawlClient, scope: &SiteScope) -> bool {
    let mut url = scope.root.clone();
    url.set_path("/llms.txt");
    url.set_query(None);
    let fetched = client.fetch_text(&url).await;
    matches!(fetched.status, Some(code) if (200..300).contains(&code))
}

fn parse_id(payload: &Value, key: &str) -> anyhow::Result<Uuid> {
    payload
        .get(key)
        .and_then(|value| value.as_str())
        .unwrap_or("")
        .parse()
        .map_err(|err| anyhow::anyhow!("{key}: {err}"))
}

fn polite_delay(settings: CrawlSettings, rules: &Robots) -> Duration {
    let asked = rules.crawl_delay().unwrap_or(Duration::ZERO);
    settings.delay.max(asked).min(Duration::from_secs(5))
}

async fn finish_now(
    deps: &CrawlDeps,
    workspace_id: Uuid,
    crawl_id: Uuid,
    done: &Finish,
) -> anyhow::Result<()> {
    let analysed = analyse(deps, workspace_id, crawl_id, done).await;
    repo::finish(&deps.pool, workspace_id, crawl_id, &analysed).await?;
    if let Ok(crawl) = repo::get_crawl(&deps.pool, workspace_id, crawl_id).await {
        schedule_next(&deps.pool, workspace_id, crawl.site_id).await?;
        if done.status == "completed" {
            crate::integrations::google::sync::enqueue_after_crawl(
                &deps.pool,
                workspace_id,
                crawl.site_id,
            )
            .await;
            crate::speed::run::enqueue_after_crawl(&deps.pool, workspace_id, crawl.site_id).await;
        }
        repo::trim_old_text(&deps.pool, workspace_id, crawl.site_id).await?;
    }
    Ok(())
}

/// Runs the checks for a crawl that completed. A failure is kept as the
/// crawl's `error` and never changes how the crawl ended.
async fn analyse(deps: &CrawlDeps, workspace_id: Uuid, crawl_id: Uuid, done: &Finish) -> Finish {
    if done.status != "completed" {
        return done.clone();
    }
    match crate::checks::store::run_for_crawl(&deps.pool, workspace_id, crawl_id).await {
        Ok(()) => done.clone(),
        Err(err) => {
            tracing::error!(%crawl_id, error = %err, "checks failed after a crawl");
            Finish {
                error: Some("The checks could not be run for this crawl.".into()),
                ..done.clone()
            }
        }
    }
}

pub async fn schedule_next(
    pool: &sqlx::PgPool,
    workspace_id: Uuid,
    site_id: Uuid,
) -> anyhow::Result<()> {
    if crate::sites::repo::get(pool, workspace_id, site_id)
        .await
        .is_err()
    {
        return Ok(());
    }
    let run_at = time::OffsetDateTime::now_utc() + time::Duration::days(7);
    crate::jobs::queue::enqueue_crawl_unless_queued(pool, workspace_id, site_id, run_at).await?;
    Ok(())
}

pub struct CrawlJob {
    pub deps: CrawlDeps,
}

#[async_trait]
impl JobHandler for CrawlJob {
    fn kind(&self) -> &'static str {
        "crawl"
    }

    async fn run(&self, job: &Job) -> anyhow::Result<()> {
        let workspace_id = parse_id(&job.payload, "workspace_id")?;
        if job.payload.get("crawl_id").is_none() && self.no_contact(workspace_id).await? {
            // A scheduled crawl waits for a contact address instead of failing.
            let site_id = parse_id(&job.payload, "site_id")?;
            return schedule_next(&self.deps.pool, workspace_id, site_id).await;
        }
        let Some(crawl_id) = self.crawl_for(job, workspace_id).await? else {
            return Ok(());
        };
        run_crawl(&self.deps, workspace_id, crawl_id).await
    }

    async fn on_dead(&self, job: &Job, error: &str) {
        let (Ok(workspace_id), Ok(crawl_id)) = (
            parse_id(&job.payload, "workspace_id"),
            parse_id(&job.payload, "crawl_id"),
        ) else {
            return;
        };
        tracing::error!(%crawl_id, %error, "crawl job failed for good");
        if let Err(err) =
            repo::fail_crawl(&self.deps.pool, workspace_id, crawl_id, CRAWL_FAILED).await
        {
            tracing::error!(%crawl_id, error = %err, "could not mark the crawl as failed");
        }
    }
}

impl CrawlJob {
    async fn no_contact(&self, workspace_id: Uuid) -> anyhow::Result<bool> {
        Ok(self.deps.for_workspace(workspace_id).await?.is_none())
    }

    /// The crawl a job runs. A scheduled job has none yet: it creates one and
    /// records it on the job, so a retry of the job finds the same row.
    async fn crawl_for(&self, job: &Job, workspace_id: Uuid) -> anyhow::Result<Option<Uuid>> {
        if job.payload.get("crawl_id").is_some() {
            return Ok(Some(parse_id(&job.payload, "crawl_id")?));
        }
        let site_id = parse_id(&job.payload, "site_id")?;
        let pool = &self.deps.pool;
        if crate::sites::repo::get(pool, workspace_id, site_id)
            .await
            .is_err()
        {
            return Ok(None);
        }
        let trigger = job.payload["trigger"].as_str().unwrap_or("scheduled");
        let crawl_id = repo::create_crawl(pool, workspace_id, site_id, trigger).await?;
        if let Err(err) = queue::attach_crawl(pool, job.id, crawl_id).await {
            repo::fail_crawl(pool, workspace_id, crawl_id, CRAWL_FAILED).await?;
            return Err(err);
        }
        Ok(Some(crawl_id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tracking_parameters_share_one_queue_slot() {
        let mut frontier = Frontier::new(500);
        let plain = Url::parse("https://example.com/a?id=2").unwrap();
        let tracked = Url::parse("https://example.com/a?id=2&utm_source=x#top").unwrap();
        frontier.push(plain, 1, false);
        frontier.push(tracked, 1, true);
        assert_eq!(frontier.queue.len(), 1);
        let popped = frontier.pop_n(5);
        assert_eq!(popped.len(), 1);
        assert!(popped[0].in_sitemap);
    }

    fn numbered(n: usize) -> Url {
        Url::parse(&format!("https://example.com/p{n}")).unwrap()
    }

    #[test]
    fn the_frontier_never_holds_more_than_20_times_the_page_cap() {
        let mut frontier = Frontier::new(5);
        for n in 0..1_000 {
            frontier.push(numbered(n), 1, false);
        }
        assert_eq!(frontier.queue.len(), 100);
        assert!(frontier.seen.len() <= 100);
    }

    #[test]
    fn the_frontier_cap_counts_pages_already_crawled() {
        let mut frontier = Frontier::new(1);
        for n in 0..15 {
            frontier.mark_seen(&numbered(n));
        }
        for n in 15..100 {
            frontier.push(numbered(n), 1, false);
        }
        assert_eq!(frontier.queue.len(), 5);
    }

    #[test]
    fn an_address_is_queued_once_however_often_it_is_found() {
        let mut frontier = Frontier::new(500);
        for _ in 0..50 {
            frontier.push(numbered(1), 1, false);
        }
        assert_eq!(frontier.queue.len(), 1);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn extraction_does_not_hold_up_the_runtime() {
        let html = format!(
            "<body>{}</body>",
            r#"<p>words <a href="/x">link</a></p>"#.repeat(60_000)
        );
        let ticks = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let counter = ticks.clone();
        let ticker = tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_millis(2)).await;
                counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            }
        });
        tokio::task::yield_now().await;
        let facts = extract_blocking(html, Url::parse("https://example.com/").unwrap()).await;
        ticker.abort();
        assert!(facts.is_some());
        assert!(ticks.load(std::sync::atomic::Ordering::SeqCst) >= 3);
    }
}
