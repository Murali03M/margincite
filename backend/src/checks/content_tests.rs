use serde_json::json;

use super::*;
use crate::checks::catalog;
use crate::checks::context::fixtures::{context, healthy, with_facts};
use crate::checks::context::{PageView, TargetKind};

const A: &str = "https://example.com/a";
const B: &str = "https://example.com/b";

fn same_text(url: &str, words: u32) -> PageView {
    with_facts(url, |f| {
        f.word_count = words;
        f.text = "shared words".repeat(words as usize);
    })
}

#[test]
fn a_healthy_page_has_no_content_findings() {
    assert_eq!(run(&context(vec![healthy(A), healthy(B)])), vec![]);
}

#[test]
fn thin_content_needs_fewer_than_150_words() {
    let thin = with_facts(A, |f| f.word_count = 149);
    let enough = with_facts(B, |f| f.word_count = 150);
    let found = run(&context(vec![thin, enough]));
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].check_id, "thin_content");
    assert_eq!(found[0].target, A);
    assert_eq!(found[0].evidence, json!({"words": 149}));
    assert!(catalog::get("thin_content").is_some());
}

#[test]
fn thin_content_skips_javascript_only_pages() {
    let page = PageView {
        needed_javascript: true,
        ..with_facts(A, |f| f.word_count = 3)
    };
    assert!(run(&context(vec![page])).is_empty());
}

#[test]
fn duplicate_content_names_the_other_pages() {
    let found = run(&context(vec![same_text(A, 200), same_text(B, 200)]));
    assert_eq!(found.len(), 2);
    assert!(found.iter().all(|f| f.check_id == "duplicate_content"));
    assert!(found.iter().all(|f| f.target_kind == TargetKind::Page));
    assert_eq!(found[0].target, A);
    assert_eq!(found[0].evidence["others"], json!([B]));
    assert_eq!(found[1].evidence["others"], json!([A]));
}

#[test]
fn duplicate_content_ignores_short_pages() {
    let found = run(&context(vec![same_text(A, 149), same_text(B, 149)]));
    assert!(found.iter().all(|f| f.check_id != "duplicate_content"));
}

#[test]
fn content_checks_skip_pages_that_are_not_working_html() {
    let broken = PageView {
        status_code: Some(500),
        facts: None,
        ..healthy(A)
    };
    assert!(run(&context(vec![broken])).is_empty());
}
