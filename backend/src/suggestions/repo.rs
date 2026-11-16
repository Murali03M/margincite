//! Reading and changing suggestions. Every function filters by workspace.

use serde::Serialize;
use serde_json::Value;
use sqlx::PgPool;
use time::OffsetDateTime;
use uuid::Uuid;

use std::collections::hash_map::Entry;
use std::collections::HashMap;

use super::reach::SiteReach;
use super::{rank, Status};
use crate::checks::context::Category;
use crate::checks::{catalog, TargetKind};
use crate::crawl::repo as crawls;
use crate::error::AppError;

fn internal(err: sqlx::Error) -> AppError {
    AppError::Internal(err.into())
}

/// One suggestion: a check and how many of its items are in each state.
#[derive(Debug, Clone, Serialize)]
pub struct SuggestionRow {
    pub id: Uuid,
    pub site_id: Uuid,
    pub check_id: String,
    pub open: i64,
    pub done: i64,
    pub dismissed: i64,
    pub priority: f64,
    pub reach: f64,
    #[serde(with = "time::serde::rfc3339")]
    pub first_seen: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub last_seen: OffsetDateTime,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ListFilter {
    pub status: Option<Status>,
    pub category: Option<Category>,
}

impl ListFilter {
    fn keeps(&self, row: &SuggestionRow) -> bool {
        let status = match self.status {
            None => true,
            Some(Status::Open) => row.open > 0,
            Some(Status::Done) => row.open == 0,
            Some(Status::Dismissed) => row.dismissed > 0,
        };
        let category = self.category.is_none_or(|wanted| {
            catalog::get(&row.check_id).is_some_and(|def| def.category == wanted)
        });
        status && category
    }
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct ItemRow {
    pub target: String,
    pub target_kind: String,
    pub status: String,
    pub evidence: Value,
    #[serde(with = "time::serde::rfc3339")]
    pub first_seen: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub last_seen: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339::option")]
    pub resolved_at: Option<OffsetDateTime>,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
pub struct Counts {
    pub open: i64,
    pub done: i64,
    pub dismissed: i64,
}

type GroupRow = (
    Uuid,
    Uuid,
    String,
    OffsetDateTime,
    OffsetDateTime,
    i64,
    i64,
    i64,
    Option<String>,
);

/// Groups for a site (or one group), with their item counts. Ranking needs the
/// working page count of the site's latest finished crawl.
async fn groups(
    pool: &PgPool,
    workspace_id: Uuid,
    site_id: Option<Uuid>,
    id: Option<Uuid>,
) -> Result<Vec<SuggestionRow>, AppError> {
    let rows: Vec<GroupRow> = sqlx::query_as(
        "SELECT s.id, s.site_id, s.check_id, s.first_seen, s.last_seen,
                COUNT(i.id) FILTER (WHERE i.status = 'open'),
                COUNT(i.id) FILTER (WHERE i.status = 'done'),
                COUNT(i.id) FILTER (WHERE i.status = 'dismissed'),
                MAX(i.target_kind)
         FROM suggestions s LEFT JOIN suggestion_items i ON i.suggestion_id = s.id
         WHERE s.workspace_id = $1
           AND ($2::uuid IS NULL OR s.site_id = $2)
           AND ($3::uuid IS NULL OR s.id = $3)
         GROUP BY s.id",
    )
    .bind(workspace_id)
    .bind(site_id)
    .bind(id)
    .fetch_all(pool)
    .await
    .map_err(internal)?;
    let mut ranked = Vec::with_capacity(rows.len());
    let mut reaches: HashMap<Uuid, SiteReach> = HashMap::new();
    for row in rows {
        let site_id = row.1;
        let html_pages = latest_html_pages(pool, workspace_id, site_id).await?;
        let site = match reaches.entry(site_id) {
            Entry::Occupied(known) => known.into_mut(),
            Entry::Vacant(slot) => {
                let mut conn = pool.acquire().await.map_err(internal)?;
                slot.insert(SiteReach::load(&mut conn, workspace_id, site_id).await?)
            }
        };
        ranked.push(to_row(row, html_pages, site));
    }
    Ok(ranked)
}

async fn latest_html_pages(
    pool: &PgPool,
    workspace_id: Uuid,
    site_id: Uuid,
) -> Result<i64, AppError> {
    let crawl = crawls::latest_completed(pool, workspace_id, site_id).await?;
    Ok(crawl.map_or(0, |crawl| i64::from(crawl.html_pages)))
}

fn to_row(row: GroupRow, html_pages: i64, site: &SiteReach) -> SuggestionRow {
    let (id, site_id, check_id, first_seen, last_seen, open, done, dismissed, kind) = row;
    let kind = if kind.as_deref() == Some("site") {
        TargetKind::Site
    } else {
        TargetKind::Page
    };
    let reach = site.reach(&check_id, open, html_pages, kind);
    let priority =
        catalog::get(&check_id).map_or(0.0, |def| rank::priority(def.severity, def.effort, reach));
    SuggestionRow {
        id,
        site_id,
        check_id,
        open,
        done,
        dismissed,
        priority,
        reach,
        first_seen,
        last_seen,
    }
}

/// The site's suggestions, best first, then by check id.
pub async fn list(
    pool: &PgPool,
    workspace_id: Uuid,
    site_id: Uuid,
    filter: ListFilter,
    page: i64,
    limit: i64,
) -> Result<(Vec<SuggestionRow>, i64), AppError> {
    let mut rows: Vec<SuggestionRow> = groups(pool, workspace_id, Some(site_id), None)
        .await?
        .into_iter()
        .filter(|row| filter.keeps(row))
        .collect();
    rows.sort_by(|a, b| {
        b.priority
            .total_cmp(&a.priority)
            .then_with(|| a.check_id.cmp(&b.check_id))
    });
    let total = rows.len() as i64;
    let start = ((page.max(1) - 1) * limit).max(0) as usize;
    let rows = rows.into_iter().skip(start).take(limit.max(0) as usize);
    Ok((rows.collect(), total))
}

pub async fn get(pool: &PgPool, workspace_id: Uuid, id: Uuid) -> Result<SuggestionRow, AppError> {
    groups(pool, workspace_id, None, Some(id))
        .await?
        .into_iter()
        .next()
        .ok_or(AppError::NotFound)
}

pub async fn items(
    pool: &PgPool,
    workspace_id: Uuid,
    suggestion_id: Uuid,
    status: Option<Status>,
    page: i64,
    limit: i64,
) -> Result<(Vec<ItemRow>, i64), AppError> {
    get(pool, workspace_id, suggestion_id).await?;
    let wanted = status.map(Status::as_str);
    let total: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM suggestion_items
         WHERE workspace_id = $1 AND suggestion_id = $2 AND ($3::text IS NULL OR status = $3)",
    )
    .bind(workspace_id)
    .bind(suggestion_id)
    .bind(wanted)
    .fetch_one(pool)
    .await
    .map_err(internal)?;
    let rows = sqlx::query_as(
        "SELECT target, target_kind, status, evidence, first_seen, last_seen, resolved_at
         FROM suggestion_items
         WHERE workspace_id = $1 AND suggestion_id = $2 AND ($3::text IS NULL OR status = $3)
         ORDER BY target LIMIT $4 OFFSET $5",
    )
    .bind(workspace_id)
    .bind(suggestion_id)
    .bind(wanted)
    .bind(limit)
    .bind((page.max(1) - 1) * limit)
    .fetch_all(pool)
    .await
    .map_err(internal)?;
    Ok((rows, total))
}

/// Sets the status of the chosen targets, or of every item. Returns how many
/// items actually changed.
pub async fn set_status(
    pool: &PgPool,
    workspace_id: Uuid,
    suggestion_id: Uuid,
    status: Status,
    targets: Option<&[String]>,
) -> Result<u64, AppError> {
    get(pool, workspace_id, suggestion_id).await?;
    let result = sqlx::query(
        "UPDATE suggestion_items
         SET status = $3,
             resolved_at = CASE WHEN $3 = 'done' THEN now() ELSE NULL END
         WHERE workspace_id = $1 AND suggestion_id = $2 AND status <> $3
           AND ($4::text[] IS NULL OR target = ANY($4))",
    )
    .bind(workspace_id)
    .bind(suggestion_id)
    .bind(status.as_str())
    .bind(targets)
    .execute(pool)
    .await
    .map_err(internal)?;
    Ok(result.rows_affected())
}

/// Item totals for a whole site.
pub async fn counts(pool: &PgPool, workspace_id: Uuid, site_id: Uuid) -> Result<Counts, AppError> {
    let (open, done, dismissed): (i64, i64, i64) = sqlx::query_as(
        "SELECT COUNT(*) FILTER (WHERE i.status = 'open'),
                COUNT(*) FILTER (WHERE i.status = 'done'),
                COUNT(*) FILTER (WHERE i.status = 'dismissed')
         FROM suggestion_items i JOIN suggestions s ON s.id = i.suggestion_id
         WHERE s.workspace_id = $1 AND s.site_id = $2 AND i.workspace_id = $1",
    )
    .bind(workspace_id)
    .bind(site_id)
    .fetch_one(pool)
    .await
    .map_err(internal)?;
    Ok(Counts {
        open,
        done,
        dismissed,
    })
}
