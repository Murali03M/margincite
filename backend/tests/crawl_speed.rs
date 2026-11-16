use std::sync::Arc;
use std::time::Duration;

use margincite::crawl::client::{CrawlClient, Limits, Network};
use margincite::crawl::render::NoRenderer;
use margincite::crawl::repo;
use margincite::crawl::runner::{self, CrawlDeps, CrawlSettings};
use sqlx::PgPool;
use uuid::Uuid;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn deps(pool: PgPool) -> CrawlDeps {
    CrawlDeps {
        pool,
        client: CrawlClient::with_limits(
            "https://example.com/bot",
            Network::AllowPrivate,
            Limits {
                timeout: Duration::from_secs(5),
                max_bytes: 1024 * 1024,
                max_redirects: 5,
            },
        ),
        renderer: Arc::new(NoRenderer),
        settings: CrawlSettings {
            concurrency: 2,
            delay: Duration::ZERO,
            ..CrawlSettings::default()
        },
        config: None,
    }
}

async fn begin(pool: &PgPool, server: &MockServer) -> (Uuid, Uuid) {
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

async fn speed_jobs(pool: &PgPool, site_id: Uuid) -> i64 {
    sqlx::query_scalar(
        "SELECT COUNT(*) FROM jobs WHERE kind = 'speed_run' AND payload->>'site_id' = $1",
    )
    .bind(site_id.to_string())
    .fetch_one(pool)
    .await
    .unwrap()
}

#[sqlx::test]
async fn a_completed_crawl_queues_one_speed_run(pool: PgPool) {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(ResponseTemplate::new(200).set_body_raw("<title>Hi</title>", "text/html"))
        .mount(&server)
        .await;
    let (workspace_id, site_id) = begin(&pool, &server).await;
    let crawl_id = repo::create_crawl(&pool, workspace_id, site_id, "manual")
        .await
        .unwrap();
    runner::run_crawl(&deps(pool.clone()), workspace_id, crawl_id)
        .await
        .unwrap();
    assert_eq!(
        repo::get_crawl(&pool, workspace_id, crawl_id)
            .await
            .unwrap()
            .status,
        "completed"
    );
    assert_eq!(speed_jobs(&pool, site_id).await, 1);
    // A second crawl pulls the waiting job forward instead of adding another.
    let again = repo::create_crawl(&pool, workspace_id, site_id, "manual")
        .await
        .unwrap();
    runner::run_crawl(&deps(pool.clone()), workspace_id, again)
        .await
        .unwrap();
    assert_eq!(speed_jobs(&pool, site_id).await, 1);
}
