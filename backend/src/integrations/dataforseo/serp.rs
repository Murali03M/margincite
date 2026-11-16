//! Reads the items of a Google organic result page: where the site ranks, the
//! top results, and whether Google's AI Overview cites the site.

use serde::Serialize;
use serde_json::Value;
use url::Url;

use crate::ai::detect::on_domain;
use crate::ai::engines::bare_host;
use crate::crawl::clean::strip_nul;

pub const MAX_TEXT_CHARS: usize = 2000;
/// Organic results kept per keyword.
pub const TOP_RESULTS: usize = 10;
const MAX_REFERENCES: usize = 200;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TopResult {
    pub position: i32,
    pub domain: String,
    pub url: String,
    pub title: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiStatus {
    Absent,
    Present,
    Cited,
    Unknown,
}

impl AiStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            AiStatus::Absent => "absent",
            AiStatus::Present => "present",
            AiStatus::Cited => "cited",
            AiStatus::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct AiOverview {
    pub status: AiStatus,
    /// Every domain the overview cites, once each, without `www.`.
    pub cited_domains: Vec<String>,
    /// The addresses it cites that belong to the site.
    pub your_urls: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Serp {
    /// The best place any of the site's organic results holds (1 is first),
    /// `None` when the site is not in the results.
    pub position: Option<i32>,
    pub url: Option<String>,
    /// The first ten organic results.
    pub top: Vec<TopResult>,
    pub ai: AiOverview,
}

fn text(value: &Value, key: &str) -> String {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(|text| strip_nul(text).chars().take(MAX_TEXT_CHARS).collect())
        .unwrap_or_default()
}

/// The domain an item states, or else the host of its address.
fn domain_of(item: &Value) -> String {
    let stated = text(item, "domain");
    if !stated.is_empty() {
        return stated;
    }
    Url::parse(&text(item, "url"))
        .ok()
        .and_then(|url| url.host_str().map(str::to_string))
        .unwrap_or_default()
}

fn organic(items: &[Value]) -> Vec<TopResult> {
    let mut found: Vec<TopResult> = items
        .iter()
        .filter(|item| item.get("type").and_then(Value::as_str) == Some("organic"))
        .filter_map(|item| {
            let position = item.get("rank_absolute")?.as_i64()?;
            let (domain, url) = (domain_of(item), text(item, "url"));
            (position >= 1 && !(domain.is_empty() && url.is_empty())).then(|| TopResult {
                position: i32::try_from(position).unwrap_or(i32::MAX),
                domain,
                url,
                title: text(item, "title"),
            })
        })
        .collect();
    found.sort_by_key(|result| result.position);
    found
}

fn references<'a>(holder: &'a Value, into: &mut Vec<&'a Value>) {
    if let Some(list) = holder.get("references").and_then(Value::as_array) {
        into.extend(list);
    }
}

/// Every reference of an overview: its own, each element's, and each expanded
/// element's components'.
fn all_references(overview: &Value) -> Vec<&Value> {
    let mut found = Vec::new();
    references(overview, &mut found);
    for element in overview
        .get("items")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        references(element, &mut found);
        for component in element
            .get("components")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            references(component, &mut found);
        }
    }
    found
}

fn read_overview(items: &[Value], site_host: &str) -> AiOverview {
    let result = |status| AiOverview {
        status,
        cited_domains: Vec::new(),
        your_urls: Vec::new(),
    };
    let Some(overview) = items
        .iter()
        .find(|item| item.get("type").and_then(Value::as_str) == Some("ai_overview"))
    else {
        return result(AiStatus::Absent);
    };
    let cited = all_references(overview);
    let mut cited_domains: Vec<String> = Vec::new();
    let mut your_urls: Vec<String> = Vec::new();
    for reference in cited.iter().take(MAX_REFERENCES) {
        let domain = bare_host(&domain_of(reference));
        if domain.is_empty() {
            continue;
        }
        if on_domain(&domain, site_host) {
            let url = text(reference, "url");
            if !url.is_empty() && !your_urls.contains(&url) {
                your_urls.push(url);
            }
        }
        if !cited_domains.contains(&domain) {
            cited_domains.push(domain);
        }
    }
    let has_content = overview
        .get("items")
        .and_then(Value::as_array)
        .is_some_and(|list| !list.is_empty());
    let status = if !your_urls.is_empty() {
        AiStatus::Cited
    } else if !cited_domains.is_empty() || has_content {
        AiStatus::Present
    } else {
        // An overview with no content: not loaded, or in a shape not known.
        AiStatus::Unknown
    };
    AiOverview {
        status,
        cited_domains,
        your_urls,
    }
}

/// Reads the `items` of one organic result page for a site on `site_host`.
pub fn read(items: &[Value], site_host: &str) -> Serp {
    let results = organic(items);
    let best = results
        .iter()
        .find(|result| on_domain(&bare_host(&result.domain), site_host));
    Serp {
        position: best.map(|result| result.position),
        url: best
            .map(|result| result.url.clone())
            .filter(|url| !url.is_empty()),
        top: results.into_iter().take(TOP_RESULTS).collect(),
        ai: read_overview(items, site_host),
    }
}

#[cfg(test)]
#[path = "serp_tests.rs"]
mod tests;
