//! No log line holds a secret. The subscriber is installed for the whole
//! process (server tasks run on other threads, where a scoped one is not
//! seen), so this lives in a test binary of its own.

mod common;

use std::io::Write;
use std::sync::{Arc, Mutex, OnceLock};

use common::google::GoogleWorld;
use common::SECRET_KEY;
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

const SECRETS: [&str; 5] = [
    "sk-log-test-OPENAI-SECRET-1",
    "log-test-GOOGLE-SECRET-2",
    "log-test-SEO-PASSWORD-3",
    "log-test-PSI-SECRET-4",
    "log-test-CLAUDE-SECRET-5",
];

#[sqlx::test]
async fn saving_and_testing_secrets_writes_none_to_the_log(pool: PgPool) {
    install();
    let w = GoogleWorld::with_settings(pool, &[("MARGINCITE_SECRET_KEY", SECRET_KEY)]).await;
    // Providers answer errors that echo the secret back.
    for (m, p, status) in [
        ("GET", "/openai/v1/models", 401),
        ("GET", "/claude/v1/models", 500),
        ("GET", "/dataforseo/v3/appendix/user_data", 401),
    ] {
        Mock::given(method(m))
            .and(path(p))
            .respond_with(ResponseTemplate::new(status).set_body_string(SECRETS.join(" ")))
            .mount(&w.mock)
            .await;
    }
    let send = |p: &'static str, body: serde_json::Value| {
        let w = &w;
        async move {
            w.app
                .client
                .put(w.url(p))
                .header("authorization", "Bearer test-token")
                .json(&body)
                .send()
                .await
                .unwrap()
                .text()
                .await
                .unwrap()
        }
    };
    send(
        "/api/v1/settings/providers/chatgpt",
        json!({ "api_key": SECRETS[0] }),
    )
    .await;
    send(
        "/api/v1/settings/providers/claude",
        json!({ "api_key": SECRETS[4] }),
    )
    .await;
    send(
        "/api/v1/settings/google",
        json!({ "client_id": "id-1", "client_secret": SECRETS[1] }),
    )
    .await;
    send(
        "/api/v1/settings/dataforseo",
        json!({ "login": "me@example.com", "password": SECRETS[2] }),
    )
    .await;
    send(
        "/api/v1/settings/pagespeed",
        json!({ "api_key": SECRETS[3] }),
    )
    .await;
    send(
        "/api/v1/settings/pagespeed",
        json!({ "api_key": 5, "other": SECRETS[3] }),
    )
    .await;
    let logged = String::from_utf8_lossy(&sink().lock().unwrap()).to_string();
    assert!(!logged.is_empty(), "the capture works");
    for secret in SECRETS.iter().chain([&SECRET_KEY, &"BwcHBw"]) {
        assert!(!logged.contains(secret), "{secret} reached the log");
    }
}
