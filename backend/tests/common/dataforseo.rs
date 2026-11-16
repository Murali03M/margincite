//! Shared helpers for DataForSEO tests: bodies shaped like the documented
//! answers, and checks on what was sent.

use base64::engine::general_purpose::STANDARD;
use base64::Engine as _;
use serde_json::{json, Value};

pub const LOGIN: &str = "ads-login-secret@example.com";
pub const PASSWORD: &str = "ads-password-SECRET-7788";

/// The `Authorization` header value a request with the test login carries.
pub fn basic() -> String {
    format!("Basic {}", STANDARD.encode(format!("{LOGIN}:{PASSWORD}")))
}

/// The base64 of `login:password`, as a logged header would show it.
pub fn token() -> String {
    STANDARD.encode(format!("{LOGIN}:{PASSWORD}"))
}

/// A successful answer whose first task holds `result`.
pub fn answer(result: Value) -> Value {
    json!({
        "version": "0.1.20250101", "status_code": 20000, "status_message": "Ok.",
        "cost": 0.022, "tasks_count": 1, "tasks_error": 0,
        "tasks": [{ "id": "t", "status_code": 20000, "status_message": "Ok.",
                    "cost": 0.022, "result_count": 1, "result": result }]
    })
}

/// A successful SERP answer with these items.
pub fn serp(items: Value) -> Value {
    answer(json!([{ "keyword": "k", "items_count": 1, "items": items }]))
}

/// An answer that is fine at the top and failed in its task.
pub fn task_failed(code: i64, message: &str) -> Value {
    json!({
        "version": "0.1.20250101", "status_code": 20000, "status_message": "Ok.",
        "tasks_count": 1, "tasks_error": 1,
        "tasks": [{ "id": "t", "status_code": code, "status_message": message, "result": null }]
    })
}

/// An answer that failed before any task ran.
pub fn top_failed(code: i64, message: &str) -> Value {
    json!({ "version": "0.1.20250101", "status_code": code, "status_message": message,
            "tasks_count": 0, "tasks_error": 0, "tasks": [] })
}

pub fn organic(rank: i64, domain: &str) -> Value {
    json!({ "type": "organic", "rank_group": rank, "rank_absolute": rank, "domain": domain,
            "url": format!("https://{domain}/page"), "title": format!("Result {rank}") })
}

pub const VOLUME_PATH: &str = "/dataforseo/v3/keywords_data/google_ads/search_volume/live";
pub const IDEAS_PATH: &str = "/dataforseo/v3/keywords_data/google_ads/keywords_for_keywords/live";

/// One Google Ads keyword item.
pub fn ads_item(
    keyword: &str,
    volume: Option<i64>,
    competition: Option<&str>,
    cpc: Option<f64>,
) -> Value {
    json!({ "keyword": keyword, "spell": null, "location_code": 2840, "language_code": "en",
            "search_partners": false, "competition": competition, "competition_index": 40,
            "search_volume": volume, "low_top_of_page_bid": 0.5, "high_top_of_page_bid": 1.5,
            "cpc": cpc, "monthly_searches": null })
}
