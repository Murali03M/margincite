//! How much of a site a suggestion touches. Without Search Console that is
//! the share of working pages; with it, the larger of that and the share of
//! the site's search impressions that land on the pages with open items. One
//! place, so the ranking and the score always agree.

use std::collections::HashMap;

use sqlx::PgConnection;
use uuid::Uuid;

use super::rank;
use crate::checks::TargetKind;
use crate::error::AppError;

fn internal(err: sqlx::Error) -> AppError {
    AppError::Internal(err.into())
}

/// Whether an item's page is in its evidence (`url`) rather than its target,
/// which for these checks reads `"keyword" on <address>`.
pub fn page_in_evidence(check_id: &str) -> bool {
    check_id.starts_with("keyword_") || check_id == "search_striking_distance"
}

/// What one site needs to work out reach by impressions.
pub struct SiteReach {
    /// Impressions per page; `None` when the site has no Search Console data.
    impressions: Option<HashMap<String, i64>>,
    /// The pages of each check's open page-level items.
    open_pages: HashMap<String, Vec<String>>,
}

impl SiteReach {
    pub async fn load(
        conn: &mut PgConnection,
        workspace_id: Uuid,
        site_id: Uuid,
    ) -> Result<SiteReach, AppError> {
        let has_data: bool =
            sqlx::query_scalar(concat!("SELECT ", crate::search_console_has_data_sql!()))
                .bind(workspace_id)
                .bind(site_id)
                .fetch_one(&mut *conn)
                .await
                .map_err(internal)?;
        if !has_data {
            return Ok(SiteReach {
                impressions: None,
                open_pages: HashMap::new(),
            });
        }
        let impressions: Vec<(String, i64)> = sqlx::query_as(
            "SELECT page, SUM(impressions)::bigint FROM search_rows
             WHERE workspace_id = $1 AND site_id = $2 GROUP BY page",
        )
        .bind(workspace_id)
        .bind(site_id)
        .fetch_all(&mut *conn)
        .await
        .map_err(internal)?;
        let items: Vec<(String, String, Option<String>)> = sqlx::query_as(
            "SELECT s.check_id, i.target, i.evidence->>'url'
             FROM suggestions s JOIN suggestion_items i ON i.suggestion_id = s.id
             WHERE s.workspace_id = $1 AND s.site_id = $2
               AND i.workspace_id = $1 AND i.status = 'open' AND i.target_kind = 'page'",
        )
        .bind(workspace_id)
        .bind(site_id)
        .fetch_all(&mut *conn)
        .await
        .map_err(internal)?;
        let mut open_pages: HashMap<String, Vec<String>> = HashMap::new();
        for (check_id, target, url) in items {
            let page = if page_in_evidence(&check_id) {
                url.unwrap_or(target)
            } else {
                target
            };
            open_pages.entry(check_id).or_default().push(page);
        }
        Ok(SiteReach {
            impressions: Some(impressions.into_iter().collect()),
            open_pages,
        })
    }

    /// Reach counted in impressions rather than pages.
    pub fn by_impressions(&self) -> bool {
        self.impressions.is_some()
    }

    /// The share of the site this check's open items touch, 0.0 to 1.0: the
    /// share of working pages, or, with search data, the share of impressions
    /// on their pages when that is larger. Traffic can raise reach but never
    /// lower it, so a page nobody visits still counts.
    pub fn reach(&self, check_id: &str, open_items: i64, html_pages: i64, kind: TargetKind) -> f64 {
        let by_pages = rank::reach(open_items, html_pages, kind);
        if kind != TargetKind::Page {
            return by_pages;
        }
        let by_impressions = self.impressions.as_ref().zip(self.open_pages.get(check_id));
        let share = by_impressions.and_then(|(impressions, pages)| {
            rank::impression_reach(pages.iter().map(String::as_str), impressions)
        });
        share.map_or(by_pages, |share| share.max(by_pages))
    }
}
