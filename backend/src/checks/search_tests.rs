use super::*;
use crate::checks::context::fixtures::{context, healthy};
use crate::checks::context::{CrawlContext, Finding, PageView, QueryPage, SearchData, TargetKind};

const A: &str = "https://example.com/a";
const B: &str = "https://example.com/b";

fn row(query: &str, page: &str, clicks: i64, impressions: i64, position: f64) -> QueryPage {
    QueryPage {
        query: query.into(),
        page: page.into(),
        clicks,
        impressions,
        ctr: if impressions > 0 {
            clicks as f64 / impressions as f64
        } else {
            0.0
        },
        position,
    }
}

fn with_search(pages: Vec<PageView>, rows: Vec<QueryPage>) -> CrawlContext {
    let mut ctx = context(pages);
    ctx.search = Some(SearchData { rows });
    ctx
}

fn only(findings: &[Finding], id: &str) -> Vec<Finding> {
    findings
        .iter()
        .filter(|f| f.check_id == id)
        .cloned()
        .collect()
}

#[test]
fn nothing_runs_without_search_data() {
    let ctx = context(vec![healthy(A)]);
    assert!(run(&ctx).is_empty());
}

#[test]
fn expected_ctr_follows_the_position() {
    assert_eq!(expected_ctr(1.0), 0.28);
    assert_eq!(expected_ctr(1.4), 0.28);
    assert_eq!(expected_ctr(2.0), 0.15);
    assert_eq!(expected_ctr(3.0), 0.10);
    assert_eq!(expected_ctr(4.0), 0.07);
    assert_eq!(expected_ctr(5.0), 0.05);
    assert_eq!(expected_ctr(6.0), 0.03);
    assert_eq!(expected_ctr(10.0), 0.03);
    assert_eq!(expected_ctr(11.0), 0.01);
    assert_eq!(expected_ctr(0.2), 0.28);
    assert_eq!(expected_ctr(80.0), 0.01);
}

#[test]
fn low_ctr_fires_below_half_of_the_expected_rate() {
    // Position 1 expects 28%; half is 14%. 100 of 1000 is 10%.
    let ctx = with_search(vec![healthy(A)], vec![row("widgets", A, 100, 1000, 1.0)]);
    let found = only(&run(&ctx), "search_low_ctr");
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].target, A);
    assert_eq!(found[0].target_kind, TargetKind::Page);
    assert_eq!(found[0].evidence["impressions"], 1000);
    assert_eq!(found[0].evidence["clicks"], 100);
    assert_eq!(found[0].evidence["ctr"], 10.0);
    assert_eq!(found[0].evidence["position"], 1.0);
}

#[test]
fn low_ctr_stays_quiet_when_ctr_is_fine_or_impressions_are_few() {
    let ctx = with_search(
        vec![healthy(A), healthy(B)],
        vec![
            row("widgets", A, 200, 1000, 1.0), // 20% against 14%: fine
            row("gadgets", B, 0, 199, 1.0),    // too few impressions
        ],
    );
    assert!(only(&run(&ctx), "search_low_ctr").is_empty());
}

#[test]
fn low_ctr_sums_a_pages_queries_and_weights_position() {
    let ctx = with_search(
        vec![healthy(A)],
        vec![row("one", A, 0, 150, 2.0), row("two", A, 0, 150, 2.0)],
    );
    let found = only(&run(&ctx), "search_low_ctr");
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].evidence["impressions"], 300);
}

#[test]
fn low_ctr_only_judges_pages_that_were_crawled_and_work() {
    let broken = PageView {
        status_code: Some(404),
        facts: None,
        ..healthy(B)
    };
    let ctx = with_search(
        vec![healthy(A), broken],
        vec![
            row("x", B, 0, 5000, 1.0),
            row("y", "https://example.com/never-crawled", 0, 5000, 1.0),
        ],
    );
    assert!(only(&run(&ctx), "search_low_ctr").is_empty());
}

#[test]
fn striking_distance_covers_positions_5_to_20_with_100_impressions() {
    let ctx = with_search(
        vec![healthy(A)],
        vec![
            row("edge low", A, 1, 100, 5.0),
            row("edge high", A, 1, 100, 20.0),
            row("too high up", A, 1, 500, 4.9),
            row("too far down", A, 1, 500, 20.1),
            row("too quiet", A, 0, 99, 8.0),
        ],
    );
    let found = only(&run(&ctx), "search_striking_distance");
    let mut targets: Vec<_> = found.iter().map(|f| f.target.clone()).collect();
    targets.sort();
    assert_eq!(
        targets,
        vec![
            format!("\"edge high\" on {A}"),
            format!("\"edge low\" on {A}")
        ]
    );
    assert_eq!(found[0].target_kind, TargetKind::Page);
    assert_eq!(found[0].evidence["url"], A);
    assert!(found[0].evidence["query"].is_string());
    assert!(found[0].evidence["position"].is_number());
}

#[test]
fn striking_distance_keeps_only_the_most_shown_pairs() {
    let rows = (0..STRIKING_LIMIT + 50)
        .map(|n| row(&format!("query {n}"), A, 0, 100 + n as i64, 8.0))
        .collect();
    let ctx = with_search(vec![healthy(A)], rows);
    let found = only(&run(&ctx), "search_striking_distance");
    assert_eq!(found.len(), STRIKING_LIMIT);
    assert!(found.iter().all(|f| f.evidence["query"] != "query 0"));
}

#[test]
fn no_impressions_lists_sitemap_pages_search_never_showed() {
    let mut off_sitemap = healthy("https://example.com/c");
    off_sitemap.in_sitemap = false;
    let ctx = with_search(
        vec![healthy(A), healthy(B), off_sitemap],
        vec![row("widgets", A, 1, 50, 3.0)],
    );
    let found = only(&run(&ctx), "search_no_impressions");
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].target, B);
    assert_eq!(found[0].target_kind, TargetKind::Page);
}

#[test]
fn no_impressions_stays_quiet_when_the_data_cannot_be_trusted() {
    // No crawled page matches any row: probably www against bare host.
    let mismatched = with_search(
        vec![healthy(A)],
        vec![row("widgets", "https://www.example.com/a", 1, 50, 3.0)],
    );
    assert!(only(&run(&mismatched), "search_no_impressions").is_empty());
    // The sync hit its row limit, so a page may simply have been cut off.
    let mut rows: Vec<_> = (0..ROW_LIMIT)
        .map(|n| row(&format!("q{n}"), A, 0, 1, 9.0))
        .collect();
    rows[0].page = A.into();
    let truncated = with_search(vec![healthy(A), healthy(B)], rows);
    assert!(only(&run(&truncated), "search_no_impressions").is_empty());
}

#[test]
fn urls_are_matched_after_normalising() {
    let ctx = with_search(
        vec![healthy("https://example.com/a?id=2")],
        vec![row(
            "widgets",
            "https://Example.com/a?id=2&utm_source=x",
            0,
            1000,
            1.0,
        )],
    );
    assert_eq!(only(&run(&ctx), "search_low_ctr").len(), 1);
}

#[test]
fn every_search_check_needs_search_console_and_nothing_else() {
    use crate::checks::catalog::{self, Available, Need};
    for id in [
        "search_low_ctr",
        "search_striking_distance",
        "search_no_impressions",
    ] {
        let check = catalog::get(id).unwrap();
        assert_eq!(check.needs, &[Need::SearchConsole], "{id}");
        assert_eq!(
            check.not_run_reason(Available::default()),
            Some(Need::SearchConsole.reason())
        );
        let connected = Available {
            search_console: true,
            ..Available::default()
        };
        assert_eq!(check.not_run_reason(connected), None, "{id}");
    }
}
