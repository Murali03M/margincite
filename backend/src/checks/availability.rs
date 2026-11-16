//! What a site has, which decides which checks can run for it.

use sqlx::PgExecutor;
use uuid::Uuid;

use super::catalog::Available;
use crate::error::AppError;

/// Reads what the site has right now: target keywords, synced Search Console
/// data, and a completed AI visibility run.
pub async fn load<'e>(
    db: impl PgExecutor<'e>,
    workspace_id: Uuid,
    site_id: Uuid,
) -> Result<Available, AppError> {
    let (keywords, search_console, ai_run, speed, rankings): (bool, bool, bool, bool, bool) = sqlx::query_as(concat!(
        "SELECT EXISTS(SELECT 1 FROM site_keywords WHERE workspace_id = $1 AND site_id = $2), ",
        crate::search_console_has_data_sql!(),
        ", EXISTS(SELECT 1 FROM ai_runs WHERE workspace_id = $1 AND site_id = $2 AND status = 'completed'), ",
        "EXISTS(SELECT 1 FROM speed_results WHERE workspace_id = $1 AND site_id = $2 AND error IS NULL), ",
        "EXISTS(SELECT 1 FROM rank_checks r JOIN sites s ON s.id = r.site_id ",
        "WHERE r.workspace_id = $1 AND r.site_id = $2 AND r.country = s.country ",
        "AND r.language = s.language AND r.device = s.device)"
    ))
    .bind(workspace_id)
    .bind(site_id)
    .fetch_one(db)
    .await
    .map_err(|err| AppError::Internal(err.into()))?;
    Ok(Available {
        keywords,
        search_console,
        ai_run,
        speed,
        rankings,
    })
}
