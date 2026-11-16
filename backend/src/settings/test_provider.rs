//! One cheap request per provider to find out whether a key works. Every
//! answer is turned into a sentence written here; nothing a provider sent is
//! repeated, so a key it echoed back cannot reach a stored row or a response.

use std::time::Duration;

use serde_json::json;

use crate::ai::engines::{claude, http_client, join, send, EngineBases, EngineError, EngineId};
use crate::integrations::dataforseo::{Credentials, DataForSeoClient, DataForSeoError};

pub const DATAFORSEO_BASE: &str = "https://api.dataforseo.com";
/// A test runs inside a request, so it waits far less than an answer does.
pub const PROBE_TIMEOUT: Duration = Duration::from_secs(15);

pub const REJECTED: &str = "The key was rejected.";

fn sentence(err: EngineError) -> String {
    match err {
        EngineError::Http(401 | 403) => REJECTED.to_string(),
        EngineError::Http(code) => format!("The provider answered with HTTP {code}."),
        EngineError::Timeout => "The provider did not answer in time.".to_string(),
        EngineError::Transport => "The provider could not be reached.".to_string(),
        EngineError::Unreadable | EngineError::Refused(_) => {
            "The provider's answer could not be read.".to_string()
        }
    }
}

/// `Ok` when the provider accepts the key.
pub async fn provider(
    bases: &EngineBases,
    id: EngineId,
    key: &str,
    model: &str,
) -> Result<(), String> {
    let client = http_client(PROBE_TIMEOUT);
    let request = match id {
        EngineId::Claude => client
            .get(join(&bases.claude, "/v1/models"))
            .header("x-api-key", key)
            .header("anthropic-version", claude::VERSION),
        EngineId::ChatGpt => client
            .get(join(&bases.openai, "/v1/models"))
            .bearer_auth(key),
        EngineId::Grok => client.get(join(&bases.grok, "/v1/models")).bearer_auth(key),
        EngineId::Gemini => client
            .get(join(&bases.gemini, "/v1beta/models"))
            .header("x-goog-api-key", key),
        EngineId::Perplexity => client
            .post(join(&bases.perplexity, "/chat/completions"))
            .bearer_auth(key)
            .json(&json!({
                "model": model,
                "max_tokens": 1,
                "messages": [{ "role": "user", "content": "Hi" }],
            })),
    };
    send(request).await.map(|_| ()).map_err(sentence)
}

/// The account balance in US dollars when the login works. The answer is read
/// by the DataForSEO client, which the rest of the app uses too; its errors
/// are turned into the sentences shown beside the Settings form.
pub async fn dataforseo(base: &str, login: &str, password: &str) -> Result<Option<f64>, String> {
    let client = DataForSeoClient::with_timeout(base, PROBE_TIMEOUT);
    client
        .account_balance(&Credentials { login, password })
        .await
        .map_err(|err| match err {
            DataForSeoError::Rejected => "The login or password was rejected.".to_string(),
            DataForSeoError::OutOfCredit => "The account is out of credit.".to_string(),
            DataForSeoError::TimedOut => "The provider did not answer in time.".to_string(),
            DataForSeoError::Unrecognised => "The provider's answer could not be read.".to_string(),
            // The client's own sentences already say these plainly.
            DataForSeoError::RateLimited | DataForSeoError::Failed => err.to_string(),
        })
}
