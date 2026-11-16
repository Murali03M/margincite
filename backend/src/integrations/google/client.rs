//! The three Google services Margincite talks to, over plain HTTPS. Their
//! hosts are fixed in `GoogleHosts::production`; tests point them at a mock.
//!
//! Nothing here logs or returns a response body, a token or the client
//! secret: errors carry a short sentence only.

use std::time::Duration;

use serde::Deserialize;
use serde_json::json;

use crate::body::{read_capped, BodyError, MAX_BODY_BYTES};
use crate::config::GoogleCredentials;

/// Search Console answers within a minute or we give up on the attempt.
const TIMEOUT: Duration = Duration::from_secs(60);
/// The most rows one Search Console request may return.
pub const ROW_LIMIT: usize = crate::checks::search::ROW_LIMIT;

/// Where each Google service lives.
#[derive(Debug, Clone)]
pub struct GoogleHosts {
    /// The consent screen people are sent to.
    pub auth_url: String,
    /// Exchanges a code for tokens and refreshes access tokens.
    pub token_url: String,
    /// The Search Console API, without a trailing slash.
    pub api_base: String,
}

impl GoogleHosts {
    pub fn production() -> GoogleHosts {
        GoogleHosts {
            auth_url: "https://accounts.google.com/o/oauth2/v2/auth".into(),
            token_url: "https://oauth2.googleapis.com/token".into(),
            api_base: "https://www.googleapis.com/webmasters/v3".into(),
        }
    }
}

/// What went wrong, in terms of what to do about it.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum GoogleError {
    /// Google refused the token or the property. The person must connect again.
    #[error("Google no longer accepts this connection.")]
    Rejected,
    /// Google is busy, unreachable or answered something unexpected. Try again later.
    #[error("{0}")]
    Transient(String),
    /// The token endpoint refused this server's own client id or secret
    /// (`invalid_client`). Nothing is wrong with the person's connection.
    #[error("Google rejected this server's Google credentials.")]
    ServerCredentials,
}

/// What the code exchange returns. `Debug` never shows the tokens.
pub struct Tokens {
    pub access_token: String,
    pub refresh_token: Option<String>,
}

impl std::fmt::Debug for Tokens {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Tokens(..)")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Property {
    pub property: String,
    pub permission: String,
}

/// One row of a Search Console report.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct ApiRow {
    #[serde(default)]
    pub keys: Vec<String>,
    #[serde(default)]
    pub clicks: f64,
    #[serde(default)]
    pub impressions: f64,
    #[serde(default)]
    pub ctr: f64,
    #[serde(default)]
    pub position: f64,
}

#[derive(Debug, Clone)]
pub struct Report<'a> {
    pub start_date: String,
    pub end_date: String,
    pub dimensions: &'a [&'a str],
}

#[derive(Clone)]
pub struct GoogleClient {
    http: reqwest::Client,
    hosts: GoogleHosts,
}

impl GoogleClient {
    pub fn new(hosts: GoogleHosts) -> GoogleClient {
        let http = reqwest::Client::builder()
            .timeout(TIMEOUT)
            .user_agent("margincite")
            .build()
            .expect("an HTTP client with default settings builds");
        GoogleClient { http, hosts }
    }

    pub fn hosts(&self) -> &GoogleHosts {
        &self.hosts
    }

    /// Trades the one-time code from the consent screen for tokens.
    pub async fn exchange_code(
        &self,
        credentials: &GoogleCredentials,
        redirect_uri: &str,
        code: &str,
    ) -> Result<Tokens, GoogleError> {
        let body = form(&[
            ("grant_type", "authorization_code"),
            ("code", code),
            ("redirect_uri", redirect_uri),
            ("client_id", &credentials.client_id),
            ("client_secret", &credentials.client_secret),
        ]);
        let reply: TokenReply = self.token_request(body).await?;
        Ok(Tokens {
            access_token: reply.access_token,
            refresh_token: reply.refresh_token.filter(|token| !token.is_empty()),
        })
    }

    /// A short-lived access token for one sync. Never stored.
    pub async fn refresh_access_token(
        &self,
        credentials: &GoogleCredentials,
        refresh_token: &str,
    ) -> Result<String, GoogleError> {
        let body = form(&[
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh_token),
            ("client_id", &credentials.client_id),
            ("client_secret", &credentials.client_secret),
        ]);
        Ok(self.token_request::<TokenReply>(body).await?.access_token)
    }

    async fn token_request<T: serde::de::DeserializeOwned>(
        &self,
        body: String,
    ) -> Result<T, GoogleError> {
        let request = self
            .http
            .post(&self.hosts.token_url)
            .header("content-type", "application/x-www-form-urlencoded")
            .body(body);
        let response = request.send().await.map_err(unreachable_error)?;
        let status = response.status().as_u16();
        // At the token endpoint a 400 or 401 means the code or token is dead,
        // unless Google says the client itself is wrong.
        if matches!(status, 400 | 401) {
            let body = read_capped(response, MAX_BODY_BYTES)
                .await
                .unwrap_or_default();
            return Err(refusal(&body));
        }
        decode(status, response).await
    }

    /// The properties the person can read in Search Console.
    pub async fn list_properties(&self, access_token: &str) -> Result<Vec<Property>, GoogleError> {
        let url = format!("{}/sites", self.hosts.api_base);
        let response = self
            .http
            .get(url)
            .bearer_auth(access_token)
            .send()
            .await
            .map_err(unreachable_error)?;
        let status = response.status().as_u16();
        let reply: SitesReply = self.api_reply(status, response).await?;
        Ok(reply
            .site_entry
            .into_iter()
            .map(|entry| Property {
                property: entry.site_url,
                permission: entry.permission_level,
            })
            .collect())
    }

    /// One Search Console report: at most `ROW_LIMIT` rows, final data only.
    pub async fn report(
        &self,
        access_token: &str,
        property: &str,
        report: &Report<'_>,
    ) -> Result<Vec<ApiRow>, GoogleError> {
        let url = format!(
            "{}/sites/{}/searchAnalytics/query",
            self.hosts.api_base,
            encode_component(property)
        );
        let body = json!({
            "startDate": report.start_date,
            "endDate": report.end_date,
            "dimensions": report.dimensions,
            "rowLimit": ROW_LIMIT,
            "dataState": "final",
        });
        let response = self
            .http
            .post(url)
            .bearer_auth(access_token)
            .json(&body)
            .send()
            .await
            .map_err(unreachable_error)?;
        let status = response.status().as_u16();
        let reply: ReportReply = self.api_reply(status, response).await?;
        Ok(reply.rows)
    }

    /// 401, 403 and 404 from the API mean the token or the property is no
    /// longer usable; anything else that is not a success is worth retrying.
    async fn api_reply<T: serde::de::DeserializeOwned>(
        &self,
        status: u16,
        response: reqwest::Response,
    ) -> Result<T, GoogleError> {
        if matches!(status, 401 | 403 | 404) {
            return Err(GoogleError::Rejected);
        }
        decode(status, response).await
    }
}

/// What a refused token request means: only `invalid_client` is about this
/// server; anything else (`invalid_grant` and the rest) is about the grant.
fn refusal(body: &[u8]) -> GoogleError {
    let code = serde_json::from_slice::<serde_json::Value>(body)
        .ok()
        .and_then(|value| value.get("error")?.as_str().map(str::to_string));
    match code.as_deref() {
        Some("invalid_client") => GoogleError::ServerCredentials,
        _ => GoogleError::Rejected,
    }
}

async fn decode<T: serde::de::DeserializeOwned>(
    status: u16,
    response: reqwest::Response,
) -> Result<T, GoogleError> {
    if !(200..300).contains(&status) {
        return Err(GoogleError::Transient(format!("Google answered {status}.")));
    }
    let body = read_capped(response, MAX_BODY_BYTES)
        .await
        .map_err(|err| match err {
            BodyError::TooLarge => {
                GoogleError::Transient("Google sent an answer that was too large.".into())
            }
            BodyError::Read(_) => {
                GoogleError::Transient("Google sent an answer we could not read.".into())
            }
        })?;
    serde_json::from_slice::<T>(&body)
        .map_err(|_| GoogleError::Transient("Google sent an answer we could not read.".into()))
}

fn unreachable_error(err: reqwest::Error) -> GoogleError {
    // The error's text can include the address but never a token; keep it out
    // anyway and say only what happened.
    let what = if err.is_timeout() {
        "Google took too long to answer."
    } else {
        "Google could not be reached."
    };
    GoogleError::Transient(what.into())
}

#[derive(Deserialize)]
struct TokenReply {
    access_token: String,
    #[serde(default)]
    refresh_token: Option<String>,
}

#[derive(Deserialize)]
struct SitesReply {
    #[serde(default, rename = "siteEntry")]
    site_entry: Vec<SiteEntry>,
}

#[derive(Deserialize)]
struct SiteEntry {
    #[serde(rename = "siteUrl")]
    site_url: String,
    #[serde(default, rename = "permissionLevel")]
    permission_level: String,
}

#[derive(Deserialize)]
struct ReportReply {
    /// Google leaves `rows` out entirely when there is no data.
    #[serde(default)]
    rows: Vec<ApiRow>,
}

fn form(pairs: &[(&str, &str)]) -> String {
    let mut out = url::form_urlencoded::Serializer::new(String::new());
    for (key, value) in pairs {
        out.append_pair(key, value);
    }
    out.finish()
}

/// Percent-encodes everything except unreserved characters, so a property such
/// as `https://example.com/` or `sc-domain:example.com` is one path segment.
pub fn encode_component(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for byte in text.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            out.push(byte as char);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}
