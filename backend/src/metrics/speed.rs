//! Page speed: the home page's PageSpeed Insights performance score, or the
//! average of the tested pages when the home page has none.

use serde::Serialize;
use sqlx::PgPool;
use time::OffsetDateTime;
use uuid::Uuid;

use super::Block;
use crate::error::AppError;

const COULD_NOT_MEASURE: &str =
    "Page speed could not be measured on the last check. Open Page speed to see why.";
const NOT_CHECKED: &str =
    "Page speed has not been checked yet. It runs after each crawl, or press Check now.";

#[derive(Debug, Serialize)]
pub struct Speed {
    /// 0 to 100, on a phone, from Google PageSpeed Insights.
    pub performance: Option<i32>,
    #[serde(with = "time::serde::rfc3339::option")]
    pub checked_at: Option<OffsetDateTime>,
    pub reason: Option<String>,
}

impl Block for Speed {
    fn unavailable(reason: &str) -> Self {
        Speed {
            performance: None,
            checked_at: None,
            reason: Some(reason.to_string()),
        }
    }
}

/// Results exist and every one of them ends in an error.
async fn every_test_failed(
    pool: &PgPool,
    workspace_id: Uuid,
    site_id: Uuid,
) -> Result<bool, AppError> {
    sqlx::query_scalar(
        "SELECT COALESCE(bool_and(error IS NOT NULL), false) FROM speed_results
         WHERE workspace_id = $1 AND site_id = $2",
    )
    .bind(workspace_id)
    .bind(site_id)
    .fetch_one(pool)
    .await
    .map_err(|err| AppError::Internal(err.into()))
}

pub async fn load(pool: &PgPool, workspace_id: Uuid, site_id: Uuid) -> Result<Speed, AppError> {
    let rows: Vec<(bool, i32, OffsetDateTime)> = sqlx::query_as(
        "SELECT (r.url = s.root_url), r.performance, r.checked_at
         FROM speed_results r JOIN sites s ON s.id = r.site_id
         WHERE r.workspace_id = $1 AND r.site_id = $2 AND r.performance IS NOT NULL",
    )
    .bind(workspace_id)
    .bind(site_id)
    .fetch_all(pool)
    .await
    .map_err(|err| AppError::Internal(err.into()))?;
    let Some(checked_at) = rows.iter().map(|row| row.2).max() else {
        return Ok(Speed::unavailable(
            if every_test_failed(pool, workspace_id, site_id).await? {
                COULD_NOT_MEASURE
            } else {
                NOT_CHECKED
            },
        ));
    };
    let performance = match rows.iter().find(|row| row.0) {
        Some(home) => home.1,
        None => {
            let sum: i64 = rows.iter().map(|row| i64::from(row.1)).sum();
            (sum as f64 / rows.len() as f64).round() as i32
        }
    };
    Ok(Speed {
        performance: Some(performance),
        checked_at: Some(checked_at),
        reason: None,
    })
}
