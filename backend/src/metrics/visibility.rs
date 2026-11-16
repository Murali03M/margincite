//! Visibility: the share of tracked keywords that rank in the top ten. From
//! the DataForSEO rankings when there are any, otherwise from Search Console.

use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;

use super::organic::{self, Stage};
use super::Block;
use crate::error::AppError;
use crate::integrations::google::search::{self, SearchNumbers};
use crate::keywords::repo as keywords;
use crate::rankings::repo;

/// A keyword counts as visible at this average position or better.
const TOP: f64 = 10.0;

#[derive(Debug, Serialize)]
pub struct Visibility {
    pub percent: Option<f64>,
    pub in_top_10: Option<i64>,
    /// How many keywords the percentage is over.
    pub keywords: i64,
    /// Where the numbers come from: `rankings` (Google's results, read by
    /// DataForSEO) or `search_console`. `null` when there are none.
    pub source: Option<&'static str>,
    pub reason: Option<String>,
}

impl Block for Visibility {
    fn unavailable(reason: &str) -> Self {
        Visibility {
            percent: None,
            in_top_10: None,
            keywords: 0,
            source: None,
            reason: Some(reason.to_string()),
        }
    }
}

pub async fn load(
    pool: &PgPool,
    workspace_id: Uuid,
    site_id: Uuid,
) -> Result<Visibility, AppError> {
    let targets = keywords::targets(pool, workspace_id, site_id).await?;
    let count = targets.len() as i64;
    let unavailable = |reason: &str| Visibility {
        keywords: count,
        ..Visibility::unavailable(reason)
    };
    if targets.is_empty() {
        return Ok(unavailable("Add target keywords to see visibility."));
    }
    let (checks, _) = repo::current(pool, workspace_id, site_id).await?;
    if !checks.is_empty() {
        let tracked = checks.len() as i64;
        let in_top_10 = checks
            .iter()
            .filter(|check| check.position.is_some_and(|p| f64::from(p) <= TOP))
            .count() as i64;
        return Ok(Visibility {
            percent: Some((in_top_10 as f64 / tracked as f64 * 1000.0).round() / 10.0),
            in_top_10: Some(in_top_10),
            keywords: tracked,
            source: Some("rankings"),
            reason: None,
        });
    }
    let stage = organic::stage(pool, workspace_id, site_id).await?;
    if let Some(reason) = stage.reason("visibility") {
        return Ok(unavailable(&reason));
    }
    debug_assert_eq!(stage, Stage::Ready);
    if !search::has_data(pool, workspace_id, site_id).await? {
        return Ok(unavailable(
            "Search Console has not reported any searches for this site yet.",
        ));
    }
    let numbers = SearchNumbers::new(search::query_stats(pool, workspace_id, site_id).await?);
    let in_top_10 = targets
        .iter()
        .filter(|target| {
            numbers
                .for_keyword(&target.keyword)
                .position
                .is_some_and(|position| position <= TOP)
        })
        .count() as i64;
    Ok(Visibility {
        percent: Some((in_top_10 as f64 / count as f64 * 1000.0).round() / 10.0),
        in_top_10: Some(in_top_10),
        keywords: count,
        source: Some("search_console"),
        reason: None,
    })
}
