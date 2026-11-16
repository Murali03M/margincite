//! Backlinks, from Bing Webmaster Tools: the sum of link counts, how many
//! pages are linked to and the five most linked.

use serde::Serialize;
use sqlx::PgPool;
use time::OffsetDateTime;
use uuid::Uuid;

use super::Block;
use crate::error::AppError;
use crate::integrations::bing::repo;

const NOT_CONNECTED: &str = "Connect Bing Webmaster Tools to see backlinks.";
const NEEDS_RECONNECTING: &str =
    "Bing no longer accepts this API key. Connect Bing Webmaster Tools again.";
const WAITING: &str = "Bing Webmaster Tools is connected. The first sync has not finished yet.";

#[derive(Debug, Serialize)]
pub struct LinkedPage {
    pub url: String,
    pub count: i64,
}

#[derive(Debug, Serialize)]
pub struct Backlinks {
    pub total: Option<i64>,
    pub linked_pages: Option<i64>,
    pub top_pages: Vec<LinkedPage>,
    pub source: &'static str,
    #[serde(with = "time::serde::rfc3339::option")]
    pub synced_at: Option<OffsetDateTime>,
    pub reason: Option<String>,
}

impl Block for Backlinks {
    fn unavailable(reason: &str) -> Self {
        Backlinks {
            total: None,
            linked_pages: None,
            top_pages: Vec::new(),
            source: "bing",
            synced_at: None,
            reason: Some(reason.to_string()),
        }
    }
}

pub async fn load(pool: &PgPool, workspace_id: Uuid, site_id: Uuid) -> Result<Backlinks, AppError> {
    let Some(connection) = repo::get_connection(pool, workspace_id, site_id).await? else {
        return Ok(Backlinks::unavailable(NOT_CONNECTED));
    };
    if connection.status != "connected" {
        return Ok(Backlinks::unavailable(NEEDS_RECONNECTING));
    }
    let Some(synced_at) = connection.synced_at else {
        return Ok(Backlinks::unavailable(WAITING));
    };
    let (total, linked_pages, top) = repo::summary(pool, workspace_id, site_id).await?;
    Ok(Backlinks {
        total: Some(total),
        linked_pages: Some(linked_pages),
        top_pages: top
            .into_iter()
            .map(|row| LinkedPage {
                url: row.target_url,
                count: row.count,
            })
            .collect(),
        source: "bing",
        synced_at: Some(synced_at),
        reason: None,
    })
}
