//! Bing Webmaster Tools helpers on top of `GoogleWorld`: the same mock server
//! answers Bing under `/bing/`.

use std::sync::Arc;

use margincite::integrations::bing::sync::BingSyncJob;
use margincite::integrations::bing::BingClient;
use margincite::jobs::queue::Job;
use margincite::jobs::worker::JobHandler;
use serde_json::{json, Value};
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, ResponseTemplate};

use super::google::{bing_base, secrets, GoogleWorld};

pub const BING_KEY: &str = "bing-api-key-secret-123";
pub const SITE_URL: &str = "https://example.com/";

impl GoogleWorld {
    pub fn bing_path(&self, tail: &str) -> String {
        format!("/api/v1/sites/{}/integrations/bing{tail}", self.site_id)
    }

    pub async fn put_bing(&self, api_key: &str, site_url: &str) -> (u16, Value) {
        self.send(
            "test-token",
            reqwest::Method::PUT,
            &self.bing_path(""),
            Some(json!({ "api_key": api_key, "site_url": site_url })),
        )
        .await
    }

    /// Bing lists these sites for the key.
    pub async fn mock_bing_sites(&self, sites: &[&str]) {
        let d: Vec<Value> = sites
            .iter()
            .map(|url| json!({ "Url": url, "IsVerified": true }))
            .collect();
        self.mock_bing("GetUserSites", &[], 200, json!({ "d": d }))
            .await;
    }

    /// One page of link counts, as `(target, count)` rows.
    pub async fn mock_bing_counts(&self, page: u32, rows: &[(&str, i64)]) {
        let links: Vec<Value> = rows
            .iter()
            .map(|(url, count)| json!({ "Url": url, "Count": count }))
            .collect();
        let body = json!({ "d": { "__type": "LinkCounts:#Microsoft.Bing.Webmaster.Api", "Links": links, "TotalPages": 2 } });
        self.mock_bing(
            "GetLinkCounts",
            &[("page", &page.to_string()), ("siteUrl", SITE_URL)],
            200,
            body,
        )
        .await;
    }

    /// The first page of links pointing at `target`, as `(source, anchor)` rows.
    pub async fn mock_bing_sources(&self, target: &str, rows: &[(&str, &str)]) {
        let details: Vec<Value> = rows
            .iter()
            .map(|(url, anchor)| json!({ "Url": url, "AnchorText": anchor }))
            .collect();
        let body = json!({ "d": { "Details": details, "TotalPages": 1 } });
        self.mock_bing(
            "GetUrlLinks",
            &[("link", target), ("page", "0"), ("siteUrl", SITE_URL)],
            200,
            body,
        )
        .await;
    }

    pub async fn mock_bing(
        &self,
        method_name: &str,
        params: &[(&str, &str)],
        status: u16,
        body: Value,
    ) {
        let mut mock = Mock::given(method("GET")).and(path(format!("/bing/{method_name}")));
        for (key, value) in params {
            mock = mock.and(query_param(*key, *value));
        }
        mock.respond_with(ResponseTemplate::new(status).set_body_json(body))
            .mount(&self.mock)
            .await;
    }

    /// A stored connection, as `PUT` would leave it.
    pub async fn insert_bing(&self, status: &str) {
        sqlx::query(
            "INSERT INTO bing_connections (site_id, workspace_id, api_key_sealed, site_url, status)
             VALUES ($1, $2, $3, $4, $5)",
        )
        .bind(self.site_id)
        .bind(self.workspace_id)
        .bind(secrets().seal(BING_KEY))
        .bind(SITE_URL)
        .bind(status)
        .execute(&self.pool)
        .await
        .unwrap();
    }

    pub fn bing_job(&self) -> Job {
        Job {
            id: uuid::Uuid::new_v4(),
            kind: "bing_sync".into(),
            payload: json!({ "workspace_id": self.workspace_id, "site_id": self.site_id }),
            attempts: 1,
        }
    }

    pub fn bing_handler(&self) -> BingSyncJob {
        let config = Arc::new(super::test_config(super::GOOGLE_SETTINGS));
        let client = Arc::new(BingClient::new(&bing_base(&self.mock)));
        BingSyncJob::new(self.pool.clone(), config, client)
    }

    pub async fn run_bing_sync(&self) -> anyhow::Result<()> {
        self.bing_handler().run(&self.bing_job()).await
    }

    pub async fn bing_count(&self, table: &str) -> i64 {
        let sql = match table {
            "bing_connections" => "SELECT COUNT(*) FROM bing_connections",
            "backlink_targets" => "SELECT COUNT(*) FROM backlink_targets",
            "backlink_sources" => "SELECT COUNT(*) FROM backlink_sources",
            "jobs" => "SELECT COUNT(*) FROM jobs WHERE kind = 'bing_sync'",
            other => panic!("unknown table {other}"),
        };
        sqlx::query_scalar(sql).fetch_one(&self.pool).await.unwrap()
    }

    pub async fn bing_status(&self) -> Option<String> {
        sqlx::query_scalar("SELECT status FROM bing_connections WHERE site_id = $1")
            .bind(self.site_id)
            .fetch_optional(&self.pool)
            .await
            .unwrap()
    }

    /// Every request Bing's mock has seen.
    pub async fn bing_requests(&self) -> Vec<wiremock::Request> {
        self.mock
            .received_requests()
            .await
            .unwrap()
            .into_iter()
            .filter(|r| r.url.path().starts_with("/bing/"))
            .collect()
    }
}
