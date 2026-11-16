//! Which crawled page should answer a keyword.

use std::collections::HashSet;

use crate::checks::context::{PageView, SearchData};
use crate::checks::keywords::words;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LandingSource {
    Set,
    SearchConsole,
    BestMatch,
    Home,
}

/// The page for a keyword, in this order: the URL the person set, when it was
/// crawled; the crawled page Search Console shows most for that query; the
/// crawled page whose title, main heading and description hold the most of the
/// keyword's words; the home page. `None` when nothing was crawled to point at.
pub fn choose(
    keyword: &str,
    target_url: Option<&str>,
    pages: &[PageView],
    search: Option<&SearchData>,
) -> Option<(String, LandingSource)> {
    let working = || pages.iter().filter(|page| page.html_facts().is_some());
    let keyword_words = words(keyword);
    if let Some(page) = target_url.and_then(|url| working().find(|p| same_url(&p.url, url))) {
        return Some((page.url.clone(), LandingSource::Set));
    }
    if let Some(url) = most_shown(&keyword_words, search, |url| {
        working().any(|p| p.url == url)
    }) {
        return Some((url, LandingSource::SearchConsole));
    }
    if let Some(page) = best_match(&keyword_words, working()) {
        return Some((page.url.clone(), LandingSource::BestMatch));
    }
    working()
        .filter(|page| page.depth == 0)
        .min_by(|a, b| a.url.cmp(&b.url))
        .map(|page| (page.url.clone(), LandingSource::Home))
}

fn same_url(a: &str, b: &str) -> bool {
    a.trim_end_matches('/') == b.trim_end_matches('/')
}

/// The crawled page with the most impressions for this query.
fn most_shown(
    keyword_words: &[String],
    search: Option<&SearchData>,
    crawled: impl Fn(&str) -> bool,
) -> Option<String> {
    let mut totals: Vec<(&str, i64)> = Vec::new();
    for row in search?
        .rows
        .iter()
        .filter(|r| words(&r.query) == keyword_words)
    {
        if !crawled(&row.page) {
            continue;
        }
        match totals.iter_mut().find(|(page, _)| *page == row.page) {
            Some((_, total)) => *total += row.impressions,
            None => totals.push((&row.page, row.impressions)),
        }
    }
    totals
        .into_iter()
        .max_by(|a, b| a.1.cmp(&b.1).then_with(|| b.0.cmp(a.0)))
        .map(|(page, _)| page.to_string())
}

/// The page holding the most distinct keyword words in its title, main
/// headings and description. Ties go to the shallower page, then the address.
fn best_match<'a>(
    keyword_words: &[String],
    pages: impl Iterator<Item = &'a PageView>,
) -> Option<&'a PageView> {
    pages
        .map(|page| (matched_words(keyword_words, page), page))
        .filter(|(score, _)| *score > 0)
        .max_by(|(sa, a), (sb, b)| {
            sa.cmp(sb)
                .then_with(|| b.depth.cmp(&a.depth))
                .then_with(|| b.url.cmp(&a.url))
        })
        .map(|(_, page)| page)
}

fn matched_words(keyword_words: &[String], page: &PageView) -> usize {
    let Some(facts) = page.html_facts() else {
        return 0;
    };
    let mut seen: HashSet<String> = HashSet::new();
    let parts = facts
        .title
        .iter()
        .chain(facts.h1s.iter())
        .chain(facts.meta_description.iter());
    for part in parts {
        seen.extend(words(part));
    }
    let wanted: HashSet<&String> = keyword_words.iter().collect();
    wanted
        .into_iter()
        .filter(|word| seen.contains(*word))
        .count()
}

#[cfg(test)]
#[path = "landing_tests.rs"]
mod tests;
