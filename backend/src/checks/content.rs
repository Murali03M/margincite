//! Content checks: pages with too little text, or the same text.

use serde_json::json;

use super::context::{CrawlContext, Finding};
use super::technical::{duplicates, html};

const THIN_BELOW_WORDS: u32 = 150;

pub fn run(ctx: &CrawlContext) -> Vec<Finding> {
    [thin(ctx), same_text(ctx)].concat()
}

fn thin(ctx: &CrawlContext) -> Vec<Finding> {
    html(ctx)
        .filter(|(page, facts)| facts.word_count < THIN_BELOW_WORDS && !page.needed_javascript)
        .map(|(page, facts)| {
            Finding::page(
                "thin_content",
                &page.url,
                json!({"words": facts.word_count}),
            )
        })
        .collect()
}

fn same_text(ctx: &CrawlContext) -> Vec<Finding> {
    duplicates(
        ctx,
        "duplicate_content",
        |facts| {
            (facts.word_count >= THIN_BELOW_WORDS && !facts.text.trim().is_empty())
                .then_some(facts.text.as_str())
        },
        |_, others| json!({"others": others}),
    )
}

#[cfg(test)]
#[path = "content_tests.rs"]
mod tests;
