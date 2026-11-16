use serde_json::json;

use super::*;
use crate::checks::catalog;
use crate::checks::context::fixtures::{context, healthy, with_facts};
use crate::checks::context::{CrawlContext, Finding, PageView, TargetKind};

const A: &str = "https://example.com/a";

fn only(findings: &[Finding], id: &str) -> Vec<Finding> {
    findings
        .iter()
        .filter(|f| f.check_id == id)
        .cloned()
        .collect()
}

fn with_robots(text: &str) -> CrawlContext {
    CrawlContext {
        robots_txt: Some(text.to_string()),
        ..context(vec![healthy(A)])
    }
}

fn blocked_names(ctx: &CrawlContext) -> Vec<String> {
    only(&run(ctx), "ai_crawler_blocked")
        .into_iter()
        .map(|f| f.target)
        .collect()
}

#[test]
fn a_healthy_page_has_no_ai_findings() {
    let ctx = context(vec![healthy(A), healthy("https://example.com/b")]);
    assert_eq!(run(&ctx), vec![]);
}

#[test]
fn every_ai_finding_is_in_the_catalog() {
    let page = PageView {
        needed_javascript: true,
        ..with_facts(A, |f| f.structured_data.clear())
    };
    for finding in run(&context(vec![page])) {
        assert!(catalog::get(finding.check_id).is_some());
    }
}

#[test]
fn no_robots_file_blocks_no_crawler() {
    assert!(blocked_names(&context(vec![healthy(A)])).is_empty());
}

#[test]
fn a_wildcard_disallow_blocks_all_nine() {
    let ctx = with_robots("User-agent: *\nDisallow: /\n");
    let found = only(&run(&ctx), "ai_crawler_blocked");
    assert_eq!(found.len(), 9);
    assert!(found.iter().all(|f| f.target_kind == TargetKind::Site));
    assert_eq!(found[0].target, "GPTBot");
}

#[test]
fn a_crawler_named_in_its_own_group_is_judged_by_that_group() {
    let ctx = with_robots("User-agent: *\nAllow: /\n\nUser-agent: GPTBot\nDisallow: /\n");
    assert_eq!(blocked_names(&ctx), ["GPTBot"]);
    let ctx = with_robots("User-agent: *\nDisallow: /\n\nUser-agent: ClaudeBot\nAllow: /\n");
    let blocked = blocked_names(&ctx);
    assert_eq!(blocked.len(), 8);
    assert!(!blocked.contains(&"ClaudeBot".to_string()));
}

#[test]
fn needs_javascript_flags_script_only_pages() {
    let page = PageView {
        needed_javascript: true,
        ..healthy(A)
    };
    let found = only(&run(&context(vec![page])), "needs_javascript");
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].target, A);
}

#[test]
fn structured_data_missing_and_invalid() {
    let none = with_facts(A, |f| f.structured_data.clear());
    let bad = with_facts("https://example.com/b", |f| {
        f.structured_data.clear();
        f.structured_data_errors = 2;
    });
    let found = run(&context(vec![none, bad]));
    let missing = only(&found, "structured_data_missing");
    assert_eq!(missing.len(), 1);
    assert_eq!(missing[0].target, A);
    let invalid = only(&found, "structured_data_invalid");
    assert_eq!(invalid.len(), 1);
    assert_eq!(invalid[0].evidence, json!({"invalid_blocks": 2}));
}

#[test]
fn date_and_author_are_needed_on_long_inner_pages_only() {
    let long = with_facts(A, |f| {
        f.published = None;
        f.author = None;
    });
    let short = with_facts("https://example.com/b", |f| {
        f.word_count = 299;
        f.published = None;
        f.author = None;
    });
    let home = PageView {
        depth: 0,
        ..with_facts("https://example.com/", |f| {
            f.published = None;
            f.author = None;
        })
    };
    let found = run(&context(vec![long, short, home]));
    assert_eq!(only(&found, "date_missing").len(), 1);
    assert_eq!(only(&found, "date_missing")[0].target, A);
    assert_eq!(only(&found, "author_missing").len(), 1);
}

/// The crawler stores pages listed in the sitemap at depth 0, like the home
/// page. They are still inner pages.
#[test]
fn a_long_sitemap_page_at_depth_0_is_not_the_home_page() {
    let from_sitemap = PageView {
        depth: 0,
        ..with_facts(A, |f| {
            f.published = None;
            f.author = None;
        })
    };
    let found = run(&context(vec![from_sitemap]));
    assert_eq!(only(&found, "date_missing").len(), 1);
    assert_eq!(only(&found, "author_missing").len(), 1);
}

#[test]
fn the_home_page_is_known_by_its_address_with_or_without_www_or_a_slash() {
    let ctx = context(vec![]);
    for url in [
        "https://example.com/",
        "https://example.com",
        "https://www.example.com/",
        "http://www.example.com",
    ] {
        assert!(ctx.is_home(&healthy(url)), "{url}");
    }
    for url in [
        "https://example.com/pricing",
        "https://blog.example.com/",
        "https://example.org/",
    ] {
        assert!(!ctx.is_home(&healthy(url)), "{url}");
    }
}

#[test]
fn a_modified_date_counts_as_a_date() {
    let page = with_facts(A, |f| {
        f.published = None;
        f.modified = Some("2026-02-02".into());
    });
    assert!(only(&run(&context(vec![page])), "date_missing").is_empty());
}

#[test]
fn llms_txt_never_produces_a_finding() {
    let mut ctx = with_robots("User-agent: *\nDisallow: /\n");
    ctx.pages.push(PageView {
        needed_javascript: true,
        ..healthy("https://example.com/llms.txt")
    });
    assert!(crate::checks::run_all(&ctx)
        .iter()
        .all(|f| !f.check_id.contains("llms")));
    assert!(catalog::all().iter().all(|c| !c.id.contains("llms")));
}
