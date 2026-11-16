//! Site health: the score of the latest completed crawl and how it has moved.

use serde::Serialize;
use sqlx::PgPool;
use time::OffsetDateTime;
use uuid::Uuid;

use super::Block;
use crate::error::AppError;

/// Points kept in the history.
const HISTORY_POINTS: usize = 12;

#[derive(Debug, Serialize)]
pub struct Point {
    #[serde(with = "time::serde::rfc3339")]
    pub at: OffsetDateTime,
    pub score: i32,
}

#[derive(Debug, Serialize)]
pub struct SiteHealth {
    pub score: Option<i32>,
    pub previous: Option<i32>,
    /// Oldest first.
    pub history: Vec<Point>,
    pub reason: Option<String>,
}

impl Block for SiteHealth {
    fn unavailable(reason: &str) -> Self {
        SiteHealth {
            score: None,
            previous: None,
            history: Vec::new(),
            reason: Some(reason.to_string()),
        }
    }
}

pub async fn load(
    pool: &PgPool,
    workspace_id: Uuid,
    site_id: Uuid,
) -> Result<SiteHealth, AppError> {
    // One more than the history, so the latest crawl always has a "before".
    let crawls: Vec<(Option<i32>, OffsetDateTime)> = sqlx::query_as(
        "SELECT score, COALESCE(finished_at, created_at) FROM crawls
         WHERE workspace_id = $1 AND site_id = $2 AND status = 'completed'
         ORDER BY COALESCE(finished_at, created_at) DESC, created_at DESC
         LIMIT $3",
    )
    .bind(workspace_id)
    .bind(site_id)
    .bind((HISTORY_POINTS + 1) as i64)
    .fetch_all(pool)
    .await
    .map_err(|err| AppError::Internal(err.into()))?;
    Ok(summarise(&crawls))
}

/// `crawls` is newest first.
fn summarise(crawls: &[(Option<i32>, OffsetDateTime)]) -> SiteHealth {
    let Some((latest, _)) = crawls.first() else {
        return SiteHealth::unavailable("Crawl the site to see its health.");
    };
    let previous = crawls[1..].iter().find_map(|(score, _)| *score);
    let mut history: Vec<Point> = crawls
        .iter()
        .take(HISTORY_POINTS)
        .filter_map(|(score, at)| score.map(|score| Point { at: *at, score }))
        .collect();
    history.reverse();
    SiteHealth {
        score: *latest,
        previous: latest.and(previous),
        history,
        reason: latest
            .is_none()
            .then(|| "This crawl found no working pages to score.".to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(secs: i64) -> OffsetDateTime {
        OffsetDateTime::UNIX_EPOCH + time::Duration::seconds(secs)
    }

    #[test]
    fn no_crawls_means_no_score_and_a_reason() {
        let health = summarise(&[]);
        assert_eq!(health.score, None);
        assert!(health.reason.is_some() && health.history.is_empty());
    }

    #[test]
    fn previous_is_the_next_older_score_and_history_runs_oldest_first() {
        let health = summarise(&[(Some(70), at(30)), (None, at(20)), (Some(60), at(10))]);
        assert_eq!((health.score, health.previous), (Some(70), Some(60)));
        let scores: Vec<i32> = health.history.iter().map(|p| p.score).collect();
        assert_eq!(scores, [60, 70]);
        assert!(health.reason.is_none());
    }

    #[test]
    fn a_latest_crawl_without_a_score_shows_no_change() {
        let health = summarise(&[(None, at(30)), (Some(60), at(10))]);
        assert_eq!((health.score, health.previous), (None, None));
        assert_eq!(health.history.len(), 1);
        assert!(health.reason.is_some());
    }
}
