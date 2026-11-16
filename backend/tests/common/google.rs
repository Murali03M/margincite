//! A running app wired to a mock Google, with one workspace and one site.

use margincite::crawl::repo::{self, Finish, NewPage};
use margincite::integrations::google::{hash_nonce, GoogleHosts};
use margincite::secrets::Secrets;
use serde_json::{json, Value};
use sqlx::PgPool;
use uuid::Uuid;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::{spawn_app_mocked, TestApp, GOOGLE_SETTINGS, SECRET_KEY};

pub const REFRESH_TOKEN: &str = "refresh-token-value-123";
pub const ACCESS_TOKEN: &str = "access-token-value-456";

pub struct GoogleWorld {
    pub app: TestApp,
    pub pool: PgPool,
    pub mock: MockServer,
    pub workspace_id: Uuid,
    pub site_id: Uuid,
}

/// The nonce the browser in tests holds; `new_state` stores its hash.
pub const NONCE: &str = "browser-nonce-0123456789abcdefghijklmnopqrstuvwxyz";
pub const OAUTH_COOKIE: &str = "margincite_oauth";

pub fn location_of(response: &reqwest::Response) -> String {
    response
        .headers()
        .get("location")
        .map(|v| v.to_str().unwrap().to_string())
        .unwrap_or_default()
}

pub fn hosts(mock: &MockServer) -> GoogleHosts {
    GoogleHosts {
        auth_url: format!("{}/auth", mock.uri()),
        token_url: format!("{}/token", mock.uri()),
        api_base: format!("{}/webmasters/v3", mock.uri()),
    }
}

/// Bing Webmaster Tools' address in tests: a path on the same mock server.
pub fn bing_base(mock: &MockServer) -> String {
    format!("{}/bing/", mock.uri())
}

pub fn secrets() -> Secrets {
    Secrets::from_base64(SECRET_KEY).unwrap()
}

impl GoogleWorld {
    pub async fn new(pool: PgPool) -> GoogleWorld {
        Self::with_settings(pool, GOOGLE_SETTINGS).await
    }

    pub async fn with_settings(pool: PgPool, settings: &[(&str, &str)]) -> GoogleWorld {
        let mock = MockServer::start().await;
        let app = spawn_app_mocked(pool.clone(), settings, hosts(&mock), bing_base(&mock)).await;
        let workspace_id = workspace_of(&app, "test-token").await;
        let site_id = add_site(&pool, workspace_id, "https://example.com/").await;
        GoogleWorld {
            app,
            pool,
            mock,
            workspace_id,
            site_id,
        }
    }

    pub fn url(&self, path: &str) -> String {
        format!("http://{}{path}", self.app.addr)
    }

    pub async fn send(
        &self,
        token: &str,
        method: reqwest::Method,
        path: &str,
        body: Option<Value>,
    ) -> (u16, Value) {
        let mut request = self
            .app
            .client
            .request(method, self.url(path))
            .header("authorization", format!("Bearer {token}"));
        if let Some(body) = body {
            request = request.json(&body);
        }
        let response = request.send().await.unwrap();
        let status = response.status().as_u16();
        (status, response.json().await.unwrap_or(Value::Null))
    }

    pub async fn get(&self, path: &str) -> (u16, Value) {
        self.send("test-token", reqwest::Method::GET, path, None)
            .await
    }

    pub async fn post(&self, path: &str) -> (u16, Value) {
        self.send("test-token", reqwest::Method::POST, path, None)
            .await
    }

    pub fn google_path(&self, tail: &str) -> String {
        format!("/api/v1/sites/{}/integrations/google{tail}", self.site_id)
    }

    /// The callback as the browser makes it that started the sign-in: it
    /// carries the nonce cookie. Redirects are not followed.
    pub async fn callback(&self, query: &str) -> (u16, String) {
        let cookie = format!("{OAUTH_COOKIE}={NONCE}");
        let response = self.callback_response(query, Some(&cookie)).await;
        let location = location_of(&response);
        (response.status().as_u16(), location)
    }

    /// The callback with exactly this `Cookie` header, or none.
    pub async fn callback_response(&self, query: &str, cookie: Option<&str>) -> reqwest::Response {
        let mut request = self
            .app
            .no_redirect
            .get(self.url(&format!("/api/v1/oauth/google/callback?{query}")));
        if let Some(cookie) = cookie {
            request = request.header("cookie", cookie);
        }
        request.send().await.unwrap()
    }

    pub fn connections_url(&self, tail: &str) -> String {
        format!(
            "http://localhost:5174/sites/{}/connections?{tail}",
            self.site_id
        )
    }

    /// A pending sign-in for this site, as `start` would have made.
    pub async fn new_state(&self) -> Uuid {
        let id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO oauth_states (id, workspace_id, site_id, provider, nonce_hash)
             VALUES ($1, $2, $3, 'google', $4)",
        )
        .bind(id)
        .bind(self.workspace_id)
        .bind(self.site_id)
        .bind(hash_nonce(NONCE))
        .execute(&self.pool)
        .await
        .unwrap();
        id
    }

    /// Google answers the code exchange and the refresh with these tokens.
    pub async fn mock_tokens(&self) {
        Mock::given(method("POST"))
            .and(path("/token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "access_token": ACCESS_TOKEN,
                "refresh_token": REFRESH_TOKEN,
                "expires_in": 3599,
                "token_type": "Bearer"
            })))
            .mount(&self.mock)
            .await;
    }

    pub async fn token_calls(&self) -> usize {
        self.requests_to("/token").await.len()
    }

    pub async fn requests_to(&self, request_path: &str) -> Vec<wiremock::Request> {
        self.mock
            .received_requests()
            .await
            .unwrap()
            .into_iter()
            .filter(|r| r.url.path() == request_path)
            .collect()
    }

    /// A connected connection with a chosen property, token sealed as the app does.
    pub async fn connect(&self, property: &str) {
        self.insert_connection(Some(property), "connected").await;
    }

    pub async fn insert_connection(&self, property: Option<&str>, status: &str) {
        sqlx::query(
            "INSERT INTO google_connections (site_id, workspace_id, refresh_token_sealed, property, status)
             VALUES ($1, $2, $3, $4, $5)
             ON CONFLICT (site_id) DO UPDATE SET property = $4, status = $5",
        )
        .bind(self.site_id)
        .bind(self.workspace_id)
        .bind(secrets().seal(REFRESH_TOKEN))
        .bind(property)
        .bind(status)
        .execute(&self.pool)
        .await
        .unwrap();
    }

    /// Search rows as a sync would have stored them: (query, page, clicks, impressions, position).
    pub async fn seed_rows(&self, rows: &[(&str, &str, i64, i64, f64)]) {
        for (query, page, clicks, impressions, position) in rows {
            sqlx::query(
                "INSERT INTO search_rows (workspace_id, site_id, query, page, clicks, impressions, ctr, position)
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
            )
            .bind(self.workspace_id)
            .bind(self.site_id)
            .bind(query)
            .bind(page)
            .bind(clicks)
            .bind(impressions)
            .bind(*clicks as f64 / (*impressions).max(1) as f64)
            .bind(position)
            .execute(&self.pool)
            .await
            .unwrap();
        }
    }

    pub async fn count(&self, table: &str) -> i64 {
        let sql = match table {
            "oauth_states" => "SELECT COUNT(*) FROM oauth_states",
            "google_connections" => "SELECT COUNT(*) FROM google_connections",
            "search_rows" => "SELECT COUNT(*) FROM search_rows",
            "search_daily" => "SELECT COUNT(*) FROM search_daily",
            "search_previous" => "SELECT COUNT(*) FROM search_previous",
            "jobs" => "SELECT COUNT(*) FROM jobs WHERE kind = 'google_sync'",
            other => panic!("unknown table {other}"),
        };
        sqlx::query_scalar(sql).fetch_one(&self.pool).await.unwrap()
    }

    pub async fn connection_status(&self) -> Option<String> {
        sqlx::query_scalar("SELECT status FROM google_connections WHERE site_id = $1")
            .bind(self.site_id)
            .fetch_optional(&self.pool)
            .await
            .unwrap()
    }

    /// A completed crawl with these `(url, in_sitemap)` working pages.
    pub async fn crawl(&self, pages: &[(&str, bool)]) -> Uuid {
        let (pool, ws) = (&self.pool, self.workspace_id);
        let crawl_id = repo::create_crawl(pool, ws, self.site_id, "manual")
            .await
            .unwrap();
        for (url, in_sitemap) in pages {
            let page = NewPage {
                url: url.to_string(),
                depth: if *url == "https://example.com/" { 0 } else { 1 },
                in_sitemap: *in_sitemap,
                status_code: Some(200),
                content_type: Some("text/html".into()),
                fetch_error: None,
                redirects: json!([]),
                response_ms: Some(100),
                bytes: Some(1000),
                is_https: true,
                x_robots_tag: None,
                needed_javascript: false,
                facts: Some(facts()),
                text: Some("some words about widgets and little else ".repeat(10)),
            };
            repo::upsert_page(pool, ws, crawl_id, &page).await.unwrap();
        }
        let done = Finish {
            status: "completed".into(),
            stop_reason: "finished".into(),
            pages_crawled: pages.len() as i32,
            pages_failed: 0,
            error: None,
        };
        repo::finish(pool, ws, crawl_id, &done).await.unwrap();
        margincite::checks::store::run_for_crawl(pool, ws, crawl_id)
            .await
            .unwrap();
        crawl_id
    }

    pub async fn add_keyword(&self, keyword: &str) {
        sqlx::query(
            "INSERT INTO site_keywords (id, workspace_id, site_id, keyword) VALUES ($1, $2, $3, $4)",
        )
        .bind(Uuid::new_v4())
        .bind(self.workspace_id)
        .bind(self.site_id)
        .bind(keyword)
        .execute(&self.pool)
        .await
        .unwrap();
    }

    /// Google's answers for the three reports a sync asks for.
    pub async fn mock_reports(&self, rows: Value, daily: Value, previous_queries: Value) {
        let windows =
            margincite::integrations::google::sync::windows(time::OffsetDateTime::now_utc().date());
        let fmt = time::macros::format_description!("[year]-[month]-[day]");
        let previous_start = windows.previous.0.format(&fmt).unwrap();
        for (dims, start, body) in [
            (vec!["query", "page"], None, rows),
            (vec!["date"], None, daily),
            (vec!["query"], Some(previous_start), previous_queries),
        ] {
            Mock::given(method("POST"))
                .and(Report { dims, start })
                .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "rows": body })))
                .mount(&self.mock)
                .await;
        }
    }

    pub async fn report_requests(&self) -> Vec<Value> {
        self.mock
            .received_requests()
            .await
            .unwrap()
            .into_iter()
            .filter(|r| r.url.path().ends_with("/searchAnalytics/query"))
            .map(|r| serde_json::from_slice(&r.body).unwrap())
            .collect()
    }
}

/// Matches a Search Console report by its exact dimensions and, optionally, start date.
struct Report {
    dims: Vec<&'static str>,
    start: Option<String>,
}

impl wiremock::Match for Report {
    fn matches(&self, request: &wiremock::Request) -> bool {
        let Ok(body) = serde_json::from_slice::<Value>(&request.body) else {
            return false;
        };
        let dims: Vec<&str> = body["dimensions"]
            .as_array()
            .map(|a| a.iter().filter_map(|d| d.as_str()).collect())
            .unwrap_or_default();
        request.url.path().ends_with("/searchAnalytics/query")
            && dims == self.dims
            && self
                .start
                .as_deref()
                .is_none_or(|start| body["startDate"] == start)
    }
}

pub fn facts() -> Value {
    json!({
        "title": "Widgets", "meta_description": "About widgets and more things", "h1s": ["Widgets"],
        "headings": [], "canonical": null, "robots_meta": null, "has_viewport": true,
        "word_count": 80, "structured_data": [], "structured_data_errors": 0, "links": [],
        "images": [], "has_scripts": false, "published": null, "modified": null, "author": null
    })
}

pub async fn workspace_of(app: &TestApp, token: &str) -> Uuid {
    let me: Value = app
        .client
        .get(format!("http://{}/api/v1/me", app.addr))
        .header("authorization", format!("Bearer {token}"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    me["data"]["workspace_id"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap()
}

pub async fn add_site(pool: &PgPool, workspace_id: Uuid, root: &str) -> Uuid {
    let site_id = Uuid::new_v4();
    let host = root
        .trim_start_matches("https://")
        .trim_end_matches('/')
        .to_string();
    sqlx::query(
        "INSERT INTO sites (id, workspace_id, root_url, host, path_prefix, display_name)
         VALUES ($1, $2, $3, $4, '/', $4)",
    )
    .bind(site_id)
    .bind(workspace_id)
    .bind(root)
    .bind(host)
    .execute(pool)
    .await
    .unwrap();
    site_id
}
