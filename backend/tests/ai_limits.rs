//! The paid AI calls are capped per site and per workspace, and deleting a
//! site does not hand its allowance back.

mod common;

use common::ai::Q1;
use common::google::{add_site, GoogleWorld};
use reqwest::Method;
use serde_json::{json, Value};
use sqlx::PgPool;
use uuid::Uuid;

const SUGGESTIONS_LIMIT: &str =
    "You have asked for suggestions 10 times today for this site. Try again tomorrow.";
const WORKSPACE_LIMIT: &str =
    "You have used today's 10 AI visibility checks for this workspace. Try again tomorrow.";

async fn call(w: &GoogleWorld, method: Method, path: String, body: Option<Value>) -> (u16, Value) {
    w.send("test-token", method, &path, body).await
}

async fn suggest(w: &GoogleWorld, site: Uuid) -> (u16, Value) {
    let path = format!("/api/v1/sites/{site}/ai/prompts/suggest");
    call(w, Method::POST, path, None).await
}

async fn start_run(w: &GoogleWorld, site: Uuid) -> (u16, Value) {
    call(
        w,
        Method::POST,
        format!("/api/v1/sites/{site}/ai/runs"),
        None,
    )
    .await
}

async fn add_question(w: &GoogleWorld, site: Uuid) {
    let path = format!("/api/v1/sites/{site}/ai/prompts");
    let (status, body) = call(w, Method::POST, path, Some(json!({ "text": Q1 }))).await;
    assert_eq!(status, 201, "{body}");
}

/// Lets a started run count as finished, so the site can start another.
async fn finish_runs(w: &GoogleWorld) {
    sqlx::query("UPDATE ai_runs SET status = 'completed' WHERE status IN ('queued', 'running')")
        .execute(&w.pool)
        .await
        .unwrap();
}

/// Deletes the site over the API and adds the same address again.
async fn delete_and_re_add(w: &GoogleWorld, site: Uuid) -> Uuid {
    let (status, _) = call(w, Method::DELETE, format!("/api/v1/sites/{site}"), None).await;
    assert_eq!(status, 200);
    let (status, body) = call(
        w,
        Method::POST,
        "/api/v1/sites".into(),
        Some(json!({ "url": "example.com" })),
    )
    .await;
    assert_eq!(status, 201, "{body}");
    body["data"]["id"].as_str().unwrap().parse().unwrap()
}

#[sqlx::test]
async fn the_eleventh_suggestion_request_of_a_day_for_a_site_is_429(pool: PgPool) {
    let w = GoogleWorld::with_ai(pool).await;
    w.mount("/claude/v1/messages", 500, json!({}), None).await;
    for n in 1..=10 {
        assert_eq!(
            suggest(&w, w.site_id).await.0,
            502,
            "request {n} reaches Claude"
        );
    }
    let (status, body) = suggest(&w, w.site_id).await;
    assert_eq!(
        (status, body["error"].as_str()),
        (429, Some(SUGGESTIONS_LIMIT))
    );
    assert_eq!(w.requests_with("/claude/v1/messages").await.len(), 10);
}

#[sqlx::test]
async fn the_suggestion_cap_is_per_site_and_resets_the_next_day(pool: PgPool) {
    let w = GoogleWorld::with_ai(pool).await;
    w.mount("/claude/v1/messages", 500, json!({}), None).await;
    let other = add_site(&w.pool, w.workspace_id, "https://other.example/").await;
    for _ in 0..10 {
        suggest(&w, w.site_id).await;
    }
    assert_eq!(suggest(&w, w.site_id).await.0, 429);
    assert_eq!(
        suggest(&w, other).await.0,
        502,
        "another site is not held back"
    );
    sqlx::query("UPDATE ai_usage SET created_at = now() - interval '2 days'")
        .execute(&w.pool)
        .await
        .unwrap();
    assert_eq!(suggest(&w, w.site_id).await.0, 502);
}

#[sqlx::test]
async fn a_workspace_may_start_ten_manual_runs_a_day_across_sites(pool: PgPool) {
    let w = GoogleWorld::with_ai(pool).await;
    let mut sites = vec![w.site_id];
    for host in ["b.example", "c.example", "d.example"] {
        sites.push(add_site(&w.pool, w.workspace_id, &format!("https://{host}/")).await);
    }
    for site in &sites {
        add_question(&w, *site).await;
    }
    // 3 + 3 + 3 + 1: no site passes its own limit of 3.
    for (site, runs) in sites.iter().zip([3, 3, 3, 1]) {
        for _ in 0..runs {
            assert_eq!(start_run(&w, *site).await.0, 202);
            finish_runs(&w).await;
        }
    }
    let (status, body) = start_run(&w, sites[3]).await;
    assert_eq!(
        (status, body["error"].as_str()),
        (429, Some(WORKSPACE_LIMIT))
    );
}

#[sqlx::test]
async fn deleting_and_re_adding_a_site_does_not_reset_the_workspace_count(pool: PgPool) {
    let w = GoogleWorld::with_ai(pool).await;
    let mut site = w.site_id;
    // Three incarnations of one site use 3 + 3 + 3 runs.
    for _ in 0..3 {
        add_question(&w, site).await;
        for _ in 0..3 {
            assert_eq!(start_run(&w, site).await.0, 202);
            finish_runs(&w).await;
        }
        site = delete_and_re_add(&w, site).await;
    }
    add_question(&w, site).await;
    assert_eq!(start_run(&w, site).await.0, 202, "the tenth is allowed");
    finish_runs(&w).await;
    let (status, body) = start_run(&w, site).await;
    assert_eq!(
        (status, body["error"].as_str()),
        (429, Some(WORKSPACE_LIMIT))
    );
}

#[sqlx::test]
async fn a_new_day_gives_the_workspace_its_runs_back(pool: PgPool) {
    let w = GoogleWorld::with_ai(pool).await;
    add_question(&w, w.site_id).await;
    sqlx::query(
        "INSERT INTO ai_usage (workspace_id, site_id, kind, created_at)
         SELECT $1, gen_random_uuid(), 'manual_run', now() - interval '2 days'
         FROM generate_series(1, 10)",
    )
    .bind(w.workspace_id)
    .execute(&w.pool)
    .await
    .unwrap();
    assert_eq!(start_run(&w, w.site_id).await.0, 202);
}

#[sqlx::test]
async fn scheduled_runs_and_refused_requests_use_no_allowance(pool: PgPool) {
    let w = GoogleWorld::with_ai(pool).await;
    add_question(&w, w.site_id).await;
    w.mock_engines("An answer.", &[], None).await;
    w.run_ai_job(None).await.unwrap();
    let used: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM ai_usage")
        .fetch_one(&w.pool)
        .await
        .unwrap();
    assert_eq!(used, 0, "a scheduled run is not counted");
    assert_eq!(start_run(&w, w.site_id).await.0, 202);
    assert_eq!(
        start_run(&w, w.site_id).await.0,
        409,
        "refused: a run is active"
    );
    let used: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM ai_usage")
        .fetch_one(&w.pool)
        .await
        .unwrap();
    assert_eq!(used, 1);
}
