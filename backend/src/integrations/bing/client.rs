//! Bing Webmaster Tools' JSON API over plain HTTPS. The base address is a
//! constructor argument; production uses `PRODUCTION_BASE`.
//!
//! The API key travels in the `apikey` query parameter, so nothing here logs
//! or returns a request address, a response body or the key: errors carry a
//! fixed sentence only, and field problems are logged by field name.

use std::time::Duration;

use serde_json::Value;
use url::Url;

use super::parse;
use crate::body::{read_capped, BodyError, MAX_BODY_BYTES};

pub const PRODUCTION_BASE: &str = "https://ssl.bing.com/webmaster/api.svc/json/";
const TIMEOUT: Duration = Duration::from_secs(60);

/// What went wrong, in terms of what to do about it.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BingError {
    /// Bing refused the key. The person must connect again.
    #[error("Bing no longer accepts this API key.")]
    Rejected,
    /// Bing is busy, unreachable or answered something unusable. Try again later.
    #[error("{0}")]
    Transient(String),
    /// Bing answered, but not in a shape this code reads. Never treated as
    /// "no data", so a format change cannot wipe what was stored.
    #[error("Bing replied in a format Margincite does not recognise. The last good data is kept.")]
    Unrecognised,
}

/// A page of your site that other sites link to, and how many links.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkCount {
    pub url: String,
    pub count: i64,
}

/// One link pointing at a page of your site.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkSource {
    pub source_url: String,
    pub anchor_text: String,
}

#[derive(Clone)]
pub struct BingClient {
    http: reqwest::Client,
    base: String,
}

impl BingClient {
    pub fn new(base_url: &str) -> BingClient {
        let http = reqwest::Client::builder()
            .timeout(TIMEOUT)
            .user_agent("margincite")
            .build()
            .expect("an HTTP client with default settings builds");
        BingClient {
            http,
            base: base_url.to_string(),
        }
    }

    pub fn production() -> BingClient {
        BingClient::new(PRODUCTION_BASE)
    }

    /// The sites in the key's Bing Webmaster Tools account.
    pub async fn user_sites(&self, api_key: &str) -> Result<Vec<String>, BingError> {
        let rows = self.rows("GetUserSites", api_key, &[]).await?;
        let found: Vec<String> = rows
            .iter()
            .filter_map(|row| parse::text(row, &["url"]))
            .collect();
        ensure_readable(rows.len(), found.len())?;
        Ok(found)
    }

    /// One 0-based page of the pages of `site_url` that are linked to.
    pub async fn link_counts(
        &self,
        api_key: &str,
        site_url: &str,
        page: u32,
    ) -> Result<Vec<LinkCount>, BingError> {
        let page = page.to_string();
        let params = [("siteUrl", site_url), ("page", page.as_str())];
        let rows = self.rows("GetLinkCounts", api_key, &params).await?;
        let mut unmapped = (0, 0);
        let found: Vec<LinkCount> = rows
            .iter()
            .filter_map(|row| {
                let Some(url) = parse::text(row, &["url"]) else {
                    unmapped.0 += 1;
                    return None;
                };
                let count = parse::count(row, "count").unwrap_or_else(|| {
                    unmapped.1 += 1;
                    0
                });
                Some(LinkCount { url, count })
            })
            .collect();
        note_unmapped("GetLinkCounts", unmapped.0, "Url");
        note_unmapped("GetLinkCounts", unmapped.1, "Count");
        ensure_readable(rows.len(), found.len())?;
        Ok(found)
    }

    /// One 0-based page of the links that point at `link`, a page of `site_url`.
    pub async fn url_links(
        &self,
        api_key: &str,
        site_url: &str,
        link: &str,
        page: u32,
    ) -> Result<Vec<LinkSource>, BingError> {
        let page = page.to_string();
        let params = [
            ("siteUrl", site_url),
            ("link", link),
            ("page", page.as_str()),
        ];
        let rows = self.rows("GetUrlLinks", api_key, &params).await?;
        let mut without_url = 0;
        let found: Vec<LinkSource> = rows
            .iter()
            .filter_map(|row| {
                let Some(source_url) = parse::text(row, &["url"]) else {
                    without_url += 1;
                    return None;
                };
                Some(LinkSource {
                    source_url,
                    anchor_text: parse::text(row, &["anchortext"]).unwrap_or_default(),
                })
            })
            .collect();
        note_unmapped("GetUrlLinks", without_url, "Url");
        ensure_readable(rows.len(), found.len())?;
        Ok(found)
    }

    /// Calls one method and returns the rows of its answer, owned.
    async fn rows(
        &self,
        method: &str,
        api_key: &str,
        params: &[(&str, &str)],
    ) -> Result<Vec<Value>, BingError> {
        let mut url = Url::parse(&format!("{}/{method}", self.base.trim_end_matches('/')))
            .map_err(|_| BingError::Transient("Bing's address is not valid.".into()))?;
        url.query_pairs_mut()
            .append_pair("apikey", api_key)
            .extend_pairs(params);
        let response = self.http.get(url).send().await.map_err(unreachable_error)?;
        let status = response.status().as_u16();
        let body = match read_capped(response, MAX_BODY_BYTES).await {
            Ok(bytes) => String::from_utf8_lossy(&bytes).into_owned(),
            Err(BodyError::TooLarge) => {
                return Err(BingError::Transient(
                    "Bing sent an answer that was too large.".into(),
                ))
            }
            Err(BodyError::Read(err)) => return Err(unreachable_error(err)),
        };
        if matches!(status, 401 | 403) || (status == 400 && says_key_is_bad(&body)) {
            return Err(BingError::Rejected);
        }
        if !(200..300).contains(&status) {
            return Err(BingError::Transient(format!("Bing answered {status}.")));
        }
        let parsed = serde_json::from_str::<Value>(&body).map(parse::unwrap_body);
        let Ok(value) = parsed else {
            tracing::warn!(method, "Bing sent an answer that is not JSON");
            return Err(BingError::Unrecognised);
        };
        // Only a list with nothing in it, or a null, means "no data".
        if value.is_null() {
            return Ok(Vec::new());
        }
        let found = parse::rows(&value);
        if !found.recognised {
            tracing::warn!(
                method,
                fields = ?parse::field_names(&value),
                "no list of rows found in Bing's answer"
            );
            return Err(BingError::Unrecognised);
        }
        Ok(found.rows.into_iter().cloned().collect())
    }
}

/// Rows came back but none could be read: the field names have changed.
fn ensure_readable(sent: usize, read: usize) -> Result<(), BingError> {
    if sent > 0 && read == 0 {
        return Err(BingError::Unrecognised);
    }
    Ok(())
}

/// Bing answers a bad key with 400 and a message naming it.
fn says_key_is_bad(body: &str) -> bool {
    let body = body.to_ascii_lowercase();
    body.contains("apikey") || body.contains("notauthorized") || body.contains("not authorized")
}

fn note_unmapped(method: &str, rows: usize, field: &str) {
    if rows > 0 {
        tracing::warn!(method, rows, field, "Bing rows lacked a field we read");
    }
}

fn unreachable_error(err: reqwest::Error) -> BingError {
    // The error's text includes the request address, which holds the key.
    let what = if err.is_timeout() {
        "Bing took too long to answer."
    } else {
        "Bing could not be reached."
    };
    BingError::Transient(what.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_400_is_a_bad_key_only_when_it_says_so() {
        assert!(says_key_is_bad(r#"{"Message":"ERROR!!! InvalidApiKey"}"#));
        assert!(says_key_is_bad("ERROR!!! NotAuthorized"));
        assert!(!says_key_is_bad(
            r#"{"Message":"ERROR!!! InvalidParameter"}"#
        ));
    }

    #[test]
    fn errors_never_carry_the_key() {
        let rejected = BingError::Rejected.to_string();
        let transient = BingError::Transient("Bing answered 500.".into()).to_string();
        assert!(!rejected.contains("apikey") && !transient.contains("apikey"));
    }
}
