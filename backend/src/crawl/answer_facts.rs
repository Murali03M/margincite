//! Facts about how easy a page is to quote in an answer, read from HTML that
//! was already fetched.

use scraper::{ElementRef, Html, Selector};
use url::Url;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AnswerFacts {
    pub question_headings: u32,
    pub lists: u32,
    pub tables: u32,
    pub first_paragraph_words: u32,
    pub faq_schema: bool,
    pub howto_schema: bool,
    pub article_schema: bool,
    pub organization_schema: bool,
    pub breadcrumb_schema: bool,
    pub outbound_citations: u32,
}

const QUESTION_WORDS: [&str; 11] = [
    "who", "what", "when", "where", "why", "how", "can", "does", "is", "are", "should",
];
const ARTICLE_TYPES: [&str; 5] = [
    "Article",
    "BlogPosting",
    "NewsArticle",
    "TechArticle",
    "ScholarlyArticle",
];
const ORGANIZATION_TYPES: [&str; 4] = ["Organization", "Corporation", "LocalBusiness", "NGO"];
/// Nesting deeper than this is ignored, so a crafted block cannot recurse far.
const MAX_SCHEMA_DEPTH: usize = 12;

fn sel(query: &str) -> Selector {
    Selector::parse(query).expect("selector")
}

pub fn read(document: &Html, page_url: &Url, structured: &[serde_json::Value]) -> AnswerFacts {
    let mut types = Vec::new();
    for value in structured {
        collect_types(value, 0, &mut types);
    }
    let has = |wanted: &[&str]| types.iter().any(|kind| wanted.contains(&kind.as_str()));
    AnswerFacts {
        question_headings: question_headings(document),
        lists: main_count(document, "ul, ol"),
        tables: main_count(document, "table"),
        first_paragraph_words: first_paragraph_words(document),
        faq_schema: has(&["FAQPage"]),
        howto_schema: has(&["HowTo"]),
        article_schema: has(&ARTICLE_TYPES),
        organization_schema: has(&ORGANIZATION_TYPES),
        breadcrumb_schema: has(&["BreadcrumbList"]),
        outbound_citations: outbound_citations(document, page_url),
    }
}

fn question_headings(document: &Html) -> u32 {
    document
        .select(&sel("h1, h2, h3, h4, h5, h6"))
        .filter(|el| {
            let text = el.text().collect::<String>();
            let text = text.trim();
            let first = text
                .split(|c: char| !c.is_alphanumeric())
                .next()
                .unwrap_or("")
                .to_lowercase();
            !text.is_empty() && (text.ends_with('?') || QUESTION_WORDS.contains(&first.as_str()))
        })
        .count() as u32
}

/// Menus, headers, footers and sidebars are not the page's own text.
fn in_boilerplate(el: &ElementRef<'_>) -> bool {
    el.ancestors().any(|ancestor| {
        matches!(
            ancestor.value(),
            scraper::Node::Element(found)
                if matches!(found.name(), "nav" | "header" | "footer" | "aside")
        )
    })
}

fn main_count(document: &Html, query: &str) -> u32 {
    document
        .select(&sel(query))
        .filter(|el| !in_boilerplate(el))
        .count() as u32
}

fn first_paragraph_words(document: &Html) -> u32 {
    document
        .select(&sel("p"))
        .filter(|el| !in_boilerplate(el))
        .map(|el| el.text().collect::<String>().split_whitespace().count() as u32)
        .find(|words| *words > 0)
        .unwrap_or(0)
}

fn bare_host(url: &Url) -> Option<String> {
    let host = url.host_str()?.to_ascii_lowercase();
    Some(host.strip_prefix("www.").unwrap_or(&host).to_string())
}

fn outbound_citations(document: &Html, page_url: &Url) -> u32 {
    let own = bare_host(page_url);
    document
        .select(&sel("a[href]"))
        .filter(|el| !in_boilerplate(el))
        .filter_map(|el| page_url.join(el.attr("href")?.trim()).ok())
        .filter(|url| matches!(url.scheme(), "http" | "https"))
        .filter(|url| bare_host(url).is_some_and(|host| Some(&host) != own.as_ref()))
        .count() as u32
}

/// A type written as a full address (`https://schema.org/Article`) is the
/// same type as the bare name.
fn type_name(raw: &str) -> String {
    let raw = raw.trim();
    ["https://schema.org/", "http://schema.org/"]
        .iter()
        .find_map(|prefix| raw.strip_prefix(prefix))
        .unwrap_or(raw)
        .to_string()
}

/// Every `@type` anywhere in the block: `@graph`, arrays and nested objects.
fn collect_types(value: &serde_json::Value, depth: usize, out: &mut Vec<String>) {
    if depth > MAX_SCHEMA_DEPTH {
        return;
    }
    match value {
        serde_json::Value::Array(items) => {
            items
                .iter()
                .for_each(|item| collect_types(item, depth + 1, out));
        }
        serde_json::Value::Object(map) => {
            match map.get("@type") {
                Some(serde_json::Value::String(kind)) => out.push(type_name(kind)),
                Some(serde_json::Value::Array(kinds)) => {
                    out.extend(kinds.iter().filter_map(|k| k.as_str().map(type_name)));
                }
                _ => {}
            }
            map.values()
                .for_each(|item| collect_types(item, depth + 1, out));
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crawl::extract::extract;

    fn facts(html: &str) -> AnswerFacts {
        let url = Url::parse("https://example.com/post").unwrap();
        let document = Html::parse_document(html);
        let (structured, _) = crate::crawl::extract::structured_blocks(&document);
        read(&document, &url, &structured)
    }

    fn ld(json: &str) -> String {
        format!(r#"<script type="application/ld+json">{json}</script>"#)
    }

    #[test]
    fn counts_question_headings() {
        let found = facts(
            "<h1>Pricing guide</h1><h2>What does it cost?</h2><h2>How to choose</h2>\
             <h3>Is it free</h3><h3>Whatever</h3><h2>Costs</h2><h2>Why wait?</h2>",
        );
        // "?" endings and question words as the first word; "Whatever" is not one.
        assert_eq!(found.question_headings, 4);
    }

    #[test]
    fn counts_lists_and_tables_in_the_main_text_only() {
        let found = facts(
            "<nav><ul><li>Home</li></ul></nav><main><ul><li>a</li></ul><ol><li>b</li></ol>\
             <table><tr><td>x</td></tr></table></main><footer><ul><li>c</li></ul></footer>",
        );
        assert_eq!((found.lists, found.tables), (2, 1));
    }

    #[test]
    fn reads_the_first_paragraph_outside_navigation() {
        let found = facts(
            "<nav><p>Skip to content now</p></nav><p> </p><p>One two three four five.</p><p>Later words here</p>",
        );
        assert_eq!(found.first_paragraph_words, 5);
        assert_eq!(facts("<h1>No paragraphs</h1>").first_paragraph_words, 0);
    }

    #[test]
    fn reads_schema_types_from_graph_arrays_and_nested_types() {
        let found = facts(&ld(
            r#"{"@context":"https://schema.org","@graph":[{"@type":["WebPage","FAQPage"]},{"@type":"BlogPosting"},{"@type":"BreadcrumbList"}]}"#,
        ));
        assert!(found.faq_schema && found.article_schema && found.breadcrumb_schema);
        assert!(!found.howto_schema && !found.organization_schema);
        let list = facts(&ld(
            r#"[{"@type":"HowTo"},{"@type":"Organization","name":"x"}]"#,
        ));
        assert!(list.howto_schema && list.organization_schema);
        let nested = facts(&ld(
            r#"{"@type":"WebPage","mainEntity":{"@type":["Thing","NewsArticle"]}}"#,
        ));
        assert!(nested.article_schema);
        // A site-search block alone does not say who runs the site.
        assert!(!facts(&ld(r#"{"@type":"WebSite"}"#)).organization_schema);
        let addresses = facts(&ld(
            r#"[{"@type":"https://schema.org/Article"},{"@type":["http://schema.org/Organization"]},{"@type":"https://schema.org/FAQPage"}]"#,
        ));
        assert!(addresses.article_schema && addresses.organization_schema && addresses.faq_schema);
        let none = facts("<p>none</p>");
        assert!(!none.faq_schema && !none.article_schema && !none.organization_schema);
    }

    #[test]
    fn counts_outbound_links_inside_the_main_text_only() {
        let found = facts(
            r#"<nav><a href="https://other.org/x">n</a></nav>
            <p><a href="https://other.org/study">a</a> <a href="https://www.example.com/x">own www</a>
            <a href="/relative">rel</a> <a href="https://news.example.net/y">b</a>
            <a href="mailto:a@b.c">m</a></p><footer><a href="https://social.example/">f</a></footer>"#,
        );
        assert_eq!(found.outbound_citations, 2);
    }

    #[test]
    fn extract_fills_every_field() {
        let url = Url::parse("https://example.com/post").unwrap();
        let facts = extract("<h2>Why?</h2><p>Short answer.</p>", &url);
        assert_eq!(facts.question_headings, Some(1));
        assert_eq!(facts.first_paragraph_words, Some(2));
        assert_eq!(facts.faq_schema, Some(false));
    }

    #[test]
    fn pages_stored_before_these_facts_read_as_unknown() {
        let url = Url::parse("https://example.com/post").unwrap();
        let mut value = serde_json::to_value(extract("<p>x</p>", &url)).unwrap();
        for key in [
            "question_headings",
            "lists",
            "tables",
            "first_paragraph_words",
            "faq_schema",
            "howto_schema",
            "article_schema",
            "organization_schema",
            "breadcrumb_schema",
            "outbound_citations",
        ] {
            value.as_object_mut().unwrap().remove(key);
        }
        let old: crate::crawl::extract::PageFacts = serde_json::from_value(value).unwrap();
        assert_eq!(old.question_headings, None);
        assert_eq!(old.faq_schema, None);
        assert_eq!(old.outbound_citations, None);
    }
}
