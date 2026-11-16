//! Ranking helpers on top of `GoogleWorld`: the same mock server answers
//! DataForSEO under `/dataforseo/`.

use std::sync::Arc;

use margincite::integrations::dataforseo::DataForSeoClient;
use margincite::jobs::queue::Job;
use margincite::rankings::run::RankRunJob;
use serde_json::{json, Value};
use uuid::Uuid;
use wiremock::matchers::{method, path};
use wiremock::{Match, Mock, MockServer, Request, ResponseTemplate};

use super::dataforseo as df;
use super::google::GoogleWorld;

pub const SERP_PATH: &str = "/dataforseo/v3/serp/google/organic/live/advanced";

pub fn dataforseo_base(mock: &MockServer) -> String {
    format!("{}/dataforseo", mock.uri())
}

/// Matches a SERP request for one keyword.
pub struct ForKeyword(pub String);

impl Match for ForKeyword {
    fn matches(&self, request: &Request) -> bool {
        serde_json::from_slice::<Value>(&request.body)
            .ok()
            .and_then(|body| body[0]["keyword"].as_str().map(str::to_string))
            .is_some_and(|keyword| keyword == self.0)
    }
}

impl GoogleWorld {
    pub fn rank_path(&self, tail: &str) -> String {
        format!("/api/v1/sites/{}/rankings{tail}", self.site_id)
    }

    /// Saves the test login for the workspace (the account test is answered).
    pub async fn save_dataforseo(&self) {
        Mock::given(method("GET"))
            .and(path("/dataforseo/v3/appendix/user_data"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(df::answer(json!([{ "money": { "balance": 50.0 } }]))),
            )
            .mount(&self.mock)
            .await;
        let (status, _) = self
            .send(
                "test-token",
                reqwest::Method::PUT,
                "/api/v1/settings/dataforseo",
                Some(json!({ "login": df::LOGIN, "password": df::PASSWORD })),
            )
            .await;
        assert_eq!(status, 200);
    }

    /// DataForSEO answers `body` with `status` for this keyword.
    pub async fn mock_serp(&self, keyword: &str, status: u16, body: Value) {
        Mock::given(method("POST"))
            .and(path(SERP_PATH))
            .and(ForKeyword(keyword.to_string()))
            .respond_with(ResponseTemplate::new(status).set_body_json(body))
            .mount(&self.mock)
            .await;
    }

    /// A SERP answer with these items for every keyword not mocked on its own.
    pub async fn mock_serp_for_all(&self, items: Value) {
        Mock::given(method("POST"))
            .and(path(SERP_PATH))
            .respond_with(ResponseTemplate::new(200).set_body_json(df::serp(items)))
            .mount(&self.mock)
            .await;
    }

    /// The tasks DataForSEO was sent for SERP results, in order.
    pub async fn serp_tasks(&self) -> Vec<Value> {
        self.requests_to(SERP_PATH)
            .await
            .iter()
            .map(|request| serde_json::from_slice::<Value>(&request.body).unwrap()[0].clone())
            .collect()
    }

    /// Every request of any kind that reached DataForSEO's part of the mock.
    pub async fn dataforseo_requests(&self) -> usize {
        self.mock
            .received_requests()
            .await
            .unwrap()
            .iter()
            .filter(|request| request.url.path().starts_with("/dataforseo/"))
            .count()
    }

    pub fn rank_handler(&self) -> RankRunJob {
        let config = Arc::new(super::test_config(super::GOOGLE_SETTINGS));
        let client = Arc::new(DataForSeoClient::new(&dataforseo_base(&self.mock)));
        RankRunJob::new(self.pool.clone(), config, client)
    }

    pub fn rank_job(&self, run_id: Option<Uuid>) -> Job {
        let mut payload = json!({ "workspace_id": self.workspace_id, "site_id": self.site_id });
        if let Some(id) = run_id {
            payload["run_id"] = json!(id);
        }
        Job {
            id: Uuid::new_v4(),
            kind: "rank_run".into(),
            payload,
            attempts: 1,
        }
    }

    /// Asks for a manual check over the API, then runs its job.
    pub async fn run_ranks(&self) -> Uuid {
        use margincite::jobs::worker::JobHandler;
        let (status, body) = self.post(&self.rank_path("/check")).await;
        assert_eq!(status, 202, "{body}");
        let id = self.latest_run_id().await;
        self.rank_handler()
            .run(&self.rank_job(Some(id)))
            .await
            .unwrap();
        id
    }

    pub async fn latest_run_id(&self) -> Uuid {
        sqlx::query_scalar(
            "SELECT id FROM rank_runs WHERE site_id = $1 ORDER BY created_at DESC LIMIT 1",
        )
        .bind(self.site_id)
        .fetch_one(&self.pool)
        .await
        .unwrap()
    }

    /// The run's status and error text.
    pub async fn run_state(&self, id: Uuid) -> (String, Option<String>) {
        sqlx::query_as("SELECT status, error FROM rank_runs WHERE id = $1")
            .bind(id)
            .fetch_one(&self.pool)
            .await
            .unwrap()
    }

    pub async fn rankings(&self) -> Value {
        let (status, body) = self.get(&self.rank_path("")).await;
        assert_eq!(status, 200, "{body}");
        body["data"].clone()
    }

    /// The ranking row for one keyword.
    pub async fn ranking_of(&self, keyword: &str) -> Value {
        self.rankings().await["keywords"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["keyword"] == keyword)
            .cloned()
            .unwrap_or_else(|| panic!("no row for {keyword}"))
    }

    pub async fn set_site(&self, body: Value) {
        let (status, reply) = self
            .send(
                "test-token",
                reqwest::Method::PUT,
                &format!("/api/v1/sites/{}/settings", self.site_id),
                Some(body),
            )
            .await;
        assert_eq!(status, 200, "{reply}");
    }

    pub async fn queued_rank_jobs(&self) -> i64 {
        sqlx::query_scalar(
            "SELECT COUNT(*) FROM jobs WHERE kind = 'rank_run' AND status = 'queued'",
        )
        .fetch_one(&self.pool)
        .await
        .unwrap()
    }

    pub async fn usage_rows(&self) -> Vec<(i64, Option<f64>)> {
        sqlx::query_as(
            "SELECT calls::bigint, estimated_cost_usd FROM usage_events
             WHERE workspace_id = $1 AND source = 'dataforseo' ORDER BY created_at",
        )
        .bind(self.workspace_id)
        .fetch_all(&self.pool)
        .await
        .unwrap()
    }
}

/// An AI Overview item citing these (domain, url) pairs in one element.
pub fn overview(references: &[(&str, &str)]) -> Value {
    let refs: Vec<Value> = references
        .iter()
        .map(|(domain, url)| {
            json!({ "type": "ai_overview_reference", "domain": domain, "url": url,
                    "title": "t", "text": "x", "source": "s" })
        })
        .collect();
    json!({ "type": "ai_overview", "rank_absolute": 1, "asynchronous_ai_overview": false,
            "items": [{ "type": "ai_overview_element", "text": "An answer.", "references": refs }],
            "references": null })
}

impl GoogleWorld {
    /// A stored result, as a finished run would have left it.
    pub async fn seed_check(
        &self,
        keyword: &str,
        position: Option<i32>,
        ai: &str,
        cited: &[&str],
        days_ago: i32,
    ) {
        sqlx::query(
            "INSERT INTO rank_checks (id, workspace_id, site_id, keyword_id, keyword, country,
                 language, device, position, url, ai_overview, ai_cited_domains, checked_at)
             VALUES (gen_random_uuid(), $1, $2, gen_random_uuid(), $3, 'US', 'en', 'desktop', $4,
                 CASE WHEN $4::int IS NULL THEN NULL ELSE 'https://example.com/a' END, $5, $6,
                 now() - make_interval(days => $7))",
        )
        .bind(self.workspace_id)
        .bind(self.site_id)
        .bind(keyword)
        .bind(position)
        .bind(ai)
        .bind(json!(cited))
        .bind(days_ago)
        .execute(&self.pool)
        .await
        .unwrap();
    }

    pub async fn metrics_block(&self, name: &str) -> Value {
        let (status, body) = self
            .get(&format!("/api/v1/sites/{}/metrics", self.site_id))
            .await;
        assert_eq!(status, 200, "{body}");
        body["data"][name].clone()
    }
}
