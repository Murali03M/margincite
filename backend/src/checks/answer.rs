//! Answer-readiness checks: is a page easy for an AI answer to quote.
//!
//! Facts a page was stored without (`None`) are unknown: the check skips the
//! page rather than failing it.

use serde_json::json;

use super::context::{CrawlContext, Finding};
use super::technical::html;
use crate::crawl::extract::PageFacts;

const MIN_WORDS: u32 = 300;
const LONG_PAGE_WORDS: u32 = 600;
const LONG_OPENING_WORDS: u32 = 80;
const FAQ_MIN_QUESTIONS: u32 = 3;

pub fn run(ctx: &CrawlContext) -> Vec<Finding> {
    let mut found = Vec::new();
    let inner =
        html(ctx).filter(|(page, facts)| facts.word_count >= MIN_WORDS && !ctx.is_home(page));
    for (page, facts) in inner {
        for id in page_checks(facts) {
            found.push(Finding::page(id.0, &page.url, id.1));
        }
    }
    found.extend(organization(ctx));
    found
}

fn page_checks(facts: &PageFacts) -> Vec<(&'static str, serde_json::Value)> {
    let long = facts.word_count >= LONG_PAGE_WORDS;
    let mut found = Vec::new();
    if facts.question_headings == Some(0) {
        found.push(("answer_no_question_headings", json!({})));
    }
    if let Some(words) = facts
        .first_paragraph_words
        .filter(|w| *w > LONG_OPENING_WORDS)
    {
        found.push(("answer_long_opening", json!({"words": words})));
    }
    if long && facts.lists == Some(0) && facts.tables == Some(0) {
        found.push((
            "answer_no_lists_or_tables",
            json!({"words": facts.word_count}),
        ));
    }
    if long && facts.outbound_citations == Some(0) {
        found.push(("answer_no_sources", json!({"words": facts.word_count})));
    }
    if article_like(facts) && facts.article_schema == Some(false) {
        found.push(("schema_no_article", json!({})));
    }
    let questions = facts.question_headings.unwrap_or(0);
    if questions >= FAQ_MIN_QUESTIONS && facts.faq_schema == Some(false) {
        found.push(("schema_faq_missing", json!({"questions": questions})));
    }
    found
}

fn has_text(text: &Option<String>) -> bool {
    text.as_deref().is_some_and(|text| !text.trim().is_empty())
}

fn article_like(facts: &PageFacts) -> bool {
    has_text(&facts.published) || has_text(&facts.modified) || has_text(&facts.author)
}

/// One site-level finding, from the home page when the crawl reached it.
fn organization(ctx: &CrawlContext) -> Vec<Finding> {
    let missing = html(ctx)
        .any(|(page, facts)| ctx.is_home(page) && facts.organization_schema == Some(false));
    if missing {
        vec![Finding::site(
            "schema_no_organization",
            ctx.root.as_str(),
            json!({}),
        )]
    } else {
        Vec::new()
    }
}

#[cfg(test)]
#[path = "answer_tests.rs"]
mod tests;
