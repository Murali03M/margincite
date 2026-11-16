//! A failing DataForSEO request made with the workspace's login leaves the
//! login, the password and the encoded `Authorization` value in no stored
//! error, no response and no log line. The subscriber is installed for the
//! whole process, so this lives in a test binary of its own.

mod common;

use std::io::Write;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use common::dataforseo::{self as df, LOGIN, PASSWORD};
use common::google::GoogleWorld;
use common::rankings::{dataforseo_base, SERP_PATH};
use margincite::integrations::dataforseo::DataForSeoClient;
use margincite::jobs::worker::JobHandler;
use margincite::rankings::run::RankRunJob;
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;
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

/// The login, the password and the header a log of the request would show.
fn secrets() -> Vec<String> {
    vec![LOGIN.to_string(), PASSWORD.to_string(), df::token()]
}

/// Makes a run and runs it with this client.
async fn run_with(w: &GoogleWorld, client: DataForSeoClient) {
    let run = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO rank_runs (id, workspace_id, site_id, status, trigger, keywords_total)
         VALUES ($1, $2, $3, 'queued', 'manual', 1)",
    )
    .bind(run)
    .bind(w.workspace_id)
    .bind(w.site_id)
    .execute(&w.pool)
    .await
    .unwrap();
    let config = Arc::new(common::test_config(common::GOOGLE_SETTINGS));
    let job = RankRunJob::new(w.pool.clone(), config, Arc::new(client));
    job.run(&w.rank_job(Some(run))).await.unwrap();
    sqlx::query("UPDATE rank_runs SET status = 'completed' WHERE status IN ('queued', 'running')")
        .execute(&w.pool)
        .await
        .unwrap();
}

async fn stored(w: &GoogleWorld) -> String {
    let runs: Vec<Option<String>> = sqlx::query_scalar("SELECT error FROM rank_runs")
        .fetch_all(&w.pool)
        .await
        .unwrap();
    let status: Vec<Option<String>> = sqlx::query_scalar("SELECT error FROM provider_status")
        .fetch_all(&w.pool)
        .await
        .unwrap();
    let usage: Vec<String> = sqlx::query_scalar("SELECT source || kind FROM usage_events")
        .fetch_all(&w.pool)
        .await
        .unwrap();
    format!("{runs:?} {status:?} {usage:?}")
}

async fn assert_clean(w: &GoogleWorld, what: &str) {
    let mut seen = vec![("stored", stored(w).await), ("log", logged())];
    for tail in ["", "/check"] {
        let (_, body) = w.get(&w.rank_path(tail)).await;
        seen.push(("rankings", body.to_string()));
    }
    let (_, settings) = w.get("/api/v1/settings").await;
    seen.push(("settings", settings.to_string()));
    let (_, usage) = w.get("/api/v1/usage?days=30").await;
    seen.push(("usage", usage.to_string()));
    for (place, text) in seen {
        for secret in secrets() {
            assert!(
                !text.contains(&secret),
                "{what}: {secret} in {place}: {text}"
            );
        }
        assert!(
            !text.contains("Basic "),
            "{what}: an Authorization value in {place}"
        );
    }
}

async fn echo(w: &GoogleWorld, status: u16, body: String) {
    w.mock.reset().await;
    Mock::given(method("POST"))
        .and(path(SERP_PATH))
        .respond_with(ResponseTemplate::new(status).set_body_string(body))
        .mount(&w.mock)
        .await;
}

#[sqlx::test]
async fn failed_requests_made_with_the_login_leak_it_nowhere(pool: PgPool) {
    install();
    let w = GoogleWorld::new(pool).await;
    // The login test answers with an error that echoes the credentials back.
    Mock::given(method("GET"))
        .and(path("/dataforseo/v3/appendix/user_data"))
        .respond_with(ResponseTemplate::new(401).set_body_string(format!("{LOGIN} {PASSWORD}")))
        .mount(&w.mock)
        .await;
    let (status, _) = w
        .send(
            "test-token",
            reqwest::Method::PUT,
            "/api/v1/settings/dataforseo",
            Some(json!({ "login": LOGIN, "password": PASSWORD })),
        )
        .await;
    assert_eq!(status, 200);
    w.add_keyword("blue widgets").await;
    assert_clean(&w, "saving").await;

    // Connection refused.
    run_with(&w, DataForSeoClient::new("http://127.0.0.1:9")).await;
    assert_clean(&w, "refused").await;

    // Timeout.
    Mock::given(method("POST"))
        .and(path(SERP_PATH))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(df::serp(json!([])))
                .set_delay(Duration::from_millis(800)),
        )
        .mount(&w.mock)
        .await;
    let base = dataforseo_base(&w.mock);
    run_with(
        &w,
        DataForSeoClient::with_timeout(&base, Duration::from_millis(100)),
    )
    .await;
    assert_clean(&w, "timeout").await;

    // Errors that echo the request, the login and the header back.
    let echoed = format!("{LOGIN} {PASSWORD} Basic {} authorization", df::token());
    for status in [401, 402, 403, 429, 500] {
        echo(&w, status, echoed.clone()).await;
        run_with(&w, DataForSeoClient::new(&base)).await;
        assert_clean(&w, &status.to_string()).await;
    }
    // HTTP 200 whose task message echoes them.
    for code in [40100, 40200, 40202, 50000] {
        echo(&w, 200, df::task_failed(code, &echoed).to_string()).await;
        run_with(&w, DataForSeoClient::new(&base)).await;
        assert_clean(&w, &code.to_string()).await;
    }
    // A readable answer whose titles echo them is stored as text, but the
    // login still reaches nothing it should not: only the credentials we
    // sent matter here, and they are not part of any of our own output.
    echo(&w, 200, "not json ".to_string() + &echoed).await;
    run_with(&w, DataForSeoClient::new(&base)).await;
    assert_clean(&w, "garbage").await;
    assert!(!logged().is_empty(), "the capture works");
}

#[sqlx::test]
async fn keyword_research_requests_leak_the_login_nowhere_either(pool: PgPool) {
    use margincite::jobs::queue::Job;
    use margincite::keywords::volumes::KeywordVolumesJob;

    install();
    let w = GoogleWorld::new(pool).await;
    Mock::given(method("GET"))
        .and(path("/dataforseo/v3/appendix/user_data"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(df::answer(json!([{ "money": { "balance": 5.0 } }]))),
        )
        .mount(&w.mock)
        .await;
    w.send(
        "test-token",
        reqwest::Method::PUT,
        "/api/v1/settings/dataforseo",
        Some(json!({ "login": LOGIN, "password": PASSWORD })),
    )
    .await;
    w.add_keyword("blue widgets").await;
    let echoed = format!("{LOGIN} {PASSWORD} Basic {} authorization", df::token());
    let config = Arc::new(common::test_config(common::GOOGLE_SETTINGS));
    let base = dataforseo_base(&w.mock);
    let ideas_path = format!("/api/v1/sites/{}/keywords/ideas", w.site_id);
    let job = Job {
        id: Uuid::new_v4(),
        kind: "keyword_volumes".into(),
        payload: json!({ "workspace_id": w.workspace_id, "site_id": w.site_id }),
        attempts: 1,
    };
    for (status, body) in [
        (401, echoed.clone()),
        (402, echoed.clone()),
        (429, echoed.clone()),
        (500, echoed.clone()),
        (200, df::task_failed(40202, &echoed).to_string()),
        (200, df::task_failed(40200, &echoed).to_string()),
    ] {
        w.mock.reset().await;
        for at in [
            common::dataforseo::IDEAS_PATH,
            common::dataforseo::VOLUME_PATH,
        ] {
            Mock::given(method("POST"))
                .and(path(at))
                .respond_with(ResponseTemplate::new(status).set_body_string(body.clone()))
                .mount(&w.mock)
                .await;
        }
        let (_, reply) = w
            .send(
                "test-token",
                reqwest::Method::POST,
                &ideas_path,
                Some(json!({ "seed": "widgets" })),
            )
            .await;
        let handler = KeywordVolumesJob::new(
            w.pool.clone(),
            config.clone(),
            Arc::new(DataForSeoClient::new(&base)),
        );
        let failure = handler
            .run(&job)
            .await
            .err()
            .map(|err| err.to_string())
            .unwrap_or_default();
        let (_, settings) = w.get("/api/v1/settings").await;
        for (place, text) in [
            ("ideas reply", reply.to_string()),
            ("job error", failure),
            ("settings", settings.to_string()),
            ("stored", stored(&w).await),
            ("log", logged()),
        ] {
            for secret in secrets() {
                assert!(
                    !text.contains(&secret),
                    "{status}: {secret} in {place}: {text}"
                );
            }
        }
    }
}
