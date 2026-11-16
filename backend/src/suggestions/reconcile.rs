//! Folds one crawl's findings into the site's suggestions.

use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use super::rank;
use super::reach::SiteReach;
use crate::checks::catalog::Available;
use crate::checks::{availability, catalog, TargetKind};
use crate::error::AppError;

fn internal(err: sqlx::Error) -> AppError {
    AppError::Internal(err.into())
}

/// Applies the crawl's findings to the site's suggestion items, then stores
/// the crawl's score and working page count. One transaction.
pub async fn apply(
    pool: &PgPool,
    workspace_id: Uuid,
    site_id: Uuid,
    crawl_id: Uuid,
) -> Result<(), AppError> {
    let mut tx = pool.begin().await.map_err(internal)?;
    let known: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM crawls WHERE id = $1 AND workspace_id = $2 AND site_id = $3)",
    )
    .bind(crawl_id)
    .bind(workspace_id)
    .bind(site_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(internal)?;
    if !known {
        return Err(AppError::NotFound);
    }
    upsert_suggestions(&mut tx, workspace_id, site_id, crawl_id).await?;
    upsert_items(&mut tx, workspace_id, site_id, crawl_id).await?;
    let available = availability::load(&mut *tx, workspace_id, site_id).await?;
    resolve_missing(&mut tx, workspace_id, site_id, crawl_id, available).await?;
    store_score(&mut tx, workspace_id, site_id, crawl_id, available).await?;
    tx.commit().await.map_err(internal)
}

async fn upsert_suggestions(
    tx: &mut Transaction<'_, Postgres>,
    workspace_id: Uuid,
    site_id: Uuid,
    crawl_id: Uuid,
) -> Result<(), AppError> {
    sqlx::query(
        "INSERT INTO suggestions (id, workspace_id, site_id, check_id)
         SELECT gen_random_uuid(), $1, $2, check_id
         FROM (SELECT DISTINCT check_id FROM findings WHERE crawl_id = $3 AND workspace_id = $1) f
         ON CONFLICT (site_id, check_id) DO UPDATE SET last_seen = now()",
    )
    .bind(workspace_id)
    .bind(site_id)
    .bind(crawl_id)
    .execute(&mut **tx)
    .await
    .map_err(internal)?;
    Ok(())
}

/// New findings open an item, an open item is refreshed, a done item reopens
/// and a dismissed item stays dismissed.
async fn upsert_items(
    tx: &mut Transaction<'_, Postgres>,
    workspace_id: Uuid,
    site_id: Uuid,
    crawl_id: Uuid,
) -> Result<(), AppError> {
    sqlx::query(
        "INSERT INTO suggestion_items (id, workspace_id, suggestion_id, target, target_kind, status, evidence)
         SELECT gen_random_uuid(), $1, s.id, f.target, f.target_kind, 'open', f.evidence
         FROM findings f
         JOIN suggestions s ON s.site_id = $2 AND s.workspace_id = $1 AND s.check_id = f.check_id
         WHERE f.crawl_id = $3 AND f.workspace_id = $1
         ON CONFLICT (suggestion_id, target) DO UPDATE SET
            last_seen = now(),
            evidence = EXCLUDED.evidence,
            target_kind = EXCLUDED.target_kind,
            status = CASE WHEN suggestion_items.status = 'done' THEN 'open' ELSE suggestion_items.status END,
            resolved_at = CASE WHEN suggestion_items.status = 'done' THEN NULL ELSE suggestion_items.resolved_at END",
    )
    .bind(workspace_id)
    .bind(site_id)
    .bind(crawl_id)
    .execute(&mut **tx)
    .await
    .map_err(internal)?;
    Ok(())
}

/// An open item the crawl did not find again is fixed, but only when the crawl
/// really looked at it: the site itself, or a page it read, meaning a page
/// with no fetch error and a status below 400. A page that timed out or
/// returned an error proves nothing, so its items keep their status. A
/// keyword item's target reads `"keyword" on <address>`, so the page it needs
/// read is the address in its evidence. A `broken_page` item follows the same rule: it is done once the page reads
/// successfully, and is found again, so stays open, while it does not. The
/// ranking items (`rank_dropped`, `ai_overview_not_cited`) are named for a
/// keyword in Google's results, so they too are settled by the page in their
/// evidence.
///
/// A check that could not run in this crawl, because the site has lost what
/// it needs (Search Console, keywords, an AI run), found nothing for lack of
/// looking. Its items are left alone.
async fn resolve_missing(
    tx: &mut Transaction<'_, Postgres>,
    workspace_id: Uuid,
    site_id: Uuid,
    crawl_id: Uuid,
    available: Available,
) -> Result<(), AppError> {
    let not_run: Vec<&str> = catalog::all()
        .iter()
        .filter(|check| check.not_run_reason(available).is_some())
        .map(|check| check.id)
        .collect();
    sqlx::query(
        "UPDATE suggestion_items i
         SET status = 'done', resolved_at = now()
         FROM suggestions s
         WHERE i.suggestion_id = s.id AND s.site_id = $2 AND s.workspace_id = $1
           AND i.workspace_id = $1 AND i.status = 'open'
           AND s.check_id <> ALL($4::text[])
           AND NOT EXISTS (
               SELECT 1 FROM findings f
               WHERE f.crawl_id = $3 AND f.check_id = s.check_id AND f.target = i.target)
           AND (i.target_kind = 'site' OR EXISTS (
               SELECT 1 FROM pages p
               WHERE p.crawl_id = $3 AND p.workspace_id = $1
                 AND p.url = CASE WHEN s.check_id LIKE 'keyword\\_%'
                                      OR s.check_id IN ('search_striking_distance', 'rank_dropped',
                                                        'ai_overview_not_cited')
                                 THEN i.evidence->>'url' ELSE i.target END
                 AND p.fetch_error IS NULL AND p.status_code < 400))",
    )
    .bind(workspace_id)
    .bind(site_id)
    .bind(crawl_id)
    .bind(&not_run)
    .execute(&mut **tx)
    .await
    .map_err(internal)?;
    Ok(())
}

async fn store_score(
    tx: &mut Transaction<'_, Postgres>,
    workspace_id: Uuid,
    site_id: Uuid,
    crawl_id: Uuid,
    available: Available,
) -> Result<(), AppError> {
    let html_pages: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM pages
         WHERE crawl_id = $1 AND workspace_id = $2 AND status_code = 200 AND facts IS NOT NULL",
    )
    .bind(crawl_id)
    .bind(workspace_id)
    .fetch_one(&mut **tx)
    .await
    .map_err(internal)?;
    let open: Vec<(String, i64, String)> = sqlx::query_as(
        "SELECT s.check_id, COUNT(*), MAX(i.target_kind)
         FROM suggestions s JOIN suggestion_items i ON i.suggestion_id = s.id
         WHERE s.site_id = $1 AND s.workspace_id = $2 AND i.status = 'open'
         GROUP BY s.check_id",
    )
    .bind(site_id)
    .bind(workspace_id)
    .fetch_all(&mut **tx)
    .await
    .map_err(internal)?;
    let reach = SiteReach::load(tx, workspace_id, site_id).await?;
    let weights: Vec<(u8, f64)> = open
        .iter()
        .filter_map(|(check_id, count, kind)| {
            let def = catalog::get(check_id)?;
            // An item kept open for lack of its source is not in this site's total.
            def.not_run_reason(available).is_none().then_some(())?;
            let kind = if kind == "site" {
                TargetKind::Site
            } else {
                TargetKind::Page
            };
            Some((
                def.severity,
                reach.reach(check_id, *count, html_pages, kind),
            ))
        })
        .collect();
    let score = rank::score(&weights, catalog::severity_total(available), html_pages);
    sqlx::query(
        "UPDATE crawls SET score = $3, html_pages = $4 WHERE id = $1 AND workspace_id = $2",
    )
    .bind(crawl_id)
    .bind(workspace_id)
    .bind(score.map(i32::from))
    .bind(html_pages as i32)
    .execute(&mut **tx)
    .await
    .map_err(internal)?;
    Ok(())
}
