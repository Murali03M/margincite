//! Keyword checks: does each target keyword show up where a page should say it,
//! and does one page, not several, answer it. Pure rules, like every check.

use std::collections::HashSet;

use serde_json::json;

use super::context::{CrawlContext, Finding, PageView, SearchData};
use crate::crawl::extract::PageFacts;
use crate::keywords::landing::{self, LandingSource};

/// The keyword as a share of the page's words above which it reads as stuffing.
const STUFFING_SHARE: f64 = 0.03;
/// A keyword said fewer times than this is never stuffing, however short the page.
const STUFFING_MIN_USES: usize = 3;
const STUFFING_TITLE_USES: usize = 3;
/// A page needs this share of a keyword's impressions to count as a competitor.
const CANNIBAL_SHARE: f64 = 0.2;

/// Lowercase words: letters and digits only, apostrophes joined into the word
/// ("men's" is "mens"), every other mark a break. Case, punctuation and extra
/// spaces never change a match, and a word only matches a whole word.
pub fn words(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut word = String::new();
    for c in text.chars() {
        if c.is_alphanumeric() {
            word.extend(c.to_lowercase());
        } else if matches!(c, '\'' | '\u{2019}') {
            continue;
        } else if !word.is_empty() {
            out.push(std::mem::take(&mut word));
        }
    }
    if !word.is_empty() {
        out.push(word);
    }
    out
}

fn has_all(haystack: &[String], needle: &[String]) -> bool {
    let present: HashSet<&String> = haystack.iter().collect();
    needle.iter().all(|word| present.contains(word))
}

/// How many times the whole phrase appears, in order.
fn phrase_count(haystack: &[String], needle: &[String]) -> usize {
    if needle.is_empty() || haystack.len() < needle.len() {
        return 0;
    }
    haystack
        .windows(needle.len())
        .filter(|w| *w == needle)
        .count()
}

/// One keyword aimed at one working page.
struct Aim<'a> {
    keyword: &'a str,
    words: Vec<String>,
    url: &'a str,
    source: LandingSource,
    facts: &'a PageFacts,
}

impl Aim<'_> {
    fn finding(&self, check: &'static str) -> Finding {
        Finding::page(
            check,
            &format!("\"{}\" on {}", self.keyword, self.url),
            json!({"keyword": self.keyword, "url": self.url, "landing_source": self.source}),
        )
    }

    fn in_title(&self) -> bool {
        has_all(
            &words(self.facts.title.as_deref().unwrap_or("")),
            &self.words,
        )
    }

    fn in_h1(&self) -> bool {
        self.facts
            .h1s
            .iter()
            .any(|h1| has_all(&words(h1), &self.words))
    }

    fn in_description(&self) -> bool {
        let text = self.facts.meta_description.as_deref().unwrap_or("");
        has_all(&words(text), &self.words)
    }

    fn stuffed(&self, body: &[String]) -> bool {
        let title = words(self.facts.title.as_deref().unwrap_or(""));
        if phrase_count(&title, &self.words) >= STUFFING_TITLE_USES {
            return true;
        }
        let uses = phrase_count(body, &self.words);
        // Every word of the phrase counts, so a two-word keyword said 3 times
        // in 150 words is 6 words of 150, not 3.
        let density = (uses * self.words.len()) as f64 / body.len() as f64;
        uses >= STUFFING_MIN_USES && density > STUFFING_SHARE
    }
}

pub fn run(ctx: &CrawlContext) -> Vec<Finding> {
    let mut findings = Vec::new();
    for target in &ctx.keywords {
        let keyword = target.keyword.trim();
        let words = words(keyword);
        if words.is_empty() {
            continue;
        }
        let chosen = landing::choose(
            keyword,
            target.target_url.as_deref(),
            &ctx.pages,
            ctx.search.as_ref(),
        );
        if let Some((url, source)) = chosen {
            let facts = ctx
                .pages
                .iter()
                .find(|p| p.url == url)
                .and_then(PageView::html_facts);
            if let Some(facts) = facts {
                let aim = Aim {
                    keyword,
                    words: words.clone(),
                    url: &url,
                    source,
                    facts,
                };
                findings.extend(page_checks(&aim));
            }
        }
        findings.extend(cannibalization(keyword, &words, ctx.search.as_ref()));
    }
    findings
}

fn page_checks(aim: &Aim) -> Vec<Finding> {
    let mut found = Vec::new();
    if !aim.in_title() {
        found.push(aim.finding("keyword_not_in_title"));
    }
    if !aim.in_h1() {
        found.push(aim.finding("keyword_not_in_h1"));
    }
    if !aim.in_description() {
        found.push(aim.finding("keyword_not_in_description"));
    }
    // A crawl that kept no text for the page proves nothing about its body.
    let body = words(&aim.facts.text);
    if !body.is_empty() && !has_all(&body, &aim.words) {
        found.push(aim.finding("keyword_not_in_body"));
    }
    if aim.stuffed(&body) {
        found.push(aim.finding("keyword_stuffing"));
    }
    found
}

/// Two or more pages that each get a real share of one keyword's searches.
fn cannibalization(
    keyword: &str,
    kw_words: &[String],
    search: Option<&SearchData>,
) -> Option<Finding> {
    let rows = search?
        .rows
        .iter()
        .filter(|row| words(&row.query) == kw_words);
    let mut by_page: Vec<(&str, i64)> = Vec::new();
    for row in rows {
        match by_page.iter_mut().find(|(page, _)| *page == row.page) {
            Some((_, total)) => *total += row.impressions,
            None => by_page.push((&row.page, row.impressions)),
        }
    }
    let total: i64 = by_page.iter().map(|(_, n)| n).sum();
    if total <= 0 {
        return None;
    }
    let mut rivals: Vec<(&str, f64)> = by_page
        .into_iter()
        .map(|(page, n)| (page, n as f64 / total as f64))
        .filter(|(_, share)| *share >= CANNIBAL_SHARE)
        .collect();
    if rivals.len() < 2 {
        return None;
    }
    rivals.sort_by(|a, b| b.1.total_cmp(&a.1).then_with(|| a.0.cmp(b.0)));
    let pages: Vec<_> = rivals
        .iter()
        .map(|(url, share)| json!({"url": url, "share": (share * 1000.0).round() / 10.0}))
        .collect();
    Some(Finding::site(
        "keyword_cannibalization",
        &format!("\"{keyword}\""),
        json!({"keyword": keyword, "pages": pages}),
    ))
}

#[cfg(test)]
#[path = "keywords_tests.rs"]
mod tests;
