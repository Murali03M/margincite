//! Google Ads keyword data through DataForSEO: search volume for a list of
//! keywords, and keyword ideas for a seed.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::client::{Credentials, DataForSeoClient, DataForSeoError};
use crate::crawl::clean::strip_nul;

pub const VOLUME_PATH: &str = "/v3/keywords_data/google_ads/search_volume/live";
pub const IDEAS_PATH: &str = "/v3/keywords_data/google_ads/keywords_for_keywords/live";
/// The most keywords one volume request holds.
pub const MAX_VOLUME_KEYWORDS: usize = 1000;
const MAX_CHARS: usize = 80;
const MAX_WORDS: usize = 10;
/// Text kept from an answer is cut at this many characters.
const MAX_KEEP_CHARS: usize = 200;

/// What the Google Ads data says about one keyword. Anything it has no data
/// for is `None`, never zero.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KeywordMetrics {
    pub keyword: String,
    pub search_volume: Option<i32>,
    pub competition: Option<String>,
    pub cpc: Option<f64>,
}

/// Whether DataForSEO accepts this keyword: at most 80 characters and 10 words.
pub fn fits(keyword: &str) -> bool {
    let words = keyword.split_whitespace().count();
    (1..=MAX_WORDS).contains(&words) && keyword.chars().count() <= MAX_CHARS
}

fn read_item(item: &Value) -> Option<KeywordMetrics> {
    let keyword: String = strip_nul(item.get("keyword")?.as_str()?)
        .chars()
        .take(MAX_KEEP_CHARS)
        .collect();
    let number = |key: &str| {
        item.get(key)
            .and_then(Value::as_f64)
            .filter(|number| number.is_finite() && *number >= 0.0)
    };
    Some(KeywordMetrics {
        keyword,
        search_volume: number("search_volume")
            .map(|volume| volume.round().min(f64::from(i32::MAX)) as i32),
        competition: item
            .get("competition")
            .and_then(Value::as_str)
            .filter(|word| !word.is_empty())
            .map(|word| strip_nul(word).chars().take(MAX_KEEP_CHARS).collect()),
        cpc: number("cpc"),
    })
}

/// The keywords of an answer's `result` array. Documented shape: each item has
/// `keyword`, and `search_volume`, `competition` (LOW, MEDIUM or HIGH) and
/// `cpc`, any of which may be null when Google has no data.
pub fn read_metrics(result: &[Value]) -> Vec<KeywordMetrics> {
    result.iter().filter_map(read_item).collect()
}

impl DataForSeoClient {
    /// Monthly search volume, competition and cost per click for these
    /// keywords (at most `MAX_VOLUME_KEYWORDS`, each one that `fits`).
    pub async fn search_volume(
        &self,
        credentials: &Credentials<'_>,
        keywords: &[String],
        location_code: u32,
        language_code: &str,
    ) -> Result<Vec<KeywordMetrics>, DataForSeoError> {
        self.ads_task(
            credentials,
            VOLUME_PATH,
            keywords,
            location_code,
            language_code,
        )
        .await
    }

    /// Keyword ideas for a seed, in the order DataForSEO sends them.
    pub async fn keyword_ideas(
        &self,
        credentials: &Credentials<'_>,
        seed: &str,
        location_code: u32,
        language_code: &str,
    ) -> Result<Vec<KeywordMetrics>, DataForSeoError> {
        let seeds = [seed.to_string()];
        self.ads_task(
            credentials,
            IDEAS_PATH,
            &seeds,
            location_code,
            language_code,
        )
        .await
    }

    async fn ads_task(
        &self,
        credentials: &Credentials<'_>,
        path: &str,
        keywords: &[String],
        location_code: u32,
        language_code: &str,
    ) -> Result<Vec<KeywordMetrics>, DataForSeoError> {
        let task = json!({
            "keywords": keywords,
            "location_code": location_code,
            "language_code": language_code,
        });
        let result = self.task_result(credentials, path, task).await?;
        Ok(read_metrics(&result))
    }
}
