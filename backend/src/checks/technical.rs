//! Technical checks: errors, titles, headings, sitemap and speed.

use std::collections::{HashMap, HashSet};

use serde_json::json;
use url::Url;

use super::context::{CrawlContext, Finding, PageView};
use crate::crawl::extract::PageFacts;

const TITLE_RANGE: (usize, usize) = (10, 65);
const DESCRIPTION_RANGE: (usize, usize) = (50, 160);
const SLOW_MS: i32 = 1500;
const HEAVY_BYTES: i32 = 2 * 1024 * 1024;
const ALT_SOURCES_SHOWN: usize = 5;

pub fn run(ctx: &CrawlContext) -> Vec<Finding> {
    let broken = broken_urls(ctx);
    [
        errors(ctx),
        broken_links(ctx, &broken),
        canonical(ctx, &broken),
        titles(ctx),
        descriptions(ctx),
        headings(ctx),
        sitemap(ctx),
        indexing(ctx),
        images(ctx),
        delivery(ctx),
    ]
    .concat()
}

fn without_fragment(url: &Url) -> String {
    let mut clean = url.clone();
    clean.set_fragment(None);
    clean.to_string()
}

fn broken_urls(ctx: &CrawlContext) -> HashSet<String> {
    ctx.pages
        .iter()
        .filter(|page| page.is_broken())
        .map(|page| page.url.clone())
        .collect()
}

pub(super) fn html(ctx: &CrawlContext) -> impl Iterator<Item = (&PageView, &PageFacts)> {
    ctx.pages
        .iter()
        .filter_map(|page| page.html_facts().map(|facts| (page, facts)))
}

fn present(text: &Option<String>) -> Option<&str> {
    text.as_deref()
        .map(str::trim)
        .filter(|text| !text.is_empty())
}

fn errors(ctx: &CrawlContext) -> Vec<Finding> {
    ctx.pages
        .iter()
        .filter(|page| page.is_broken())
        .map(|page| {
            let evidence = json!({"status": page.status_code, "error": page.fetch_error});
            Finding::page("broken_page", &page.url, evidence)
        })
        .collect()
}

fn broken_links(ctx: &CrawlContext, broken: &HashSet<String>) -> Vec<Finding> {
    html(ctx)
        .filter_map(|(page, facts)| {
            let mut hits: Vec<String> = facts
                .links
                .iter()
                .map(|link| without_fragment(&link.url))
                .filter(|url| broken.contains(url))
                .collect();
            hits.sort();
            hits.dedup();
            (!hits.is_empty())
                .then(|| Finding::page("broken_internal_link", &page.url, json!({"broken": hits})))
        })
        .collect()
}

fn canonical(ctx: &CrawlContext, broken: &HashSet<String>) -> Vec<Finding> {
    html(ctx)
        .filter_map(|(page, facts)| {
            let target = without_fragment(facts.canonical.as_ref()?);
            broken
                .contains(&target)
                .then(|| Finding::page("canonical_broken", &page.url, json!({"canonical": target})))
        })
        .collect()
}

fn length_finding(
    id: &'static str,
    page: &PageView,
    text: &str,
    (min, max): (usize, usize),
) -> Option<Finding> {
    let length = text.chars().count();
    (length < min || length > max)
        .then(|| Finding::page(id, &page.url, json!({"length": length, "text": text})))
}

fn titles(ctx: &CrawlContext) -> Vec<Finding> {
    let mut found = Vec::new();
    for (page, facts) in html(ctx) {
        match present(&facts.title) {
            None => found.push(Finding::page("title_missing", &page.url, json!({}))),
            Some(title) => found.extend(length_finding("title_length", page, title, TITLE_RANGE)),
        }
    }
    found.extend(duplicates(
        ctx,
        "title_duplicate",
        |facts| present(&facts.title),
        shared_text,
    ));
    found
}

fn descriptions(ctx: &CrawlContext) -> Vec<Finding> {
    let mut found = Vec::new();
    for (page, facts) in html(ctx) {
        match present(&facts.meta_description) {
            None => found.push(Finding::page("description_missing", &page.url, json!({}))),
            Some(text) => found.extend(length_finding(
                "description_length",
                page,
                text,
                DESCRIPTION_RANGE,
            )),
        }
    }
    found.extend(duplicates(
        ctx,
        "description_duplicate",
        |facts| present(&facts.meta_description),
        shared_text,
    ));
    found
}

/// One finding per page whose value is shared with at least one other page.
/// `evidence` is built from the shared value and the addresses of the others.
pub(super) fn duplicates<'a>(
    ctx: &'a CrawlContext,
    id: &'static str,
    value: impl Fn(&'a PageFacts) -> Option<&'a str>,
    evidence: impl Fn(&str, &[&str]) -> serde_json::Value,
) -> Vec<Finding> {
    let mut groups: HashMap<&str, Vec<&str>> = HashMap::new();
    for (page, facts) in html(ctx) {
        if let Some(text) = value(facts) {
            groups.entry(text).or_default().push(&page.url);
        }
    }
    html(ctx)
        .filter_map(|(page, facts)| {
            let text = value(facts)?;
            let others: Vec<&str> = groups[text]
                .iter()
                .copied()
                .filter(|url| *url != page.url)
                .collect();
            (!others.is_empty()).then(|| Finding::page(id, &page.url, evidence(text, &others)))
        })
        .collect()
}

fn shared_text(text: &str, others: &[&str]) -> serde_json::Value {
    json!({"text": text, "others": others})
}

fn headings(ctx: &CrawlContext) -> Vec<Finding> {
    html(ctx)
        .filter_map(|(page, facts)| match facts.h1s.len() {
            0 => Some(Finding::page("h1_missing", &page.url, json!({}))),
            1 => None,
            count => {
                let evidence = json!({"count": count, "headings": facts.h1s});
                Some(Finding::page("h1_multiple", &page.url, evidence))
            }
        })
        .collect()
}

fn sitemap(ctx: &CrawlContext) -> Vec<Finding> {
    if !ctx.sitemap_found {
        return vec![Finding::site(
            "sitemap_missing",
            ctx.root.as_str(),
            json!({}),
        )];
    }
    html(ctx)
        .filter(|(page, _)| !page.in_sitemap)
        .map(|(page, _)| Finding::page("not_in_sitemap", &page.url, json!({})))
        .collect()
}

fn has_noindex(value: Option<&str>) -> bool {
    value.is_some_and(|text| text.to_ascii_lowercase().contains("noindex"))
}

fn indexing(ctx: &CrawlContext) -> Vec<Finding> {
    ctx.pages
        .iter()
        .filter(|page| page.in_sitemap && !page.is_broken())
        .filter_map(|page| {
            let meta = page
                .facts
                .as_ref()
                .and_then(|facts| facts.robots_meta.as_deref());
            let header = page.x_robots_tag.as_deref();
            (has_noindex(meta) || has_noindex(header)).then(|| {
                let evidence = json!({"robots_meta": meta, "x_robots_tag": header});
                Finding::page("noindex_in_sitemap", &page.url, evidence)
            })
        })
        .collect()
}

fn images(ctx: &CrawlContext) -> Vec<Finding> {
    html(ctx)
        .filter_map(|(page, facts)| {
            let missing: Vec<&str> = facts
                .images
                .iter()
                .filter(|image| image.alt.is_none())
                .map(|image| image.src.as_str())
                .collect();
            (!missing.is_empty()).then(|| {
                let shown: Vec<&str> = missing.iter().take(ALT_SOURCES_SHOWN).copied().collect();
                let evidence = json!({"count": missing.len(), "sources": shown});
                Finding::page("image_alt_missing", &page.url, evidence)
            })
        })
        .collect()
}

fn delivery(ctx: &CrawlContext) -> Vec<Finding> {
    let mut found = Vec::new();
    for page in &ctx.pages {
        if page.redirects.len() >= 2 {
            let evidence = json!({"redirects": page.redirects});
            found.push(Finding::page("redirect_chain", &page.url, evidence));
        }
        if page.is_html_ok() && !page.is_https {
            found.push(Finding::page("not_https", &page.url, json!({})));
        }
        if let Some(ms) = page.response_ms.filter(|ms| *ms > SLOW_MS) {
            found.push(Finding::page(
                "slow_response",
                &page.url,
                json!({"response_ms": ms}),
            ));
        }
        if let Some(bytes) = page.bytes.filter(|bytes| *bytes > HEAVY_BYTES) {
            found.push(Finding::page(
                "heavy_page",
                &page.url,
                json!({"bytes": bytes}),
            ));
        }
    }
    for (page, facts) in html(ctx) {
        if !facts.has_viewport {
            found.push(Finding::page("viewport_missing", &page.url, json!({})));
        }
    }
    found
}

#[cfg(test)]
#[path = "technical_tests.rs"]
mod tests;
