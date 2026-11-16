//! DataForSEO over plain HTTPS with Basic auth. The base address is a
//! constructor argument; production uses `PRODUCTION_BASE`.
//!
//! The login and password travel in the `Authorization` header, so nothing
//! here logs a request, a header or a response body, and every error is a
//! fixed sentence built from nothing the provider sent.

use std::time::Duration;

use serde_json::{json, Value};

use crate::body::{read_capped, BodyError, MAX_BODY_BYTES};

pub const PRODUCTION_BASE: &str = "https://api.dataforseo.com";
/// A live task may run up to 120 seconds on DataForSEO's side.
const TIMEOUT: Duration = Duration::from_secs(130);
/// How many results to ask for. DataForSEO bills each 10 results as one page.
pub const SERP_DEPTH: u32 = 100;

/// A DataForSEO login. Not `Debug` on purpose: it holds a secret.
pub struct Credentials<'a> {
    pub login: &'a str,
    pub password: &'a str,
}

/// What went wrong with one request. Every message is a fixed sentence: it is
/// stored and shown, so it never holds a response body, a status message or
/// anything that could echo the login.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum DataForSeoError {
    #[error("DataForSEO rejected the login or password. Check them in Settings.")]
    Rejected,
    #[error("Your DataForSEO balance has run out.")]
    OutOfCredit,
    #[error("DataForSEO is limiting requests right now. Try again later.")]
    RateLimited,
    #[error("DataForSEO took too long to answer.")]
    TimedOut,
    #[error("DataForSEO could not answer this request.")]
    Failed,
    #[error("DataForSEO replied in a format Margincite does not recognise.")]
    Unrecognised,
}

impl DataForSeoError {
    /// Nothing else will work until the person fixes this in Settings or tops
    /// up, so a run stops asking.
    pub fn is_fatal(self) -> bool {
        matches!(
            self,
            DataForSeoError::Rejected | DataForSeoError::OutOfCredit
        )
    }

    /// Worth asking again later without any change from the person.
    pub fn is_temporary(self) -> bool {
        matches!(
            self,
            DataForSeoError::RateLimited | DataForSeoError::TimedOut | DataForSeoError::Failed
        )
    }
}

/// One live Google organic request.
pub struct SerpQuery<'a> {
    pub keyword: &'a str,
    pub location_code: u32,
    pub language_code: &'a str,
    pub device: &'a str,
}

#[derive(Clone)]
pub struct DataForSeoClient {
    http: reqwest::Client,
    base: String,
}

impl DataForSeoClient {
    pub fn new(base_url: &str) -> DataForSeoClient {
        DataForSeoClient::with_timeout(base_url, TIMEOUT)
    }

    /// For tests, which cannot wait two minutes.
    pub fn with_timeout(base_url: &str, timeout: Duration) -> DataForSeoClient {
        let http = reqwest::Client::builder()
            .timeout(timeout)
            .user_agent("margincite")
            .build()
            .expect("an HTTP client with default settings builds");
        DataForSeoClient {
            http,
            base: base_url.trim_end_matches('/').to_string(),
        }
    }

    pub fn production() -> DataForSeoClient {
        DataForSeoClient::new(PRODUCTION_BASE)
    }

    /// The items of the first Google organic result page for the query.
    /// Documented shape: `tasks[0].result[0].items[]`.
    pub async fn serp(
        &self,
        credentials: &Credentials<'_>,
        query: &SerpQuery<'_>,
    ) -> Result<Vec<Value>, DataForSeoError> {
        let task = json!({
            "keyword": query.keyword,
            "location_code": query.location_code,
            "language_code": query.language_code,
            "device": query.device,
            "depth": SERP_DEPTH,
            "load_async_ai_overview": true,
        });
        let result = self
            .task_result(credentials, "/v3/serp/google/organic/live/advanced", task)
            .await?;
        let page = result.first().ok_or(DataForSeoError::Unrecognised)?;
        match page.get("items") {
            // No results at all is an answer: the site is not in them.
            Some(Value::Null) => Ok(Vec::new()),
            Some(Value::Array(items)) => Ok(items.clone()),
            _ => Err(DataForSeoError::Unrecognised),
        }
    }

    /// The account balance in US dollars (`None` when the answer has none).
    /// Documented shape: `tasks[0].result[0].money.balance`. Also the cheapest
    /// way to find out whether a login works.
    pub async fn account_balance(
        &self,
        credentials: &Credentials<'_>,
    ) -> Result<Option<f64>, DataForSeoError> {
        let request = self
            .http
            .get(format!("{}/v3/appendix/user_data", self.base));
        let result = self.send(credentials, request).await?;
        Ok(result
            .first()
            .and_then(|account| account.pointer("/money/balance"))
            .and_then(Value::as_f64))
    }

    /// Posts one task and returns its `result` array.
    pub(crate) async fn task_result(
        &self,
        credentials: &Credentials<'_>,
        path: &str,
        task: Value,
    ) -> Result<Vec<Value>, DataForSeoError> {
        let request = self.http.post(format!("{}{path}", self.base)).json(&[task]);
        self.send(credentials, request).await
    }

    /// Sends a request with Basic auth, the one place the login is used.
    /// HTTP is almost always 200, so the answer's own status codes decide.
    async fn send(
        &self,
        credentials: &Credentials<'_>,
        request: reqwest::RequestBuilder,
    ) -> Result<Vec<Value>, DataForSeoError> {
        let response = request
            .basic_auth(credentials.login, Some(credentials.password))
            .send()
            .await
            .map_err(network_error)?;
        let status = response.status().as_u16();
        if !(200..300).contains(&status) {
            tracing::warn!(status, "DataForSEO answered with an error");
            return Err(match status {
                401 | 403 => DataForSeoError::Rejected,
                402 => DataForSeoError::OutOfCredit,
                429 => DataForSeoError::RateLimited,
                _ => DataForSeoError::Failed,
            });
        }
        let body = read_capped(response, MAX_BODY_BYTES)
            .await
            .map_err(|err| match err {
                BodyError::TooLarge => {
                    tracing::warn!("DataForSEO sent an answer that was too large");
                    DataForSeoError::Failed
                }
                BodyError::Read(err) => network_error(err),
            })?;
        let value: Value = serde_json::from_slice(&body).map_err(|_| {
            tracing::warn!("DataForSEO sent an answer that is not JSON");
            DataForSeoError::Unrecognised
        })?;
        read_result(&value)
    }
}

/// A transport failure. The address is dropped from the error before it can
/// reach a log line or a message.
fn network_error(err: reqwest::Error) -> DataForSeoError {
    let err = err.without_url();
    tracing::warn!(
        timeout = err.is_timeout(),
        connect = err.is_connect(),
        "DataForSEO request failed"
    );
    if err.is_timeout() {
        DataForSeoError::TimedOut
    } else {
        DataForSeoError::Failed
    }
}

/// What a status code in the answer means, from DataForSEO's error list.
fn code_error(code: i64) -> Option<DataForSeoError> {
    match code {
        20000 => None,
        40100..=40199 => Some(DataForSeoError::Rejected),
        40200 | 40203 | 40210 => Some(DataForSeoError::OutOfCredit),
        40202 | 40209 => Some(DataForSeoError::RateLimited),
        50401 => Some(DataForSeoError::TimedOut),
        _ => Some(DataForSeoError::Failed),
    }
}

fn read_result(value: &Value) -> Result<Vec<Value>, DataForSeoError> {
    let code = |holder: &Value| holder.get("status_code").and_then(Value::as_i64);
    let failed = |found: i64| {
        let err = code_error(found)?;
        tracing::warn!(code = found, "DataForSEO reported a problem");
        Some(err)
    };
    let top = code(value).ok_or_else(|| {
        tracing::warn!("DataForSEO's answer has no status code");
        DataForSeoError::Unrecognised
    })?;
    if let Some(err) = failed(top) {
        return Err(err);
    }
    let task = value.pointer("/tasks/0");
    let task_code = task.and_then(code).ok_or_else(|| {
        tracing::warn!("DataForSEO's answer has no task status code");
        DataForSeoError::Unrecognised
    })?;
    if let Some(err) = failed(task_code) {
        return Err(err);
    }
    match task.and_then(|task| task.get("result")) {
        Some(Value::Array(result)) => Ok(result.clone()),
        _ => {
            tracing::warn!("DataForSEO's answer has no result");
            Err(DataForSeoError::Unrecognised)
        }
    }
}
