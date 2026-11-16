//! Organic traffic from Search Console, last 28 days against the 28 before.
//! Clicks and impressions come from the daily series, whose totals include
//! searches Google hides; the previous period is on the same basis.

use serde::Serialize;
use sqlx::PgPool;
use time::Date;
use uuid::Uuid;

use super::Block;
use crate::error::AppError;
use crate::integrations::google::repo;

const WINDOW_DAYS: i64 = 28;

#[derive(Debug, Serialize)]
pub struct Day {
    pub date: String,
    pub clicks: i64,
    pub impressions: i64,
}

#[derive(Debug, Serialize)]
pub struct Organic {
    pub clicks: Option<i64>,
    pub impressions: Option<i64>,
    pub keywords: Option<i64>,
    pub avg_position: Option<f64>,
    pub clicks_previous: Option<i64>,
    pub keywords_previous: Option<i64>,
    /// Oldest first.
    pub daily: Vec<Day>,
    pub source: &'static str,
    pub reason: Option<String>,
}

impl Block for Organic {
    fn unavailable(reason: &str) -> Self {
        Organic {
            clicks: None,
            impressions: None,
            keywords: None,
            avg_position: None,
            clicks_previous: None,
            keywords_previous: None,
            daily: Vec::new(),
            source: "search_console",
            reason: Some(reason.to_string()),
        }
    }
}

/// How far along a site is with Search Console.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    NotConnected,
    ChooseProperty,
    NeedsReconnecting,
    /// Connected, no sync has finished yet.
    Syncing,
    /// Synced at least once.
    Ready,
}

impl Stage {
    /// The sentence for a number that cannot be shown at this stage.
    pub fn reason(self, what: &str) -> Option<String> {
        match self {
            Stage::NotConnected => Some(format!("Connect Google Search Console to see {what}.")),
            Stage::ChooseProperty => {
                Some(format!("Choose a Search Console property to see {what}."))
            }
            Stage::NeedsReconnecting => {
                Some(format!("Reconnect Google Search Console to see {what}."))
            }
            Stage::Syncing => {
                Some("Google Search Console is syncing. Check back in a few minutes.".into())
            }
            Stage::Ready => None,
        }
    }
}

pub async fn stage(pool: &PgPool, workspace_id: Uuid, site_id: Uuid) -> Result<Stage, AppError> {
    let Some(connection) = repo::get_connection(pool, workspace_id, site_id).await? else {
        return Ok(Stage::NotConnected);
    };
    Ok(match connection.status.as_str() {
        "needs_reconnecting" => Stage::NeedsReconnecting,
        _ if connection.property.is_none() => Stage::ChooseProperty,
        _ if connection.synced_at.is_none() => Stage::Syncing,
        _ => Stage::Ready,
    })
}

fn internal(err: sqlx::Error) -> AppError {
    AppError::Internal(err.into())
}

pub async fn load(pool: &PgPool, workspace_id: Uuid, site_id: Uuid) -> Result<Organic, AppError> {
    let stage = stage(pool, workspace_id, site_id).await?;
    if let Some(reason) = stage.reason("organic traffic") {
        return Ok(Organic::unavailable(&reason));
    }
    let daily: Vec<(Date, i64, i64)> = sqlx::query_as(
        "SELECT date, clicks, impressions FROM search_daily
         WHERE workspace_id = $1 AND site_id = $2 ORDER BY date",
    )
    .bind(workspace_id)
    .bind(site_id)
    .fetch_all(pool)
    .await
    .map_err(internal)?;
    let (keywords, avg_position): (i64, Option<f64>) = sqlx::query_as(
        "SELECT COUNT(DISTINCT query) FILTER (WHERE impressions > 0),
                (SUM(position * impressions) / NULLIF(SUM(impressions), 0))::float8
         FROM search_rows WHERE workspace_id = $1 AND site_id = $2",
    )
    .bind(workspace_id)
    .bind(site_id)
    .fetch_one(pool)
    .await
    .map_err(internal)?;
    let previous: Option<(i64, i64)> = sqlx::query_as(
        "SELECT clicks, keywords FROM search_previous WHERE workspace_id = $1 AND site_id = $2",
    )
    .bind(workspace_id)
    .bind(site_id)
    .fetch_optional(pool)
    .await
    .map_err(internal)?;
    let (clicks, impressions) = last_window_totals(&daily);
    Ok(Organic {
        clicks: Some(clicks),
        impressions: Some(impressions),
        keywords: Some(keywords),
        avg_position,
        clicks_previous: previous.map(|(clicks, _)| clicks),
        keywords_previous: previous.map(|(_, keywords)| keywords),
        daily: daily
            .iter()
            .map(|(date, clicks, impressions)| Day {
                date: date.to_string(),
                clicks: *clicks,
                impressions: *impressions,
            })
            .collect(),
        source: "search_console",
        reason: None,
    })
}

/// Clicks and impressions over the 28 days ending on the latest day with data.
fn last_window_totals(daily: &[(Date, i64, i64)]) -> (i64, i64) {
    let Some(end) = daily.iter().map(|(date, _, _)| *date).max() else {
        return (0, 0);
    };
    let start = end - time::Duration::days(WINDOW_DAYS - 1);
    daily
        .iter()
        .filter(|(date, _, _)| *date >= start)
        .fold((0, 0), |(c, i), (_, clicks, impressions)| {
            (c + clicks, i + impressions)
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::macros::date;

    #[test]
    fn totals_cover_the_28_days_ending_on_the_latest_day() {
        let daily = [
            (date!(2026 - 09 - 01), 100, 1000),
            (date!(2026 - 10 - 15), 1, 10),
            (date!(2026 - 11 - 11), 2, 20),
        ];
        // 28 days ending 11 Nov start on 15 Oct.
        assert_eq!(last_window_totals(&daily), (3, 30));
        assert_eq!(last_window_totals(&[]), (0, 0));
    }

    #[test]
    fn each_stage_says_what_to_do_next() {
        assert!(Stage::NotConnected
            .reason("x")
            .unwrap()
            .starts_with("Connect"));
        assert!(Stage::ChooseProperty
            .reason("x")
            .unwrap()
            .starts_with("Choose"));
        assert!(Stage::NeedsReconnecting
            .reason("x")
            .unwrap()
            .starts_with("Reconnect"));
        assert!(Stage::Syncing.reason("x").unwrap().contains("syncing"));
        assert_eq!(Stage::Ready.reason("x"), None);
    }
}
