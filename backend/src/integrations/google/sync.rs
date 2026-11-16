//! The `google_sync` job: pulls a site's Search Console numbers and swaps them
//! in. 429s, 5xx and anything unexpected fail the job so the queue retries;
//! a refused token or property marks the connection `needs_reconnecting`.

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use sqlx::PgPool;
use time::macros::format_description;
use time::{Date, Duration, OffsetDateTime};
use url::Url;
use uuid::Uuid;

use super::client::{ApiRow, GoogleClient, GoogleError, Report};
use super::repo::{self, DailyTotal, NewRow, Previous, SyncData};
use crate::config::Config;
use crate::crawl::clean::strip_nul;
use crate::crawl::repo as crawls;
use crate::crawl::urls::normalize;
use crate::jobs::queue::{self, Job};
use crate::jobs::worker::JobHandler;

pub const KIND: &str = "google_sync";
/// Search Console data is final about three days behind.
const DELAY_DAYS: i64 = 3;
const WINDOW_DAYS: i64 = 28;
const DAILY_DAYS: i64 = 90;

const NEEDS_RECONNECTING: &str =
    "Google no longer accepts this connection. Connect Google Search Console again.";
const PROPERTY_DENIED: &str =
    "Google denied access to this property. Connect again, and check you can see it in Search Console.";
/// Shown on the connection when Google refuses this server's own client.
pub const SERVER_CREDENTIALS: &str = "Google rejected this server's Google credentials. Check GOOGLE_CLIENT_ID and GOOGLE_CLIENT_SECRET on the server.";
const UNREADABLE_TOKEN: &str = "The saved Google connection could not be read. Connect again.";
const SYNC_FAILED: &str = "The last sync failed. It will be tried again, or press Sync now.";

/// The days a sync asks about, counted back from the last day with final data.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Windows {
    pub current: (Date, Date),
    pub previous: (Date, Date),
    pub daily: (Date, Date),
}

pub fn windows(today: Date) -> Windows {
    let end = today - Duration::days(DELAY_DAYS);
    let start = end - Duration::days(WINDOW_DAYS - 1);
    let previous_end = start - Duration::days(1);
    Windows {
        current: (start, end),
        previous: (previous_end - Duration::days(WINDOW_DAYS - 1), previous_end),
        daily: (end - Duration::days(DAILY_DAYS - 1), end),
    }
}

const DATE_FORMAT: &[time::format_description::BorrowedFormatItem<'static>] =
    format_description!("[year]-[month]-[day]");

fn day(date: Date) -> String {
    date.format(DATE_FORMAT).expect("a date always formats")
}

fn parse_day(text: &str) -> Option<Date> {
    Date::parse(text, DATE_FORMAT).ok()
}

pub struct GoogleSyncJob {
    pool: PgPool,
    config: Arc<Config>,
    client: Arc<GoogleClient>,
}

impl GoogleSyncJob {
    pub fn new(pool: PgPool, config: Arc<Config>, client: Arc<GoogleClient>) -> Self {
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
impl JobHandler for GoogleSyncJob {
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
        tracing::error!(%site_id, %error, "google sync failed for good");
        if let Err(err) = repo::record_error(&self.pool, workspace_id, site_id, SYNC_FAILED).await {
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

/// After a crawl: sync again, but only for a site that is connected.
pub async fn enqueue_after_crawl(pool: &PgPool, workspace_id: Uuid, site_id: Uuid) {
    let connected = repo::get_connection(pool, workspace_id, site_id)
        .await
        .ok()
        .flatten()
        .is_some_and(|c| c.status == "connected" && c.property.is_some());
    if connected {
        if let Err(err) = enqueue_now(pool, workspace_id, site_id).await {
            tracing::error!(%site_id, error = %err, "could not queue a Search Console sync");
        }
    }
}

async fn schedule_next(pool: &PgPool, workspace_id: Uuid, site_id: Uuid) {
    let run_at = OffsetDateTime::now_utc() + Duration::days(1);
    if let Err(err) =
        queue::enqueue_site_job_unless_queued(pool, KIND, workspace_id, site_id, run_at).await
    {
        tracing::error!(%site_id, error = %err, "could not schedule the next Search Console sync");
    }
}

/// What a failed Google call means for the sync.
enum Stop {
    /// The connection is dead; say so and finish the job.
    Reconnect(&'static str),
    /// Try again later.
    Retry(anyhow::Error),
    /// The server's own Google credentials were refused. The connection is
    /// fine; say what to fix and try again tomorrow.
    ServerCredentials,
}

impl From<GoogleError> for Stop {
    fn from(err: GoogleError) -> Self {
        match err {
            GoogleError::Rejected => Stop::Reconnect(PROPERTY_DENIED),
            GoogleError::Transient(message) => Stop::Retry(anyhow::anyhow!(message)),
            GoogleError::ServerCredentials => Stop::ServerCredentials,
        }
    }
}

pub async fn sync_site(
    pool: &PgPool,
    config: &Config,
    client: &GoogleClient,
    workspace_id: Uuid,
    site_id: Uuid,
) -> anyhow::Result<()> {
    let credentials = crate::settings::resolve::google(pool, config, workspace_id).await?;
    let (Some(credentials), Some(secrets)) = (credentials, config.secret_key.as_ref()) else {
        return Ok(());
    };
    let Some(connection) = repo::get_connection(pool, workspace_id, site_id).await? else {
        return Ok(());
    };
    let (true, Some(property)) = (connection.status == "connected", connection.property) else {
        return Ok(());
    };
    let Ok(refresh_token) = secrets.open(&connection.refresh_token_sealed) else {
        repo::mark_needs_reconnecting(pool, workspace_id, site_id, UNREADABLE_TOKEN).await?;
        return Ok(());
    };
    let fetched = match client
        .refresh_access_token(&credentials, &refresh_token)
        .await
    {
        Ok(access) => fetch(client, &access, &property).await,
        Err(GoogleError::Rejected) => Err(Stop::Reconnect(NEEDS_RECONNECTING)),
        Err(other) => Err(other.into()),
    };
    let data = match fetched {
        Ok(data) => data,
        Err(Stop::Reconnect(message)) => {
            repo::mark_needs_reconnecting(pool, workspace_id, site_id, message).await?;
            return Ok(());
        }
        Err(Stop::Retry(err)) => return Err(err),
        Err(Stop::ServerCredentials) => {
            repo::record_error(pool, workspace_id, site_id, SERVER_CREDENTIALS).await?;
            schedule_next(pool, workspace_id, site_id).await;
            return Ok(());
        }
    };
    if !repo::replace_data(pool, workspace_id, site_id, &data).await? {
        return Ok(());
    }
    refresh_checks(pool, workspace_id, site_id).await;
    schedule_next(pool, workspace_id, site_id).await;
    Ok(())
}

/// The checks that read Search Console data only know about it once they run
/// again, so the latest finished crawl is checked against the new numbers.
pub async fn refresh_checks(pool: &PgPool, workspace_id: Uuid, site_id: Uuid) {
    let latest = match crawls::latest_completed(pool, workspace_id, site_id).await {
        Ok(latest) => latest,
        Err(err) => {
            tracing::error!(%site_id, error = %err, "could not look up the latest crawl");
            return;
        }
    };
    if let Some(crawl) = latest {
        if let Err(err) = crate::checks::store::run_for_crawl(pool, workspace_id, crawl.id).await {
            tracing::error!(%site_id, error = %err, "could not re-run the checks after a sync");
        }
    }
}

async fn fetch(client: &GoogleClient, access: &str, property: &str) -> Result<SyncData, Stop> {
    let windows = windows(OffsetDateTime::now_utc().date());
    fn report<'a>(range: (Date, Date), dimensions: &'a [&'a str]) -> Report<'a> {
        Report {
            start_date: day(range.0),
            end_date: day(range.1),
            dimensions,
        }
    }
    let rows = client
        .report(
            access,
            property,
            &report(windows.current, &["query", "page"]),
        )
        .await?;
    let daily = client
        .report(access, property, &report(windows.daily, &["date"]))
        .await?;
    let previous_queries = client
        .report(access, property, &report(windows.previous, &["query"]))
        .await?;
    Ok(SyncData {
        rows: merge_rows(rows),
        previous: previous_totals(&daily, &previous_queries, windows.previous),
        daily: daily_totals(&daily),
    })
}

fn count(value: f64) -> i64 {
    if value.is_finite() {
        value.round().max(0.0) as i64
    } else {
        0
    }
}

/// An address in the form crawled pages use, so the two can be matched.
fn normal_page(address: &str) -> String {
    match Url::parse(address) {
        Ok(url) => normalize(&url).to_string(),
        Err(_) => address.to_string(),
    }
}

/// Query and page rows, with pages normalised. Two addresses that normalise
/// alike (a tracking parameter apart) become one row.
pub fn merge_rows(rows: Vec<ApiRow>) -> Vec<NewRow> {
    let mut merged: Vec<NewRow> = Vec::new();
    let mut at: HashMap<(String, String), usize> = HashMap::new();
    for row in rows {
        let [query, page] = row.keys.as_slice() else {
            continue;
        };
        let (query, page) = (
            strip_nul(query).into_owned(),
            strip_nul(&normal_page(page)).into_owned(),
        );
        let (clicks, impressions) = (count(row.clicks), count(row.impressions));
        match at.get(&(query.clone(), page.clone())) {
            Some(&index) => {
                let seen = &mut merged[index];
                let total = seen.impressions + impressions;
                if total > 0 {
                    seen.position = (seen.position * seen.impressions as f64
                        + row.position * impressions as f64)
                        / total as f64;
                }
                seen.clicks += clicks;
                seen.impressions = total;
                seen.ctr = if total > 0 {
                    seen.clicks as f64 / total as f64
                } else {
                    0.0
                };
            }
            None => {
                at.insert((query.clone(), page.clone()), merged.len());
                merged.push(NewRow {
                    query,
                    page,
                    clicks,
                    impressions,
                    ctr: row.ctr,
                    position: row.position,
                });
            }
        }
    }
    merged
}

fn daily_totals(rows: &[ApiRow]) -> Vec<DailyTotal> {
    rows.iter()
        .filter_map(|row| {
            Some(DailyTotal {
                date: parse_day(row.keys.first()?)?,
                clicks: count(row.clicks),
                impressions: count(row.impressions),
            })
        })
        .collect()
}

/// Clicks and impressions of the 28 days before the current window come from
/// the daily series, the same basis as the current numbers. The keyword count
/// is the queries that had impressions in that window.
fn previous_totals(daily: &[ApiRow], queries: &[ApiRow], window: (Date, Date)) -> Previous {
    let (mut clicks, mut impressions) = (0, 0);
    for total in daily_totals(daily) {
        if total.date >= window.0 && total.date <= window.1 {
            clicks += total.clicks;
            impressions += total.impressions;
        }
    }
    Previous {
        clicks,
        impressions,
        keywords: queries.iter().filter(|row| row.impressions > 0.0).count() as i64,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::macros::date;

    #[test]
    fn windows_end_three_days_back_and_do_not_overlap() {
        let w = windows(date!(2026 - 11 - 14));
        assert_eq!(w.current, (date!(2026 - 10 - 15), date!(2026 - 11 - 11)));
        assert_eq!(w.previous, (date!(2026 - 09 - 17), date!(2026 - 10 - 14)));
        assert_eq!(w.daily.1, date!(2026 - 11 - 11));
        assert_eq!((w.daily.1 - w.daily.0).whole_days(), 89);
        assert!(w.daily.0 <= w.previous.0);
    }

    fn api(keys: &[&str], clicks: f64, impressions: f64, position: f64) -> ApiRow {
        ApiRow {
            keys: keys.iter().map(|k| k.to_string()).collect(),
            clicks,
            impressions,
            ctr: if impressions > 0.0 {
                clicks / impressions
            } else {
                0.0
            },
            position,
        }
    }

    #[test]
    fn rows_are_normalised_and_merged() {
        let rows = merge_rows(vec![
            api(
                &["widgets", "https://Example.com/a?utm_source=x"],
                2.0,
                10.0,
                4.0,
            ),
            api(&["widgets", "https://example.com/a"], 3.0, 30.0, 8.0),
            api(&["only one key"], 1.0, 1.0, 1.0),
        ]);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].page, "https://example.com/a");
        assert_eq!((rows[0].clicks, rows[0].impressions), (5, 40));
        assert!((rows[0].position - 7.0).abs() < 1e-9);
        assert!((rows[0].ctr - 0.125).abs() < 1e-9);
    }

    #[test]
    fn previous_totals_use_the_daily_series_inside_the_window() {
        let daily = vec![
            api(&["2026-10-14"], 5.0, 50.0, 0.0),
            api(&["2026-10-15"], 100.0, 1000.0, 0.0),
            api(&["2026-09-17"], 7.0, 70.0, 0.0),
            api(&["2026-09-16"], 9.0, 90.0, 0.0),
            api(&["not a date"], 1.0, 1.0, 0.0),
        ];
        let queries = vec![api(&["a"], 1.0, 10.0, 3.0), api(&["b"], 0.0, 0.0, 3.0)];
        let window = (date!(2026 - 09 - 17), date!(2026 - 10 - 14));
        let previous = previous_totals(&daily, &queries, window);
        assert_eq!(
            previous,
            Previous {
                clicks: 12,
                impressions: 120,
                keywords: 1
            }
        );
    }

    #[test]
    fn counts_never_go_negative_or_non_finite() {
        assert_eq!(count(-3.0), 0);
        assert_eq!(count(f64::NAN), 0);
        assert_eq!(count(2.5), 3);
    }
}
