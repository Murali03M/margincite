//! Checks that read the site's search results from DataForSEO. They run only
//! when a keyword has been checked.

use serde_json::json;

use super::context::{CrawlContext, Finding};
use crate::crawl::urls::normalize;

/// Places lost, since the check before, that count as a drop.
const DROP_PLACES: i32 = 5;

/// The page a finding about this keyword points at: the site's page that
/// ranks, else the home page. The suggestions list settles the finding when
/// that page has been read again and the finding is gone.
fn page(ctx: &CrawlContext, url: Option<&String>) -> String {
    url.cloned()
        .unwrap_or_else(|| normalize(&ctx.root).to_string())
}

fn target(keyword: &str) -> String {
    format!("\"{keyword}\" in Google results")
}

pub fn run(ctx: &CrawlContext) -> Vec<Finding> {
    let Some(rankings) = ctx.rankings.as_ref() else {
        return Vec::new();
    };
    let mut found = Vec::new();
    for keyword in &rankings.keywords {
        if dropped(keyword.position, keyword.previous) {
            found.push(Finding::page(
                "rank_dropped",
                &target(&keyword.keyword),
                json!({
                    "keyword": keyword.keyword,
                    "url": page(ctx, keyword.url.as_ref()),
                    "position": keyword.position,
                    "previous": keyword.previous.flatten(),
                }),
            ));
        }
        // An overview that names no sources cites no one else either.
        if keyword.ai_overview == "present" && !keyword.cited_domains.is_empty() {
            found.push(Finding::page(
                "ai_overview_not_cited",
                &target(&keyword.keyword),
                json!({
                    "keyword": keyword.keyword,
                    "url": page(ctx, keyword.url.as_ref()),
                    "cited_domains": keyword.cited_domains,
                }),
            ));
        }
    }
    found
}

/// Fell by `DROP_PLACES` or more, or left the top 100, since the check before.
/// With no check before there is nothing to fall from.
fn dropped(position: Option<i32>, previous: Option<Option<i32>>) -> bool {
    match (position, previous) {
        (None, Some(Some(_))) => true,
        (Some(now), Some(Some(before))) => now - before >= DROP_PLACES,
        _ => false,
    }
}

#[cfg(test)]
#[path = "rankings_tests.rs"]
mod tests;
