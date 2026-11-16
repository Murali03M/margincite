//! Page speed helpers on top of `GoogleWorld`: the same mock server answers
//! PageSpeed Insights under `/pagespeed/`.

use std::sync::Arc;

use margincite::integrations::pagespeed::PageSpeedClient;
use margincite::jobs::queue::Job;
use margincite::speed::run::SpeedRunJob;
use serde_json::{json, Value};
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::google::GoogleWorld;

pub const PSI_KEY: &str = "psi-key-secret-4242";

pub fn pagespeed_base(mock: &MockServer) -> String {
    format!("{}/pagespeed", mock.uri())
}

/// A lab-only PageSpeed answer.
pub fn lab(score: f64, lcp: f64, cls: f64, tbt: f64) -> Value {
    json!({ "lighthouseResult": {
        "categories": { "performance": { "score": score } },
        "audits": {
            "largest-contentful-paint": { "numericValue": lcp },
            "cumulative-layout-shift": { "numericValue": cls },
            "total-blocking-time": { "numericValue": tbt }
        }
    }})
}

impl GoogleWorld {
    pub fn speed_path(&self, tail: &str) -> String {
        format!("/api/v1/sites/{}/speed{tail}", self.site_id)
    }

    /// PageSpeed answers `body` with `status` for this page.
    pub async fn mock_speed(&self, page: &str, status: u16, body: Value) {
        Mock::given(method("GET"))
            .and(path("/pagespeed/runPagespeed"))
            .and(query_param("url", page))
            .respond_with(ResponseTemplate::new(status).set_body_json(body))
            .mount(&self.mock)
            .await;
    }

    /// Every other page gets this answer.
    pub async fn mock_speed_for_all(&self, body: Value) {
        Mock::given(method("GET"))
            .and(path("/pagespeed/runPagespeed"))
            .respond_with(ResponseTemplate::new(200).set_body_json(body))
            .mount(&self.mock)
            .await;
    }

    /// The pages PageSpeed was asked about, in order, and whether a key came.
    pub async fn speed_requests(&self) -> Vec<(String, bool)> {
        self.requests_to("/pagespeed/runPagespeed")
            .await
            .iter()
            .map(|request| {
                let pairs: Vec<(String, String)> = request
                    .url
                    .query_pairs()
                    .map(|(k, v)| (k.to_string(), v.to_string()))
                    .collect();
                let url = pairs.iter().find(|(k, _)| k == "url").unwrap().1.clone();
                (url, pairs.iter().any(|(k, _)| k == "key"))
            })
            .collect()
    }

    pub fn speed_job(&self) -> Job {
        Job {
            id: uuid::Uuid::new_v4(),
            kind: "speed_run".into(),
            payload: json!({ "workspace_id": self.workspace_id, "site_id": self.site_id }),
            attempts: 1,
        }
    }

    pub fn speed_handler(&self) -> SpeedRunJob {
        let config = Arc::new(super::test_config(super::GOOGLE_SETTINGS));
        let client = Arc::new(PageSpeedClient::new(&pagespeed_base(&self.mock)));
        SpeedRunJob::new(self.pool.clone(), config, client)
    }
}
