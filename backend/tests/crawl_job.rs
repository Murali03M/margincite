use std::time::Duration;

use margincite::crawl::client::{CrawlClient, Limits, Network};
use margincite::crawl::render::NoRenderer;
use margincite::crawl::repo;
use margincite::crawl::runner::{self, CrawlDeps, CrawlJob, CrawlSettings};
use margincite::jobs::queue::{self, Job};
use margincite::jobs::worker::JobHandler;
use serde_json::{json, Value};
use sqlx::PgPool;
use time::OffsetDateTime;
use uuid::Uuid;

fn deps(pool: PgPool) -> CrawlDeps {
    CrawlDeps {
        pool,
        client: CrawlClient::with_limits(
            "https://example.com/bot",
            Network::AllowPrivate,
            Limits {
                timeout: Duration::from_secs(2),
                max_bytes: 1024 * 1024,
                max_redirects: 3,
            },
        ),
        renderer: std::sync::Arc::new(NoRenderer),
        settings: CrawlSettings::default(),
        config: None,
    }
}

/// A site whose stored address cannot be read, so `run_crawl` returns an error.
async fn broken_site(pool: &PgPool) -> (Uuid, Uuid) {
    let workspace_id = Uuid::new_v4();
    let site_id = Uuid::new_v4();
    sqlx::query("INSERT INTO workspaces (id, name) VALUES ($1, 'test')")
        .bind(workspace_id)
        .execute(pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO sites (id, workspace_id, root_url, host, path_prefix, display_name)
         VALUES ($1, $2, 'not a url', 'example.com', '/', 'example.com')",
    )
    .bind(site_id)
    .bind(workspace_id)
    .execute(pool)
    .await
    .unwrap();
    (workspace_id, site_id)
}

#[sqlx::test]
async fn a_crawl_that_errors_is_marked_failed_with_a_plain_error(pool: PgPool) {
    let (workspace_id, site_id) = broken_site(&pool).await;
    let crawl_id = repo::create_crawl(&pool, workspace_id, site_id, "manual")
        .await
        .unwrap();
    let result = runner::run_crawl(&deps(pool.clone()), workspace_id, crawl_id).await;
    assert!(result.is_err());
    let crawl = repo::get_crawl(&pool, workspace_id, crawl_id)
        .await
        .unwrap();
    assert_eq!(crawl.status, "failed");
    let error = crawl.error.unwrap();
    assert!(!error.is_empty());
    assert!(!error.contains("relative URL"), "{error}");
    let finished: bool =
        sqlx::query_scalar("SELECT finished_at IS NOT NULL FROM crawls WHERE id = $1")
            .bind(crawl_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(finished);
    let in_progress = repo::crawl_in_progress(&pool, workspace_id, site_id)
        .await
        .unwrap();
    assert!(!in_progress, "Crawl again must be possible");
}

#[sqlx::test]
async fn a_retried_scheduled_job_reuses_its_crawl_row(pool: PgPool) {
    let (workspace_id, site_id) = broken_site(&pool).await;
    queue::enqueue(
        &pool,
        "crawl",
        &json!({ "workspace_id": workspace_id, "site_id": site_id, "trigger": "scheduled" }),
        OffsetDateTime::now_utc(),
    )
    .await
    .unwrap();
    let job = queue::claim(&pool, "test").await.unwrap().unwrap();
    let handler = CrawlJob {
        deps: deps(pool.clone()),
    };
    assert!(handler.run(&job).await.is_err());
    let payload: Value = sqlx::query_scalar("SELECT payload FROM jobs WHERE id = $1")
        .bind(job.id)
        .fetch_one(&pool)
        .await
        .unwrap();
    let retried = Job { payload, ..job };
    assert!(handler.run(&retried).await.is_err());
    let crawls: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM crawls WHERE site_id = $1")
        .bind(site_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(crawls, 1);
}

#[sqlx::test]
async fn a_job_that_fails_for_good_marks_its_crawl_failed(pool: PgPool) {
    let (workspace_id, site_id) = broken_site(&pool).await;
    let crawl_id = repo::create_crawl(&pool, workspace_id, site_id, "manual")
        .await
        .unwrap();
    repo::mark_running(&pool, workspace_id, crawl_id)
        .await
        .unwrap();
    let job = Job {
        id: Uuid::new_v4(),
        kind: "crawl".into(),
        payload: json!({ "workspace_id": workspace_id, "crawl_id": crawl_id }),
        attempts: 3,
    };
    CrawlJob {
        deps: deps(pool.clone()),
    }
    .on_dead(&job, "boom")
    .await;
    let crawl = repo::get_crawl(&pool, workspace_id, crawl_id)
        .await
        .unwrap();
    assert_eq!(crawl.status, "failed");
    assert!(crawl.error.is_some());
}

// ------------------------------------------------- the workspace's contact

mod contact {
    use super::*;
    use std::sync::Arc;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn config(vars: &'static [(&'static str, &'static str)]) -> Arc<margincite::config::Config> {
        Arc::new(
            margincite::config::Config::from_env(|key| {
                if key == "DATABASE_URL" {
                    return Some("postgres://localhost/margincite_dev".into());
                }
                vars.iter()
                    .find(|(k, _)| *k == key)
                    .map(|(_, v)| v.to_string())
            })
            .unwrap(),
        )
    }

    async fn site_on(pool: &PgPool, server: &MockServer) -> (Uuid, Uuid) {
        let workspace_id = Uuid::new_v4();
        let site_id = Uuid::new_v4();
        sqlx::query("INSERT INTO workspaces (id, name) VALUES ($1, 'test')")
            .bind(workspace_id)
            .execute(pool)
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO sites (id, workspace_id, root_url, host, path_prefix, display_name)
             VALUES ($1, $2, $3, '127.0.0.1', '/', '127.0.0.1')",
        )
        .bind(site_id)
        .bind(workspace_id)
        .bind(format!("{}/", server.uri()))
        .execute(pool)
        .await
        .unwrap();
        (workspace_id, site_id)
    }

    async fn serve(server: &MockServer) {
        Mock::given(method("GET"))
            .and(path("/"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_raw("<html><body>hi</body></html>", "text/html"),
            )
            .mount(server)
            .await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(404))
            .mount(server)
            .await;
    }

    fn with_config(pool: PgPool, config: Arc<margincite::config::Config>) -> CrawlDeps {
        CrawlDeps {
            config: Some(config),
            settings: CrawlSettings {
                delay: Duration::ZERO,
                ..CrawlSettings::default()
            },
            ..deps(pool)
        }
    }

    async fn agents(server: &MockServer) -> Vec<String> {
        server
            .received_requests()
            .await
            .unwrap()
            .iter()
            .map(|r| {
                r.headers
                    .get("user-agent")
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .to_string()
            })
            .collect()
    }

    #[sqlx::test]
    async fn a_crawl_announces_the_workspaces_own_contact_before_the_environments(pool: PgPool) {
        let server = MockServer::start().await;
        serve(&server).await;
        let (ws, site) = site_on(&pool, &server).await;
        sqlx::query("INSERT INTO workspace_settings (workspace_id, key, value_plain) VALUES ($1, 'crawler.contact', 'mine@example.org')")
            .bind(ws)
            .execute(&pool)
            .await
            .unwrap();
        let config = config(&[("CRAWLER_CONTACT", "https://env.example.com/bot")]);
        let id = repo::create_crawl(&pool, ws, site, "manual").await.unwrap();
        runner::run_crawl(&with_config(pool.clone(), config), ws, id)
            .await
            .unwrap();
        let seen = agents(&server).await;
        assert!(!seen.is_empty());
        assert!(
            seen.iter().all(|a| a.contains("mine@example.org")),
            "{seen:?}"
        );
    }

    #[sqlx::test]
    async fn the_environment_contact_is_the_fallback(pool: PgPool) {
        let server = MockServer::start().await;
        serve(&server).await;
        let (ws, site) = site_on(&pool, &server).await;
        let config = config(&[("CRAWLER_CONTACT", "https://env.example.com/bot")]);
        let id = repo::create_crawl(&pool, ws, site, "manual").await.unwrap();
        runner::run_crawl(&with_config(pool.clone(), config), ws, id)
            .await
            .unwrap();
        assert!(agents(&server)
            .await
            .iter()
            .all(|a| a.contains("env.example.com")));
    }

    #[sqlx::test]
    async fn a_crawl_without_any_contact_fails_plainly_and_asks_nothing(pool: PgPool) {
        let server = MockServer::start().await;
        serve(&server).await;
        let (ws, site) = site_on(&pool, &server).await;
        let id = repo::create_crawl(&pool, ws, site, "manual").await.unwrap();
        runner::run_crawl(&with_config(pool.clone(), config(&[])), ws, id)
            .await
            .unwrap();
        let crawl = repo::get_crawl(&pool, ws, id).await.unwrap();
        assert_eq!(crawl.status, "failed");
        assert!(crawl.error.unwrap().contains("contact address in Settings"));
        assert!(server.received_requests().await.unwrap().is_empty());
    }

    #[sqlx::test]
    async fn a_scheduled_crawl_without_a_contact_waits_a_week_instead_of_failing(pool: PgPool) {
        let server = MockServer::start().await;
        serve(&server).await;
        let (ws, site) = site_on(&pool, &server).await;
        let job = CrawlJob {
            deps: with_config(pool.clone(), config(&[])),
        };
        let payload = json!({ "workspace_id": ws, "site_id": site, "trigger": "scheduled" });
        let job_row = Job {
            id: Uuid::new_v4(),
            kind: "crawl".into(),
            payload,
            attempts: 1,
        };
        job.run(&job_row).await.unwrap();
        let crawls: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM crawls WHERE site_id = $1")
            .bind(site)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(crawls, 0, "no crawl row to be left failed");
        let days: f64 = sqlx::query_scalar(
            "SELECT EXTRACT(EPOCH FROM (run_at - now()))::float8 / 86400 FROM jobs WHERE kind = 'crawl' AND status = 'queued'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert!((6.9..7.1).contains(&days), "{days}");
        assert!(server.received_requests().await.unwrap().is_empty());
    }
}
