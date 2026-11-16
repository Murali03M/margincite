use serde_json::json;

use super::*;
use crate::checks::catalog::{self, Need};
use crate::checks::context::fixtures::{context, healthy};
use crate::checks::context::{SpeedData, SpeedPage};

const A: &str = "https://example.com/a";

fn page(edit: impl FnOnce(&mut SpeedPage)) -> SpeedPage {
    let mut page = SpeedPage {
        url: A.into(),
        lcp_ms: Some(1800),
        cls: Some(0.05),
        inp_ms: None,
        tbt_ms: Some(100),
    };
    edit(&mut page);
    page
}

fn found(pages: Vec<SpeedPage>) -> Vec<(&'static str, serde_json::Value)> {
    let mut ctx = context(vec![healthy(A)]);
    ctx.speed = Some(SpeedData { pages });
    run(&ctx)
        .into_iter()
        .map(|f| (f.check_id, f.evidence))
        .collect()
}

#[test]
fn a_site_without_results_has_no_speed_findings() {
    assert_eq!(run(&context(vec![healthy(A)])), vec![]);
}

#[test]
fn a_fast_page_has_no_findings() {
    assert_eq!(found(vec![page(|_| {})]), vec![]);
    // Exactly on a limit is not over it.
    let on_limits = page(|p| {
        p.lcp_ms = Some(2500);
        p.cls = Some(0.1);
        p.inp_ms = Some(200);
    });
    assert_eq!(found(vec![on_limits]), vec![]);
}

#[test]
fn slow_lcp_fires_over_2500_ms() {
    let slow = found(vec![page(|p| p.lcp_ms = Some(2501))]);
    assert_eq!(slow, vec![("speed_slow_lcp", json!({"lcp_ms": 2501}))]);
}

#[test]
fn layout_shift_fires_over_a_tenth() {
    let shifty = found(vec![page(|p| p.cls = Some(0.11))]);
    assert_eq!(shifty, vec![("speed_layout_shift", json!({"cls": 0.11}))]);
}

#[test]
fn interaction_uses_inp_when_there_is_field_data_and_tbt_when_there_is_not() {
    let field_slow = found(vec![page(|p| p.inp_ms = Some(250))]);
    assert_eq!(
        field_slow,
        vec![("speed_slow_interaction", json!({"inp_ms": 250}))]
    );
    // A good INP is not overruled by a high lab blocking time.
    assert_eq!(
        found(vec![page(|p| {
            p.inp_ms = Some(150);
            p.tbt_ms = Some(900);
        })]),
        vec![]
    );
    let lab_slow = found(vec![page(|p| p.tbt_ms = Some(301))]);
    assert_eq!(
        lab_slow,
        vec![("speed_slow_interaction", json!({"tbt_ms": 301}))]
    );
    assert_eq!(found(vec![page(|p| p.tbt_ms = Some(300))]), vec![]);
}

#[test]
fn unknown_numbers_never_fire() {
    let unknown = page(|p| {
        p.lcp_ms = None;
        p.cls = None;
        p.inp_ms = None;
        p.tbt_ms = None;
    });
    assert_eq!(found(vec![unknown]), vec![]);
}

#[test]
fn findings_target_the_tested_page() {
    let mut ctx = context(vec![healthy(A)]);
    ctx.speed = Some(SpeedData {
        pages: vec![page(|p| p.lcp_ms = Some(9000))],
    });
    let finding = &run(&ctx)[0];
    assert_eq!(finding.target, A);
    assert_eq!(finding.target_kind, crate::checks::TargetKind::Page);
}

#[test]
fn the_speed_checks_are_technical_and_need_speed_results() {
    for id in [
        "speed_slow_lcp",
        "speed_layout_shift",
        "speed_slow_interaction",
    ] {
        let check = catalog::get(id).unwrap();
        assert_eq!(check.category, crate::checks::Category::Technical);
        assert_eq!(check.needs, &[Need::Speed]);
        assert_eq!(check.severity, 2);
    }
    assert_eq!(catalog::get("speed_layout_shift").unwrap().effort, 2);
    assert_eq!(catalog::get("speed_slow_lcp").unwrap().effort, 3);
    assert!(!Need::Speed.reason().is_empty());
}
