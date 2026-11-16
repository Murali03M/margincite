//! A failing PageSpeed request made with a key leaves the key in no stored
//! error, no response and no log line. The subscriber is installed for the
//! whole process, so this lives in a test binary of its own.

mod common;

use std::io::Write;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use common::google::GoogleWorld;
use common::speed::{lab, PSI_KEY};
use margincite::integrations::pagespeed::PageSpeedClient;
use margincite::jobs::worker::JobHandler;
use margincite::speed::run::SpeedRunJob;
use serde_json::json;
use sqlx::PgPool;
use wiremock::matchers::{method, path};
use wiremock::{Mock, ResponseTemplate};

fn sink() -> &'static Arc<Mutex<Vec<u8>>> {
    static SINK: OnceLock<Arc<Mutex<Vec<u8>>>> = OnceLock::new();
    SINK.get_or_init(|| Arc::new(Mutex::new(Vec::new())))
}

struct Capture;

impl Write for Capture {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        sink().lock().unwrap().extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

fn install() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        tracing_subscriber::fmt()
            .with_max_level(tracing::Level::TRACE)
            .with_writer(|| Capture)
            .init();
    });
}

fn logged() -> String {
    String::from_utf8_lossy(&sink().lock().unwrap()).into_owned()
}

async fn run_with(w: &GoogleWorld, client: PageSpeedClient) {
    let config = Arc::new(common::test_config(common::GOOGLE_SETTINGS));
    let job = SpeedRunJob::new(w.pool.clone(), config, Arc::new(client));
    job.run(&w.speed_job()).await.unwrap();
}

async fn stored(w: &GoogleWorld) -> String {
    let errors: Vec<Option<String>> =
        sqlx::query_scalar("SELECT error FROM speed_results WHERE site_id = $1")
            .bind(w.site_id)
            .fetch_all(&w.pool)
            .await
            .unwrap();
    format!("{errors:?}")
}

async fn assert_clean(w: &GoogleWorld, what: &str) {
    let (_, body) = w.get(&w.speed_path("")).await;
    for (place, text) in [
        ("stored", stored(w).await),
        ("response", body.to_string()),
        ("log", logged()),
    ] {
        assert!(!text.contains(PSI_KEY), "{what}: key in {place}: {text}");
        assert!(!text.contains("key="), "{what}: key= in {place}: {text}");
    }
    assert!(body["data"]["pages"][0]["error"].is_string(), "{what}");
}

#[sqlx::test]
async fn failed_requests_made_with_a_key_leak_it_nowhere(pool: PgPool) {
    install();
    let w = GoogleWorld::new(pool).await;
    let (status, _) = w
        .send(
            "test-token",
            reqwest::Method::PUT,
            "/api/v1/settings/pagespeed",
            Some(json!({ "api_key": PSI_KEY })),
        )
        .await;
    assert_eq!(status, 200);

    // Connection refused.
    run_with(&w, PageSpeedClient::new("http://127.0.0.1:9")).await;
    assert_clean(&w, "refused").await;

    // Timeout.
    Mock::given(method("GET"))
        .and(path("/pagespeed/runPagespeed"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(lab(0.5, 1.0, 0.0, 1.0))
                .set_delay(Duration::from_millis(800)),
        )
        .mount(&w.mock)
        .await;
    let base = common::speed::pagespeed_base(&w.mock);
    run_with(
        &w,
        PageSpeedClient::with_timeout(&base, Duration::from_millis(100)),
    )
    .await;
    assert_clean(&w, "timeout").await;

    // 500 and 429 that echo the request back.
    for status in [500, 429, 403, 400] {
        w.mock.reset().await;
        Mock::given(method("GET"))
            .and(path("/pagespeed/runPagespeed"))
            .respond_with(
                ResponseTemplate::new(status)
                    .set_body_string(format!("bad request key={PSI_KEY} {PSI_KEY}")),
            )
            .mount(&w.mock)
            .await;
        run_with(&w, PageSpeedClient::new(&base)).await;
        assert_clean(&w, &status.to_string()).await;
    }
}
