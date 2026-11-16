//! Everything a person sets up in the app: provider keys, Google sign-in,
//! DataForSEO, PageSpeed and the crawler's contact address. Secrets are sealed
//! and write-only. Every feature reads a setting through `resolve`: the
//! workspace's own value first, the environment second, a default last.

pub mod repo;
pub mod resolve;
pub mod routes;
pub mod test_provider;

/// Setting keys. A key ending in a secret's name is stored sealed.
pub mod keys {
    pub const GOOGLE_CLIENT_ID: &str = "google.client_id";
    pub const GOOGLE_CLIENT_SECRET: &str = "google.client_secret";
    pub const DATAFORSEO_LOGIN: &str = "dataforseo.login";
    pub const DATAFORSEO_PASSWORD: &str = "dataforseo.password";
    pub const PAGESPEED_KEY: &str = "pagespeed.api_key";
    pub const CRAWLER_CONTACT: &str = "crawler.contact";

    pub fn provider_key(name: &str) -> String {
        format!("provider.{name}.api_key")
    }

    pub fn provider_model(name: &str) -> String {
        format!("provider.{name}.model")
    }
}

/// Names in `provider_status.provider` for sources that are not AI providers.
pub const DATAFORSEO: &str = "dataforseo";
