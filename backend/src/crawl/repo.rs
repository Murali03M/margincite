use serde_json::Value;
use sqlx::PgPool;
use uuid::Uuid;

use super::clean::{strip_nul, strip_nul_json, strip_nul_owned};
use crate::checks::{CrawlContext, PageView};
use crate::crawl::extract::PageFacts;
use crate::error::AppError;
use crate::keywords::repo as keywords;

/// How much of a robots.txt is kept.
const ROBOTS_TXT_LIMIT: usize = 500_000;

#[derive(Debug, Clone, sqlx::FromRow, serde::Serialize)]
pub struct Crawl {
    pub id: Uuid,
    pub site_id: Uuid,
    pub status: String,
    pub trigger: String,
    pub stop_reason: Option<String>,
    /// Whether the site has a robots.txt. The text itself is never sent out.
    pub robots_txt_found: bool,
    pub sitemap_found: bool,
    pub llms_txt_found: bool,
    pub pages_crawled: i32,
    pub pages_failed: i32,
    pub error: Option<String>,
    pub score: Option<i32>,
    pub html_pages: i32,
}

#[derive(Debug, Clone, sqlx::FromRow, serde::Serialize)]
pub struct Page {
    pub id: Uuid,
    pub crawl_id: Uuid,
    pub url: String,
    pub depth: i32,
    pub in_sitemap: bool,
    pub status_code: Option<i32>,
    pub content_type: Option<String>,
    pub fetch_error: Option<String>,
    pub response_ms: Option<i32>,
    pub bytes: Option<i32>,
    pub is_https: bool,
    pub x_robots_tag: Option<String>,
    pub needed_javascript: bool,
    pub facts: Option<Value>,
    pub text: Option<String>,
}

pub async fn create_crawl(
    pool: &PgPool,
    workspace_id: Uuid,
    site_id: Uuid,
    trigger: &str,
) -> Result<Uuid, AppError> {
    let id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO crawls (id, workspace_id, site_id, status, trigger) VALUES ($1, $2, $3, 'queued', $4)",
    )
    .bind(id)
    .bind(workspace_id)
    .bind(site_id)
    .bind(trigger)
    .execute(pool)
    .await
    .map_err(|err| AppError::Internal(err.into()))?;
    Ok(id)
}

pub async fn get_crawl(pool: &PgPool, workspace_id: Uuid, id: Uuid) -> Result<Crawl, AppError> {
    sqlx::query_as(
        "SELECT id, site_id, status, trigger, stop_reason, (robots_txt IS NOT NULL) AS robots_txt_found, sitemap_found, llms_txt_found,
                pages_crawled, pages_failed, error, score, html_pages
         FROM crawls WHERE id = $1 AND workspace_id = $2",
    )
    .bind(id)
    .bind(workspace_id)
    .fetch_optional(pool)
    .await
    .map_err(|err| AppError::Internal(err.into()))?
    .ok_or(AppError::NotFound)
}

pub async fn list_crawls(
    pool: &PgPool,
    workspace_id: Uuid,
    site_id: Uuid,
) -> Result<Vec<Crawl>, AppError> {
    sqlx::query_as(
        "SELECT id, site_id, status, trigger, stop_reason, (robots_txt IS NOT NULL) AS robots_txt_found, sitemap_found, llms_txt_found,
                pages_crawled, pages_failed, error, score, html_pages
         FROM crawls WHERE workspace_id = $1 AND site_id = $2 ORDER BY created_at",
    )
    .bind(workspace_id)
    .bind(site_id)
    .fetch_all(pool)
    .await
    .map_err(|err| AppError::Internal(err.into()))
}

pub async fn mark_running(pool: &PgPool, workspace_id: Uuid, id: Uuid) -> Result<(), AppError> {
    sqlx::query(
        "UPDATE crawls
         SET status = 'running', started_at = COALESCE(started_at, now()),
             finished_at = NULL, error = NULL
         WHERE id = $1 AND workspace_id = $2",
    )
    .bind(id)
    .bind(workspace_id)
    .execute(pool)
    .await
    .map_err(|err| AppError::Internal(err.into()))?;
    Ok(())
}

pub async fn note_discovery(
    pool: &PgPool,
    workspace_id: Uuid,
    id: Uuid,
    robots_txt: Option<&str>,
    sitemap_found: bool,
    llms_txt_found: bool,
) -> Result<(), AppError> {
    let robots_txt = robots_txt.map(|text| {
        strip_nul(text)
            .chars()
            .take(ROBOTS_TXT_LIMIT)
            .collect::<String>()
    });
    sqlx::query(
        "UPDATE crawls SET robots_txt = $3, sitemap_found = $4, llms_txt_found = $5
         WHERE id = $1 AND workspace_id = $2",
    )
    .bind(id)
    .bind(workspace_id)
    .bind(&robots_txt)
    .bind(sitemap_found)
    .bind(llms_txt_found)
    .execute(pool)
    .await
    .map_err(|err| AppError::Internal(err.into()))?;
    Ok(())
}

#[derive(Debug, Clone)]
pub struct Finish {
    pub status: String,
    pub stop_reason: String,
    pub pages_crawled: i32,
    pub pages_failed: i32,
    pub error: Option<String>,
}

pub async fn finish(
    pool: &PgPool,
    workspace_id: Uuid,
    id: Uuid,
    done: &Finish,
) -> Result<(), AppError> {
    let error = done
        .error
        .as_deref()
        .map(|text| strip_nul(text).into_owned());
    sqlx::query(
        "UPDATE crawls
         SET status = $3, stop_reason = $4, pages_crawled = $5, pages_failed = $6, error = $7, finished_at = now()
         WHERE id = $1 AND workspace_id = $2",
    )
    .bind(id)
    .bind(workspace_id)
    .bind(&done.status)
    .bind(&done.stop_reason)
    .bind(done.pages_crawled)
    .bind(done.pages_failed)
    .bind(&error)
    .execute(pool)
    .await
    .map_err(|err| AppError::Internal(err.into()))?;
    Ok(())
}

/// Ends a crawl that did not finish, so it no longer reads as running and the
/// site can be crawled again. A crawl that already ended is left as it is.
pub async fn fail_crawl(
    pool: &PgPool,
    workspace_id: Uuid,
    id: Uuid,
    message: &str,
) -> Result<(), AppError> {
    sqlx::query(
        "UPDATE crawls SET status = 'failed', error = $3, finished_at = now()
         WHERE id = $1 AND workspace_id = $2 AND status IN ('queued', 'running')",
    )
    .bind(id)
    .bind(workspace_id)
    .bind(strip_nul(message).as_ref())
    .execute(pool)
    .await
    .map_err(|err| AppError::Internal(err.into()))?;
    Ok(())
}

pub struct NewPage {
    pub url: String,
    pub depth: i32,
    pub in_sitemap: bool,
    pub status_code: Option<i32>,
    pub content_type: Option<String>,
    pub fetch_error: Option<String>,
    pub redirects: Value,
    pub response_ms: Option<i32>,
    pub bytes: Option<i32>,
    pub is_https: bool,
    pub x_robots_tag: Option<String>,
    pub needed_javascript: bool,
    pub facts: Option<Value>,
    pub text: Option<String>,
}

impl NewPage {
    /// The page with every NUL character removed, ready to store.
    fn cleaned(&self) -> NewPage {
        let text = |value: &Option<String>| value.clone().map(strip_nul_owned);
        NewPage {
            url: strip_nul(&self.url).into_owned(),
            depth: self.depth,
            in_sitemap: self.in_sitemap,
            status_code: self.status_code,
            content_type: text(&self.content_type),
            fetch_error: text(&self.fetch_error),
            redirects: strip_nul_json(&self.redirects),
            response_ms: self.response_ms,
            bytes: self.bytes,
            is_https: self.is_https,
            x_robots_tag: text(&self.x_robots_tag),
            needed_javascript: self.needed_javascript,
            facts: self.facts.as_ref().map(strip_nul_json),
            text: text(&self.text),
        }
    }
}

pub async fn upsert_page(
    pool: &PgPool,
    workspace_id: Uuid,
    crawl_id: Uuid,
    page: &NewPage,
) -> Result<(), AppError> {
    let page = &page.cleaned();
    sqlx::query(
        "INSERT INTO pages (
            id, workspace_id, crawl_id, url, depth, in_sitemap, status_code, content_type,
            fetch_error, redirects, response_ms, bytes, is_https, x_robots_tag,
            needed_javascript, facts, text
         ) VALUES (
            $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17
         )
         ON CONFLICT (crawl_id, url) DO NOTHING",
    )
    .bind(Uuid::new_v4())
    .bind(workspace_id)
    .bind(crawl_id)
    .bind(&page.url)
    .bind(page.depth)
    .bind(page.in_sitemap)
    .bind(page.status_code)
    .bind(&page.content_type)
    .bind(&page.fetch_error)
    .bind(&page.redirects)
    .bind(page.response_ms)
    .bind(page.bytes)
    .bind(page.is_https)
    .bind(&page.x_robots_tag)
    .bind(page.needed_javascript)
    .bind(&page.facts)
    .bind(&page.text)
    .execute(pool)
    .await
    .map_err(|err| AppError::Internal(err.into()))?;
    Ok(())
}

pub async fn crawled_urls(
    pool: &PgPool,
    workspace_id: Uuid,
    crawl_id: Uuid,
) -> Result<Vec<String>, AppError> {
    sqlx::query_scalar("SELECT url FROM pages WHERE workspace_id = $1 AND crawl_id = $2")
        .bind(workspace_id)
        .bind(crawl_id)
        .fetch_all(pool)
        .await
        .map_err(|err| AppError::Internal(err.into()))
}

pub async fn list_pages(
    pool: &PgPool,
    workspace_id: Uuid,
    crawl_id: Uuid,
) -> Result<Vec<Page>, AppError> {
    sqlx::query_as(
        "SELECT id, crawl_id, url, depth, in_sitemap, status_code, content_type, fetch_error,
                response_ms, bytes, is_https, x_robots_tag, needed_javascript, facts, text
         FROM pages WHERE workspace_id = $1 AND crawl_id = $2 ORDER BY created_at",
    )
    .bind(workspace_id)
    .bind(crawl_id)
    .fetch_all(pool)
    .await
    .map_err(|err| AppError::Internal(err.into()))
}

pub async fn get_page(pool: &PgPool, workspace_id: Uuid, id: Uuid) -> Result<Page, AppError> {
    sqlx::query_as(
        "SELECT id, crawl_id, url, depth, in_sitemap, status_code, content_type, fetch_error,
                response_ms, bytes, is_https, x_robots_tag, needed_javascript, facts, text
         FROM pages WHERE workspace_id = $1 AND id = $2",
    )
    .bind(workspace_id)
    .bind(id)
    .fetch_optional(pool)
    .await
    .map_err(|err| AppError::Internal(err.into()))?
    .ok_or(AppError::NotFound)
}

pub async fn trim_old_text(
    pool: &PgPool,
    workspace_id: Uuid,
    site_id: Uuid,
) -> Result<(), AppError> {
    sqlx::query(
        "UPDATE pages SET text = NULL
         WHERE workspace_id = $1 AND crawl_id IN (
             SELECT id FROM crawls WHERE workspace_id = $1 AND site_id = $2
             ORDER BY created_at DESC OFFSET 2
         )",
    )
    .bind(workspace_id)
    .bind(site_id)
    .execute(pool)
    .await
    .map_err(|err| AppError::Internal(err.into()))?;
    Ok(())
}

/// Crawls the person started today. A site's first crawl counts like any other.
pub async fn limited_crawls_today(pool: &PgPool, workspace_id: Uuid) -> Result<i64, AppError> {
    sqlx::query_scalar(
        "SELECT COUNT(*) FROM crawls
         WHERE workspace_id = $1 AND trigger IN ('manual', 'first')
           AND created_at >= date_trunc('day', now() AT TIME ZONE 'UTC') AT TIME ZONE 'UTC'",
    )
    .bind(workspace_id)
    .fetch_one(pool)
    .await
    .map_err(|err| AppError::Internal(err.into()))
}

pub async fn crawl_in_progress(
    pool: &PgPool,
    workspace_id: Uuid,
    site_id: Uuid,
) -> Result<bool, AppError> {
    let found: bool = sqlx::query_scalar(
        "SELECT EXISTS(
            SELECT 1 FROM crawls
            WHERE workspace_id = $1 AND site_id = $2 AND status IN ('queued', 'running')
         )",
    )
    .bind(workspace_id)
    .bind(site_id)
    .fetch_one(pool)
    .await
    .map_err(|err| AppError::Internal(err.into()))?;
    Ok(found)
}

#[derive(Debug, Clone, sqlx::FromRow, serde::Serialize)]
pub struct PageSummary {
    pub id: Uuid,
    pub crawl_id: Uuid,
    pub url: String,
    pub depth: i32,
    pub in_sitemap: bool,
    pub status_code: Option<i32>,
    pub content_type: Option<String>,
    pub fetch_error: Option<String>,
    pub needed_javascript: bool,
}

pub async fn list_pages_page(
    pool: &PgPool,
    workspace_id: Uuid,
    crawl_id: Uuid,
    limit: i64,
    offset: i64,
) -> Result<(Vec<PageSummary>, i64), AppError> {
    let total: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM pages WHERE workspace_id = $1 AND crawl_id = $2")
            .bind(workspace_id)
            .bind(crawl_id)
            .fetch_one(pool)
            .await
            .map_err(|err| AppError::Internal(err.into()))?;
    let pages = sqlx::query_as(
        "SELECT id, crawl_id, url, depth, in_sitemap, status_code, content_type, fetch_error, needed_javascript
         FROM pages WHERE workspace_id = $1 AND crawl_id = $2
         ORDER BY created_at LIMIT $3 OFFSET $4",
    )
    .bind(workspace_id)
    .bind(crawl_id)
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await
    .map_err(|err| AppError::Internal(err.into()))?;
    Ok((pages, total))
}

pub async fn list_crawls_page(
    pool: &PgPool,
    workspace_id: Uuid,
    site_id: Uuid,
    limit: i64,
    offset: i64,
) -> Result<(Vec<Crawl>, i64), AppError> {
    let total: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM crawls WHERE workspace_id = $1 AND site_id = $2")
            .bind(workspace_id)
            .bind(site_id)
            .fetch_one(pool)
            .await
            .map_err(|err| AppError::Internal(err.into()))?;
    let crawls = sqlx::query_as(
        "SELECT id, site_id, status, trigger, stop_reason, (robots_txt IS NOT NULL) AS robots_txt_found, sitemap_found, llms_txt_found,
                pages_crawled, pages_failed, error, score, html_pages
         FROM crawls WHERE workspace_id = $1 AND site_id = $2
         ORDER BY created_at DESC LIMIT $3 OFFSET $4",
    )
    .bind(workspace_id)
    .bind(site_id)
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await
    .map_err(|err| AppError::Internal(err.into()))?;
    Ok((crawls, total))
}

pub async fn stored_links(
    pool: &PgPool,
    workspace_id: Uuid,
    crawl_id: Uuid,
) -> Result<Vec<(String, i32, bool)>, AppError> {
    let rows: Vec<(String, i32, bool, Option<Value>)> = sqlx::query_as(
        "SELECT url, depth, in_sitemap, facts FROM pages WHERE workspace_id = $1 AND crawl_id = $2",
    )
    .bind(workspace_id)
    .bind(crawl_id)
    .fetch_all(pool)
    .await
    .map_err(|err| AppError::Internal(err.into()))?;
    let mut links = Vec::new();
    for (url, depth, in_sitemap, facts) in rows {
        links.push((url, depth, in_sitemap));
        if let Some(list) = facts
            .as_ref()
            .and_then(|facts| facts.get("links"))
            .and_then(|v| v.as_array())
        {
            for link in list {
                if let Some(href) = link.get("url").and_then(|v| v.as_str()) {
                    links.push((href.to_string(), depth + 1, false));
                }
            }
        }
    }
    Ok(links)
}

/// The most recent crawl of a site, whatever its state.
pub async fn latest_crawl(
    pool: &PgPool,
    workspace_id: Uuid,
    site_id: Uuid,
) -> Result<Option<Crawl>, AppError> {
    latest_where(pool, workspace_id, site_id, false).await
}

/// The most recent crawl of a site that finished as `completed`.
pub async fn latest_completed(
    pool: &PgPool,
    workspace_id: Uuid,
    site_id: Uuid,
) -> Result<Option<Crawl>, AppError> {
    latest_where(pool, workspace_id, site_id, true).await
}

async fn latest_where(
    pool: &PgPool,
    workspace_id: Uuid,
    site_id: Uuid,
    completed_only: bool,
) -> Result<Option<Crawl>, AppError> {
    sqlx::query_as(
        "SELECT id, site_id, status, trigger, stop_reason, (robots_txt IS NOT NULL) AS robots_txt_found, sitemap_found, llms_txt_found,
                pages_crawled, pages_failed, error, score, html_pages
         FROM crawls
         WHERE workspace_id = $1 AND site_id = $2 AND (NOT $3 OR status = 'completed')
         ORDER BY created_at DESC LIMIT 1",
    )
    .bind(workspace_id)
    .bind(site_id)
    .bind(completed_only)
    .fetch_optional(pool)
    .await
    .map_err(|err| AppError::Internal(err.into()))
}

/// Keeps a message on a crawl without changing how it ended.
pub async fn note_error(
    pool: &PgPool,
    workspace_id: Uuid,
    id: Uuid,
    message: &str,
) -> Result<(), AppError> {
    sqlx::query("UPDATE crawls SET error = $3 WHERE id = $1 AND workspace_id = $2")
        .bind(id)
        .bind(workspace_id)
        .bind(strip_nul(message).as_ref())
        .execute(pool)
        .await
        .map_err(|err| AppError::Internal(err.into()))?;
    Ok(())
}

type PageRow = (
    String,
    i32,
    bool,
    Option<i32>,
    Option<String>,
    Option<String>,
    Value,
    Option<i32>,
    Option<i32>,
    bool,
    Option<String>,
    bool,
    Option<Value>,
    Option<String>,
);

/// Everything the checks need from one crawl, rebuilt from what was stored.
pub async fn load_context(
    pool: &PgPool,
    workspace_id: Uuid,
    crawl_id: Uuid,
) -> Result<CrawlContext, AppError> {
    let (root, robots_txt, sitemap_found, site_id, site_host): (
        String,
        Option<String>,
        bool,
        Uuid,
        String,
    ) = sqlx::query_as(
        "SELECT s.root_url, c.robots_txt, c.sitemap_found, c.site_id, s.host
         FROM crawls c JOIN sites s ON s.id = c.site_id AND s.workspace_id = c.workspace_id
         WHERE c.id = $1 AND c.workspace_id = $2",
    )
    .bind(crawl_id)
    .bind(workspace_id)
    .fetch_optional(pool)
    .await
    .map_err(|err| AppError::Internal(err.into()))?
    .ok_or(AppError::NotFound)?;
    let rows: Vec<PageRow> = sqlx::query_as(
        "SELECT url, depth, in_sitemap, status_code, content_type, fetch_error, redirects,
                response_ms, bytes, is_https, x_robots_tag, needed_javascript, facts, text
         FROM pages WHERE workspace_id = $1 AND crawl_id = $2 ORDER BY created_at, url",
    )
    .bind(workspace_id)
    .bind(crawl_id)
    .fetch_all(pool)
    .await
    .map_err(|err| AppError::Internal(err.into()))?;
    Ok(CrawlContext {
        root: url::Url::parse(&root).map_err(|err| AppError::Internal(err.into()))?,
        robots_txt,
        sitemap_found,
        pages: rows.into_iter().map(page_view).collect(),
        keywords: keywords::targets(pool, workspace_id, site_id).await?,
        search: crate::integrations::google::search::load_search_data(pool, workspace_id, site_id)
            .await?,
        ai: crate::ai::answers::load_ai_data(pool, workspace_id, site_id).await?,
        speed: crate::speed::repo::load_speed_data(pool, workspace_id, site_id).await?,
        rankings: crate::rankings::repo::load_ranking_data(pool, workspace_id, site_id).await?,
        site_host,
    })
}

fn page_view(row: PageRow) -> PageView {
    let (url, depth, in_sitemap, status_code, content_type, fetch_error, redirects) =
        (row.0, row.1, row.2, row.3, row.4, row.5, row.6);
    let facts = row
        .12
        .and_then(|value| match serde_json::from_value::<PageFacts>(value) {
            Ok(mut facts) => {
                if let Some(text) = &row.13 {
                    facts.text = text.clone();
                }
                Some(facts)
            }
            Err(err) => {
                tracing::warn!(%url, error = %err, "stored page facts could not be read");
                None
            }
        });
    PageView {
        redirects: serde_json::from_value(redirects).unwrap_or_default(),
        url,
        depth,
        in_sitemap,
        status_code,
        content_type,
        fetch_error,
        response_ms: row.7,
        bytes: row.8,
        is_https: row.9,
        x_robots_tag: row.10,
        needed_javascript: row.11,
        facts,
    }
}
