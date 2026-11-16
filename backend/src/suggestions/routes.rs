use axum::extract::{Path, State};
use axum::routing::get;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use super::repo::{self, Counts, ItemRow, ListFilter, SuggestionRow};
use super::Status;
use crate::auth::Principal;
use crate::checks::availability;
use crate::checks::catalog::{self, CheckDef};
use crate::checks::context::Category;
use crate::checks::store::findings_for_page;
use crate::crawl::repo::{self as crawls, Crawl};
use crate::crawl::routes::paging;
use crate::envelope::{Envelope, Meta};
use crate::error::AppError;
use crate::request::{Body, Params};
use crate::sites::repo::{self as sites, Site};
use crate::AppState;

const MOST_TARGETS: usize = 10_000;
const TOP_COUNT: usize = 5;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/api/v1/sites/{id}/summary", get(summary))
        .route("/api/v1/sites/{id}/checks", get(list_checks))
        .route("/api/v1/sites/{id}/suggestions", get(list_suggestions))
        .route(
            "/api/v1/suggestions/{id}",
            get(get_suggestion).patch(set_status),
        )
        .route("/api/v1/pages/{id}/findings", get(page_findings))
}

#[derive(Serialize)]
struct SuggestionOut {
    id: Uuid,
    check_id: String,
    category: Category,
    severity: u8,
    effort: u8,
    title: &'static str,
    open: i64,
    done: i64,
    dismissed: i64,
    priority: f64,
}

fn present(row: &SuggestionRow, def: &CheckDef) -> SuggestionOut {
    SuggestionOut {
        id: row.id,
        check_id: row.check_id.clone(),
        category: def.category,
        severity: def.severity,
        effort: def.effort,
        title: def.title,
        open: row.open,
        done: row.done,
        dismissed: row.dismissed,
        priority: row.priority,
    }
}

/// Rows for checks that no longer exist in the catalog are left out.
fn present_all(rows: &[SuggestionRow]) -> Vec<SuggestionOut> {
    rows.iter()
        .filter_map(|row| catalog::get(&row.check_id).map(|def| present(row, def)))
        .collect()
}

/// `None` means every status; only a known name or `all` is accepted.
fn parse_status(text: Option<&str>, default: Option<Status>) -> Result<Option<Status>, AppError> {
    match text {
        None | Some("") => Ok(default),
        Some("all") => Ok(None),
        Some(text) => Status::parse(text).map(Some).ok_or_else(|| {
            AppError::Invalid("status must be open, done, dismissed or all.".into())
        }),
    }
}

fn parse_category(text: Option<&str>) -> Result<Option<Category>, AppError> {
    match text {
        None | Some("") => Ok(None),
        Some(text) => Category::parse(text).map(Some).ok_or_else(|| {
            AppError::Invalid("category must be technical, ai_readiness or content.".into())
        }),
    }
}

#[derive(Deserialize)]
struct ListQuery {
    status: Option<String>,
    category: Option<String>,
    page: Option<i64>,
    limit: Option<i64>,
}

async fn list_suggestions(
    State(state): State<AppState>,
    principal: Principal,
    Path(site_id): Path<Uuid>,
    Params(query): Params<ListQuery>,
) -> Result<Json<Envelope<Vec<SuggestionOut>>>, AppError> {
    let ws = principal.workspace_id;
    sites::get(&state.pool, ws, site_id).await?;
    let filter = ListFilter {
        status: parse_status(query.status.as_deref(), Some(Status::Open))?,
        category: parse_category(query.category.as_deref())?,
    };
    let (page, limit) = paging(query.page, query.limit);
    let (rows, total) = repo::list(&state.pool, ws, site_id, filter, page, limit).await?;
    let meta = Meta { total, page, limit };
    Ok(Json(Envelope::page(present_all(&rows), meta)))
}

#[derive(Serialize)]
struct CategoryCount {
    category: Category,
    open: i64,
}

#[derive(Serialize)]
struct Info {
    pages_crawled: i32,
    html_pages: i32,
    sitemap_found: bool,
    llms_txt_found: bool,
    reach_basis: &'static str,
}

#[derive(Serialize)]
struct Summary {
    site: Site,
    latest_crawl: Option<Crawl>,
    score: Option<i32>,
    counts: Counts,
    by_category: Vec<CategoryCount>,
    top: Vec<SuggestionOut>,
    info: Info,
}

async fn summary(
    State(state): State<AppState>,
    principal: Principal,
    Path(site_id): Path<Uuid>,
) -> Result<Json<Envelope<Summary>>, AppError> {
    let (pool, ws) = (&state.pool, principal.workspace_id);
    let site = sites::get(pool, ws, site_id).await?;
    let latest_crawl = crawls::latest_crawl(pool, ws, site_id).await?;
    let finished = crawls::latest_completed(pool, ws, site_id).await?;
    let counts = repo::counts(pool, ws, site_id).await?;
    let available = availability::load(pool, ws, site_id).await?;
    let everything = ListFilter::default();
    let (rows, _) = repo::list(pool, ws, site_id, everything, 1, i64::MAX / 2).await?;
    let all = present_all(&rows);
    let by_category = Category::ALL
        .into_iter()
        .map(|category| CategoryCount {
            category,
            open: all
                .iter()
                .filter(|s| s.category == category)
                .map(|s| s.open)
                .sum(),
        })
        .collect();
    let top = all
        .into_iter()
        .filter(|s| s.open > 0)
        .take(TOP_COUNT)
        .collect();
    let info = Info {
        pages_crawled: finished.as_ref().map_or(0, |c| c.pages_crawled),
        html_pages: finished.as_ref().map_or(0, |c| c.html_pages),
        sitemap_found: finished.as_ref().is_some_and(|c| c.sitemap_found),
        llms_txt_found: finished.as_ref().is_some_and(|c| c.llms_txt_found),
        reach_basis: if available.search_console {
            "impressions"
        } else {
            "pages"
        },
    };
    Ok(Json(Envelope::ok(Summary {
        site,
        latest_crawl,
        score: finished.and_then(|c| c.score),
        counts,
        by_category,
        top,
        info,
    })))
}

#[derive(Deserialize)]
struct ItemsQuery {
    status: Option<String>,
    page: Option<i64>,
    limit: Option<i64>,
}

#[derive(Serialize)]
struct SuggestionDetail {
    suggestion: SuggestionOut,
    why: &'static str,
    fix: &'static str,
    items: Vec<ItemRow>,
}

async fn get_suggestion(
    State(state): State<AppState>,
    principal: Principal,
    Path(id): Path<Uuid>,
    Params(query): Params<ItemsQuery>,
) -> Result<Json<Envelope<SuggestionDetail>>, AppError> {
    let ws = principal.workspace_id;
    let row = repo::get(&state.pool, ws, id).await?;
    let def = catalog::get(&row.check_id).ok_or(AppError::NotFound)?;
    let status = parse_status(query.status.as_deref(), None)?;
    let (page, limit) = paging(query.page, query.limit);
    let (items, total) = repo::items(&state.pool, ws, id, status, page, limit).await?;
    let detail = SuggestionDetail {
        suggestion: present(&row, def),
        why: def.why,
        fix: def.fix,
        items,
    };
    Ok(Json(Envelope::page(detail, Meta { total, page, limit })))
}

#[derive(Deserialize)]
struct StatusBody {
    status: String,
    targets: Option<Vec<String>>,
}

#[derive(Serialize)]
struct Changed {
    changed: u64,
}

async fn set_status(
    State(state): State<AppState>,
    principal: Principal,
    Path(id): Path<Uuid>,
    Body(body): Body<StatusBody>,
) -> Result<Json<Envelope<Changed>>, AppError> {
    let status = Status::parse(&body.status)
        .ok_or_else(|| AppError::Invalid("status must be open, done or dismissed.".into()))?;
    if let Some(targets) = &body.targets {
        if targets.is_empty() || targets.len() > MOST_TARGETS {
            return Err(AppError::Invalid(
                "targets must list between 1 and 10000 addresses, or be left out.".into(),
            ));
        }
    }
    let changed = repo::set_status(
        &state.pool,
        principal.workspace_id,
        id,
        status,
        body.targets.as_deref(),
    )
    .await?;
    Ok(Json(Envelope::ok(Changed { changed })))
}

#[derive(Serialize)]
struct PageFinding {
    check_id: String,
    category: Category,
    severity: u8,
    title: &'static str,
    evidence: Value,
}

async fn page_findings(
    State(state): State<AppState>,
    principal: Principal,
    Path(id): Path<Uuid>,
) -> Result<Json<Envelope<Vec<PageFinding>>>, AppError> {
    let ws = principal.workspace_id;
    let page = crawls::get_page(&state.pool, ws, id).await?;
    let found = findings_for_page(&state.pool, ws, page.crawl_id, &page.url).await?;
    let mut out: Vec<PageFinding> = found
        .into_iter()
        .filter_map(|(check_id, evidence)| {
            let def = catalog::get(&check_id)?;
            Some(PageFinding {
                category: def.category,
                severity: def.severity,
                title: def.title,
                check_id,
                evidence,
            })
        })
        .collect();
    out.sort_by(|a, b| {
        b.severity
            .cmp(&a.severity)
            .then_with(|| a.check_id.cmp(&b.check_id))
    });
    Ok(Json(Envelope::ok(out)))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
enum CheckStatus {
    Passed,
    Failed,
    NotRun,
}

#[derive(Serialize)]
struct CheckOut {
    check_id: &'static str,
    title: &'static str,
    category: Category,
    status: CheckStatus,
    open: i64,
    reason: Option<&'static str>,
}

#[derive(Serialize)]
struct ChecksOut {
    crawl_id: Option<Uuid>,
    checks: Vec<CheckOut>,
}

const NO_CRAWL_REASON: &str = "Crawl the site to run this check.";

/// One entry per check in the catalog. `not_run` when the site lacks what the
/// check needs, or has no finished crawl yet; `failed` when it has open items;
/// otherwise `passed`.
async fn list_checks(
    State(state): State<AppState>,
    principal: Principal,
    Path(site_id): Path<Uuid>,
) -> Result<Json<Envelope<ChecksOut>>, AppError> {
    let (pool, ws) = (&state.pool, principal.workspace_id);
    sites::get(pool, ws, site_id).await?;
    let crawl = crawls::latest_completed(pool, ws, site_id).await?;
    let available = availability::load(pool, ws, site_id).await?;
    let (rows, _) = repo::list(pool, ws, site_id, ListFilter::default(), 1, i64::MAX / 2).await?;
    let checks = catalog::all()
        .iter()
        .map(|def| {
            let open = rows
                .iter()
                .find(|row| row.check_id == def.id)
                .map_or(0, |row| row.open);
            let missing = def
                .not_run_reason(available)
                .or(crawl.is_none().then_some(NO_CRAWL_REASON));
            let status = match (missing, open) {
                (Some(_), _) => CheckStatus::NotRun,
                (None, 0) => CheckStatus::Passed,
                (None, _) => CheckStatus::Failed,
            };
            CheckOut {
                check_id: def.id,
                title: def.title,
                category: def.category,
                status,
                open: if status == CheckStatus::NotRun {
                    0
                } else {
                    open
                },
                reason: missing,
            }
        })
        .collect();
    Ok(Json(Envelope::ok(ChecksOut {
        crawl_id: crawl.map(|c| c.id),
        checks,
    })))
}
