//! Reading what Search Console synced. Every query filters by workspace.

use std::collections::HashMap;

use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;

use crate::checks::context::{QueryPage, SearchData};
use crate::checks::keywords::words;
use crate::error::AppError;

fn internal(err: sqlx::Error) -> AppError {
    AppError::Internal(err.into())
}

/// The one test for "this site has Search Console data": a `connected`
/// connection with at least one synced row. Binds `$1` workspace, `$2` site.
/// The checks, the checks list, the score and the crawl context all use it.
#[macro_export]
macro_rules! search_console_has_data_sql {
    () => {
        "EXISTS(
    SELECT 1 FROM google_connections c
    WHERE c.workspace_id = $1 AND c.site_id = $2 AND c.status = 'connected'
      AND EXISTS(SELECT 1 FROM search_rows r WHERE r.site_id = c.site_id AND r.workspace_id = c.workspace_id))"
    };
}

pub async fn has_data(pool: &PgPool, workspace_id: Uuid, site_id: Uuid) -> Result<bool, AppError> {
    sqlx::query_scalar(concat!("SELECT ", search_console_has_data_sql!()))
        .bind(workspace_id)
        .bind(site_id)
        .fetch_one(pool)
        .await
        .map_err(internal)
}

/// The synced rows as the checks read them, or `None` without data.
pub async fn load_search_data(
    pool: &PgPool,
    workspace_id: Uuid,
    site_id: Uuid,
) -> Result<Option<SearchData>, AppError> {
    if !has_data(pool, workspace_id, site_id).await? {
        return Ok(None);
    }
    let rows: Vec<(String, String, i64, i64, f64, f64)> = sqlx::query_as(
        "SELECT query, page, clicks, impressions, ctr, position FROM search_rows
         WHERE workspace_id = $1 AND site_id = $2 ORDER BY id",
    )
    .bind(workspace_id)
    .bind(site_id)
    .fetch_all(pool)
    .await
    .map_err(internal)?;
    Ok(Some(SearchData {
        rows: rows
            .into_iter()
            .map(
                |(query, page, clicks, impressions, ctr, position)| QueryPage {
                    query,
                    page,
                    clicks,
                    impressions,
                    ctr,
                    position,
                },
            )
            .collect(),
    }))
}

/// Impressions per page, for sharing out a suggestion's reach.
pub async fn page_impressions(
    pool: &PgPool,
    workspace_id: Uuid,
    site_id: Uuid,
) -> Result<HashMap<String, i64>, AppError> {
    let rows: Vec<(String, i64)> = sqlx::query_as(
        "SELECT page, SUM(impressions)::bigint FROM search_rows
         WHERE workspace_id = $1 AND site_id = $2 GROUP BY page",
    )
    .bind(workspace_id)
    .bind(site_id)
    .fetch_all(pool)
    .await
    .map_err(internal)?;
    Ok(rows.into_iter().collect())
}

/// One query's totals over every page that showed for it.
#[derive(Debug, Clone, PartialEq, Serialize, sqlx::FromRow)]
pub struct QueryStat {
    pub query: String,
    pub clicks: i64,
    pub impressions: i64,
    /// Average position weighted by impressions.
    pub position: f64,
}

/// Every query with its totals, most shown first.
pub async fn query_stats(
    pool: &PgPool,
    workspace_id: Uuid,
    site_id: Uuid,
) -> Result<Vec<QueryStat>, AppError> {
    sqlx::query_as(
        "SELECT query, SUM(clicks)::bigint AS clicks, SUM(impressions)::bigint AS impressions,
                (SUM(position * impressions) / NULLIF(SUM(impressions), 0))::float8 AS position
         FROM search_rows WHERE workspace_id = $1 AND site_id = $2
         GROUP BY query HAVING SUM(impressions) > 0
         ORDER BY impressions DESC, query",
    )
    .bind(workspace_id)
    .bind(site_id)
    .fetch_all(pool)
    .await
    .map_err(internal)
}

/// What Search Console shows for one target keyword.
#[derive(Debug, Clone, PartialEq)]
pub struct Shown {
    pub clicks: i64,
    pub impressions: i64,
    /// Impressions-weighted average position; `None` when it was never shown.
    pub position: Option<f64>,
}

/// Query totals grouped the way keywords are matched: whole words, any case,
/// punctuation ignored. A keyword and a query that read the same are one.
pub struct SearchNumbers {
    by_words: HashMap<Vec<String>, (i64, i64, f64)>,
}

impl SearchNumbers {
    pub fn new(stats: Vec<QueryStat>) -> SearchNumbers {
        let mut by_words: HashMap<Vec<String>, (i64, i64, f64)> = HashMap::new();
        for stat in stats {
            let entry = by_words.entry(words(&stat.query)).or_default();
            entry.0 += stat.clicks;
            entry.1 += stat.impressions;
            entry.2 += stat.position * stat.impressions as f64;
        }
        SearchNumbers { by_words }
    }

    pub fn for_keyword(&self, keyword: &str) -> Shown {
        match self.by_words.get(&words(keyword)) {
            Some((clicks, impressions, weighted)) if *impressions > 0 => Shown {
                clicks: *clicks,
                impressions: *impressions,
                position: Some(weighted / *impressions as f64),
            },
            _ => Shown {
                clicks: 0,
                impressions: 0,
                position: None,
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuerySort {
    Clicks,
    Impressions,
    /// Best position first.
    Position,
}

impl QuerySort {
    pub fn parse(text: Option<&str>) -> Result<QuerySort, AppError> {
        match text {
            None | Some("") | Some("clicks") => Ok(QuerySort::Clicks),
            Some("impressions") => Ok(QuerySort::Impressions),
            Some("position") => Ok(QuerySort::Position),
            Some(_) => Err(AppError::Invalid(
                "sort must be clicks, impressions or position.".into(),
            )),
        }
    }

    fn order_by(self) -> &'static str {
        match self {
            QuerySort::Clicks => "clicks DESC, impressions DESC, query",
            QuerySort::Impressions => "impressions DESC, clicks DESC, query",
            QuerySort::Position => "position ASC, impressions DESC, query",
        }
    }
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct QueryRowOut {
    pub query: String,
    pub clicks: i64,
    pub impressions: i64,
    pub ctr: f64,
    pub position: f64,
    /// The page that showed most often for this query.
    pub top_page: String,
}

pub async fn list_queries(
    pool: &PgPool,
    workspace_id: Uuid,
    site_id: Uuid,
    sort: QuerySort,
    page: i64,
    limit: i64,
) -> Result<(Vec<QueryRowOut>, i64), AppError> {
    let total: i64 = sqlx::query_scalar(
        "SELECT COUNT(DISTINCT query) FROM search_rows WHERE workspace_id = $1 AND site_id = $2",
    )
    .bind(workspace_id)
    .bind(site_id)
    .fetch_one(pool)
    .await
    .map_err(internal)?;
    let sql = format!(
        "SELECT query, SUM(clicks)::bigint AS clicks, SUM(impressions)::bigint AS impressions,
                COALESCE(SUM(clicks)::float8 / NULLIF(SUM(impressions), 0), 0)::float8 AS ctr,
                COALESCE(SUM(position * impressions) / NULLIF(SUM(impressions), 0), 0)::float8 AS position,
                (ARRAY_AGG(page ORDER BY impressions DESC, page))[1] AS top_page
         FROM search_rows WHERE workspace_id = $1 AND site_id = $2
         GROUP BY query ORDER BY {} LIMIT $3 OFFSET $4",
        sort.order_by()
    );
    // The only dynamic part is `order_by`, one of three fixed strings.
    let rows = sqlx::query_as(sqlx::AssertSqlSafe(sql))
        .bind(workspace_id)
        .bind(site_id)
        .bind(limit)
        .bind((page.max(1) - 1) * limit)
        .fetch_all(pool)
        .await
        .map_err(internal)?;
    Ok((rows, total))
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct PageRowOut {
    pub url: String,
    pub clicks: i64,
    pub impressions: i64,
    pub ctr: f64,
    pub position: f64,
}

pub async fn list_pages(
    pool: &PgPool,
    workspace_id: Uuid,
    site_id: Uuid,
    page: i64,
    limit: i64,
) -> Result<(Vec<PageRowOut>, i64), AppError> {
    let total: i64 = sqlx::query_scalar(
        "SELECT COUNT(DISTINCT page) FROM search_rows WHERE workspace_id = $1 AND site_id = $2",
    )
    .bind(workspace_id)
    .bind(site_id)
    .fetch_one(pool)
    .await
    .map_err(internal)?;
    let rows = sqlx::query_as(
        "SELECT page AS url, SUM(clicks)::bigint AS clicks, SUM(impressions)::bigint AS impressions,
                COALESCE(SUM(clicks)::float8 / NULLIF(SUM(impressions), 0), 0)::float8 AS ctr,
                COALESCE(SUM(position * impressions) / NULLIF(SUM(impressions), 0), 0)::float8 AS position
         FROM search_rows WHERE workspace_id = $1 AND site_id = $2
         GROUP BY page ORDER BY impressions DESC, clicks DESC, page LIMIT $3 OFFSET $4",
    )
    .bind(workspace_id)
    .bind(site_id)
    .bind(limit)
    .bind((page.max(1) - 1) * limit)
    .fetch_all(pool)
    .await
    .map_err(internal)?;
    Ok((rows, total))
}
