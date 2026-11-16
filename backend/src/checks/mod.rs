//! Checks are pure rules: a crawl's pages in, findings out.

pub mod ai;
pub mod ai_answers;
pub mod answer;
pub mod availability;
pub mod catalog;
pub mod content;
pub mod context;
pub mod keywords;
pub mod rankings;
pub mod search;
pub mod speed;
pub mod store;
pub mod technical;

pub use context::{Category, CrawlContext, Finding, PageView, TargetKind};

/// Every check, sorted by check id then target so a rerun gives the same list.
pub fn run_all(ctx: &CrawlContext) -> Vec<Finding> {
    let mut findings = [
        technical::run(ctx),
        ai::run(ctx),
        ai_answers::run(ctx),
        answer::run(ctx),
        content::run(ctx),
        keywords::run(ctx),
        search::run(ctx),
        speed::run(ctx),
        rankings::run(ctx),
    ]
    .concat();
    findings.sort_by(|a, b| (a.check_id, &a.target).cmp(&(b.check_id, &b.target)));
    findings
}

#[cfg(test)]
mod run_all_tests {
    use super::*;
    use crate::checks::context::fixtures::{context, healthy};

    #[test]
    fn run_all_is_sorted_and_stable() {
        let bad = |url: &str| PageView {
            status_code: Some(500),
            facts: None,
            needed_javascript: true,
            ..healthy(url)
        };
        let mut ctx = context(vec![
            bad("https://example.com/z"),
            bad("https://example.com/a"),
        ]);
        ctx.robots_txt = Some("User-agent: *\nDisallow: /\n".into());
        let first = run_all(&ctx);
        assert!(first.len() > 10);
        let keys: Vec<_> = first
            .iter()
            .map(|f| (f.check_id, f.target.clone()))
            .collect();
        let mut sorted = keys.clone();
        sorted.sort();
        assert_eq!(keys, sorted);
        assert_eq!(first, run_all(&ctx));
    }
}

#[cfg(test)]
mod catalog_tests {
    use super::catalog;
    use std::collections::HashSet;

    #[test]
    fn catalog_ids_are_unique_and_every_field_is_filled() {
        let mut seen = HashSet::new();
        for check in catalog::all() {
            assert!(seen.insert(check.id), "duplicate id {}", check.id);
            assert!(!check.id.is_empty() && !check.title.is_empty());
            assert!(!check.why.is_empty() && !check.fix.is_empty());
            assert!((1..=3).contains(&check.severity), "{}", check.id);
            assert!((1..=3).contains(&check.effort), "{}", check.id);
            assert_eq!(catalog::get(check.id).map(|c| c.id), Some(check.id));
        }
        assert!(catalog::get("nope").is_none());
    }

    #[test]
    fn checks_that_need_something_are_not_run_without_it() {
        use catalog::{Available, Need};
        let none = Available::default();
        let title = catalog::get("keyword_not_in_title").unwrap();
        assert_eq!(title.needs, &[Need::Keywords]);
        assert_eq!(title.not_run_reason(none), Some(Need::Keywords.reason()));
        let keywords = Available {
            keywords: true,
            ..none
        };
        assert_eq!(title.not_run_reason(keywords), None);
        let cannibal = catalog::get("keyword_cannibalization").unwrap();
        assert_eq!(
            cannibal.not_run_reason(keywords),
            Some(Need::SearchConsole.reason())
        );
        let both = Available {
            search_console: true,
            ..keywords
        };
        assert_eq!(cannibal.not_run_reason(both), None);
        assert_eq!(
            catalog::get("title_missing").unwrap().not_run_reason(none),
            None
        );
        assert!(Available {
            ai_run: true,
            ..none
        }
        .has(Need::AiRun));
        assert!(!Need::AiRun.reason().is_empty());
    }

    #[test]
    fn the_severity_total_grows_only_with_what_a_site_has() {
        use catalog::{severity_total, Available};
        let none = severity_total(Available::default());
        let keywords = severity_total(Available {
            keywords: true,
            ..Available::default()
        });
        let everything = severity_total(Available {
            keywords: true,
            search_console: true,
            ai_run: true,
            speed: true,
            rankings: true,
        });
        assert!(none < keywords && keywords < everything);
        let all: u32 = catalog::all().iter().map(|c| u32::from(c.severity)).sum();
        assert_eq!(everything, all);
    }

    #[test]
    fn catalog_copy_never_promises_results() {
        for check in catalog::all() {
            for text in [check.title, check.why, check.fix] {
                let lower = text.to_lowercase();
                for banned in ["guarantee", "will rank", "will be cited", "boost"] {
                    assert!(!lower.contains(banned), "{}: {text}", check.id);
                }
            }
        }
    }
}
