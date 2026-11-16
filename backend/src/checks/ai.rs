//! AI readiness checks: can AI tools reach, read and trust the pages.

use serde_json::json;

use super::context::{CrawlContext, Finding};
use super::technical::html;
use crate::crawl::robots::Robots;

pub const AI_CRAWLERS: [&str; 9] = [
    "GPTBot",
    "OAI-SearchBot",
    "ChatGPT-User",
    "ClaudeBot",
    "Claude-SearchBot",
    "Claude-User",
    "PerplexityBot",
    "Google-Extended",
    "Bingbot",
];

const LONG_PAGE_WORDS: u32 = 300;

pub fn run(ctx: &CrawlContext) -> Vec<Finding> {
    [
        crawlers(ctx),
        javascript(ctx),
        structured(ctx),
        authorship(ctx),
    ]
    .concat()
}

fn crawlers(ctx: &CrawlContext) -> Vec<Finding> {
    let Some(text) = ctx.robots_txt.as_deref() else {
        return Vec::new();
    };
    let rules = Robots::parse(text);
    AI_CRAWLERS
        .iter()
        .filter(|agent| !rules.allows_agent(agent, &ctx.root))
        .map(|agent| Finding::site("ai_crawler_blocked", agent, json!({"page": ctx.root})))
        .collect()
}

fn javascript(ctx: &CrawlContext) -> Vec<Finding> {
    ctx.pages
        .iter()
        .filter(|page| page.needed_javascript && !page.is_broken())
        .map(|page| Finding::page("needs_javascript", &page.url, json!({})))
        .collect()
}

fn structured(ctx: &CrawlContext) -> Vec<Finding> {
    let mut found = Vec::new();
    for (page, facts) in html(ctx) {
        if facts.structured_data_errors > 0 {
            let evidence = json!({"invalid_blocks": facts.structured_data_errors});
            found.push(Finding::page(
                "structured_data_invalid",
                &page.url,
                evidence,
            ));
        } else if facts.structured_data.is_empty() {
            found.push(Finding::page(
                "structured_data_missing",
                &page.url,
                json!({}),
            ));
        }
    }
    found
}

fn blank(text: &Option<String>) -> bool {
    text.as_deref().is_none_or(|text| text.trim().is_empty())
}

fn authorship(ctx: &CrawlContext) -> Vec<Finding> {
    let mut found = Vec::new();
    let long_inner =
        html(ctx).filter(|(page, facts)| facts.word_count >= LONG_PAGE_WORDS && !ctx.is_home(page));
    for (page, facts) in long_inner {
        let words = json!({"words": facts.word_count});
        if blank(&facts.published) && blank(&facts.modified) {
            found.push(Finding::page("date_missing", &page.url, words.clone()));
        }
        if blank(&facts.author) {
            found.push(Finding::page("author_missing", &page.url, words));
        }
    }
    found
}

#[cfg(test)]
#[path = "ai_tests.rs"]
mod tests;
