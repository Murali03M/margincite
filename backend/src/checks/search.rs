//! Checks that read Search Console data. They run only when the site has it.

use std::collections::HashSet;

use serde_json::json;
use url::Url;

use crate::checks::context::{CrawlContext, Finding, QueryPage, SearchData};
use crate::crawl::urls::normalize;

/// The most rows a sync asks Search Console for. A sync that gets this many
/// may have been cut off.
pub const ROW_LIMIT: usize = 25_000;
/// The most "striking distance" findings kept, best shown first.
pub const STRIKING_LIMIT: usize = 200;

const LOW_CTR_MIN_IMPRESSIONS: i64 = 200;
const STRIKING_MIN_IMPRESSIONS: i64 = 100;
const STRIKING_FROM: f64 = 5.0;
const STRIKING_TO: f64 = 20.0;

/// The click-through rate a page at this average position usually gets.
pub fn expected_ctr(position: f64) -> f64 {
    match position.round().max(1.0) as u32 {
        1 => 0.28,
        2 => 0.15,
        3 => 0.10,
        4 => 0.07,
        5 => 0.05,
        6..=10 => 0.03,
        _ => 0.01,
    }
}

/// An address in the form crawled pages use; one that cannot be read is kept as it is.
fn normal(address: &str) -> String {
    Url::parse(address)
        .map(|url| normalize(&url).to_string())
        .unwrap_or_else(|_| address.to_string())
}

pub fn run(ctx: &CrawlContext) -> Vec<Finding> {
    let Some(search) = ctx.search.as_ref() else {
        return Vec::new();
    };
    let rows: Vec<QueryPage> = search
        .rows
        .iter()
        .map(|row| QueryPage {
            page: normal(&row.page),
            ..row.clone()
        })
        .collect();
    let working: HashSet<String> = ctx
        .pages
        .iter()
        .filter(|page| page.html_facts().is_some())
        .map(|page| normal(&page.url))
        .collect();
    let data = SearchData { rows };
    let mut findings = low_ctr(&data, &working);
    findings.extend(striking_distance(&data, &working));
    findings.extend(no_impressions(ctx, &data, &working));
    findings
}

fn percent(share: f64) -> f64 {
    (share * 1000.0).round() / 10.0
}

fn one_decimal(value: f64) -> f64 {
    (value * 10.0).round() / 10.0
}

fn low_ctr(data: &SearchData, working: &HashSet<String>) -> Vec<Finding> {
    data.pages()
        .into_iter()
        .filter(|(page, stats)| {
            working.contains(*page)
                && stats.impressions >= LOW_CTR_MIN_IMPRESSIONS
                && stats.ctr < expected_ctr(stats.position) / 2.0
        })
        .map(|(page, stats)| {
            Finding::page(
                "search_low_ctr",
                page,
                json!({
                    "url": page,
                    "impressions": stats.impressions,
                    "clicks": stats.clicks,
                    "ctr": percent(stats.ctr),
                    "position": one_decimal(stats.position),
                    "expected_ctr": percent(expected_ctr(stats.position)),
                }),
            )
        })
        .collect()
}

fn striking_distance(data: &SearchData, working: &HashSet<String>) -> Vec<Finding> {
    let mut near: Vec<&QueryPage> = data
        .rows
        .iter()
        .filter(|row| {
            working.contains(&row.page)
                && row.impressions >= STRIKING_MIN_IMPRESSIONS
                && (STRIKING_FROM..=STRIKING_TO).contains(&row.position)
        })
        .collect();
    near.sort_by(|a, b| {
        b.impressions
            .cmp(&a.impressions)
            .then_with(|| a.query.cmp(&b.query))
            .then_with(|| a.page.cmp(&b.page))
    });
    near.into_iter()
        .take(STRIKING_LIMIT)
        .map(|row| {
            Finding::page(
                "search_striking_distance",
                &format!("\"{}\" on {}", row.query, row.page),
                json!({
                    "query": row.query,
                    "url": row.page,
                    "position": one_decimal(row.position),
                    "impressions": row.impressions,
                    "clicks": row.clicks,
                }),
            )
        })
        .collect()
}

/// Sitemap pages with no impressions. Skipped when the data cannot be
/// trusted to say so: the sync may have been cut off at its row limit, or no
/// crawled page matches any row (often `www` against the bare host).
fn no_impressions(
    ctx: &CrawlContext,
    data: &SearchData,
    working: &HashSet<String>,
) -> Vec<Finding> {
    if data.rows.len() >= ROW_LIMIT || !data.rows.iter().any(|row| working.contains(&row.page)) {
        return Vec::new();
    }
    let shown: HashSet<&str> = data
        .rows
        .iter()
        .filter(|row| row.impressions > 0)
        .map(|row| row.page.as_str())
        .collect();
    ctx.pages
        .iter()
        .filter(|page| page.in_sitemap && page.html_facts().is_some())
        .filter(|page| !shown.contains(normal(&page.url).as_str()))
        .map(|page| {
            Finding::page(
                "search_no_impressions",
                &page.url,
                json!({ "url": page.url }),
            )
        })
        .collect()
}

#[cfg(test)]
#[path = "search_tests.rs"]
mod tests;
