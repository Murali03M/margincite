mod common;

use common::google::GoogleWorld;
use common::speed::lab;
use margincite::jobs::worker::JobHandler;
use serde_json::{json, Value};
use sqlx::PgPool;

const HOME: &str = "https://example.com/";
const A: &str = "https://example.com/a";

async fn metrics(w: &GoogleWorld) -> Value {
    let (status, body) = w.get(&format!("/api/v1/sites/{}/metrics", w.site_id)).await;
    assert_eq!(status, 200);
    body["data"]["speed"].clone()
}

async fn queued_runs(w: &GoogleWorld) -> i64 {
    sqlx::query_scalar("SELECT COUNT(*) FROM jobs WHERE kind = 'speed_run' AND status = 'queued'")
        .fetch_one(&w.pool)
        .await
        .unwrap()
}

async fn seed(w: &GoogleWorld, url: &str, performance: Option<i32>, error: Option<&str>) {
    sqlx::query(
        "INSERT INTO speed_results (workspace_id, site_id, url, strategy, performance, error)
         VALUES ($1, $2, $3, 'mobile', $4, $5)",
    )
    .bind(w.workspace_id)
    .bind(w.site_id)
    .bind(url)
    .bind(performance)
    .bind(error)
    .execute(&w.pool)
    .await
    .unwrap();
}

#[sqlx::test]
async fn a_site_never_tested_has_an_empty_list_and_says_so_in_metrics(pool: PgPool) {
    let w = GoogleWorld::new(pool).await;
    let (status, body) = w.get(&w.speed_path("")).await;
    assert_eq!(status, 200);
    assert_eq!(body["data"], json!({ "checked_at": null, "pages": [] }));
    assert_eq!(
        metrics(&w).await,
        json!({ "performance": null, "checked_at": null,
                "reason": "Page speed has not been checked yet. It runs after each crawl, or press Check now." })
    );
}

#[sqlx::test]
async fn the_metric_is_the_home_pages_score(pool: PgPool) {
    let w = GoogleWorld::new(pool).await;
    seed(&w, A, Some(40), None).await;
    seed(&w, HOME, Some(71), None).await;
    let speed = metrics(&w).await;
    assert_eq!(speed["performance"], 71);
    assert!(speed["checked_at"].is_string());
    assert!(speed["reason"].is_null());
}

#[sqlx::test]
async fn without_a_home_score_the_metric_is_the_average(pool: PgPool) {
    let w = GoogleWorld::new(pool).await;
    seed(
        &w,
        HOME,
        None,
        Some("Google PageSpeed is limiting requests right now."),
    )
    .await;
    seed(&w, A, Some(40), None).await;
    seed(&w, "https://example.com/b", Some(61), None).await;
    // 50.5 rounds to 51.
    assert_eq!(metrics(&w).await["performance"], 51);
}

#[sqlx::test]
async fn a_site_whose_every_test_failed_shows_no_number(pool: PgPool) {
    let w = GoogleWorld::new(pool).await;
    seed(&w, HOME, None, Some("It failed.")).await;
    let speed = metrics(&w).await;
    assert!(speed["performance"].is_null());
    assert_eq!(
        speed["reason"],
        "Page speed could not be measured on the last check. Open Page speed to see why."
    );
}

#[sqlx::test]
async fn speed_checks_are_not_run_until_a_page_has_a_good_result(pool: PgPool) {
    let w = GoogleWorld::new(pool).await;
    w.crawl(&[(HOME, true), (A, true)]).await;
    let checks = |body: &Value| -> Vec<Value> {
        body["data"]["checks"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|c| c["check_id"].as_str().unwrap().starts_with("speed_"))
            .cloned()
            .collect()
    };
    let (_, before) = w.get(&format!("/api/v1/sites/{}/checks", w.site_id)).await;
    let before = checks(&before);
    assert_eq!(before.len(), 3);
    for check in &before {
        assert_eq!(check["status"], "not_run");
        assert_eq!(check["reason"], "Run a page speed check to see this.");
    }
    // Only an error so far: still not run.
    seed(&w, HOME, None, Some("Failed.")).await;
    let (_, still) = w.get(&format!("/api/v1/sites/{}/checks", w.site_id)).await;
    assert!(checks(&still).iter().all(|c| c["status"] == "not_run"));
    sqlx::query("UPDATE speed_results SET performance = 90, error = NULL")
        .execute(&w.pool)
        .await
        .unwrap();
    let (_, after) = w.get(&format!("/api/v1/sites/{}/checks", w.site_id)).await;
    assert!(checks(&after).iter().all(|c| c["status"] == "passed"));
}

#[sqlx::test]
async fn open_speed_items_stay_open_and_out_of_the_score_when_the_source_goes_missing(
    pool: PgPool,
) {
    let w = GoogleWorld::new(pool).await;
    w.crawl(&[(HOME, true), (A, true)]).await;
    w.mock_speed(A, 200, lab(0.3, 6000.0, 0.0, 10.0)).await;
    w.mock_speed_for_all(lab(0.9, 1000.0, 0.0, 10.0)).await;
    w.speed_handler().run(&w.speed_job()).await.unwrap();
    let open = |w: &GoogleWorld| {
        let pool = w.pool.clone();
        let site = w.site_id;
        async move {
            sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM suggestion_items i JOIN suggestions s ON s.id = i.suggestion_id
                 WHERE s.site_id = $1 AND s.check_id = 'speed_slow_lcp' AND i.status = 'open'",
            )
            .bind(site)
            .fetch_one(&pool)
            .await
            .unwrap()
        }
    };
    assert_eq!(open(&w).await, 1);
    // Every result now carries an error: the source is missing, the item is
    // left open rather than resolved.
    sqlx::query("UPDATE speed_results SET error = 'Failed.'")
        .execute(&w.pool)
        .await
        .unwrap();
    let crawl_id: uuid::Uuid = sqlx::query_scalar("SELECT id FROM crawls WHERE site_id = $1")
        .bind(w.site_id)
        .fetch_one(&w.pool)
        .await
        .unwrap();
    margincite::checks::store::run_for_crawl(&w.pool, w.workspace_id, crawl_id)
        .await
        .unwrap();
    assert_eq!(open(&w).await, 1);
}

#[sqlx::test]
async fn check_now_queues_one_run_and_answers_202(pool: PgPool) {
    let w = GoogleWorld::new(pool).await;
    let (status, body) = w.post(&w.speed_path("/check")).await;
    assert_eq!(status, 202);
    assert_eq!(body["data"], json!({ "queued": true }));
    assert_eq!(queued_runs(&w).await, 1);
    let (status, _) = w.post(&w.speed_path("/check")).await;
    assert_eq!(status, 202);
    assert_eq!(queued_runs(&w).await, 1);
}

#[sqlx::test]
async fn the_sixth_manual_check_of_a_day_is_429_and_deleting_the_site_does_not_reset_it(
    pool: PgPool,
) {
    let w = GoogleWorld::new(pool).await;
    for _ in 0..5 {
        assert_eq!(w.post(&w.speed_path("/check")).await.0, 202);
    }
    let (status, body) = w.post(&w.speed_path("/check")).await;
    assert_eq!(status, 429);
    assert_eq!(
        body["error"],
        "You have used today's 5 page speed checks for this site. Try again tomorrow."
    );
    assert_eq!(queued_runs(&w).await, 1);
    // Yesterday's checks do not count.
    sqlx::query("UPDATE speed_manual_checks SET created_at = now() - interval '2 days'")
        .execute(&w.pool)
        .await
        .unwrap();
    assert_eq!(w.post(&w.speed_path("/check")).await.0, 202);
    let used: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM speed_manual_checks WHERE site_id = $1")
            .bind(w.site_id)
            .fetch_one(&w.pool)
            .await
            .unwrap();
    assert_eq!(used, 6);
}

#[sqlx::test]
async fn another_workspace_gets_404_and_nothing_is_queued(pool: PgPool) {
    let w = GoogleWorld::new(pool).await;
    seed(&w, HOME, Some(80), None).await;
    let (status, _) = w
        .send("other-token", reqwest::Method::GET, &w.speed_path(""), None)
        .await;
    assert_eq!(status, 404);
    let (status, _) = w
        .send(
            "other-token",
            reqwest::Method::POST,
            &w.speed_path("/check"),
            None,
        )
        .await;
    assert_eq!(status, 404);
    assert_eq!(queued_runs(&w).await, 0);
    let used: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM speed_manual_checks")
        .fetch_one(&w.pool)
        .await
        .unwrap();
    assert_eq!(used, 0);
}

#[sqlx::test]
async fn the_routes_need_a_token(pool: PgPool) {
    let w = GoogleWorld::new(pool).await;
    let get = w
        .app
        .client
        .get(w.url(&w.speed_path("")))
        .send()
        .await
        .unwrap();
    assert_eq!(get.status(), 401);
    let post = w
        .app
        .client
        .post(w.url(&w.speed_path("/check")))
        .send()
        .await
        .unwrap();
    assert_eq!(post.status(), 401);
}

#[sqlx::test]
async fn deleting_the_site_removes_its_results(pool: PgPool) {
    let w = GoogleWorld::new(pool).await;
    seed(&w, HOME, Some(80), None).await;
    let (status, _) = w
        .send(
            "test-token",
            reqwest::Method::DELETE,
            &format!("/api/v1/sites/{}", w.site_id),
            None,
        )
        .await;
    assert!(status == 200 || status == 204, "{status}");
    let left: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM speed_results")
        .fetch_one(&w.pool)
        .await
        .unwrap();
    assert_eq!(left, 0);
}
