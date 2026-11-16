use super::*;
use crate::checks::context::fixtures::{context, healthy, with_facts};
use crate::checks::context::{KeywordTarget, PageView, QueryPage, SearchData, TargetKind};
use serde_json::json;

type Edit = fn(&mut crate::crawl::extract::PageFacts);

const A: &str = "https://example.com/a";
const B: &str = "https://example.com/b";

/// A site with one keyword aimed at `page`.
fn aimed(keyword: &str, page: PageView) -> CrawlContext {
    let mut ctx = context(vec![page.clone()]);
    ctx.keywords = vec![KeywordTarget {
        keyword: keyword.into(),
        target_url: Some(page.url),
    }];
    ctx
}

fn ids(findings: &[Finding]) -> Vec<&'static str> {
    findings.iter().map(|f| f.check_id).collect()
}

fn only<'a>(findings: &'a [Finding], id: &str) -> Vec<&'a Finding> {
    findings.iter().filter(|f| f.check_id == id).collect()
}

/// A page that mentions `kw` everywhere it should, once in the text.
fn good(kw: &str) -> PageView {
    with_facts(A, |facts| {
        facts.title = Some(format!("The best {kw} guide"));
        facts.h1s = vec![format!("All about {kw}")];
        facts.meta_description = Some(format!("Learn about {kw} with clear steps."));
        facts.text = format!("{} {kw}", "word ".repeat(400));
    })
}

#[test]
fn a_page_that_uses_the_keyword_everywhere_is_clean() {
    assert_eq!(
        ids(&run(&aimed("blue widgets", good("blue widgets")))),
        Vec::<&str>::new()
    );
}

#[test]
fn no_keywords_means_no_findings() {
    assert!(run(&context(vec![healthy(A)])).is_empty());
}

#[test]
fn title_check_fires_and_reports_the_evidence() {
    let page = with_facts(A, |f| f.title = Some("Something else".into()));
    let findings = run(&aimed("widgets", with_all_but_title(page)));
    let hit = only(&findings, "keyword_not_in_title");
    assert_eq!(hit.len(), 1);
    assert_eq!(hit[0].target, format!("\"widgets\" on {A}"));
    assert_eq!(hit[0].target_kind, TargetKind::Page);
    assert_eq!(
        hit[0].evidence,
        json!({"keyword": "widgets", "url": A, "landing_source": "set"})
    );
    assert!(only(&findings, "keyword_not_in_h1").is_empty());
}

/// Puts the keyword "widgets" everywhere except the title.
fn with_all_but_title(mut page: PageView) -> PageView {
    let facts = page.facts.as_mut().unwrap();
    facts.h1s = vec!["Widgets".into()];
    facts.meta_description = Some("About widgets".into());
    facts.text = format!("{} widgets", "word ".repeat(400));
    page
}

#[test]
fn h1_description_and_body_checks_each_fire_alone() {
    let cases: [(&str, Edit); 3] = [
        ("keyword_not_in_h1", |f| f.h1s = vec!["Other".into()]),
        ("keyword_not_in_description", |f| f.meta_description = None),
        ("keyword_not_in_body", |f| f.text = "word ".repeat(400)),
    ];
    for (check, edit) in cases {
        let page = with_facts(A, |facts| {
            facts.title = Some("widgets".into());
            facts.h1s = vec!["widgets".into()];
            facts.meta_description = Some("widgets".into());
            facts.text = format!("{} widgets", "word ".repeat(400));
            edit(facts);
        });
        assert_eq!(ids(&run(&aimed("widgets", page))), vec![check], "{check}");
    }
}

#[test]
fn any_main_heading_may_carry_the_keyword() {
    let page = with_facts(A, |f| {
        f.title = Some("widgets".into());
        f.h1s = vec!["Intro".into(), "Our widgets".into()];
        f.meta_description = Some("widgets".into());
        f.text = format!("{} widgets", "word ".repeat(400));
    });
    assert!(run(&aimed("widgets", page)).is_empty());
}

fn sorted(findings: &[Finding]) -> Vec<&'static str> {
    let mut found = ids(findings);
    found.sort_unstable();
    found
}

#[test]
fn a_multi_word_keyword_needs_every_word_in_any_order() {
    let ok = with_facts(A, |f| {
        f.title = Some("Widgets, blue and shiny".into());
        f.h1s = vec!["Blue widgets".into()];
        f.meta_description = Some("Our widgets are blue".into());
        f.text = format!("{} blue widgets", "word ".repeat(400));
    });
    assert!(run(&aimed("blue widgets", ok)).is_empty());
    let partial = with_facts(A, |f| {
        f.title = Some("Blue things".into());
        f.h1s = vec!["Blue".into()];
        f.meta_description = Some("Widgets".into());
        f.text = format!("{} blue", "word ".repeat(400));
    });
    assert_eq!(
        sorted(&run(&aimed("blue widgets", partial))),
        [
            "keyword_not_in_body",
            "keyword_not_in_description",
            "keyword_not_in_h1",
            "keyword_not_in_title"
        ]
    );
}

#[test]
fn matching_ignores_case_punctuation_and_extra_spaces() {
    let page = with_facts(A, |f| {
        f.title = Some("BLUE,   widgets!".into());
        f.h1s = vec!["(Blue) Widgets?".into()];
        f.meta_description = Some("blue-widgets".into());
        f.text = format!("{} Blue\n\nWIDGETS", "word ".repeat(400));
    });
    assert!(run(&aimed("  blue   Widgets ", page)).is_empty());
    let apostrophe = with_facts(A, |f| {
        f.title = Some("Men's shoes".into());
        f.h1s = vec!["Mens shoes".into()];
        f.meta_description = Some("mens shoes".into());
        f.text = format!("{} Men\u{2019}s shoes", "word ".repeat(400));
    });
    assert!(run(&aimed("men's shoes", apostrophe)).is_empty());
}

#[test]
fn matching_is_on_whole_words() {
    let page = with_facts(A, |f| {
        f.title = Some("cbuddyai".into());
        f.h1s = vec!["pineapple".into()];
        f.meta_description = Some("buddying".into());
        f.text = format!("{} cbuddy", "word ".repeat(400));
    });
    assert_eq!(
        sorted(&run(&aimed("buddy", page))),
        [
            "keyword_not_in_body",
            "keyword_not_in_description",
            "keyword_not_in_h1",
            "keyword_not_in_title"
        ]
    );
    let apple = with_facts(A, |f| {
        f.title = Some("Apple".into());
        f.h1s = vec!["Apple".into()];
        f.meta_description = Some("Apple".into());
        f.text = format!("{} apple", "word ".repeat(400));
    });
    assert!(run(&aimed("apple", apple)).is_empty());
}

#[test]
fn a_keyword_that_is_over_3_percent_of_the_words_is_stuffing() {
    let mut page = good("widgets");
    page.facts.as_mut().unwrap().text = format!("{} {}", "word ".repeat(90), "widgets ".repeat(10));
    let found = run(&aimed("widgets", page));
    assert_eq!(ids(&found), vec!["keyword_stuffing"]);
    let mut fine = good("widgets");
    fine.facts.as_mut().unwrap().text = format!("{} {}", "word ".repeat(97), "widgets ".repeat(3));
    assert!(only(&run(&aimed("widgets", fine)), "keyword_stuffing").is_empty());
}

#[test]
fn a_two_word_keyword_counts_both_words_towards_the_density() {
    // Three uses of a two-word keyword in 150 words: 6 of 150 words, 4%.
    let mut page = good("blue widgets");
    page.facts.as_mut().unwrap().text =
        format!("{} {}", "word ".repeat(144), "blue widgets ".repeat(3));
    assert_eq!(
        ids(&run(&aimed("blue widgets", page))),
        vec!["keyword_stuffing"]
    );
    // The same three uses in 300 words: 6 of 300 words, 2%.
    let mut fine = good("blue widgets");
    fine.facts.as_mut().unwrap().text =
        format!("{} {}", "word ".repeat(294), "blue widgets ".repeat(3));
    assert!(only(&run(&aimed("blue widgets", fine)), "keyword_stuffing").is_empty());
}

#[test]
fn three_repeats_in_the_title_is_stuffing() {
    let mut page = good("widgets");
    page.facts.as_mut().unwrap().title = Some("Widgets widgets widgets for sale".into());
    assert_eq!(ids(&run(&aimed("widgets", page))), vec!["keyword_stuffing"]);
    let mut two = good("widgets");
    two.facts.as_mut().unwrap().title = Some("Widgets and more widgets".into());
    assert!(run(&aimed("widgets", two)).is_empty());
}

#[test]
fn an_unstored_body_does_not_fire_the_body_or_stuffing_checks() {
    let mut page = good("widgets");
    page.facts.as_mut().unwrap().text = String::new();
    assert!(run(&aimed("widgets", page)).is_empty());
}

#[test]
fn a_keyword_with_no_words_is_ignored() {
    assert!(run(&aimed("!!! ???", healthy(A))).is_empty());
}

#[test]
fn a_keyword_with_no_landing_page_is_skipped() {
    let mut ctx = context(vec![]);
    ctx.keywords = vec![KeywordTarget {
        keyword: "x".into(),
        target_url: None,
    }];
    assert!(run(&ctx).is_empty());
}

#[test]
fn each_keyword_is_checked_on_its_own_landing_page() {
    let mut ctx = context(vec![good("alpha"), PageView { ..good("beta") }]);
    ctx.pages[1].url = B.into();
    ctx.keywords = vec![
        KeywordTarget {
            keyword: "alpha".into(),
            target_url: Some(A.into()),
        },
        KeywordTarget {
            keyword: "beta".into(),
            target_url: Some(A.into()),
        },
    ];
    let found = run(&ctx);
    assert!(found.iter().all(|f| f.evidence["keyword"] == "beta"));
    assert!(!found.is_empty());
}

fn row(query: &str, page: &str, impressions: i64) -> QueryPage {
    QueryPage {
        query: query.into(),
        page: page.into(),
        clicks: 0,
        impressions,
        ctr: 0.0,
        position: 3.0,
    }
}

fn two_pages(rows: Vec<QueryPage>) -> CrawlContext {
    let mut ctx = context(vec![good("widgets"), {
        let mut other = good("widgets");
        other.url = B.into();
        other
    }]);
    ctx.keywords = vec![KeywordTarget {
        keyword: "widgets".into(),
        target_url: Some(A.into()),
    }];
    ctx.search = Some(SearchData { rows });
    ctx
}

#[test]
fn cannibalization_needs_two_pages_with_a_fifth_of_the_impressions_each() {
    let found = run(&two_pages(vec![
        row("Widgets", A, 600),
        row("widgets", B, 300),
        row("widgets", "https://example.com/c", 100),
    ]));
    let hit = only(&found, "keyword_cannibalization");
    assert_eq!(hit.len(), 1);
    assert_eq!(hit[0].target, "\"widgets\"");
    assert_eq!(hit[0].target_kind, TargetKind::Site);
    assert_eq!(
        hit[0].evidence,
        json!({"keyword": "widgets", "pages": [{"url": A, "share": 60.0}, {"url": B, "share": 30.0}]})
    );
}

#[test]
fn one_dominant_page_is_not_cannibalization() {
    let found = run(&two_pages(vec![
        row("widgets", A, 900),
        row("widgets", B, 100),
    ]));
    assert!(only(&found, "keyword_cannibalization").is_empty());
}

#[test]
fn without_search_data_or_for_another_query_it_does_not_run() {
    let mut none = two_pages(vec![]);
    none.search = None;
    assert!(only(&run(&none), "keyword_cannibalization").is_empty());
    let other = run(&two_pages(vec![
        row("gadgets", A, 500),
        row("gadgets", B, 500),
    ]));
    assert!(only(&other, "keyword_cannibalization").is_empty());
    let empty = run(&two_pages(vec![]));
    assert!(only(&empty, "keyword_cannibalization").is_empty());
}
