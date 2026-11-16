use std::sync::Arc;

use sqlx::PgPool;

pub mod ai;
pub mod auth;
pub mod body;
pub mod checks;
pub mod config;
pub mod crawl;
pub mod envelope;
pub mod error;
mod http;
pub mod integrations;
pub mod jobs;
pub mod keywords;
pub mod metrics;
pub mod rankings;
pub mod request;
pub mod secrets;
pub mod settings;
pub mod sites;
pub mod speed;
pub mod suggestions;
pub mod usage;
pub mod workspaces;

use ai::engines::{EngineBases, EngineFactory};
use auth::{Authenticator, DevTokenAuthenticator};
use config::Config;
use integrations::bing::BingClient;
use integrations::dataforseo::DataForSeoClient;
use integrations::google::{GoogleClient, GoogleHosts};
use integrations::pagespeed::PageSpeedClient;

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub config: Arc<Config>,
    /// Talks to Google. Tests swap in one that points at a mock.
    pub google: Arc<GoogleClient>,
    /// Talks to Bing Webmaster Tools. Tests swap in one that points at a mock.
    pub bing: Arc<BingClient>,
    /// Talks to PageSpeed Insights. Tests swap in one that points at a mock.
    pub pagespeed: Arc<PageSpeedClient>,
    /// Builds each workspace's AI engines. Tests swap in one that points at a mock.
    pub engines: Arc<EngineFactory>,
    /// DataForSEO's address, for the credential test. Tests swap in a mock.
    pub dataforseo_base: String,
    /// Talks to DataForSEO for rankings and keyword data. Tests swap in one
    /// that points at a mock.
    pub dataforseo: Arc<DataForSeoClient>,
    pub(crate) auth: Arc<dyn Authenticator>,
    /// Counts every request by the caller's address.
    pub(crate) limiter: Arc<http::RateLimiter>,
    /// Counts signed-in requests by workspace.
    pub(crate) workspace_limiter: Arc<http::RateLimiter>,
}

impl AppState {
    pub fn new(pool: PgPool, config: Arc<Config>) -> Self {
        let auth = Arc::new(DevTokenAuthenticator::new(
            pool.clone(),
            config.dev_tokens.clone(),
        ));
        let engines = Arc::new(EngineFactory::production());
        Self {
            pool,
            config,
            google: Arc::new(GoogleClient::new(GoogleHosts::production())),
            bing: Arc::new(BingClient::production()),
            pagespeed: Arc::new(PageSpeedClient::production()),
            engines,
            dataforseo_base: settings::test_provider::DATAFORSEO_BASE.to_string(),
            dataforseo: Arc::new(DataForSeoClient::production()),
            auth,
            limiter: Arc::new(http::RateLimiter::new(120)),
            workspace_limiter: Arc::new(http::RateLimiter::new(120)),
        }
    }
}

impl AppState {
    /// The same state talking to different Google hosts. For tests.
    pub fn with_google(mut self, hosts: GoogleHosts) -> Self {
        self.google = Arc::new(GoogleClient::new(hosts));
        self
    }
}

impl AppState {
    /// The same state talking to a different Bing address. For tests.
    pub fn with_bing(mut self, base_url: &str) -> Self {
        self.bing = Arc::new(BingClient::new(base_url));
        self
    }
}

impl AppState {
    /// The same state talking to a different PageSpeed address. For tests.
    pub fn with_pagespeed(mut self, base_url: &str) -> Self {
        self.pagespeed = Arc::new(PageSpeedClient::new(base_url));
        self
    }
}

impl AppState {
    /// The same state asking AI engines at different addresses, with a
    /// different timeout. For tests.
    pub fn with_engines(mut self, bases: &EngineBases, timeout: std::time::Duration) -> Self {
        self.engines = Arc::new(EngineFactory::new(bases.clone(), timeout));
        self
    }

    /// The same state talking to a different DataForSEO address. For tests.
    pub fn with_dataforseo(mut self, base_url: &str) -> Self {
        self.dataforseo_base = base_url.to_string();
        self.dataforseo = Arc::new(DataForSeoClient::new(base_url));
        self
    }
}

pub fn app(state: AppState) -> axum::Router {
    http::router(state)
}

/// The app as a service that tells each request its caller's address, which
/// the rate limiter keys on. Serve this, not `app`, outside of tests.
pub fn service(
    state: AppState,
) -> axum::extract::connect_info::IntoMakeServiceWithConnectInfo<axum::Router, std::net::SocketAddr>
{
    app(state).into_make_service_with_connect_info::<std::net::SocketAddr>()
}
