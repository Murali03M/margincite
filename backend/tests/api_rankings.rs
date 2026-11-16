//! Rankings over the API: the check endpoint's limits, workspace separation,
//! and the rule that nothing paid is asked without the workspace's login.

mod common;

use common::dataforseo as df;
use common::google::{add_site, workspace_of, GoogleWorld};
use margincite::jobs::worker::JobHandler;
use serde_json::{json, Value};
use sqlx::PgPool;
use uuid::Uuid;

async fn world_with_login(pool: PgPool, keywords: &[&str]) -> GoogleWorld {
    let w = GoogleWorld::new(pool).await;
    w.save_dataforseo().await;
    for keyword in keywords {
        w.add_keyword(keyword).await;
    }
    w
}

async fn runs(w: &GoogleWorld) -> i64 {
    sqlx::query_scalar("SELECT COUNT(*) FROM rank_runs WHERE site_id = $1")
        .bind(w.site_id)
        .fetch_one(&w.pool)
        .await
        .unwrap()
}

async fn finish_all_runs(w: &GoogleWorld) {
    sqlx::query("UPDATE rank_runs SET status = 'completed', finished_at = now()")
        .execute(&w.pool)
        .await
        .unwrap();
}

#[sqlx::test]
async fn a_check_is_accepted_and_queued_with_its_run(pool: PgPool) {
    let w = world_with_login(pool, &["a", "b"]).await;
    let (status, body) = w.post(&w.rank_path("/check")).await;
    assert_eq!(status, 202, "{body}");
    assert_eq!(body["data"]["queued"], true);
    let (kind, payload): (String, Value) = sqlx::query_as(
        "SELECT kind, payload FROM jobs WHERE kind = 'rank_run' AND payload ? 'run_id'",
    )
    .fetch_one(&w.pool)
    .await
    .unwrap();
    assert_eq!(kind, "rank_run");
    assert_eq!(payload["run_id"], w.latest_run_id().await.to_string());
    let total: i32 = sqlx::query_scalar("SELECT keywords_total FROM rank_runs WHERE site_id = $1")
        .bind(w.site_id)
        .fetch_one(&w.pool)
        .await
        .unwrap();
    assert_eq!(total, 2);
}

#[sqlx::test]
async fn without_a_login_a_check_is_refused_and_nothing_is_asked(pool: PgPool) {
    let w = GoogleWorld::new(pool).await;
    w.add_keyword("a").await;
    let (status, body) = w.post(&w.rank_path("/check")).await;
    assert_eq!(status, 422);
    assert_eq!(
        body["error"],
        "Rank tracking needs a DataForSEO login. Add it in Settings, then try again."
    );
    assert_eq!(runs(&w).await, 0);
    assert_eq!(w.dataforseo_requests().await, 0);
}

#[sqlx::test]
async fn a_login_from_the_environment_is_enough(pool: PgPool) {
    let mut settings = common::GOOGLE_SETTINGS.to_vec();
    settings.push(("DATAFORSEO_LOGIN", df::LOGIN));
    settings.push(("DATAFORSEO_PASSWORD", df::PASSWORD));
    let w = GoogleWorld::with_settings(pool, &settings).await;
    w.add_keyword("a").await;
    let (status, _) = w.post(&w.rank_path("/check")).await;
    assert_eq!(status, 202);
}

#[sqlx::test]
async fn a_check_without_keywords_is_refused_without_using_the_allowance(pool: PgPool) {
    let w = world_with_login(pool, &[]).await;
    let (status, body) = w.post(&w.rank_path("/check")).await;
    assert_eq!(status, 422);
    assert_eq!(
        body["error"],
        "Add at least one keyword before checking rankings."
    );
    assert_eq!(runs(&w).await, 0);
}

#[sqlx::test]
async fn a_second_check_while_one_is_queued_or_running_is_409_and_costs_nothing(pool: PgPool) {
    let w = world_with_login(pool, &["a"]).await;
    assert_eq!(w.post(&w.rank_path("/check")).await.0, 202);
    for _ in 0..3 {
        let (status, body) = w.post(&w.rank_path("/check")).await;
        assert_eq!(status, 409);
        assert_eq!(
            body["error"],
            "A ranking check is already queued or running for this site."
        );
    }
    assert_eq!(runs(&w).await, 1);
    sqlx::query("UPDATE rank_runs SET status = 'running'")
        .execute(&w.pool)
        .await
        .unwrap();
    assert_eq!(w.post(&w.rank_path("/check")).await.0, 409);
    finish_all_runs(&w).await;
    assert_eq!(w.post(&w.rank_path("/check")).await.0, 202);
}

#[sqlx::test]
async fn the_sixth_manual_check_of_a_day_is_429(pool: PgPool) {
    let w = world_with_login(pool, &["a"]).await;
    for _ in 0..5 {
        assert_eq!(w.post(&w.rank_path("/check")).await.0, 202);
        finish_all_runs(&w).await;
    }
    let (status, body) = w.post(&w.rank_path("/check")).await;
    assert_eq!(status, 429);
    assert_eq!(
        body["error"],
        "You have used today's 5 ranking checks for this site. Try again tomorrow."
    );
    assert_eq!(runs(&w).await, 5);
    // Yesterday's checks do not count.
    sqlx::query("UPDATE rank_runs SET created_at = now() - interval '2 days'")
        .execute(&w.pool)
        .await
        .unwrap();
    assert_eq!(w.post(&w.rank_path("/check")).await.0, 202);
}

#[sqlx::test]
async fn scheduled_runs_do_not_use_the_manual_allowance(pool: PgPool) {
    let w = world_with_login(pool, &["a"]).await;
    for _ in 0..6 {
        sqlx::query(
            "INSERT INTO rank_runs (id, workspace_id, site_id, status, trigger, finished_at)
             VALUES (gen_random_uuid(), $1, $2, 'completed', 'scheduled', now())",
        )
        .bind(w.workspace_id)
        .bind(w.site_id)
        .execute(&w.pool)
        .await
        .unwrap();
    }
    assert_eq!(w.post(&w.rank_path("/check")).await.0, 202);
}

#[sqlx::test]
async fn another_workspace_cannot_read_or_start_anything(pool: PgPool) {
    let w = world_with_login(pool, &["a"]).await;
    w.mock_serp_for_all(json!([df::organic(2, "example.com")]))
        .await;
    w.run_ranks().await;
    for (method, path) in [
        (reqwest::Method::GET, w.rank_path("")),
        (reqwest::Method::POST, w.rank_path("/check")),
        (
            reqwest::Method::GET,
            format!("/api/v1/sites/{}/ai-overviews", w.site_id),
        ),
    ] {
        let (status, body) = w.send("other-token", method.clone(), &path, None).await;
        assert_eq!(status, 404, "{method} {path}");
        assert!(!body.to_string().contains("example.com/page"));
    }
    // Unknown sites and missing tokens.
    let (status, _) = w
        .get(&format!("/api/v1/sites/{}/rankings", Uuid::new_v4()))
        .await;
    assert_eq!(status, 404);
    let (status, _) = w
        .send("nope", reqwest::Method::GET, &w.rank_path(""), None)
        .await;
    assert_eq!(status, 401);
}

#[sqlx::test]
async fn a_workspace_without_a_login_makes_no_paid_call_by_any_path(pool: PgPool) {
    // Workspace A has a login; workspace B, with keywords and a site of its
    // own, has none.
    let a = world_with_login(pool, &["a"]).await;
    a.mock_serp_for_all(json!([])).await;
    let b_ws = workspace_of(&a.app, "other-token").await;
    let b_site = add_site(&a.pool, b_ws, "https://other.example.org/").await;
    sqlx::query("INSERT INTO site_keywords (id, workspace_id, site_id, keyword) VALUES (gen_random_uuid(), $1, $2, 'b word')")
        .bind(b_ws)
        .bind(b_site)
        .execute(&a.pool)
        .await
        .unwrap();
    let b_path = format!("/api/v1/sites/{b_site}/rankings/check");
    // Manual.
    let (status, _) = a
        .send("other-token", reqwest::Method::POST, &b_path, None)
        .await;
    assert_eq!(status, 422);
    // Scheduled, run by the worker with B's site in its payload.
    let job = margincite::jobs::queue::Job {
        id: Uuid::new_v4(),
        kind: "rank_run".into(),
        payload: json!({ "workspace_id": b_ws, "site_id": b_site }),
        attempts: 1,
    };
    a.rank_handler().run(&job).await.unwrap();
    // A job that carries a run id (made while a login existed) as well.
    sqlx::query(
        "INSERT INTO rank_runs (id, workspace_id, site_id, status, trigger, keywords_total)
         VALUES ($1, $2, $3, 'queued', 'manual', 1)",
    )
    .bind(Uuid::new_v4())
    .bind(b_ws)
    .bind(b_site)
    .execute(&a.pool)
    .await
    .unwrap();
    let run: Uuid = sqlx::query_scalar("SELECT id FROM rank_runs WHERE site_id = $1")
        .bind(b_site)
        .fetch_one(&a.pool)
        .await
        .unwrap();
    let mut with_run = job.clone();
    with_run.payload["run_id"] = json!(run);
    a.rank_handler().run(&with_run).await.unwrap();
    // Not one request about B's keyword, nor anything else, reached the provider.
    let asked = a.serp_tasks().await;
    assert!(asked.is_empty(), "{asked:?}");
    let user_data = a
        .requests_to("/dataforseo/v3/appendix/user_data")
        .await
        .len();
    assert_eq!(
        a.dataforseo_requests().await,
        user_data,
        "only A's login test"
    );
    let usage: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM usage_events WHERE workspace_id = $1")
            .bind(b_ws)
            .fetch_one(&a.pool)
            .await
            .unwrap();
    assert_eq!(usage, 0);
}

// ---------------------------------------------------------------- schedule

async fn queued_at_days(w: &GoogleWorld) -> Vec<f64> {
    sqlx::query_scalar(
        "SELECT (EXTRACT(EPOCH FROM (run_at - now())) / 86400)::float8
         FROM jobs WHERE kind = 'rank_run' AND status = 'queued' ORDER BY run_at",
    )
    .fetch_all(&w.pool)
    .await
    .unwrap()
}

async fn run_scheduled(w: &GoogleWorld) {
    w.rank_handler().run(&w.rank_job(None)).await.unwrap();
}

#[sqlx::test]
async fn a_scheduled_run_checks_every_keyword_and_queues_the_next_weekly_run(pool: PgPool) {
    let w = world_with_login(pool, &["a", "b"]).await;
    w.mock_serp_for_all(json!([df::organic(3, "example.com")]))
        .await;
    run_scheduled(&w).await;
    assert_eq!(w.serp_tasks().await.len(), 2);
    assert_eq!(w.ranking_of("a").await["position"], 3);
    let (trigger, status): (String, String) =
        sqlx::query_as("SELECT trigger, status FROM rank_runs WHERE site_id = $1")
            .bind(w.site_id)
            .fetch_one(&w.pool)
            .await
            .unwrap();
    assert_eq!(
        (trigger.as_str(), status.as_str()),
        ("scheduled", "completed")
    );
    let next = queued_at_days(&w).await;
    assert_eq!(next.len(), 1);
    assert!((next[0] - 7.0).abs() < 0.01, "{next:?}");
}

#[sqlx::test]
async fn a_daily_site_is_queued_for_tomorrow(pool: PgPool) {
    let w = world_with_login(pool, &["a"]).await;
    w.set_site(json!({ "ranking_frequency": "daily" })).await;
    sqlx::query("DELETE FROM jobs")
        .execute(&w.pool)
        .await
        .unwrap();
    w.mock_serp_for_all(json!([])).await;
    run_scheduled(&w).await;
    let next = queued_at_days(&w).await;
    assert_eq!(next.len(), 1);
    assert!((next[0] - 1.0).abs() < 0.01, "{next:?}");
}

#[sqlx::test]
async fn a_scheduled_run_asks_nothing_when_tracking_is_off_or_there_is_nothing_to_track(
    pool: PgPool,
) {
    let w = world_with_login(pool, &["a"]).await;
    sqlx::query("DELETE FROM jobs")
        .execute(&w.pool)
        .await
        .unwrap();
    w.mock_serp_for_all(json!([])).await;
    w.set_site(json!({ "track_rankings": false })).await;
    run_scheduled(&w).await;
    sqlx::query("DELETE FROM site_keywords")
        .execute(&w.pool)
        .await
        .unwrap();
    w.set_site(json!({ "track_rankings": true })).await;
    run_scheduled(&w).await;
    assert!(w.serp_tasks().await.is_empty());
    assert_eq!(runs(&w).await, 0);
    assert_eq!(
        w.queued_rank_jobs().await,
        0,
        "the schedule stops until something changes"
    );
}

#[sqlx::test]
async fn a_scheduled_run_right_after_a_manual_one_does_not_ask_again(pool: PgPool) {
    let w = world_with_login(pool, &["a"]).await;
    w.mock_serp_for_all(json!([])).await;
    w.run_ranks().await;
    assert_eq!(w.serp_tasks().await.len(), 1);
    sqlx::query("DELETE FROM jobs")
        .execute(&w.pool)
        .await
        .unwrap();
    run_scheduled(&w).await;
    assert_eq!(w.serp_tasks().await.len(), 1, "the manual check counts");
    assert_eq!(w.queued_rank_jobs().await, 1, "and the schedule goes on");
}

#[sqlx::test]
async fn a_scheduled_job_that_meets_a_running_run_starts_nothing(pool: PgPool) {
    let w = world_with_login(pool, &["a"]).await;
    w.mock_serp_for_all(json!([])).await;
    assert_eq!(w.post(&w.rank_path("/check")).await.0, 202);
    run_scheduled(&w).await;
    assert!(w.serp_tasks().await.is_empty());
    assert_eq!(runs(&w).await, 1);
}

#[sqlx::test]
async fn the_schedule_is_job_recorded_so_a_retry_resumes_the_same_run(pool: PgPool) {
    let w = world_with_login(pool, &["a"]).await;
    w.mock_serp_for_all(json!([])).await;
    let job = w.rank_job(None);
    sqlx::query("INSERT INTO jobs (id, kind, payload, status, run_at) VALUES ($1, 'rank_run', $2, 'running', now())")
        .bind(job.id)
        .bind(&job.payload)
        .execute(&w.pool)
        .await
        .unwrap();
    w.rank_handler().run(&job).await.unwrap();
    let payload: Value = sqlx::query_scalar("SELECT payload FROM jobs WHERE id = $1")
        .bind(job.id)
        .fetch_one(&w.pool)
        .await
        .unwrap();
    assert_eq!(payload["run_id"], w.latest_run_id().await.to_string());
}

// ------------------------------------------------------------------- hooks

#[sqlx::test]
async fn adding_a_keyword_starts_the_schedule_only_with_a_login(pool: PgPool) {
    let w = GoogleWorld::new(pool).await;
    let add = |text: &'static str| {
        let w = &w;
        async move {
            let (status, _) = w
                .send(
                    "test-token",
                    reqwest::Method::POST,
                    &format!("/api/v1/sites/{}/keywords", w.site_id),
                    Some(json!({ "keyword": text })),
                )
                .await;
            assert_eq!(status, 201);
        }
    };
    add("first").await;
    assert_eq!(w.queued_rank_jobs().await, 0, "no login, no schedule");
    w.save_dataforseo().await;
    assert_eq!(
        w.queued_rank_jobs().await,
        1,
        "saving the login starts it for sites with keywords"
    );
    let waiting = queued_at_days(&w).await;
    assert!(
        waiting[0] > 0.0 && waiting[0] < 0.02,
        "a few minutes: {waiting:?}"
    );
    add("second").await;
    assert_eq!(
        w.queued_rank_jobs().await,
        1,
        "one job, however many keywords"
    );
    assert_eq!(
        w.dataforseo_requests().await,
        1,
        "only the login test was sent"
    );
}

#[sqlx::test]
async fn site_settings_start_the_schedule_but_never_bring_a_waiting_run_closer(pool: PgPool) {
    let w = world_with_login(pool, &["a"]).await;
    sqlx::query("DELETE FROM jobs")
        .execute(&w.pool)
        .await
        .unwrap();
    w.set_site(json!({ "display_name": "Renamed" })).await;
    assert_eq!(w.queued_rank_jobs().await, 1);
    sqlx::query("UPDATE jobs SET run_at = now() + interval '7 days' WHERE kind = 'rank_run'")
        .execute(&w.pool)
        .await
        .unwrap();
    w.set_site(json!({ "country": "GB" })).await;
    w.set_site(json!({ "display_name": "Renamed again" })).await;
    let waiting = queued_at_days(&w).await;
    assert_eq!(waiting.len(), 1);
    assert!((waiting[0] - 7.0).abs() < 0.01, "{waiting:?}");
    // Tracking off starts nothing.
    sqlx::query("DELETE FROM jobs")
        .execute(&w.pool)
        .await
        .unwrap();
    w.set_site(json!({ "track_rankings": false })).await;
    assert_eq!(w.queued_rank_jobs().await, 0);
}

async fn seed_run(w: &GoogleWorld, status: &str, error: Option<&str>, days_ago: i32) -> Uuid {
    let id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO rank_runs (id, workspace_id, site_id, status, trigger, error, finished_at,
                                created_at)
         VALUES ($1, $2, $3, $4, 'manual', $5,
                 CASE WHEN $4 IN ('completed', 'failed')
                      THEN now() - make_interval(days => $6) END,
                 now() - make_interval(days => $6))",
    )
    .bind(id)
    .bind(w.workspace_id)
    .bind(w.site_id)
    .bind(status)
    .bind(error)
    .bind(days_ago)
    .execute(&w.pool)
    .await
    .unwrap();
    id
}

#[sqlx::test]
async fn a_site_never_checked_has_no_last_run(pool: PgPool) {
    let w = GoogleWorld::new(pool).await;
    let (status, body) = w.get(&w.rank_path("")).await;
    assert_eq!(status, 200, "{body}");
    assert!(body["data"]["last_run"].is_null());
    assert!(body["data"]["checked_at"].is_null());
}

#[sqlx::test]
async fn the_last_run_is_the_newest_of_any_outcome_and_checked_at_keeps_its_meaning(pool: PgPool) {
    let w = world_with_login(pool, &["a"]).await;
    let old = seed_run(&w, "completed", None, 3).await;
    sqlx::query("INSERT INTO rank_checks (id, workspace_id, site_id, run_id, keyword_id, keyword, country, language, device, ai_overview) SELECT gen_random_uuid(), $1, $2, $3, id, 'a', 'US', 'en', 'desktop', 'absent' FROM site_keywords WHERE site_id = $2")
        .bind(w.workspace_id)
        .bind(w.site_id)
        .bind(old)
        .execute(&w.pool)
        .await
        .unwrap();
    let (_, before) = w.get(&w.rank_path("")).await;
    let failed = seed_run(
        &w,
        "failed",
        Some("DataForSEO refused the login. Check it in Settings."),
        1,
    )
    .await;
    let (_, after) = w.get(&w.rank_path("")).await;
    let last = &after["data"]["last_run"];
    assert_eq!(last["id"], failed.to_string());
    assert_eq!(last["status"], "failed");
    assert_eq!(
        last["error"],
        "DataForSEO refused the login. Check it in Settings."
    );
    assert!(last["finished_at"].is_string());
    assert!(!before["data"]["checked_at"].is_null());
    assert_eq!(before["data"]["checked_at"], after["data"]["checked_at"]);
}

#[sqlx::test]
async fn a_queued_or_running_run_has_no_finish_time_or_error(pool: PgPool) {
    let w = world_with_login(pool, &["a"]).await;
    let (status, _) = w.post(&w.rank_path("/check")).await;
    assert_eq!(status, 202);
    let (_, body) = w.get(&w.rank_path("")).await;
    let last = &body["data"]["last_run"];
    assert_eq!(last["id"], w.latest_run_id().await.to_string());
    assert_eq!(last["status"], "queued");
    assert!(last["error"].is_null() && last["finished_at"].is_null());
    sqlx::query("UPDATE rank_runs SET status = 'running'")
        .execute(&w.pool)
        .await
        .unwrap();
    let (_, body) = w.get(&w.rank_path("")).await;
    assert_eq!(body["data"]["last_run"]["status"], "running");
}
