use serde_json::json;
use url::Url;

use super::*;
use crate::checks::catalog;
use crate::checks::context::fixtures::{context, healthy, with_facts};
use crate::checks::context::{Finding, PageView, TargetKind};
use crate::crawl::extract::Image;

const A: &str = "https://example.com/a";
const B: &str = "https://example.com/b";

fn ids(findings: &[Finding]) -> Vec<&'static str> {
    findings.iter().map(|f| f.check_id).collect()
}

fn only(findings: &[Finding], id: &str) -> Vec<Finding> {
    findings
        .iter()
        .filter(|f| f.check_id == id)
        .cloned()
        .collect()
}

fn broken(url: &str, status: i32) -> PageView {
    PageView {
        status_code: Some(status),
        facts: None,
        ..healthy(url)
    }
}

fn link(to: &str) -> crate::crawl::extract::Link {
    crate::crawl::extract::Link {
        url: Url::parse(to).unwrap(),
        text: "x".into(),
        nofollow: false,
    }
}

#[test]
fn a_healthy_page_has_no_technical_findings() {
    let ctx = context(vec![healthy(A), healthy(B)]);
    assert_eq!(run(&ctx), vec![]);
}

#[test]
fn every_technical_finding_is_in_the_catalog() {
    let ctx = context(vec![broken(A, 500)]);
    for finding in run(&ctx) {
        assert!(catalog::get(finding.check_id).is_some());
    }
}

#[test]
fn broken_page_flags_error_statuses_and_fetch_errors() {
    let failed = PageView {
        status_code: None,
        fetch_error: Some("timed out".into()),
        facts: None,
        ..healthy(B)
    };
    let found = only(&run(&context(vec![broken(A, 404), failed])), "broken_page");
    assert_eq!(found.len(), 2);
    assert_eq!(found[0].target, A);
    assert_eq!(found[0].evidence["status"], 404);
    assert_eq!(found[1].evidence["error"], "timed out");
}

#[test]
fn broken_internal_link_targets_the_linking_page() {
    let linker = with_facts(A, |f| {
        f.links = vec![link(B), link("https://example.com/b#top")]
    });
    let found = only(
        &run(&context(vec![linker, broken(B, 404)])),
        "broken_internal_link",
    );
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].target, A);
    assert_eq!(found[0].evidence["broken"], json!([B]));
}

#[test]
fn redirect_chain_needs_two_redirects() {
    let one = PageView {
        redirects: vec!["x".into()],
        ..healthy(A)
    };
    let two = PageView {
        redirects: vec!["x".into(), "y".into()],
        ..healthy(B)
    };
    let found = only(&run(&context(vec![one, two])), "redirect_chain");
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].target, B);
    assert_eq!(found[0].evidence["redirects"], json!(["x", "y"]));
}

#[test]
fn title_missing_flags_none_and_blank() {
    let none = with_facts(A, |f| f.title = None);
    let blank = with_facts(B, |f| f.title = Some("   ".into()));
    let found = only(&run(&context(vec![none, blank])), "title_missing");
    assert_eq!(
        found.iter().map(|f| f.target.as_str()).collect::<Vec<_>>(),
        [A, B]
    );
}

#[test]
fn title_length_flags_short_and_long() {
    let short = with_facts(A, |f| f.title = Some("Short".into()));
    let long = with_facts(B, |f| f.title = Some("x".repeat(66)));
    let ok = with_facts("https://example.com/c", |f| f.title = Some("x".repeat(65)));
    let found = only(&run(&context(vec![short, long, ok])), "title_length");
    assert_eq!(found.len(), 2);
    assert_eq!(found[0].evidence["length"], 5);
    assert_eq!(found[1].evidence["length"], 66);
}

#[test]
fn duplicates_name_each_other_in_evidence() {
    let one = with_facts(A, |f| {
        f.title = Some("Same title across pages".into());
        f.meta_description = Some("d".repeat(80));
    });
    let two = with_facts(B, |f| {
        f.title = Some("Same title across pages".into());
        f.meta_description = Some("d".repeat(80));
    });
    let found = run(&context(vec![one, two]));
    let titles = only(&found, "title_duplicate");
    assert_eq!(titles.len(), 2);
    assert_eq!(titles[0].target, A);
    assert_eq!(titles[0].evidence["others"], json!([B]));
    assert_eq!(titles[1].evidence["others"], json!([A]));
    assert_eq!(only(&found, "description_duplicate").len(), 2);
}

#[test]
fn description_missing_and_length() {
    let none = with_facts(A, |f| f.meta_description = None);
    let short = with_facts(B, |f| f.meta_description = Some("tiny".into()));
    let long = with_facts("https://example.com/c", |f| {
        f.meta_description = Some("x".repeat(161))
    });
    let found = run(&context(vec![none, short, long]));
    assert_eq!(only(&found, "description_missing").len(), 1);
    let lengths = only(&found, "description_length");
    assert_eq!(lengths.len(), 2);
    assert_eq!(lengths[0].target, B);
}

#[test]
fn h1_missing_and_multiple() {
    let none = with_facts(A, |f| f.h1s = vec![]);
    let many = with_facts(B, |f| f.h1s = vec!["One".into(), "Two".into()]);
    let found = run(&context(vec![none, many]));
    assert_eq!(only(&found, "h1_missing")[0].target, A);
    let multiple = only(&found, "h1_multiple");
    assert_eq!(multiple[0].target, B);
    assert_eq!(multiple[0].evidence["count"], 2);
}

#[test]
fn canonical_broken_points_at_a_broken_crawled_page() {
    let page = with_facts(A, |f| f.canonical = Some(Url::parse(B).unwrap()));
    let found = only(
        &run(&context(vec![page, broken(B, 410)])),
        "canonical_broken",
    );
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].target, A);
    assert_eq!(found[0].evidence["canonical"], B);
}

#[test]
fn noindex_in_sitemap_reads_meta_and_header() {
    let meta = with_facts(A, |f| f.robots_meta = Some("NoIndex, follow".into()));
    let header = PageView {
        x_robots_tag: Some("noindex".into()),
        ..healthy(B)
    };
    let outside = PageView {
        in_sitemap: false,
        ..with_facts("https://example.com/c", |f| {
            f.robots_meta = Some("noindex".into())
        })
    };
    let found = only(
        &run(&context(vec![meta, header, outside])),
        "noindex_in_sitemap",
    );
    assert_eq!(
        found.iter().map(|f| f.target.as_str()).collect::<Vec<_>>(),
        [A, B]
    );
}

#[test]
fn sitemap_missing_is_site_level_and_not_in_sitemap_needs_a_sitemap() {
    let outside = PageView {
        in_sitemap: false,
        ..healthy(A)
    };
    let mut with_map = context(vec![outside.clone()]);
    let found = run(&with_map);
    assert_eq!(only(&found, "not_in_sitemap")[0].target, A);
    assert!(only(&found, "sitemap_missing").is_empty());

    with_map.sitemap_found = false;
    let found = run(&with_map);
    assert!(only(&found, "not_in_sitemap").is_empty());
    let missing = only(&found, "sitemap_missing");
    assert_eq!(missing.len(), 1);
    assert_eq!(missing[0].target_kind, TargetKind::Site);
    assert_eq!(missing[0].target, "https://example.com/");
}

#[test]
fn image_alt_missing_counts_and_caps_sources() {
    let images = (0..7)
        .map(|n| Image {
            src: format!("/i{n}.png"),
            alt: None,
        })
        .chain([Image {
            src: "/ok.png".into(),
            alt: Some(String::new()),
        }])
        .collect();
    let page = with_facts(A, |f| f.images = images);
    let found = only(&run(&context(vec![page])), "image_alt_missing");
    assert_eq!(found[0].evidence["count"], 7);
    assert_eq!(found[0].evidence["sources"].as_array().unwrap().len(), 5);
}

#[test]
fn viewport_https_speed_and_weight() {
    let no_viewport = with_facts(A, |f| f.has_viewport = false);
    let http = PageView {
        is_https: false,
        ..healthy(B)
    };
    let slow = PageView {
        response_ms: Some(1501),
        ..healthy("https://example.com/c")
    };
    let heavy = PageView {
        bytes: Some(2 * 1024 * 1024 + 1),
        ..healthy("https://example.com/d")
    };
    let found = run(&context(vec![no_viewport, http, slow, heavy]));
    assert_eq!(only(&found, "viewport_missing")[0].target, A);
    assert_eq!(only(&found, "not_https")[0].target, B);
    assert_eq!(
        only(&found, "slow_response")[0].evidence["response_ms"],
        1501
    );
    assert_eq!(
        only(&found, "heavy_page")[0].evidence["bytes"],
        2 * 1024 * 1024 + 1
    );
    let at_limit = PageView {
        response_ms: Some(1500),
        bytes: Some(2 * 1024 * 1024),
        ..healthy(A)
    };
    assert!(run(&context(vec![at_limit])).is_empty());
}

#[test]
fn checks_skip_pages_that_are_not_working_html() {
    let pdf = PageView {
        content_type: Some("application/pdf".into()),
        facts: None,
        ..healthy(A)
    };
    let found = run(&context(vec![pdf, broken(B, 500)]));
    assert_eq!(ids(&found), ["broken_page"]);
}

#[test]
fn an_empty_crawl_produces_only_site_level_findings() {
    let mut ctx = context(vec![]);
    ctx.sitemap_found = false;
    let found = run(&ctx);
    assert_eq!(ids(&found), ["sitemap_missing"]);
    assert!(found.iter().all(|f| f.target_kind == TargetKind::Site));
}
