use super::*;
use crate::checks::context::fixtures::{healthy, with_facts};
use crate::checks::context::{QueryPage, SearchData};

const HOME: &str = "https://example.com/";
const PRICING: &str = "https://example.com/pricing";
const BLOG: &str = "https://example.com/blog/seo-tips";

fn home() -> PageView {
    PageView {
        depth: 0,
        ..healthy(HOME)
    }
}

fn titled(url: &str, title: &str) -> PageView {
    with_facts(url, |facts| facts.title = Some(title.to_string()))
}

fn search(rows: &[(&str, &str, i64)]) -> SearchData {
    SearchData {
        rows: rows
            .iter()
            .map(|(query, page, impressions)| QueryPage {
                query: query.to_string(),
                page: page.to_string(),
                clicks: 0,
                impressions: *impressions,
                ctr: 0.0,
                position: 3.0,
            })
            .collect(),
    }
}

#[test]
fn a_set_url_that_was_crawled_wins_over_everything() {
    let pages = [home(), titled(PRICING, "Plans"), titled(BLOG, "SEO tips")];
    let data = search(&[("seo tips", BLOG, 500)]);
    let chosen = choose("seo tips", Some(PRICING), &pages, Some(&data));
    assert_eq!(chosen, Some((PRICING.to_string(), LandingSource::Set)));
}

#[test]
fn a_set_url_matches_ignoring_a_trailing_slash() {
    let pages = [home(), titled(PRICING, "Plans")];
    let chosen = choose("plans", Some("https://example.com/pricing/"), &pages, None);
    assert_eq!(chosen, Some((PRICING.to_string(), LandingSource::Set)));
}

#[test]
fn a_set_url_that_was_not_crawled_falls_through() {
    let pages = [home(), titled(BLOG, "SEO tips")];
    let chosen = choose("seo tips", Some(PRICING), &pages, None);
    assert_eq!(chosen, Some((BLOG.to_string(), LandingSource::BestMatch)));
}

#[test]
fn search_console_picks_the_page_with_most_impressions_for_the_query() {
    let pages = [home(), titled(PRICING, "Plans"), titled(BLOG, "Unrelated")];
    let data = search(&[
        ("SEO  Tips", PRICING, 40),
        ("seo tips", BLOG, 90),
        ("other query", PRICING, 9_000),
    ]);
    let chosen = choose("seo tips", None, &pages, Some(&data));
    assert_eq!(
        chosen,
        Some((BLOG.to_string(), LandingSource::SearchConsole))
    );
}

#[test]
fn search_console_pages_that_were_not_crawled_are_skipped() {
    let pages = [home(), titled(PRICING, "Plans")];
    let data = search(&[
        ("plans", "https://example.com/gone", 900),
        ("plans", PRICING, 5),
    ]);
    let chosen = choose("plans", None, &pages, Some(&data));
    assert_eq!(
        chosen,
        Some((PRICING.to_string(), LandingSource::SearchConsole))
    );
}

#[test]
fn best_match_counts_keyword_words_across_title_heading_and_description() {
    let weak = titled(PRICING, "Pricing for teams");
    let strong = with_facts(BLOG, |facts| {
        facts.title = Some("Local SEO".into());
        facts.h1s = vec!["A guide for small shops".into()];
        facts.meta_description = Some("Tips for a small shop".into());
    });
    let pages = [home(), weak, strong];
    let chosen = choose("local seo for small shops", None, &pages, None);
    assert_eq!(chosen, Some((BLOG.to_string(), LandingSource::BestMatch)));
}

#[test]
fn best_match_needs_whole_words() {
    let pages = [home(), titled(BLOG, "cbuddyai news")];
    let chosen = choose("buddy", None, &pages, None);
    assert_eq!(chosen, Some((HOME.to_string(), LandingSource::Home)));
}

#[test]
fn no_match_and_no_search_data_lands_on_the_home_page() {
    let pages = [titled(PRICING, "Plans"), home()];
    assert_eq!(
        choose("zebra", None, &pages, Some(&SearchData::default())),
        Some((HOME.to_string(), LandingSource::Home))
    );
}

#[test]
fn broken_pages_are_never_chosen_and_no_home_means_none() {
    let broken = PageView {
        status_code: Some(404),
        ..titled(PRICING, "zebra")
    };
    assert_eq!(choose("zebra", None, &[broken], None), None);
    assert_eq!(choose("zebra", None, &[], None), None);
}
