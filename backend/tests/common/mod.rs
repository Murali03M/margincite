use std::net::SocketAddr;
use std::sync::Arc;

use margincite::ai::engines::EngineBases;
use margincite::config::Config;
use margincite::integrations::google::GoogleHosts;
use margincite::{service, AppState};
use sqlx::PgPool;

pub struct TestApp {
    pub addr: SocketAddr,
    pub client: reqwest::Client,
    /// Does not follow redirects, so a redirect can be inspected.
    pub no_redirect: reqwest::Client,
}

/// The sealing key tests use: 32 bytes of 7, base64.
#[allow(dead_code)]
pub const SECRET_KEY: &str = "BwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwc=";

/// Settings that turn Google sign-in on.
#[allow(dead_code)]
pub const GOOGLE_SETTINGS: &[(&str, &str)] = &[
    ("MARGINCITE_SECRET_KEY", SECRET_KEY),
    ("GOOGLE_CLIENT_ID", "test-client-id"),
    ("GOOGLE_CLIENT_SECRET", "test-client-secret"),
];

/// Keys for all five AI engines. They are fake and easy to search for.
#[allow(dead_code)]
pub const ENGINE_SETTINGS: &[(&str, &str)] = &[
    ("ANTHROPIC_API_KEY", "sk-ant-secret-KEY-111"),
    ("OPENAI_API_KEY", "sk-openai-secret-KEY-222"),
    ("PERPLEXITY_API_KEY", "pplx-secret-KEY-333"),
    ("GEMINI_API_KEY", "gem-secret-KEY-444"),
    ("XAI_API_KEY", "xai-secret-KEY-555"),
];

/// An address that nothing listens on (the discard port).
pub const DEAD_BASE: &str = "http://127.0.0.1:9";

/// Engine addresses that nothing listens on, so a test that forgets to mock
/// an engine fails instead of reaching a real service.
pub fn dead_engine_bases() -> EngineBases {
    let dead = DEAD_BASE.to_string();
    EngineBases {
        claude: dead.clone(),
        openai: dead.clone(),
        perplexity: dead.clone(),
        grok: dead.clone(),
        gemini: dead,
    }
}

/// The four engines as path prefixes on one mock server.
#[allow(dead_code)]
pub fn engine_bases(mock_root: &str) -> EngineBases {
    EngineBases {
        claude: format!("{mock_root}/claude"),
        openai: format!("{mock_root}/openai"),
        perplexity: format!("{mock_root}/perplexity"),
        grok: format!("{mock_root}/grok"),
        gemini: format!("{mock_root}/gemini"),
    }
}

/// The config a test app runs with, plus these extra settings.
pub fn test_config(extra: &[(&str, &str)]) -> Config {
    Config::from_env(|key| match key {
        _ if extra.iter().any(|(name, _)| *name == key) => extra
            .iter()
            .find(|(name, _)| *name == key)
            .map(|(_, value)| value.to_string()),
        "DATABASE_URL" => Some("postgres://127.0.0.1:5432/margincite_dev".into()),
        "CRAWLER_CONTACT" => Some("https://example.com/margincite-bot".into()),
        "DEV_API_TOKENS" => Some("test-token=ada@example.com,other-token=bea@example.com".into()),
        _ => None,
    })
    .expect("test config")
}

#[allow(dead_code)]
pub async fn spawn_app(pool: PgPool) -> TestApp {
    spawn_app_with(pool, &[]).await
}

/// Like `spawn_app`, with extra environment settings.
#[allow(dead_code)]
pub async fn spawn_app_with(pool: PgPool, extra: &[(&str, &str)]) -> TestApp {
    spawn(pool, extra, None, None, dead_engine_bases()).await
}

/// Like `spawn_app_with`, with Google's three hosts replaced.
#[allow(dead_code)]
pub async fn spawn_app_google(pool: PgPool, extra: &[(&str, &str)], hosts: GoogleHosts) -> TestApp {
    spawn(pool, extra, Some(hosts), None, dead_engine_bases()).await
}

/// Like `spawn_app_google`, with Bing Webmaster Tools' address replaced too.
#[allow(dead_code)]
pub async fn spawn_app_mocked(
    pool: PgPool,
    extra: &[(&str, &str)],
    hosts: GoogleHosts,
    bing_base: String,
) -> TestApp {
    // The mock server's root is the Bing base without its `/bing/` path; the
    // engines live under it too.
    let root = bing_base.trim_end_matches("/bing/").to_string();
    let engines = engine_bases(&root);
    spawn(pool, extra, Some(hosts), Some(bing_base), engines).await
}

async fn spawn(
    pool: PgPool,
    extra: &[(&str, &str)],
    hosts: Option<GoogleHosts>,
    bing_base: Option<String>,
    engines: EngineBases,
) -> TestApp {
    let config = Arc::new(test_config(extra));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind test server");
    let addr = listener.local_addr().expect("local addr");
    let mut state =
        AppState::new(pool, config).with_engines(&engines, std::time::Duration::from_secs(10));
    // Nothing listens here, so a test that forgets to mock a paid or
    // third-party service fails instead of reaching the real one.
    state = state.with_dataforseo(DEAD_BASE).with_pagespeed(DEAD_BASE);
    if let Some(hosts) = hosts {
        state = state.with_google(hosts);
    }
    if let Some(base) = bing_base {
        // DataForSEO lives beside Bing on the same mock server.
        let root = base.trim_end_matches("/bing/").to_string();
        state = state.with_dataforseo(&format!("{root}/dataforseo"));
        state = state.with_pagespeed(&format!("{root}/pagespeed"));
        state = state.with_bing(&base);
    }
    tokio::spawn(async move {
        axum::serve(listener, service(state))
            .await
            .expect("test server");
    });
    TestApp {
        addr,
        client: reqwest::Client::builder().build().expect("test client"),
        no_redirect: reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .expect("test client"),
    }
}

#[allow(dead_code)]
pub mod bing;
#[allow(dead_code)]
pub mod dataforseo;
#[allow(dead_code)]
pub mod google;

#[allow(dead_code)]
pub mod ai;

#[allow(dead_code)]
pub mod rankings;

#[allow(dead_code)]
pub mod settings;

#[allow(dead_code)]
pub mod speed;

#[allow(dead_code)]
pub mod suggestions;
