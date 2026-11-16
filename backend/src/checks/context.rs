//! The inputs and outputs shared by every check.

use serde::{Deserialize, Serialize};
use url::Url;

use crate::crawl::extract::PageFacts;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Category {
    Technical,
    AiReadiness,
    Content,
}

impl Category {
    pub const ALL: [Category; 3] = [
        Category::Technical,
        Category::AiReadiness,
        Category::Content,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Category::Technical => "technical",
            Category::AiReadiness => "ai_readiness",
            Category::Content => "content",
        }
    }

    pub fn parse(text: &str) -> Option<Category> {
        Category::ALL.into_iter().find(|c| c.as_str() == text)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TargetKind {
    Page,
    Site,
}

impl TargetKind {
    pub fn as_str(self) -> &'static str {
        match self {
            TargetKind::Page => "page",
            TargetKind::Site => "site",
        }
    }
}

/// One crawled page, as the checks see it.
#[derive(Debug, Clone)]
pub struct PageView {
    pub url: String,
    pub depth: i32,
    pub in_sitemap: bool,
    pub status_code: Option<i32>,
    pub content_type: Option<String>,
    pub fetch_error: Option<String>,
    pub redirects: Vec<String>,
    pub response_ms: Option<i32>,
    pub bytes: Option<i32>,
    pub is_https: bool,
    pub x_robots_tag: Option<String>,
    pub needed_javascript: bool,
    pub facts: Option<PageFacts>,
}

impl PageView {
    /// A page that answered 200 and was read as HTML.
    pub fn is_html_ok(&self) -> bool {
        self.status_code == Some(200) && self.facts.is_some()
    }

    /// The page's facts, only when it is working HTML.
    pub fn html_facts(&self) -> Option<&PageFacts> {
        if self.is_html_ok() {
            self.facts.as_ref()
        } else {
            None
        }
    }

    /// An error status or a failed fetch.
    pub fn is_broken(&self) -> bool {
        self.fetch_error.is_some() || self.status_code.is_some_and(|code| code >= 400)
    }
}

/// A word or phrase a person wants the site to be found for, and the page
/// they chose for it, if any.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeywordTarget {
    pub keyword: String,
    pub target_url: Option<String>,
}

/// One search query and one page that Search Console showed for it, over the
/// last 28 days. `page` is stored in `crawl::urls::normalize` form, the same
/// form crawled pages use.
#[derive(Debug, Clone, PartialEq)]
pub struct QueryPage {
    pub query: String,
    pub page: String,
    pub clicks: i64,
    pub impressions: i64,
    pub ctr: f64,
    /// Average position over the impressions in this row (1 is the top).
    pub position: f64,
}

/// What Search Console knows about one page, summed over its query rows.
#[derive(Debug, Clone, PartialEq)]
pub struct PageStats {
    pub clicks: i64,
    pub impressions: i64,
    /// Click-through rate, 0.0 when there were no impressions.
    pub ctr: f64,
    /// Average position weighted by impressions.
    pub position: f64,
}

/// What Search Console knows about the site, bounded by the sync's row limit.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SearchData {
    pub rows: Vec<QueryPage>,
}

impl SearchData {
    /// Per-page totals, keyed by page address.
    pub fn pages(&self) -> std::collections::HashMap<&str, PageStats> {
        let mut sums: std::collections::HashMap<&str, (i64, i64, f64)> = Default::default();
        for row in &self.rows {
            let entry = sums.entry(row.page.as_str()).or_default();
            entry.0 += row.clicks;
            entry.1 += row.impressions;
            entry.2 += row.position * row.impressions as f64;
        }
        sums.into_iter()
            .map(|(page, (clicks, impressions, weighted))| {
                let impressions_f = impressions.max(1) as f64;
                (
                    page,
                    PageStats {
                        clicks,
                        impressions,
                        ctr: if impressions > 0 {
                            clicks as f64 / impressions as f64
                        } else {
                            0.0
                        },
                        position: weighted / impressions_f,
                    },
                )
            })
            .collect()
    }
}

/// One engine's answer to one question, as the checks see it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiAnswerView {
    pub prompt: String,
    pub engine: String,
    pub mentioned: bool,
    pub cited: bool,
    /// The engine gave no answer (an error, a refusal, a timeout).
    pub failed: bool,
    /// The domains the answer cited, each once, in citation order.
    pub cited_domains: Vec<String>,
    pub competitors: Vec<String>,
}

/// The answers of the latest completed AI visibility run, for the questions
/// still on the list.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AiData {
    pub answers: Vec<AiAnswerView>,
}

/// One page's PageSpeed numbers. Unknown ones are `None`.
#[derive(Debug, Clone, PartialEq)]
pub struct SpeedPage {
    pub url: String,
    pub lcp_ms: Option<i32>,
    pub cls: Option<f64>,
    pub inp_ms: Option<i32>,
    pub tbt_ms: Option<i32>,
}

/// The site's latest PageSpeed results.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SpeedData {
    pub pages: Vec<SpeedPage>,
}

/// A keyword's newest search result and the one before it, in the same place.
#[derive(Debug, Clone, PartialEq)]
pub struct RankedKeyword {
    pub keyword: String,
    /// The site's page that ranks, when one does.
    pub url: Option<String>,
    /// `None` is "not in the top 100".
    pub position: Option<i32>,
    /// `None` when there is no earlier check; `Some(None)` when the earlier
    /// check had the site outside the top 100.
    pub previous: Option<Option<i32>>,
    /// `cited`, `present`, `absent` or `unknown`.
    pub ai_overview: String,
    /// The domains Google's AI Overview cited.
    pub cited_domains: Vec<String>,
}

/// The site's latest search results for its keywords.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RankingData {
    pub keywords: Vec<RankedKeyword>,
}

/// Everything a crawl found, loaded once and read by every check.
#[derive(Debug, Clone)]
pub struct CrawlContext {
    pub root: Url,
    pub robots_txt: Option<String>,
    pub sitemap_found: bool,
    pub pages: Vec<PageView>,
    /// The site's target keywords, oldest first. Empty when none were added.
    pub keywords: Vec<KeywordTarget>,
    /// `None` until Search Console is connected and has synced.
    pub search: Option<SearchData>,
    /// `None` until an AI visibility run has completed.
    pub ai: Option<AiData>,
    /// `None` until a page has been tested without an error.
    pub speed: Option<SpeedData>,
    /// `None` until a keyword has been checked in the site's search results.
    pub rankings: Option<RankingData>,
    /// The site's host, without `www.`.
    pub site_host: String,
}

impl CrawlContext {
    /// The page at the site's own address. Depth cannot tell: pages listed in
    /// the sitemap are stored at depth 0 as well. `www.`, the scheme and a
    /// trailing slash are ignored, since a site often redirects between them.
    pub fn is_home(&self, page: &PageView) -> bool {
        let Ok(url) = Url::parse(&page.url) else {
            return false;
        };
        url.query().is_none()
            && bare_host(&url) == bare_host(&self.root)
            && url.path().trim_end_matches('/') == self.root.path().trim_end_matches('/')
    }
}

fn bare_host(url: &Url) -> String {
    let host = url.host_str().unwrap_or_default().to_ascii_lowercase();
    host.strip_prefix("www.").unwrap_or(&host).to_string()
}

#[derive(Debug, Clone, PartialEq)]
pub struct Finding {
    pub check_id: &'static str,
    pub target: String,
    pub target_kind: TargetKind,
    pub evidence: serde_json::Value,
}

impl Finding {
    pub fn page(check_id: &'static str, target: &str, evidence: serde_json::Value) -> Self {
        Self {
            check_id,
            target: target.to_string(),
            target_kind: TargetKind::Page,
            evidence,
        }
    }

    pub fn site(check_id: &'static str, target: &str, evidence: serde_json::Value) -> Self {
        Self {
            check_id,
            target: target.to_string(),
            target_kind: TargetKind::Site,
            evidence,
        }
    }
}

#[cfg(test)]
pub mod fixtures {
    use super::*;

    pub fn root() -> Url {
        Url::parse("https://example.com/").unwrap()
    }

    /// A working page that triggers no check at all.
    pub fn healthy(url: &str) -> PageView {
        let words = "word ".repeat(400);
        let facts = PageFacts {
            title: Some(format!("A healthy page title for {url}")),
            meta_description: Some(format!(
                "A clear description of what this page offers to visitors, written for {url}"
            )),
            h1s: vec!["Heading".into()],
            headings: vec![],
            canonical: None,
            robots_meta: None,
            has_viewport: true,
            word_count: 400,
            text: format!("{words}{url}"),
            structured_data: vec![serde_json::json!({"@type": "Article"})],
            structured_data_errors: 0,
            links: vec![],
            images: vec![],
            has_scripts: false,
            published: Some("2026-01-01".into()),
            modified: None,
            author: Some("A. Writer".into()),
            question_headings: Some(1),
            lists: Some(1),
            tables: Some(0),
            first_paragraph_words: Some(40),
            faq_schema: Some(false),
            howto_schema: Some(false),
            article_schema: Some(true),
            organization_schema: Some(true),
            breadcrumb_schema: Some(false),
            outbound_citations: Some(1),
        };
        PageView {
            url: url.to_string(),
            depth: 1,
            in_sitemap: true,
            status_code: Some(200),
            content_type: Some("text/html".into()),
            fetch_error: None,
            redirects: vec![],
            response_ms: Some(200),
            bytes: Some(5_000),
            is_https: true,
            x_robots_tag: None,
            needed_javascript: false,
            facts: Some(facts),
        }
    }

    pub fn with_facts(url: &str, edit: impl FnOnce(&mut PageFacts)) -> PageView {
        let mut page = healthy(url);
        if let Some(facts) = page.facts.as_mut() {
            edit(facts);
        }
        page
    }

    pub fn context(pages: Vec<PageView>) -> CrawlContext {
        CrawlContext {
            root: root(),
            robots_txt: None,
            sitemap_found: true,
            pages,
            keywords: vec![],
            search: None,
            ai: None,
            speed: None,
            rankings: None,
            site_host: "example.com".into(),
        }
    }
}
