use serde_json::json;

use super::*;
use crate::checks::catalog::{self, Need};
use crate::checks::context::fixtures::{context, healthy};
use crate::checks::context::{RankedKeyword, RankingData};

const HOME: &str = "https://example.com/";
const A: &str = "https://example.com/a";

fn keyword(edit: impl FnOnce(&mut RankedKeyword)) -> RankedKeyword {
    let mut keyword = RankedKeyword {
        keyword: "blue widgets".into(),
        url: Some(A.into()),
        position: Some(5),
        previous: Some(Some(5)),
        ai_overview: "absent".into(),
        cited_domains: vec![],
    };
    edit(&mut keyword);
    keyword
}

fn found(keywords: Vec<RankedKeyword>) -> Vec<(&'static str, String, serde_json::Value)> {
    let mut ctx = context(vec![healthy(HOME), healthy(A)]);
    ctx.rankings = Some(RankingData { keywords });
    run(&ctx)
        .into_iter()
        .map(|f| (f.check_id, f.target, f.evidence))
        .collect()
}

#[test]
fn a_site_without_results_has_no_ranking_findings() {
    assert_eq!(run(&context(vec![healthy(HOME)])), vec![]);
}

#[test]
fn a_fall_of_five_places_fires_and_four_does_not() {
    let fell = found(vec![keyword(|k| {
        k.previous = Some(Some(3));
        k.position = Some(8);
    })]);
    assert_eq!(fell.len(), 1);
    assert_eq!(fell[0].0, "rank_dropped");
    assert_eq!(fell[0].1, "\"blue widgets\" in Google results");
    assert_eq!(
        fell[0].2,
        json!({ "keyword": "blue widgets", "url": A, "position": 8, "previous": 3 })
    );
    let small = found(vec![keyword(|k| {
        k.previous = Some(Some(3));
        k.position = Some(7);
    })]);
    assert_eq!(small, vec![]);
}

#[test]
fn leaving_the_top_100_fires_and_the_evidence_points_at_a_crawled_page() {
    let left = found(vec![keyword(|k| {
        k.previous = Some(Some(40));
        k.position = None;
        k.url = None;
    })]);
    assert_eq!(left.len(), 1);
    assert_eq!(left[0].2["position"], serde_json::Value::Null);
    assert_eq!(left[0].2["previous"], 40);
    // With no page of its own to name, the home page stands in.
    assert_eq!(left[0].2["url"], HOME);
}

#[test]
fn steady_improving_new_and_never_ranked_keywords_do_not_fire() {
    let none = found(vec![
        keyword(|k| k.previous = Some(Some(5))),
        keyword(|k| {
            k.previous = Some(Some(9));
            k.position = Some(2);
        }),
        // No earlier check to compare with.
        keyword(|k| {
            k.previous = None;
            k.position = None;
        }),
        // Was outside the top 100 and still is, or has entered it.
        keyword(|k| {
            k.previous = Some(None);
            k.position = None;
        }),
        keyword(|k| {
            k.previous = Some(None);
            k.position = Some(60);
        }),
    ]);
    assert_eq!(none, vec![]);
}

#[test]
fn an_overview_citing_others_and_not_the_site_fires_with_the_domains() {
    let hit = found(vec![keyword(|k| {
        k.ai_overview = "present".into();
        k.cited_domains = vec!["a.com".into(), "b.com".into()];
    })]);
    assert_eq!(hit.len(), 1);
    assert_eq!(hit[0].0, "ai_overview_not_cited");
    assert_eq!(hit[0].1, "\"blue widgets\" in Google results");
    assert_eq!(
        hit[0].2,
        json!({ "keyword": "blue widgets", "url": A, "cited_domains": ["a.com", "b.com"] })
    );
}

#[test]
fn the_overview_check_is_quiet_when_the_site_is_cited_or_there_is_nothing_to_compare() {
    let quiet = |status: &str, domains: Vec<String>| {
        found(vec![keyword(|k| {
            k.ai_overview = status.into();
            k.cited_domains = domains;
        })])
    };
    assert_eq!(
        quiet("cited", vec!["example.com".into(), "a.com".into()]),
        vec![]
    );
    assert_eq!(quiet("absent", vec![]), vec![]);
    assert_eq!(quiet("unknown", vec![]), vec![]);
    // An overview with no sources named cites no one else either.
    assert_eq!(quiet("present", vec![]), vec![]);
}

#[test]
fn both_checks_need_rankings() {
    for (id, category) in [
        ("rank_dropped", crate::checks::Category::Technical),
        (
            "ai_overview_not_cited",
            crate::checks::Category::AiReadiness,
        ),
    ] {
        let check = catalog::get(id).unwrap();
        assert_eq!(check.category, category);
        assert_eq!(check.needs, &[Need::Rankings]);
        assert_eq!(check.severity, 2);
    }
    assert_eq!(catalog::get("rank_dropped").unwrap().effort, 2);
    assert_eq!(catalog::get("ai_overview_not_cited").unwrap().effort, 3);
    assert!(!Need::Rankings.reason().is_empty());
}
