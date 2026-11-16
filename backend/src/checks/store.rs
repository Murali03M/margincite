//! Saving a crawl's findings, and the step that runs after a crawl completes.

use serde_json::Value;
use sqlx::PgPool;
use uuid::Uuid;

use super::{run_all, Finding};
use crate::crawl::repo;
use crate::error::AppError;
use crate::suggestions::reconcile;

/// Replaces the crawl's findings with these, in one transaction.
pub async fn replace_findings(
    pool: &PgPool,
    workspace_id: Uuid,
    crawl_id: Uuid,
    findings: &[Finding],
) -> Result<(), AppError> {
    let internal = |err: sqlx::Error| AppError::Internal(err.into());
    let mut tx = pool.begin().await.map_err(internal)?;
    sqlx::query("DELETE FROM findings WHERE workspace_id = $1 AND crawl_id = $2")
        .bind(workspace_id)
        .bind(crawl_id)
        .execute(&mut *tx)
        .await
        .map_err(internal)?;
    let ids: Vec<Uuid> = findings.iter().map(|_| Uuid::new_v4()).collect();
    let checks: Vec<&str> = findings.iter().map(|f| f.check_id).collect();
    let targets: Vec<&str> = findings.iter().map(|f| f.target.as_str()).collect();
    let kinds: Vec<&str> = findings.iter().map(|f| f.target_kind.as_str()).collect();
    let evidence: Vec<Value> = findings.iter().map(|f| f.evidence.clone()).collect();
    sqlx::query(
        "INSERT INTO findings (id, workspace_id, crawl_id, check_id, target, target_kind, evidence)
         SELECT id, $2, $3, check_id, target, target_kind, evidence
         FROM UNNEST($1::uuid[], $4::text[], $5::text[], $6::text[], $7::jsonb[])
              AS f(id, check_id, target, target_kind, evidence)
         ON CONFLICT (crawl_id, check_id, target) DO NOTHING",
    )
    .bind(&ids)
    .bind(workspace_id)
    .bind(crawl_id)
    .bind(&checks)
    .bind(&targets)
    .bind(&kinds)
    .bind(&evidence)
    .execute(&mut *tx)
    .await
    .map_err(internal)?;
    tx.commit().await.map_err(internal)
}

/// Runs every check on a finished crawl, stores what they found and folds the
/// findings into the site's suggestions.
pub async fn run_for_crawl(
    pool: &PgPool,
    workspace_id: Uuid,
    crawl_id: Uuid,
) -> Result<(), AppError> {
    let crawl = repo::get_crawl(pool, workspace_id, crawl_id).await?;
    let ctx = repo::load_context(pool, workspace_id, crawl_id).await?;
    let findings = run_all(&ctx);
    replace_findings(pool, workspace_id, crawl_id, &findings).await?;
    reconcile::apply(pool, workspace_id, crawl.site_id, crawl_id).await
}

/// What the checks found against one page of a crawl.
pub async fn findings_for_page(
    pool: &PgPool,
    workspace_id: Uuid,
    crawl_id: Uuid,
    url: &str,
) -> Result<Vec<(String, Value)>, AppError> {
    sqlx::query_as(
        "SELECT check_id, evidence FROM findings
         WHERE workspace_id = $1 AND crawl_id = $2 AND target_kind = 'page' AND target = $3",
    )
    .bind(workspace_id)
    .bind(crawl_id)
    .bind(url)
    .fetch_all(pool)
    .await
    .map_err(|err| AppError::Internal(err.into()))
}
