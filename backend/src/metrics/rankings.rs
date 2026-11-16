//! Rankings: how the site's keywords stand in Google's results, from the
//! latest DataForSEO check at the site's country, language and device.

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
    "Rankings have not been checked yet. Add a DataForSEO login in Settings, then press Check now.";

#[derive(Debug, Serialize)]
pub struct Rankings {
    /// Keywords with a result at the site's location.
    pub tracked: i64,
    pub in_top_3: Option<i64>,
    pub in_top_10: Option<i64>,
    /// Over the keywords that rank at all; `null` when none does.
    pub average_position: Option<f64>,
    #[serde(with = "time::serde::rfc3339::option")]
    pub checked_at: Option<OffsetDateTime>,
    pub reason: Option<String>,
}

impl Block for Rankings {
    fn unavailable(reason: &str) -> Self {
        Rankings {
            tracked: 0,
            in_top_3: None,
            in_top_10: None,
            average_position: None,
            checked_at: None,
            reason: Some(reason.to_string()),
        }
    }
}

pub async fn load(pool: &PgPool, workspace_id: Uuid, site_id: Uuid) -> Result<Rankings, AppError> {
    let (checks, checked_at) = repo::current(pool, workspace_id, site_id).await?;
    if checks.is_empty() {
        let any = !keywords::list(pool, workspace_id, site_id)
            .await?
            .is_empty();
        return Ok(Rankings::unavailable(if any {
            NOT_CHECKED
        } else {
            NO_KEYWORDS
        }));
    }
    let positions: Vec<i32> = checks.iter().filter_map(|check| check.position).collect();
    let within = |limit: i32| positions.iter().filter(|p| **p <= limit).count() as i64;
    let average = (!positions.is_empty()).then(|| {
        let sum: i64 = positions.iter().map(|p| i64::from(*p)).sum();
        (sum as f64 / positions.len() as f64 * 10.0).round() / 10.0
    });
    Ok(Rankings {
        tracked: checks.len() as i64,
        in_top_3: Some(within(3)),
        in_top_10: Some(within(10)),
        average_position: average,
        checked_at,
        reason: None,
    })
}
