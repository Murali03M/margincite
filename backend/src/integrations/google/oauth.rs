//! The addresses of the sign-in: where people are sent to approve access,
//! where Google sends them back, and where they land in the web app.

use url::Url;
use uuid::Uuid;

use super::client::GoogleHosts;

/// What Margincite asks for: read-only Search Console data.
pub const SCOPE: &str = "https://www.googleapis.com/auth/webmasters.readonly";
pub const PROVIDER: &str = "google";
/// A sign-in must come back within this long.
pub const STATE_MINUTES: i64 = 10;

/// The cookie that ties a sign-in to the browser that started it.
pub const COOKIE_NAME: &str = "margincite_oauth";
const COOKIE_PATH: &str = "/api/v1/oauth/google";
const NONCE_BYTES: usize = 32;

/// A fresh browser nonce: 32 bytes from the system CSPRNG, base64url.
pub fn new_nonce() -> String {
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    use base64::Engine;
    use chacha20poly1305::aead::Generate;
    let bytes: [u8; NONCE_BYTES] = chacha20poly1305::Key::generate().into();
    URL_SAFE_NO_PAD.encode(bytes)
}

/// What is stored in place of the nonce: SHA-256, base64url.
pub fn hash_nonce(nonce: &str) -> String {
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    use base64::Engine;
    use sha2::{Digest, Sha256};
    URL_SAFE_NO_PAD.encode(Sha256::digest(nonce.as_bytes()))
}

/// The `Set-Cookie` value that hands the nonce to the browser. `Secure` only
/// when the app is served over https.
pub fn nonce_cookie(nonce: &str, public_base_url: &Url) -> String {
    let secure = if public_base_url.scheme() == "https" {
        "; Secure"
    } else {
        ""
    };
    format!(
        "{COOKIE_NAME}={nonce}; Path={COOKIE_PATH}; HttpOnly; SameSite=Lax; Max-Age={}{secure}",
        STATE_MINUTES * 60
    )
}

/// The `Set-Cookie` value that removes the nonce.
pub fn clear_cookie(public_base_url: &Url) -> String {
    let secure = if public_base_url.scheme() == "https" {
        "; Secure"
    } else {
        ""
    };
    format!("{COOKIE_NAME}=; Path={COOKIE_PATH}; HttpOnly; SameSite=Lax; Max-Age=0{secure}")
}

/// The nonce from the request's `Cookie` headers, if there is one.
pub fn nonce_from_cookies(headers: &axum::http::HeaderMap) -> Option<String> {
    headers
        .get_all(axum::http::header::COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(';'))
        .filter_map(|pair| pair.trim().split_once('='))
        .find(|(name, _)| *name == COOKIE_NAME)
        .map(|(_, value)| value.to_string())
}

/// Whether the browser's nonce is the one this sign-in started with, in
/// constant time. A state without a stored hash never matches.
pub fn nonce_matches(stored_hash: Option<&str>, browser_nonce: Option<&str>) -> bool {
    use subtle::ConstantTimeEq;
    match (stored_hash, browser_nonce) {
        (Some(stored), Some(nonce)) => {
            let given = hash_nonce(nonce);
            stored.as_bytes().ct_eq(given.as_bytes()).into()
        }
        _ => false,
    }
}

/// Where Google sends people back. Register exactly this address with the
/// OAuth client. Never ends with a slash twice.
pub fn redirect_uri(public_base_url: &Url) -> String {
    format!(
        "{}/api/v1/oauth/google/callback",
        public_base_url.as_str().trim_end_matches('/')
    )
}

/// The consent screen address for one sign-in attempt.
pub fn authorization_url(
    hosts: &GoogleHosts,
    client_id: &str,
    redirect_uri: &str,
    state: Uuid,
) -> Result<String, url::ParseError> {
    let mut url = Url::parse(&hosts.auth_url)?;
    url.query_pairs_mut()
        .append_pair("client_id", client_id)
        .append_pair("redirect_uri", redirect_uri)
        .append_pair("response_type", "code")
        .append_pair("scope", SCOPE)
        .append_pair("access_type", "offline")
        .append_pair("prompt", "consent")
        .append_pair("state", &state.to_string());
    Ok(url.into())
}

/// How a sign-in ended, as the web app is told. The reason is always one of
/// these fixed codes, never text from the request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Connected,
    Error(Reason),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reason {
    /// The state is unknown, used before, or does not belong to its site.
    InvalidState,
    /// The state is older than 10 minutes.
    Expired,
    /// The person said no on Google's screen.
    Denied,
    MissingCode,
    /// Google would not trade the code for tokens.
    TokenRejected,
    /// Google did not give a refresh token, so the connection could not last.
    NoRefreshToken,
    GoogleUnavailable,
    /// Google sign-in or the secret key is not set up on this server.
    NotConfigured,
    ServerError,
}

impl Reason {
    pub fn code(self) -> &'static str {
        match self {
            Reason::InvalidState => "invalid_state",
            Reason::Expired => "expired",
            Reason::Denied => "denied",
            Reason::MissingCode => "missing_code",
            Reason::TokenRejected => "token_rejected",
            Reason::NoRefreshToken => "no_refresh_token",
            Reason::GoogleUnavailable => "google_unavailable",
            Reason::NotConfigured => "not_configured",
            Reason::ServerError => "server_error",
        }
    }
}

/// Where the browser is sent at the end: the site's Connections screen when
/// the site is known, the app's front page otherwise. Built only from
/// `PUBLIC_BASE_URL` and the site id found in our own table.
pub fn app_redirect(public_base_url: &Url, site_id: Option<Uuid>, outcome: Outcome) -> String {
    let mut url = public_base_url.clone();
    {
        let mut path = url
            .path_segments_mut()
            .expect("an http(s) base URL can have path segments");
        path.pop_if_empty();
        if let Some(site_id) = site_id {
            path.extend(["sites", &site_id.to_string(), "connections"]);
        } else {
            path.push("");
        }
    }
    url.set_query(None);
    url.set_fragment(None);
    {
        let mut query = url.query_pairs_mut();
        match outcome {
            Outcome::Connected => query.append_pair("google", "connected"),
            Outcome::Error(reason) => query
                .append_pair("google", "error")
                .append_pair("reason", reason.code()),
        };
    }
    url.into()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base(text: &str) -> Url {
        Url::parse(text).unwrap()
    }

    #[test]
    fn the_redirect_uri_has_one_slash_before_the_path() {
        for text in ["http://localhost:5174", "http://localhost:5174/"] {
            assert_eq!(
                redirect_uri(&base(text)),
                "http://localhost:5174/api/v1/oauth/google/callback"
            );
        }
    }

    #[test]
    fn the_app_redirect_is_built_from_the_base_and_the_site() {
        let site = Uuid::nil();
        assert_eq!(
            app_redirect(
                &base("http://localhost:5174"),
                Some(site),
                Outcome::Connected
            ),
            format!("http://localhost:5174/sites/{site}/connections?google=connected")
        );
        assert_eq!(
            app_redirect(
                &base("https://app.example.com/margincite"),
                Some(site),
                Outcome::Error(Reason::Expired)
            ),
            format!(
                "https://app.example.com/margincite/sites/{site}/connections?google=error&reason=expired"
            )
        );
        assert_eq!(
            app_redirect(
                &base("http://localhost:5174/"),
                None,
                Outcome::Error(Reason::InvalidState)
            ),
            "http://localhost:5174/?google=error&reason=invalid_state"
        );
    }

    #[test]
    fn every_reason_has_a_distinct_short_code() {
        let all = [
            Reason::InvalidState,
            Reason::Expired,
            Reason::Denied,
            Reason::MissingCode,
            Reason::TokenRejected,
            Reason::NoRefreshToken,
            Reason::GoogleUnavailable,
            Reason::NotConfigured,
            Reason::ServerError,
        ];
        let codes: std::collections::HashSet<_> = all.iter().map(|r| r.code()).collect();
        assert_eq!(codes.len(), all.len());
        assert!(codes.iter().all(|c| c.len() < 24 && !c.contains(' ')));
    }
}
