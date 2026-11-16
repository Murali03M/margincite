//! AI visibility, from the latest completed run: the share of answers that
//! mention or cite the site, counting only answers that have no error.

use serde::Serialize;
use sqlx::PgPool;
use time::OffsetDateTime;
use uuid::Uuid;

use super::Block;
use crate::ai::answers::{self, EngineTally};
use crate::ai::repo;
use crate::error::AppError;

const NO_RUN: &str = "Run an AI visibility check to see this.";
const LAST_FAILED: &str =
    "The last AI visibility check could not get an answer from any engine. Run it again.";

#[derive(Debug, Serialize)]
pub struct EngineShare {
    pub engine: String,
    pub percent: f64,
    pub mentions: i64,
    pub answers: i64,
}

#[derive(Debug, Serialize)]
pub struct AiVisibility {
    pub percent: Option<f64>,
    pub mentions: Option<i64>,
    pub citations: Option<i64>,
    pub answers: i64,
    #[serde(with = "time::serde::rfc3339::option")]
    pub last_run_at: Option<OffsetDateTime>,
    pub engines: Vec<EngineShare>,
    pub reason: Option<String>,
}

impl Block for AiVisibility {
    fn unavailable(reason: &str) -> Self {
        AiVisibility {
            percent: None,
            mentions: None,
            citations: None,
            answers: 0,
            last_run_at: None,
            engines: Vec::new(),
            reason: Some(reason.to_string()),
        }
    }
}

/// One decimal place, like the other percentages on the card.
fn percent(part: i64, whole: i64) -> f64 {
    if whole <= 0 {
        return 0.0;
    }
    (part as f64 / whole as f64 * 1000.0).round() / 10.0
}

pub async fn load(
    pool: &PgPool,
    workspace_id: Uuid,
    site_id: Uuid,
) -> Result<AiVisibility, AppError> {
    let Some(run) = repo::latest_completed(pool, workspace_id, site_id).await? else {
        let failed = repo::latest_status(pool, workspace_id, site_id).await?;
        let reason = if failed.as_deref() == Some("failed") {
            LAST_FAILED
        } else {
            NO_RUN
        };
        return Ok(AiVisibility::unavailable(reason));
    };
    let tallies = answers::tally(pool, workspace_id, run.id).await?;
    let sum = |pick: fn(&EngineTally) -> i64| tallies.iter().map(pick).sum::<i64>();
    let answered = sum(|t| t.answers);
    if answered == 0 {
        return Ok(AiVisibility::unavailable(LAST_FAILED));
    }
    Ok(AiVisibility {
        percent: Some(percent(sum(|t| t.visible), answered)),
        mentions: Some(sum(|t| t.mentions)),
        citations: Some(sum(|t| t.citations)),
        answers: answered,
        last_run_at: run.finished_at,
        engines: tallies
            .iter()
            .map(|t| EngineShare {
                engine: t.engine.clone(),
                percent: percent(t.visible, t.answers),
                mentions: t.mentions,
                answers: t.answers,
            })
            .collect(),
        reason: None,
    })
}
