//! AI Overviews: for how many tracked keywords Google showed one, and for how
//! many it cited the site. A sample from the latest check, not a measure.

use serde::Serialize;
use sqlx::PgPool;
use time::OffsetDateTime;
use uuid::Uuid;

use super::Block;
use crate::error::AppError;
use crate::keywords::repo as keywords;
use crate::rankings::repo;

const NO_KEYWORDS: &str = "Add target keywords to track rankings.";
const NOT_CHECKED: &str =
    "AI Overviews have not been checked yet. They are read with each rankings check, which needs a DataForSEO login in Settings.";

#[derive(Debug, Serialize)]
pub struct AiOverviews {
    pub tracked: i64,
    /// Keywords whose results had an AI Overview, whether or not it cited the site.
    pub present: Option<i64>,
    pub cited: Option<i64>,
    #[serde(with = "time::serde::rfc3339::option")]
    pub checked_at: Option<OffsetDateTime>,
    pub reason: Option<String>,
}

impl Block for AiOverviews {
    fn unavailable(reason: &str) -> Self {
        AiOverviews {
            tracked: 0,
            present: None,
            cited: None,
            checked_at: None,
            reason: Some(reason.to_string()),
        }
    }
}

pub async fn load(
    pool: &PgPool,
    workspace_id: Uuid,
    site_id: Uuid,
) -> Result<AiOverviews, AppError> {
    let (checks, checked_at) = repo::current(pool, workspace_id, site_id).await?;
    if checks.is_empty() {
        let any = !keywords::list(pool, workspace_id, site_id)
            .await?
            .is_empty();
        return Ok(AiOverviews::unavailable(if any {
            NOT_CHECKED
        } else {
            NO_KEYWORDS
        }));
    }
    let count = |statuses: &[&str]| {
        checks
            .iter()
            .filter(|check| statuses.contains(&check.ai_overview.as_str()))
            .count() as i64
    };
    Ok(AiOverviews {
        tracked: checks.len() as i64,
        present: Some(count(&["present", "cited"])),
        cited: Some(count(&["cited"])),
        checked_at,
        reason: None,
    })
}
