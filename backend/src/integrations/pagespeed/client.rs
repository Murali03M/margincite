//! PageSpeed Insights over plain HTTPS. The base address is a constructor
//! argument; production uses `PRODUCTION_BASE`.
//!
//! The API key travels in the `key` query parameter, so nothing here logs or
//! returns a request address, a response body or the key: every error is a
//! fixed sentence, built from a status code at most.

use std::time::Duration;

use serde_json::Value;
use url::Url;

use crate::body::{read_capped, BodyError, MAX_BODY_BYTES};

pub const PRODUCTION_BASE: &str = "https://www.googleapis.com/pagespeedonline/v5";
/// A real browser test of a slow page takes a while.
/// The most of an error answer read to tell a bad key from a bad page.
const MAX_ERROR_BODY: usize = 64 * 1024;
const TIMEOUT: Duration = Duration::from_secs(90);

/// What went wrong with one page's test. Every message is a fixed sentence:
/// it is stored and shown, so it never holds a request address (the key is in
/// it), a response body or a status code. Details go to the server log, also
/// without the address.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum PageSpeedError {
    #[error("PageSpeed rejected the API key. Check it in Settings.")]
    Rejected,
    #[error("PageSpeed is limiting requests right now. Try again later.")]
    RateLimited,
    /// Requests without a key share one allowance across everyone, and it is
    /// usually spent. Waiting does not help; a key does.
    #[error("Google is not accepting page speed tests without an API key right now. Add a free PageSpeed key in Settings.")]
    NoKeyLimit,
    #[error("PageSpeed took too long to test this page.")]
    TimedOut,
    #[error("PageSpeed could not test this page.")]
    Failed,
    #[error("PageSpeed replied in a format Margincite does not recognise.")]
    Unrecognised,
}

/// What PageSpeed measured for one page on a phone. Anything it did not
/// report is `None`, never zero.
#[derive(Debug, Clone, PartialEq)]
pub struct Measurement {
    /// 0 to 100.
    pub performance: Option<i32>,
    pub lcp_ms: Option<i32>,
    pub cls: Option<f64>,
    pub inp_ms: Option<i32>,
    pub tbt_ms: Option<i32>,
    /// At least one of the numbers above is from real visitors (Chrome UX
    /// Report) and not from the lab test.
    pub field_data: bool,
}

#[derive(Clone)]
pub struct PageSpeedClient {
    http: reqwest::Client,
    base: String,
}

impl PageSpeedClient {
    pub fn new(base_url: &str) -> PageSpeedClient {
        PageSpeedClient::with_timeout(base_url, TIMEOUT)
    }

    /// For tests, which cannot wait 90 seconds.
    pub fn with_timeout(base_url: &str, timeout: Duration) -> PageSpeedClient {
        let http = reqwest::Client::builder()
            .timeout(timeout)
            .user_agent("margincite")
            .build()
            .expect("an HTTP client with default settings builds");
        PageSpeedClient {
            http,
            base: base_url.to_string(),
        }
    }

    pub fn production() -> PageSpeedClient {
        PageSpeedClient::new(PRODUCTION_BASE)
    }

    /// Tests `page_url` as a phone would load it. A key is optional, but
    /// Google often refuses requests without one.
    pub async fn measure(
        &self,
        page_url: &str,
        api_key: Option<&str>,
    ) -> Result<Measurement, PageSpeedError> {
        let mut url = Url::parse(&format!("{}/runPagespeed", self.base.trim_end_matches('/')))
            .map_err(|_| PageSpeedError::Failed)?;
        {
            let mut query = url.query_pairs_mut();
            query
                .append_pair("url", page_url)
                .append_pair("strategy", "mobile")
                .append_pair("category", "performance");
            if let Some(key) = api_key {
                query.append_pair("key", key);
            }
        }
        let response = self.http.get(url).send().await.map_err(network_error)?;
        let status = response.status().as_u16();
        if !(200..300).contains(&status) {
            return Err(refusal(status, response, api_key.is_some()).await);
        }
        let body = read_capped(response, MAX_BODY_BYTES)
            .await
            .map_err(|err| match err {
                BodyError::TooLarge => {
                    tracing::warn!("PageSpeed sent an answer that was too large");
                    PageSpeedError::Failed
                }
                BodyError::Read(err) => network_error(err),
            })?;
        let value: Value = serde_json::from_slice(&body).map_err(|_| {
            tracing::warn!("PageSpeed sent an answer that is not JSON");
            PageSpeedError::Unrecognised
        })?;
        read_measurement(&value)
    }
}

/// A transport failure. The address is dropped from the error before it can
/// reach a log line or a message.
fn network_error(err: reqwest::Error) -> PageSpeedError {
    let err = err.without_url();
    tracing::warn!(
        timeout = err.is_timeout(),
        connect = err.is_connect(),
        "PageSpeed request failed"
    );
    if err.is_timeout() {
        PageSpeedError::TimedOut
    } else {
        PageSpeedError::Failed
    }
}

/// A non-success answer. Only a 400 reads its body, to tell a bad key from a
/// bad page; the body is never kept.
async fn refusal(status: u16, response: reqwest::Response, has_key: bool) -> PageSpeedError {
    tracing::warn!(status, "PageSpeed answered with an error");
    match status {
        401 | 403 => PageSpeedError::Rejected,
        429 if has_key => PageSpeedError::RateLimited,
        429 => PageSpeedError::NoKeyLimit,
        400 => match read_capped(response, MAX_ERROR_BODY).await {
            Ok(body) if mentions_key(&body) => PageSpeedError::Rejected,
            _ => PageSpeedError::Failed,
        },
        _ => PageSpeedError::Failed,
    }
}

fn mentions_key(body: &[u8]) -> bool {
    String::from_utf8_lossy(body)
        .to_ascii_lowercase()
        .contains("api key")
}

fn number_at<'a>(value: &'a Value, path: &[&str]) -> Option<&'a Value> {
    path.iter().try_fold(value, |found, key| found.get(*key))
}

fn rounded(value: Option<f64>) -> Option<i32> {
    value
        .filter(|number| number.is_finite() && *number >= 0.0)
        .map(|number| number.round().min(f64::from(i32::MAX)) as i32)
}

/// The percentile of one field metric, when Chrome UX Report has it for this
/// page. When Google fell back to the whole site's numbers
/// (`origin_fallback`), they are not this page's and read as missing.
fn field(value: &Value, name: &str) -> Option<f64> {
    let fallback = value
        .pointer("/loadingExperience/origin_fallback")
        .and_then(Value::as_bool);
    if fallback == Some(true) {
        return None;
    }
    number_at(value, &["loadingExperience", "metrics", name, "percentile"])?.as_f64()
}

fn lab(value: &Value, audit: &str) -> Option<f64> {
    number_at(
        value,
        &["lighthouseResult", "audits", audit, "numericValue"],
    )?
    .as_f64()
}

fn read_measurement(value: &Value) -> Result<Measurement, PageSpeedError> {
    if value.get("lighthouseResult").is_none() {
        tracing::warn!("no lighthouseResult in PageSpeed's answer");
        return Err(PageSpeedError::Unrecognised);
    }
    let score = number_at(
        value,
        &["lighthouseResult", "categories", "performance", "score"],
    )
    .and_then(Value::as_f64);
    let field_lcp = field(value, "LARGEST_CONTENTFUL_PAINT_MS");
    // The field percentile of layout shift is the score times 100.
    let field_cls = field(value, "CUMULATIVE_LAYOUT_SHIFT_SCORE").map(|p| p / 100.0);
    let field_inp = field(value, "INTERACTION_TO_NEXT_PAINT");
    let found = Measurement {
        performance: rounded(score.map(|score| score * 100.0)).map(|score| score.min(100)),
        lcp_ms: rounded(field_lcp.or_else(|| lab(value, "largest-contentful-paint"))),
        cls: field_cls
            .or_else(|| lab(value, "cumulative-layout-shift"))
            .filter(|cls| cls.is_finite() && *cls >= 0.0),
        inp_ms: rounded(field_inp),
        tbt_ms: rounded(lab(value, "total-blocking-time")),
        field_data: field_lcp.is_some() || field_cls.is_some() || field_inp.is_some(),
    };
    if found.performance.is_none()
        && found.lcp_ms.is_none()
        && found.cls.is_none()
        && found.inp_ms.is_none()
        && found.tbt_ms.is_none()
    {
        tracing::warn!("PageSpeed measured nothing for a page");
        return Err(PageSpeedError::Failed);
    }
    Ok(found)
}
