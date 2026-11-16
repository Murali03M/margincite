use serde_json::json;

use super::*;
use crate::checks::catalog;
use crate::checks::context::fixtures::{context, healthy, with_facts};
use crate::checks::context::{PageView, TargetKind};
use crate::crawl::extract::PageFacts;

const A: &str = "https://example.com/a";
const HOME: &str = "https://example.com/";

fn ids(ctx: &CrawlContext) -> Vec<&'static str> {
    run(ctx).iter().map(|f| f.check_id).collect()
}

fn page(words: u32, edit: impl FnOnce(&mut PageFacts)) -> PageView {
    with_facts(A, |f| {
        f.word_count = words;
        edit(f);
    })
}

fn fires(view: PageView, id: &str) -> bool {
    ids(&context(vec![view])).contains(&id)
}

#[test]
fn a_healthy_page_has_no_answer_findings() {
    assert_eq!(run(&context(vec![healthy(A), healthy(HOME)])), vec![]);
}

#[test]
fn every_answer_check_is_in_the_catalog_as_ai_readiness() {
    for id in [
        "answer_no_question_headings",
        "answer_long_opening",
        "answer_no_lists_or_tables",
        "answer_no_sources",
        "schema_no_article",
        "schema_no_organization",
        "schema_faq_missing",
    ] {
        let check = catalog::get(id).unwrap_or_else(|| panic!("{id}"));
        assert_eq!(check.category, crate::checks::Category::AiReadiness);
        assert!(check.needs.is_empty());
    }
}

#[test]
fn no_question_headings_fires_and_clears() {
    assert!(fires(
        page(400, |f| f.question_headings = Some(0)),
        "answer_no_question_headings"
    ));
    assert!(!fires(
        page(400, |f| f.question_headings = Some(1)),
        "answer_no_question_headings"
    ));
}

#[test]
fn a_long_opening_fires_above_80_words() {
    assert!(fires(
        page(400, |f| f.first_paragraph_words = Some(81)),
        "answer_long_opening"
    ));
    assert!(!fires(
        page(400, |f| f.first_paragraph_words = Some(80)),
        "answer_long_opening"
    ));
    let found = run(&context(vec![page(400, |f| {
        f.first_paragraph_words = Some(120)
    })]));
    let finding = found
        .iter()
        .find(|f| f.check_id == "answer_long_opening")
        .unwrap();
    assert_eq!(finding.evidence, json!({"words": 120}));
    assert_eq!(finding.target_kind, TargetKind::Page);
}

#[test]
fn lists_and_tables_are_wanted_from_600_words() {
    let bare = |f: &mut PageFacts| {
        f.lists = Some(0);
        f.tables = Some(0);
    };
    assert!(fires(page(600, bare), "answer_no_lists_or_tables"));
    assert!(!fires(page(599, bare), "answer_no_lists_or_tables"));
    assert!(!fires(
        page(600, |f| {
            f.lists = Some(0);
            f.tables = Some(1);
        }),
        "answer_no_lists_or_tables"
    ));
    assert!(!fires(
        page(600, |f| {
            f.lists = Some(2);
            f.tables = Some(0);
        }),
        "answer_no_lists_or_tables"
    ));
}

#[test]
fn outside_sources_are_wanted_from_600_words() {
    assert!(fires(
        page(600, |f| f.outbound_citations = Some(0)),
        "answer_no_sources"
    ));
    assert!(!fires(
        page(599, |f| f.outbound_citations = Some(0)),
        "answer_no_sources"
    ));
    assert!(!fires(
        page(600, |f| f.outbound_citations = Some(1)),
        "answer_no_sources"
    ));
}

#[test]
fn an_article_like_page_wants_article_markup() {
    let dated = |f: &mut PageFacts| {
        f.article_schema = Some(false);
        f.published = Some("2026-01-01".into());
        f.author = None;
    };
    assert!(fires(page(400, dated), "schema_no_article"));
    assert!(fires(
        page(400, |f| {
            f.article_schema = Some(false);
            f.published = None;
            f.modified = None;
            f.author = Some("Ada".into());
        }),
        "schema_no_article"
    ));
    assert!(!fires(
        page(400, |f| {
            f.article_schema = Some(false);
            f.published = None;
            f.modified = None;
            f.author = None;
        }),
        "schema_no_article"
    ));
    assert!(!fires(
        page(400, |f| f.article_schema = Some(true)),
        "schema_no_article"
    ));
}

#[test]
fn faq_markup_is_wanted_from_three_question_headings() {
    let questions = |n| {
        move |f: &mut PageFacts| {
            f.question_headings = Some(n);
            f.faq_schema = Some(false);
        }
    };
    assert!(fires(page(400, questions(3)), "schema_faq_missing"));
    assert!(!fires(page(400, questions(2)), "schema_faq_missing"));
    assert!(!fires(
        page(400, |f| {
            f.question_headings = Some(5);
            f.faq_schema = Some(true);
        }),
        "schema_faq_missing"
    ));
}

#[test]
fn the_home_page_without_organization_markup_is_one_site_finding() {
    let home = with_facts(HOME, |f| f.organization_schema = Some(false));
    let found = run(&context(vec![home, healthy(A)]));
    let finding = found
        .iter()
        .find(|f| f.check_id == "schema_no_organization")
        .unwrap();
    assert_eq!(finding.target_kind, TargetKind::Site);
    assert_eq!(finding.target, HOME);
    let fine = with_facts(HOME, |f| f.organization_schema = Some(true));
    assert!(!ids(&context(vec![fine])).contains(&"schema_no_organization"));
    // A crawl that never reached the home page cannot say.
    let inner = with_facts(A, |f| f.organization_schema = Some(false));
    assert!(!ids(&context(vec![inner])).contains(&"schema_no_organization"));
}

#[test]
fn the_home_page_is_exempt_from_page_checks() {
    let home = with_facts(HOME, |f| {
        f.word_count = 900;
        f.question_headings = Some(0);
        f.first_paragraph_words = Some(200);
        f.lists = Some(0);
        f.tables = Some(0);
        f.outbound_citations = Some(0);
    });
    assert_eq!(run(&context(vec![home])), vec![]);
}

#[test]
fn pages_under_300_words_are_skipped() {
    let short = page(299, |f| {
        f.question_headings = Some(0);
        f.first_paragraph_words = Some(200);
        f.article_schema = Some(false);
    });
    assert_eq!(run(&context(vec![short])), vec![]);
}

#[test]
fn pages_stored_before_these_facts_are_skipped_not_failed() {
    let old = page(900, |f| {
        f.question_headings = None;
        f.lists = None;
        f.tables = None;
        f.first_paragraph_words = None;
        f.faq_schema = None;
        f.article_schema = None;
        f.organization_schema = None;
        f.outbound_citations = None;
    });
    let old_home = with_facts(HOME, |f| f.organization_schema = None);
    assert_eq!(run(&context(vec![old, old_home])), vec![]);
    // Half known: tables unknown means lists alone cannot prove "no list or table".
    let half = page(900, |f| {
        f.lists = Some(0);
        f.tables = None;
    });
    assert!(!fires(half, "answer_no_lists_or_tables"));
}

#[test]
fn broken_and_non_html_pages_are_skipped() {
    let broken = PageView {
        status_code: Some(500),
        ..page(900, |f| f.question_headings = Some(0))
    };
    assert_eq!(run(&context(vec![broken])), vec![]);
}
