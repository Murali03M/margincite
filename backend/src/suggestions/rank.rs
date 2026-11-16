//! Ranking and the site score. Plain arithmetic, no I/O.

use std::collections::{HashMap, HashSet};

use crate::checks::{catalog, TargetKind};

/// Severity times reach, divided by effort.
pub fn priority(severity: u8, effort: u8, reach: f64) -> f64 {
    f64::from(severity) * reach / f64::from(effort.max(1))
}

/// The share of working pages a suggestion touches, from 0.0 to 1.0.
/// A site-level check with any open item reaches the whole site.
pub fn reach(open_items: i64, html_pages: i64, kind: TargetKind) -> f64 {
    if kind == TargetKind::Site && open_items > 0 {
        return 1.0;
    }
    if html_pages <= 0 || open_items <= 0 {
        return 0.0;
    }
    (open_items as f64 / html_pages as f64).min(1.0)
}

/// The share of the site's search impressions that land on these pages, from
/// 0.0 to 1.0. Each page counts once however many items point at it. `None`
/// when the site has no impressions to share out, so callers fall back to
/// the share of pages.
pub fn impression_reach<'a>(
    open_pages: impl IntoIterator<Item = &'a str>,
    by_page: &HashMap<String, i64>,
) -> Option<f64> {
    let total: i64 = by_page.values().sum();
    if total <= 0 {
        return None;
    }
    let distinct: HashSet<&str> = open_pages.into_iter().collect();
    let landing: i64 = distinct.iter().filter_map(|page| by_page.get(*page)).sum();
    Some((landing as f64 / total as f64).clamp(0.0, 1.0))
}

/// 100 minus the weight of everything still open, or `None` when the crawl
/// had no working HTML page to judge.
pub fn score(per_check: &[(u8, f64)], catalog_severity_total: u32, html_pages: i64) -> Option<u8> {
    if html_pages <= 0 || catalog_severity_total == 0 {
        return None;
    }
    let lost: f64 = per_check
        .iter()
        .map(|(severity, reach)| f64::from(*severity) * reach)
        .sum();
    let share = lost / f64::from(catalog_severity_total);
    Some((100.0 * (1.0 - share)).round().clamp(0.0, 100.0) as u8)
}

/// The sum of every check's severity: the most a site could lose.
pub fn catalog_severity_total() -> u32 {
    catalog::all().iter().map(|c| u32::from(c.severity)).sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn priority_is_severity_times_reach_over_effort() {
        assert!((priority(3, 2, 0.5) - 0.75).abs() < 1e-9);
        assert_eq!(priority(3, 0, 1.0), 3.0);
    }

    #[test]
    fn reach_never_exceeds_one_and_never_divides_by_zero() {
        assert_eq!(reach(5, 0, TargetKind::Page), 0.0);
        assert_eq!(reach(0, 10, TargetKind::Page), 0.0);
        assert_eq!(reach(50, 10, TargetKind::Page), 1.0);
        assert_eq!(reach(1, 4, TargetKind::Page), 0.25);
        assert_eq!(reach(1, 0, TargetKind::Site), 1.0);
        assert_eq!(reach(0, 5, TargetKind::Site), 0.0);
    }

    fn impressions(pairs: &[(&str, i64)]) -> HashMap<String, i64> {
        pairs.iter().map(|(p, n)| (p.to_string(), *n)).collect()
    }

    #[test]
    fn impression_reach_is_the_share_of_impressions_on_the_open_pages() {
        let by_page = impressions(&[("/a", 600), ("/b", 300), ("/c", 100)]);
        assert_eq!(impression_reach(["/a"], &by_page), Some(0.6));
        assert_eq!(impression_reach(["/b", "/c"], &by_page), Some(0.4));
        assert_eq!(impression_reach(["/a", "/a", "/a"], &by_page), Some(0.6));
        assert_eq!(impression_reach(["/unseen"], &by_page), Some(0.0));
        assert_eq!(impression_reach([], &by_page), Some(0.0));
    }

    #[test]
    fn impression_reach_is_none_without_impressions() {
        assert_eq!(impression_reach(["/a"], &HashMap::new()), None);
        assert_eq!(impression_reach(["/a"], &impressions(&[("/a", 0)])), None);
    }

    #[test]
    fn score_is_100_with_nothing_open_and_falls_with_more() {
        let total = catalog_severity_total();
        assert_eq!(score(&[], total, 3), Some(100));
        let some = score(&[(3, 1.0)], total, 3).unwrap();
        let more = score(&[(3, 1.0), (3, 1.0)], total, 3).unwrap();
        assert!(some < 100 && more < some);
    }

    #[test]
    fn score_is_clamped_and_none_without_working_pages() {
        assert_eq!(score(&[(3, 1.0)], 3, 0), None);
        assert_eq!(score(&[(3, 1.0), (3, 1.0)], 3, 1), Some(0));
        assert_eq!(score(&[(3, 1.0)], 0, 1), None);
    }
}
