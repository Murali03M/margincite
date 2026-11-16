use scraper::{Html, Selector};
use serde::{Deserialize, Serialize};
use url::Url;

use super::answer_facts;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Heading {
    pub level: u8,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Link {
    pub url: Url,
    pub text: String,
    pub nofollow: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Image {
    pub src: String,
    pub alt: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PageFacts {
    pub title: Option<String>,
    pub meta_description: Option<String>,
    pub h1s: Vec<String>,
    pub headings: Vec<Heading>,
    pub canonical: Option<Url>,
    pub robots_meta: Option<String>,
    pub has_viewport: bool,
    pub word_count: u32,
    #[serde(default)]
    pub text: String,
    pub structured_data: Vec<serde_json::Value>,
    pub structured_data_errors: u32,
    pub links: Vec<Link>,
    pub images: Vec<Image>,
    pub has_scripts: bool,
    pub published: Option<String>,
    pub modified: Option<String>,
    pub author: Option<String>,
    /// Answer-readiness facts. Pages stored before these existed read as
    /// `None`, which means unknown, never "missing".
    #[serde(default)]
    pub question_headings: Option<u32>,
    #[serde(default)]
    pub lists: Option<u32>,
    #[serde(default)]
    pub tables: Option<u32>,
    #[serde(default)]
    pub first_paragraph_words: Option<u32>,
    #[serde(default)]
    pub faq_schema: Option<bool>,
    #[serde(default)]
    pub howto_schema: Option<bool>,
    #[serde(default)]
    pub article_schema: Option<bool>,
    #[serde(default)]
    pub organization_schema: Option<bool>,
    #[serde(default)]
    pub breadcrumb_schema: Option<bool>,
    #[serde(default)]
    pub outbound_citations: Option<u32>,
}

const TEXT_LIMIT: usize = 200_000;
/// A crafted page cannot make one page's facts grow without bound.
const MAX_LINKS: usize = 2_000;
const MAX_IMAGES: usize = 500;
const MAX_JSON_LD_BLOCKS: usize = 50;

pub fn extract(html: &str, page_url: &Url) -> PageFacts {
    let document = Html::parse_document(html);
    let text = visible_text(&document);
    let word_count = text.split_whitespace().count() as u32;
    let text = text.chars().take(TEXT_LIMIT).collect();
    let (structured_data, structured_data_errors) = structured_blocks(&document);
    let answer = answer_facts::read(&document, page_url, &structured_data);
    PageFacts {
        title: first_text(&document, "title"),
        meta_description: meta_named(&document, "description"),
        h1s: headings_of(&document)
            .into_iter()
            .filter(|heading| heading.level == 1)
            .map(|heading| heading.text)
            .collect(),
        headings: headings_of(&document),
        canonical: attr(&document, "link[rel=canonical]", "href")
            .and_then(|href| resolve(page_url, &base_url(&document, page_url), &href)),
        robots_meta: meta_named(&document, "robots"),
        has_viewport: meta_named(&document, "viewport").is_some(),
        word_count,
        text,
        published: meta_property(&document, "article:published_time")
            .or_else(|| json_string(&structured_data, "datePublished")),
        modified: meta_property(&document, "article:modified_time")
            .or_else(|| json_string(&structured_data, "dateModified")),
        author: meta_named(&document, "author").or_else(|| json_author(&structured_data)),
        structured_data,
        structured_data_errors,
        links: links(&document, page_url),
        images: images(&document),
        has_scripts: document.select(&sel("script")).next().is_some(),
        question_headings: Some(answer.question_headings),
        lists: Some(answer.lists),
        tables: Some(answer.tables),
        first_paragraph_words: Some(answer.first_paragraph_words),
        faq_schema: Some(answer.faq_schema),
        howto_schema: Some(answer.howto_schema),
        article_schema: Some(answer.article_schema),
        organization_schema: Some(answer.organization_schema),
        breadcrumb_schema: Some(answer.breadcrumb_schema),
        outbound_citations: Some(answer.outbound_citations),
    }
}

pub fn looks_javascript_only(facts: &PageFacts) -> bool {
    facts.word_count < 50 && facts.has_scripts
}

fn sel(query: &str) -> Selector {
    Selector::parse(query).expect("selector")
}

fn visible_text(document: &Html) -> String {
    let mut raw = String::new();
    for node in document.tree.root().descendants() {
        let scraper::Node::Text(text) = node.value() else {
            continue;
        };
        if !inside_body(&node) || skipped_ancestor(&node) {
            continue;
        }
        raw.push_str(text);
        raw.push(' ');
    }
    collapse(&raw)
}

fn inside_body(node: &ego_tree::NodeRef<'_, scraper::Node>) -> bool {
    node.ancestors().any(
        |ancestor| matches!(ancestor.value(), scraper::Node::Element(el) if el.name() == "body"),
    )
}

fn skipped_ancestor(node: &ego_tree::NodeRef<'_, scraper::Node>) -> bool {
    node.ancestors().any(|ancestor| {
        matches!(
            ancestor.value(),
            scraper::Node::Element(el)
                if matches!(el.name(), "script" | "style" | "noscript" | "template" | "svg")
        )
    })
}

fn collapse(raw: &str) -> String {
    raw.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn first_text(document: &Html, query: &str) -> Option<String> {
    let text = collapse(
        &document
            .select(&sel(query))
            .next()?
            .text()
            .collect::<String>(),
    );
    if text.is_empty() {
        None
    } else {
        Some(text)
    }
}

fn headings_of(document: &Html) -> Vec<Heading> {
    document
        .select(&sel("h1, h2, h3, h4, h5, h6"))
        .filter(|el| !skipped_heading(el))
        .map(|el| Heading {
            level: el
                .value()
                .name()
                .trim_start_matches('h')
                .parse()
                .unwrap_or(0),
            text: collapse(&el.text().collect::<String>()),
        })
        .filter(|heading| !heading.text.is_empty())
        .collect()
}

fn skipped_heading(el: &scraper::ElementRef<'_>) -> bool {
    matches!(
        el.value().name(),
        "script" | "style" | "noscript" | "template" | "svg"
    )
}

fn meta_named(document: &Html, name: &str) -> Option<String> {
    document.select(&sel("meta[name]")).find_map(|el| {
        let found = el.attr("name")?;
        if found.eq_ignore_ascii_case(name) {
            el.attr("content").map(str::trim).map(str::to_string)
        } else {
            None
        }
    })
}

fn meta_property(document: &Html, property: &str) -> Option<String> {
    document.select(&sel("meta[property]")).find_map(|el| {
        let found = el.attr("property")?;
        if found.eq_ignore_ascii_case(property) {
            el.attr("content").map(str::to_string)
        } else {
            None
        }
    })
}

fn attr(document: &Html, query: &str, name: &str) -> Option<String> {
    document
        .select(&sel(query))
        .next()
        .and_then(|el| el.attr(name).map(str::to_string))
}

fn base_url(document: &Html, page_url: &Url) -> Url {
    attr(document, "base[href]", "href")
        .and_then(|href| page_url.join(&href).ok())
        .unwrap_or_else(|| page_url.clone())
}

fn links(document: &Html, page_url: &Url) -> Vec<Link> {
    let base = base_url(document, page_url);
    document
        .select(&sel("a[href]"))
        .filter_map(|el| {
            let href = el.attr("href")?;
            let url = resolve(page_url, &base, href)?;
            let rel = el.attr("rel").unwrap_or("");
            Some(Link {
                url,
                text: collapse(&el.text().collect::<String>()),
                nofollow: rel
                    .split_whitespace()
                    .any(|part| part.eq_ignore_ascii_case("nofollow")),
            })
        })
        .take(MAX_LINKS)
        .collect()
}

fn resolve(page_url: &Url, base: &Url, href: &str) -> Option<Url> {
    let href = href.trim();
    if href.is_empty() {
        return None;
    }
    let lower = href.to_ascii_lowercase();
    if lower.starts_with("javascript:") || lower.starts_with("mailto:") || lower.starts_with("tel:")
    {
        return None;
    }
    let url = base.join(href).ok()?;
    if matches!(url.scheme(), "javascript" | "mailto" | "tel") {
        return None;
    }
    let _ = page_url;
    Some(url)
}

fn images(document: &Html) -> Vec<Image> {
    document
        .select(&sel("img[src]"))
        .map(|el| Image {
            src: el.attr("src").unwrap_or("").to_string(),
            alt: el.attr("alt").map(str::to_string),
        })
        .take(MAX_IMAGES)
        .collect()
}

pub(crate) fn structured_blocks(document: &Html) -> (Vec<serde_json::Value>, u32) {
    let mut values = Vec::new();
    let mut errors = 0;
    let script = sel("script");
    let blocks = document.select(&script).filter(|el| {
        el.attr("type")
            .is_some_and(|kind| kind.eq_ignore_ascii_case("application/ld+json"))
    });
    for el in blocks.take(MAX_JSON_LD_BLOCKS) {
        let raw = el.text().collect::<String>();
        match serde_json::from_str(&raw) {
            Ok(value) => values.push(value),
            Err(_) => errors += 1,
        }
    }
    (values, errors)
}

fn json_string(values: &[serde_json::Value], key: &str) -> Option<String> {
    values.iter().find_map(|value| {
        value
            .get(key)
            .and_then(|item| item.as_str())
            .map(str::to_string)
            .or_else(|| {
                value
                    .get("@graph")
                    .and_then(|graph| graph.as_array())
                    .and_then(|items| json_string(items, key))
            })
    })
}

fn json_author(values: &[serde_json::Value]) -> Option<String> {
    values.iter().find_map(|value| {
        value.get("author").and_then(person_name).or_else(|| {
            value
                .get("@graph")
                .and_then(|graph| graph.as_array())
                .and_then(|items| json_author(items))
        })
    })
}

fn person_name(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::String(name) => Some(name.clone()),
        serde_json::Value::Array(items) => items.iter().find_map(person_name),
        serde_json::Value::Object(_) => value
            .get("name")
            .and_then(|name| name.as_str())
            .map(str::to_string),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn page() -> Url {
        Url::parse("https://example.com/article").unwrap()
    }

    #[test]
    fn reads_title_description_and_canonical() {
        let facts = extract(
            r#"<title> Hello </title><meta name="description" content="A page"><link rel="canonical" href="/root">"#,
            &page(),
        );
        assert_eq!(facts.title.as_deref(), Some("Hello"));
        assert_eq!(facts.meta_description.as_deref(), Some("A page"));
        assert_eq!(
            facts.canonical.unwrap().as_str(),
            "https://example.com/root"
        );
    }

    #[test]
    fn collects_headings_in_order_with_levels() {
        let facts = extract("<h2>Second</h2><h1>First</h1><h3>Third</h3>", &page());
        assert_eq!(
            facts.headings,
            vec![
                Heading {
                    level: 2,
                    text: "Second".into()
                },
                Heading {
                    level: 1,
                    text: "First".into()
                },
                Heading {
                    level: 3,
                    text: "Third".into()
                },
            ]
        );
        assert_eq!(facts.h1s, vec!["First".to_string()]);
    }

    #[test]
    fn counts_visible_words_only() {
        let facts = extract(
            "<body>one two<script>three four</script><style>five</style></body>",
            &page(),
        );
        assert_eq!(facts.text, "one two");
        assert_eq!(facts.word_count, 2);
    }

    #[test]
    fn resolves_relative_links_and_honours_base() {
        let facts = extract(
            r#"<base href="https://example.com/dir/"><a href="page">Page</a><a href="/root">Root</a>"#,
            &page(),
        );
        assert_eq!(facts.links[0].url.as_str(), "https://example.com/dir/page");
        assert_eq!(facts.links[1].url.as_str(), "https://example.com/root");
    }

    #[test]
    fn drops_javascript_mailto_and_tel_links() {
        let facts = extract(
            r#"<a href="javascript:alert(1)">x</a><a href="mailto:a@b.c">y</a><a href="tel:1">z</a><a href="/ok">ok</a>"#,
            &page(),
        );
        assert_eq!(facts.links.len(), 1);
        assert!(facts.links[0].url.path().ends_with("/ok"));
    }

    #[test]
    fn notes_nofollow_links() {
        let facts = extract(r#"<a href="/a" rel="nofollow noopener">A</a>"#, &page());
        assert!(facts.links[0].nofollow);
    }

    #[test]
    fn an_image_without_alt_has_none_and_an_empty_alt_has_some_empty() {
        let facts = extract(r#"<img src="/a.png"><img src="/b.png" alt="">"#, &page());
        assert_eq!(facts.images[0].alt, None);
        assert_eq!(facts.images[1].alt.as_deref(), Some(""));
    }

    #[test]
    fn parses_json_ld_and_counts_broken_blocks() {
        let facts = extract(
            r#"<script type="application/ld+json">{"@type":"Article"}</script><script type="application/ld+json">{nope</script>"#,
            &page(),
        );
        assert_eq!(facts.structured_data.len(), 1);
        assert_eq!(facts.structured_data_errors, 1);
    }

    #[test]
    fn reads_robots_meta_and_viewport() {
        let facts = extract(
            r#"<meta name="robots" content="noindex"><meta name="viewport" content="width=device-width">"#,
            &page(),
        );
        assert_eq!(facts.robots_meta.as_deref(), Some("noindex"));
        assert!(facts.has_viewport);
    }

    #[test]
    fn reads_dates_and_author_from_meta_and_json_ld() {
        let facts = extract(
            r#"<meta property="article:published_time" content="2020-01-02"><meta property="article:modified_time" content="2020-03-04"><script type="application/ld+json">{"author":{"name":"Ada"}}</script>"#,
            &page(),
        );
        assert_eq!(facts.published.as_deref(), Some("2020-01-02"));
        assert_eq!(facts.modified.as_deref(), Some("2020-03-04"));
        assert_eq!(facts.author.as_deref(), Some("Ada"));
    }

    #[test]
    fn an_app_shell_looks_javascript_only() {
        let facts = extract(
            r#"<div id="root"></div><script src="/app.js"></script>"#,
            &page(),
        );
        assert!(looks_javascript_only(&facts));
    }

    #[test]
    fn a_normal_article_does_not() {
        let words = "word ".repeat(60);
        let facts = extract(&format!("<body>{words}</body>"), &page());
        assert!(!looks_javascript_only(&facts));
    }

    #[test]
    fn survives_malformed_and_empty_html() {
        for html in ["", "<<<>>>", "<p>unclosed"] {
            let facts = extract(html, &page());
            assert!(facts.word_count < 5);
        }
    }

    #[test]
    fn cuts_very_long_text_at_the_limit() {
        let facts = extract(&format!("<body>{}</body>", "a".repeat(250_000)), &page());
        assert_eq!(facts.text.chars().count(), TEXT_LIMIT);
    }

    #[test]
    fn keeps_at_most_2000_links() {
        let html: String = (0..3000)
            .map(|n| format!(r#"<a href="/p{n}">p</a>"#))
            .collect();
        assert_eq!(extract(&html, &page()).links.len(), 2000);
    }

    #[test]
    fn keeps_at_most_500_images() {
        let html: String = (0..800)
            .map(|n| format!(r#"<img src="/i{n}.png">"#))
            .collect();
        assert_eq!(extract(&html, &page()).images.len(), 500);
    }

    #[test]
    fn keeps_at_most_50_json_ld_blocks() {
        let html: String = (0..120)
            .map(|n| format!(r#"<script type="application/ld+json">{{"n":{n}}}</script>"#))
            .collect();
        let facts = extract(&html, &page());
        assert_eq!(facts.structured_data.len(), 50);
        assert_eq!(facts.structured_data_errors, 0);
    }

    #[test]
    fn broken_json_ld_blocks_count_toward_the_50() {
        let html: String = (0..120)
            .map(|_| r#"<script type="application/ld+json">{nope</script>"#)
            .collect();
        let facts = extract(&html, &page());
        assert_eq!(facts.structured_data_errors, 50);
    }
}
