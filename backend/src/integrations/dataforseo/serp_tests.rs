use serde_json::{json, Value};

use super::*;

const SITE: &str = "example.com";

fn organic(rank: i64, domain: &str, url: &str) -> Value {
    json!({ "type": "organic", "rank_group": rank, "rank_absolute": rank,
            "domain": domain, "url": url, "title": format!("Title {rank}") })
}

fn reference(domain: &str, url: &str) -> Value {
    json!({ "type": "ai_overview_reference", "domain": domain, "url": url,
            "title": "t", "text": "x", "source": "s" })
}

fn overview(items: Value, references: Value) -> Value {
    json!({ "type": "ai_overview", "rank_absolute": 1, "asynchronous_ai_overview": false,
            "items": items, "references": references })
}

fn element(references: Value) -> Value {
    json!({ "type": "ai_overview_element", "text": "An answer.", "references": references })
}

fn read_items(items: Vec<Value>) -> Serp {
    read(&items, SITE)
}

#[test]
fn a_site_result_gives_the_best_position_and_its_address() {
    let serp = read_items(vec![
        organic(1, "a.com", "https://a.com/"),
        organic(7, "www.example.com", "https://www.example.com/widgets"),
        organic(9, "example.com", "https://example.com/other"),
    ]);
    assert_eq!(serp.position, Some(7));
    assert_eq!(serp.url.as_deref(), Some("https://www.example.com/widgets"));
}

#[test]
fn a_site_missing_from_the_results_has_no_position() {
    let serp = read_items(vec![organic(1, "a.com", "https://a.com/")]);
    assert_eq!(serp.position, None);
    assert_eq!(serp.url, None);
    assert_eq!(serp.top.len(), 1);
    assert_eq!(read_items(vec![]).position, None);
}

#[test]
fn subdomains_count_but_lookalike_names_do_not() {
    let hit = read_items(vec![organic(
        3,
        "blog.example.com",
        "https://blog.example.com/p",
    )]);
    assert_eq!(hit.position, Some(3));
    for domain in ["notexample.com", "example.com.au", "example.community"] {
        let miss = read_items(vec![organic(2, domain, "https://x.test/")]);
        assert_eq!(miss.position, None, "{domain}");
    }
}

#[test]
fn only_organic_results_give_a_position() {
    let snippet = json!({ "type": "featured_snippet", "rank_absolute": 1,
                          "domain": "example.com", "url": "https://example.com/" });
    let paa = json!({ "type": "people_also_ask", "rank_absolute": 2, "items": [] });
    assert_eq!(read_items(vec![snippet, paa]).position, None);
}

#[test]
fn the_domain_comes_from_the_address_when_the_item_has_none() {
    let item = json!({ "type": "organic", "rank_absolute": 4,
                       "url": "https://example.com/x", "title": null });
    let serp = read_items(vec![item]);
    assert_eq!(serp.position, Some(4));
    assert_eq!(serp.top[0].domain, "example.com");
    assert_eq!(serp.top[0].title, "");
}

#[test]
fn the_top_list_is_the_first_ten_in_order() {
    let mut items: Vec<Value> = (1..=15)
        .rev()
        .map(|rank| organic(rank, "a.com", &format!("https://a.com/{rank}")))
        .collect();
    items.push(json!({ "type": "organic", "domain": "b.com", "url": "https://b.com/" }));
    let serp = read_items(items);
    let positions: Vec<i32> = serp.top.iter().map(|r| r.position).collect();
    assert_eq!(positions, (1..=10).collect::<Vec<_>>());
    assert_eq!(serp.top[0].url, "https://a.com/1");
}

#[test]
fn no_overview_item_is_absent() {
    let serp = read_items(vec![organic(1, "a.com", "https://a.com/")]);
    assert_eq!(serp.ai.status, AiStatus::Absent);
    assert!(serp.ai.cited_domains.is_empty());
}

#[test]
fn an_overview_citing_others_is_present_and_lists_the_domains_once() {
    let refs = json!([
        reference("www.a.com", "https://www.a.com/1"),
        reference("b.com", "https://b.com/"),
        reference("a.com", "https://a.com/2"),
    ]);
    let serp = read_items(vec![overview(json!([element(refs)]), Value::Null)]);
    assert_eq!(serp.ai.status, AiStatus::Present);
    assert_eq!(serp.ai.cited_domains, vec!["a.com", "b.com"]);
    assert!(serp.ai.your_urls.is_empty());
}

#[test]
fn an_overview_citing_the_site_is_cited_wherever_the_reference_sits() {
    let mine = reference("blog.example.com", "https://blog.example.com/post");
    let other = reference("a.com", "https://a.com/");
    // In an element's references.
    let in_element = overview(
        json!([element(json!([other.clone(), mine.clone()]))]),
        Value::Null,
    );
    // In the overview's own references.
    let at_top = overview(
        json!([element(Value::Null)]),
        json!([other.clone(), mine.clone()]),
    );
    // In a component of an expanded element.
    let expanded = json!({ "type": "ai_overview_expanded_element", "references": null,
        "components": [{ "type": "ai_overview_expanded_component", "references": [mine.clone()] }] });
    let in_component = overview(
        json!([element(json!([other.clone()])), expanded]),
        Value::Null,
    );
    for item in [in_element, at_top, in_component] {
        let serp = read_items(vec![item]);
        assert_eq!(serp.ai.status, AiStatus::Cited);
        assert_eq!(serp.ai.your_urls, vec!["https://blog.example.com/post"]);
        assert!(serp.ai.cited_domains.contains(&"a.com".to_string()));
    }
}

#[test]
fn an_overview_with_text_and_no_references_is_present() {
    let serp = read_items(vec![overview(json!([element(Value::Null)]), Value::Null)]);
    assert_eq!(serp.ai.status, AiStatus::Present);
    assert!(serp.ai.cited_domains.is_empty());
}

#[test]
fn an_overview_that_could_not_be_read_is_unknown_not_absent() {
    let not_loaded =
        json!({ "type": "ai_overview", "asynchronous_ai_overview": true, "items": null });
    let empty = overview(json!([]), Value::Null);
    let odd = json!({ "type": "ai_overview", "items": "text" });
    for item in [not_loaded, empty, odd] {
        let serp = read_items(vec![item.clone()]);
        assert_eq!(serp.ai.status, AiStatus::Unknown, "{item}");
    }
}

#[test]
fn a_knowledge_graph_overview_item_is_not_an_overview() {
    let other = json!({ "type": "knowledge_graph_ai_overview_item", "items": [] });
    assert_eq!(read_items(vec![other]).ai.status, AiStatus::Absent);
}

#[test]
fn strings_are_cleaned_and_cut() {
    let long = "x".repeat(5000);
    let item = json!({ "type": "organic", "rank_absolute": 1, "domain": "a.com",
                       "url": "https://a.com/\u{0}p", "title": long });
    let serp = read_items(vec![item]);
    assert_eq!(serp.top[0].url, "https://a.com/p");
    assert_eq!(serp.top[0].title.chars().count(), MAX_TEXT_CHARS);
}

#[test]
fn the_status_words_are_the_ones_the_api_uses() {
    assert_eq!(AiStatus::Absent.as_str(), "absent");
    assert_eq!(AiStatus::Present.as_str(), "present");
    assert_eq!(AiStatus::Cited.as_str(), "cited");
    assert_eq!(AiStatus::Unknown.as_str(), "unknown");
}
