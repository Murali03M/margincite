//! Stored page speed results and the daily allowance for manual checks.

use serde::Serialize;
use sqlx::{PgConnection, PgPool};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::checks::context::{SpeedData, SpeedPage};
use crate::error::AppError;
use crate::integrations::pagespeed::client::Measurement;

/// The only test strategy for now.
pub const STRATEGY: &str = "mobile";
/// Manual checks a site may start in a UTC day.
pub const MAX_MANUAL_PER_DAY: i64 = 5;
/// Pages tested besides the home page.
pub const EXTRA_PAGES: i64 = 4;

fn internal(err: sqlx::Error) -> AppError {
    AppError::Internal(err.into())
}

/// One tested page. `error` is why the last attempt failed; the numbers are
/// then from the last good test, or all unknown.
#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct PageResult {
    pub url: String,
    pub strategy: String,
    pub performance: Option<i32>,
    pub lcp_ms: Option<i32>,
    pub cls: Option<f64>,
    pub inp_ms: Option<i32>,
    pub tbt_ms: Option<i32>,
    pub field_data: bool,
    pub error: Option<String>,
    /// When the last attempt was made, good or not.
    #[serde(skip)]
    pub attempted_at: OffsetDateTime,
}

/// What one page's test came to: its numbers, or the plain sentence saying
/// why there are none.
pub enum Outcome {
    Measured(Measurement),
    Failed(String),
}

/// Stores a whole run in one transaction: each page's result, then removes
/// the site's rows for pages that were not tested this time, so a page that
/// dropped out of the tested set stops counting.
pub async fn save_run(
    pool: &PgPool,
    workspace_id: Uuid,
    site_id: Uuid,
    results: &[(String, Outcome)],
) -> Result<(), AppError> {
    let mut tx = pool.begin().await.map_err(internal)?;
    for (url, outcome) in results {
        match outcome {
            Outcome::Measured(found) => {
                save_measurement(&mut tx, workspace_id, site_id, url, found).await?
            }
            Outcome::Failed(message) => {
                save_error(&mut tx, workspace_id, site_id, url, message).await?
            }
        }
    }
    let tested: Vec<String> = results.iter().map(|(url, _)| url.clone()).collect();
    sqlx::query(
        "DELETE FROM speed_results
         WHERE workspace_id = $1 AND site_id = $2 AND NOT (url = ANY($3))",
    )
    .bind(workspace_id)
    .bind(site_id)
    .bind(&tested)
    .execute(&mut *tx)
    .await
    .map_err(internal)?;
    tx.commit().await.map_err(internal)
}

/// A good test replaces the page's numbers and clears its error.
async fn save_measurement(
    conn: &mut PgConnection,
    workspace_id: Uuid,
    site_id: Uuid,
    url: &str,
    found: &Measurement,
) -> Result<(), AppError> {
    sqlx::query(
        "INSERT INTO speed_results
             (workspace_id, site_id, url, strategy, performance, lcp_ms, cls, inp_ms, tbt_ms,
              field_data, error, checked_at, attempted_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, NULL, now(), now())
         ON CONFLICT (site_id, url, strategy) DO UPDATE SET
             performance = excluded.performance, lcp_ms = excluded.lcp_ms, cls = excluded.cls,
             inp_ms = excluded.inp_ms, tbt_ms = excluded.tbt_ms, field_data = excluded.field_data,
             error = NULL, checked_at = now(), attempted_at = now()",
    )
    .bind(workspace_id)
    .bind(site_id)
    .bind(url)
    .bind(STRATEGY)
    .bind(found.performance)
    .bind(found.lcp_ms)
    .bind(found.cls)
    .bind(found.inp_ms)
    .bind(found.tbt_ms)
    .bind(found.field_data)
    .execute(conn)
    .await
    .map_err(internal)?;
    Ok(())
}

/// A failed test records why and when, and keeps the last good numbers and
/// the date they were measured.
async fn save_error(
    conn: &mut PgConnection,
    workspace_id: Uuid,
    site_id: Uuid,
    url: &str,
    message: &str,
) -> Result<(), AppError> {
    sqlx::query(
        "INSERT INTO speed_results (workspace_id, site_id, url, strategy, field_data, error)
         VALUES ($1, $2, $3, $4, false, $5)
         ON CONFLICT (site_id, url, strategy)
         DO UPDATE SET error = excluded.error, attempted_at = now()",
    )
    .bind(workspace_id)
    .bind(site_id)
    .bind(url)
    .bind(STRATEGY)
    .bind(message)
    .execute(conn)
    .await
    .map_err(internal)?;
    Ok(())
}

/// The site's results, its home page first.
pub async fn list(
    pool: &PgPool,
    workspace_id: Uuid,
    site_id: Uuid,
    root_url: &str,
) -> Result<Vec<PageResult>, AppError> {
    sqlx::query_as(
        "SELECT url, strategy, performance, lcp_ms, cls, inp_ms, tbt_ms, field_data, error, attempted_at
         FROM speed_results WHERE workspace_id = $1 AND site_id = $2
         ORDER BY (url = $3) DESC, url",
    )
    .bind(workspace_id)
    .bind(site_id)
    .bind(root_url)
    .fetch_all(pool)
    .await
    .map_err(internal)
}

/// The pages a check tests: the home page, then the crawled working pages
/// with the most Search Console impressions or, without any, the shallowest.
pub async fn pages_to_test(
    pool: &PgPool,
    workspace_id: Uuid,
    site_id: Uuid,
    root_url: &str,
) -> Result<Vec<String>, AppError> {
    let extra: Vec<String> = sqlx::query_scalar(
        "SELECT p.url
         FROM pages p
         JOIN crawls c ON c.id = p.crawl_id
         LEFT JOIN (SELECT page, SUM(impressions) AS seen FROM search_rows
                    WHERE workspace_id = $1 AND site_id = $2 GROUP BY page) s ON s.page = p.url
         WHERE c.workspace_id = $1 AND c.site_id = $2
           AND c.id = (SELECT id FROM crawls WHERE workspace_id = $1 AND site_id = $2
                       AND status = 'completed' ORDER BY created_at DESC LIMIT 1)
           AND p.fetch_error IS NULL AND p.status_code = 200
           AND p.content_type ILIKE '%html%' AND p.url <> $3
         ORDER BY COALESCE(s.seen, 0) DESC, p.depth, p.url
         LIMIT $4",
    )
    .bind(workspace_id)
    .bind(site_id)
    .bind(root_url)
    .bind(EXTRA_PAGES)
    .fetch_all(pool)
    .await
    .map_err(internal)?;
    let mut pages = vec![root_url.to_string()];
    pages.extend(extra);
    Ok(pages)
}

/// Counts a manual check against the site's allowance for today (UTC), or
/// says it is used up. Counting and recording happen under one lock.
pub async fn reserve_manual_check(
    pool: &PgPool,
    workspace_id: Uuid,
    site_id: Uuid,
) -> Result<(), AppError> {
    let mut tx = pool.begin().await.map_err(internal)?;
    sqlx::query("SELECT id FROM sites WHERE id = $1 AND workspace_id = $2 FOR UPDATE")
        .bind(site_id)
        .bind(workspace_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(internal)?
        .ok_or(AppError::NotFound)?;
    let used: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM speed_manual_checks
         WHERE site_id = $1
           AND created_at >= (date_trunc('day', now() AT TIME ZONE 'UTC') AT TIME ZONE 'UTC')",
    )
    .bind(site_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(internal)?;
    if used >= MAX_MANUAL_PER_DAY {
        return Err(AppError::DailyLimit(
            "You have used today's 5 page speed checks for this site. Try again tomorrow.".into(),
        ));
    }
    sqlx::query("INSERT INTO speed_manual_checks (workspace_id, site_id) VALUES ($1, $2)")
        .bind(workspace_id)
        .bind(site_id)
        .execute(&mut *tx)
        .await
        .map_err(internal)?;
    tx.commit().await.map_err(internal)
}

type DataRow = (
    String,
    Option<i32>,
    Option<f64>,
    Option<i32>,
    Option<i32>,
    bool,
);

/// What the speed checks read: every page with a number, once at least one
/// page has a result without an error. A page whose last test failed still
/// counts with its last good numbers, so a passing hiccup does not make its
/// items flicker.
pub async fn load_speed_data(
    pool: &PgPool,
    workspace_id: Uuid,
    site_id: Uuid,
) -> Result<Option<SpeedData>, AppError> {
    let rows: Vec<DataRow> = sqlx::query_as(
        "SELECT url, lcp_ms, cls, inp_ms, tbt_ms, (error IS NULL)
             FROM speed_results WHERE workspace_id = $1 AND site_id = $2 ORDER BY url",
    )
    .bind(workspace_id)
    .bind(site_id)
    .fetch_all(pool)
    .await
    .map_err(internal)?;
    if !rows.iter().any(|row| row.5) {
        return Ok(None);
    }
    Ok(Some(SpeedData {
        pages: rows
            .into_iter()
            .map(|(url, lcp_ms, cls, inp_ms, tbt_ms, _)| SpeedPage {
                url,
                lcp_ms,
                cls,
                inp_ms,
                tbt_ms,
            })
            .collect(),
    }))
}
