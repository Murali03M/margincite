//! The rank run: what is stored for each keyword, and what a failure does.

mod common;

use common::dataforseo as df;
use common::google::GoogleWorld;
use common::rankings::overview;
use margincite::jobs::worker::JobHandler;
use serde_json::{json, Value};
use sqlx::PgPool;

async fn world_with_login(pool: PgPool, keywords: &[&str]) -> GoogleWorld {
    let w = GoogleWorld::new(pool).await;
    w.save_dataforseo().await;
    for keyword in keywords {
        w.add_keyword(keyword).await;
    }
    w
}

fn serp(items: Value) -> Value {
    df::serp(items)
}

#[sqlx::test]
async fn a_keyword_in_position_seven_is_stored_with_its_page_and_the_top_results(pool: PgPool) {
    let w = world_with_login(pool, &["blue widgets"]).await;
    let mut items: Vec<Value> = (1..=12)
        .map(|rank| df::organic(rank, &format!("site{rank}.com")))
        .collect();
    items[6] = df::organic(7, "www.example.com");
    w.mock_serp("blue widgets", 200, serp(json!(items))).await;
    let run = w.run_ranks().await;
    assert_eq!(w.run_state(run).await.0, "completed");
    let row = w.ranking_of("blue widgets").await;
    assert_eq!(row["position"], 7);
    assert_eq!(row["previous"], Value::Null);
    assert_eq!(row["url"], "https://www.example.com/page");
    assert_eq!(row["ai_overview"], "absent");
    assert_eq!(row["top"].as_array().unwrap().len(), 10);
    assert_eq!(row["top"][0]["position"], 1);
    assert_eq!(row["top"][0]["domain"], "site1.com");
    assert_eq!(row["top"][0]["title"], "Result 1");
    assert_eq!(row["history"].as_array().unwrap().len(), 1);
    assert_eq!(row["history"][0]["position"], 7);
    assert!(row["history"][0]["at"].is_string());
}

#[sqlx::test]
async fn the_request_asks_for_the_sites_location_and_device(pool: PgPool) {
    let w = world_with_login(pool, &["blue widgets"]).await;
    w.set_site(json!({ "country": "GB", "language": "en", "device": "mobile" }))
        .await;
    w.mock_serp_for_all(json!([])).await;
    w.run_ranks().await;
    let tasks = w.serp_tasks().await;
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0]["location_code"], 2826);
    assert_eq!(tasks[0]["language_code"], "en");
    assert_eq!(tasks[0]["device"], "mobile");
    assert_eq!(tasks[0]["depth"], 100);
    assert_eq!(tasks[0]["load_async_ai_overview"], true);
}

#[sqlx::test]
async fn a_keyword_missing_from_the_top_100_is_stored_as_not_ranking_not_as_an_error(pool: PgPool) {
    let w = world_with_login(pool, &["blue widgets"]).await;
    w.mock_serp_for_all(json!([df::organic(1, "a.com"), df::organic(2, "b.com")]))
        .await;
    let run = w.run_ranks().await;
    assert_eq!(w.run_state(run).await.0, "completed");
    let row = w.ranking_of("blue widgets").await;
    assert_eq!(row["position"], Value::Null);
    assert_eq!(row["url"], Value::Null);
    assert_eq!(row["history"].as_array().unwrap().len(), 1);
    assert_eq!(row["history"][0]["position"], Value::Null);
    assert_eq!(row["top"].as_array().unwrap().len(), 2);
}

#[sqlx::test]
async fn a_subdomain_of_the_site_counts_as_the_site(pool: PgPool) {
    let w = world_with_login(pool, &["blog"]).await;
    w.mock_serp_for_all(json!([df::organic(3, "blog.example.com")]))
        .await;
    w.run_ranks().await;
    assert_eq!(w.ranking_of("blog").await["position"], 3);
}

#[sqlx::test]
async fn ai_overviews_are_absent_present_or_cited(pool: PgPool) {
    let w = world_with_login(pool, &["none", "other", "mine", "odd"]).await;
    let page = |extra: Vec<Value>| {
        let mut items = vec![df::organic(2, "a.com")];
        items.extend(extra);
        serp(json!(items))
    };
    w.mock_serp("none", 200, page(vec![])).await;
    w.mock_serp(
        "other",
        200,
        page(vec![overview(&[
            ("a.com", "https://a.com/x"),
            ("www.b.com", "https://www.b.com/"),
        ])]),
    )
    .await;
    w.mock_serp(
        "mine",
        200,
        page(vec![overview(&[
            ("a.com", "https://a.com/x"),
            ("example.com", "https://example.com/guide"),
        ])]),
    )
    .await;
    w.mock_serp(
        "odd",
        200,
        page(vec![
            json!({ "type": "ai_overview", "asynchronous_ai_overview": true, "items": null }),
        ]),
    )
    .await;
    w.run_ranks().await;
    assert_eq!(w.ranking_of("none").await["ai_overview"], "absent");
    assert_eq!(w.ranking_of("other").await["ai_overview"], "present");
    assert_eq!(w.ranking_of("mine").await["ai_overview"], "cited");
    assert_eq!(w.ranking_of("odd").await["ai_overview"], "unknown");
    let (status, body) = w
        .get(&format!("/api/v1/sites/{}/ai-overviews", w.site_id))
        .await;
    assert_eq!(status, 200);
    let rows = body["data"].as_array().unwrap();
    let by = |keyword: &str| {
        rows.iter()
            .find(|r| r["keyword"] == keyword)
            .unwrap()
            .clone()
    };
    assert_eq!(by("other")["status"], "present");
    assert_eq!(by("other")["cited_domains"], json!(["a.com", "b.com"]));
    assert_eq!(by("other")["your_urls"], json!([]));
    assert_eq!(by("mine")["status"], "cited");
    assert_eq!(
        by("mine")["your_urls"],
        json!(["https://example.com/guide"])
    );
    assert_eq!(by("none")["cited_domains"], json!([]));
    assert!(by("none")["checked_at"].is_string());
    assert!(by("none")["keyword_id"].is_string());
}

#[sqlx::test]
async fn a_failed_keyword_does_not_fail_the_run_and_keeps_its_old_result(pool: PgPool) {
    let w = world_with_login(pool, &["good", "bad"]).await;
    w.mock_serp("good", 200, serp(json!([df::organic(4, "example.com")])))
        .await;
    w.mock_serp("bad", 500, json!({ "oops": true })).await;
    let run = w.run_ranks().await;
    let (status, error) = w.run_state(run).await;
    assert_eq!(status, "completed");
    assert_eq!(
        error.as_deref(),
        Some("1 of 2 keywords could not be checked.")
    );
    assert_eq!(w.ranking_of("good").await["position"], 4);
    let bad = w.ranking_of("bad").await;
    assert_eq!(bad["position"], Value::Null);
    assert_eq!(bad["history"], json!([]));
    assert_eq!(bad["top"], json!([]));
    assert_eq!(bad["ai_overview"], "unknown");
    // Only the keyword that was checked has a stored result.
    let stored: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM rank_checks WHERE site_id = $1")
        .bind(w.site_id)
        .fetch_one(&w.pool)
        .await
        .unwrap();
    assert_eq!(stored, 1);
}

#[sqlx::test]
async fn a_run_where_every_keyword_failed_is_failed_and_says_why(pool: PgPool) {
    let w = world_with_login(pool, &["a", "b"]).await;
    w.mock_serp("a", 200, df::task_failed(50000, "internal error."))
        .await;
    w.mock_serp("b", 200, df::task_failed(50000, "internal error."))
        .await;
    let run = w.run_ranks().await;
    let (status, error) = w.run_state(run).await;
    assert_eq!(status, "failed");
    assert_eq!(
        error.as_deref(),
        Some("DataForSEO could not answer this request.")
    );
    // A failed run leaves no result, and the answer still lists the keywords.
    assert_eq!(w.rankings().await["checked_at"], Value::Null);
}

#[sqlx::test]
async fn running_out_of_credit_stops_the_run_and_marks_the_setting(pool: PgPool) {
    let keywords: Vec<String> = (0..8).map(|n| format!("keyword {n}")).collect();
    let refs: Vec<&str> = keywords.iter().map(String::as_str).collect();
    let w = world_with_login(pool, &refs).await;
    for keyword in &keywords[..4] {
        w.mock_serp(keyword, 402, json!({ "x": 1 })).await;
    }
    w.mock_serp_for_all(json!([])).await;
    let run = w.run_ranks().await;
    let (status, error) = w.run_state(run).await;
    assert_eq!(status, "failed");
    assert_eq!(
        error.as_deref(),
        Some("Your DataForSEO balance has run out.")
    );
    // At most the four in flight were asked; the rest were left alone.
    assert!(w.serp_tasks().await.len() <= 4);
    let (_, settings) = w.get("/api/v1/settings").await;
    assert_eq!(settings["data"]["dataforseo"]["status"], "error");
    assert_eq!(
        settings["data"]["dataforseo"]["error"],
        "Your DataForSEO balance has run out."
    );
}

#[sqlx::test]
async fn a_rejected_login_marks_the_setting_with_a_plain_sentence(pool: PgPool) {
    let w = world_with_login(pool, &["a"]).await;
    w.mock_serp("a", 401, json!({ "status_message": df::PASSWORD }))
        .await;
    let run = w.run_ranks().await;
    let (status, error) = w.run_state(run).await;
    assert_eq!(status, "failed");
    assert_eq!(
        error.as_deref(),
        Some("DataForSEO rejected the login or password. Check them in Settings.")
    );
    let (_, settings) = w.get("/api/v1/settings").await;
    assert_eq!(settings["data"]["dataforseo"]["status"], "error");
}

#[sqlx::test]
async fn each_checked_keyword_is_counted_as_one_estimated_paid_call(pool: PgPool) {
    let w = world_with_login(pool, &["a", "b", "c"]).await;
    w.mock_serp("c", 500, json!({})).await;
    w.mock_serp_for_all(json!([])).await;
    w.run_ranks().await;
    let rows = w.usage_rows().await;
    let calls: i64 = rows.iter().map(|r| r.0).sum();
    let cost: f64 = rows.iter().filter_map(|r| r.1).sum();
    assert_eq!(calls, 2, "{rows:?}");
    assert!((cost - 0.044).abs() < 1e-9, "{cost}");
    let (_, usage) = w.get("/api/v1/usage?days=30").await;
    assert_eq!(usage["data"][0]["source"], "dataforseo");
    assert_eq!(usage["data"][0]["kind"], "serp");
}

#[sqlx::test]
async fn a_second_run_gives_a_previous_position_and_a_history(pool: PgPool) {
    let w = world_with_login(pool, &["blue widgets"]).await;
    w.mock_serp(
        "blue widgets",
        200,
        serp(json!([df::organic(9, "example.com")])),
    )
    .await;
    w.run_ranks().await;
    w.mock.reset().await;
    w.mock_serp(
        "blue widgets",
        200,
        serp(json!([df::organic(4, "example.com")])),
    )
    .await;
    w.run_ranks().await;
    let row = w.ranking_of("blue widgets").await;
    assert_eq!(row["position"], 4);
    assert_eq!(row["previous"], 9);
    let history: Vec<Value> = row["history"]
        .as_array()
        .unwrap()
        .iter()
        .map(|h| h["position"].clone())
        .collect();
    assert_eq!(history, vec![json!(9), json!(4)]);
}

#[sqlx::test]
async fn history_is_cut_to_the_latest_thirty(pool: PgPool) {
    let w = world_with_login(pool, &["blue widgets"]).await;
    for n in 0..35 {
        sqlx::query(
            "INSERT INTO rank_checks (id, workspace_id, site_id, keyword_id, keyword, country,
                 language, device, position, ai_overview, checked_at)
             VALUES (gen_random_uuid(), $1, $2, gen_random_uuid(), 'blue widgets', 'US', 'en',
                 'desktop', $3, 'absent', now() - make_interval(days => $4))",
        )
        .bind(w.workspace_id)
        .bind(w.site_id)
        .bind(n + 1)
        .bind(40 - n)
        .execute(&w.pool)
        .await
        .unwrap();
    }
    let row = w.ranking_of("blue widgets").await;
    let history = row["history"].as_array().unwrap();
    assert_eq!(history.len(), 30);
    assert_eq!(history[29]["position"], 35);
    assert_eq!(history[0]["position"], 6);
    assert_eq!(row["position"], 35);
    assert_eq!(row["previous"], 34);
}

#[sqlx::test]
async fn changing_the_country_starts_a_separate_history(pool: PgPool) {
    let w = world_with_login(pool, &["blue widgets"]).await;
    w.mock_serp_for_all(json!([df::organic(5, "example.com")]))
        .await;
    w.run_ranks().await;
    w.set_site(json!({ "country": "GB" })).await;
    // Nothing is checked for the new location yet.
    let before = w.rankings().await;
    assert_eq!(
        before["location"],
        json!({ "country": "GB", "language": "en", "device": "desktop" })
    );
    assert_eq!(before["checked_at"], Value::Null);
    assert_eq!(before["keywords"][0]["history"], json!([]));
    assert_eq!(before["keywords"][0]["position"], Value::Null);
    w.mock.reset().await;
    w.mock_serp_for_all(json!([df::organic(2, "example.com")]))
        .await;
    w.run_ranks().await;
    let gb = w.ranking_of("blue widgets").await;
    assert_eq!(gb["position"], 2);
    assert_eq!(
        gb["previous"],
        Value::Null,
        "the US check is not the previous one"
    );
    assert_eq!(gb["history"].as_array().unwrap().len(), 1);
    // Going back finds the old trend, untouched.
    w.set_site(json!({ "country": "US" })).await;
    let us = w.ranking_of("blue widgets").await;
    assert_eq!(us["position"], 5);
    assert_eq!(us["history"].as_array().unwrap().len(), 1);
}

#[sqlx::test]
async fn checked_at_changes_only_when_the_run_has_finished(pool: PgPool) {
    let w = world_with_login(pool, &["a", "b"]).await;
    assert_eq!(w.rankings().await["checked_at"], Value::Null);
    // A run in progress that has stored one keyword.
    sqlx::query(
        "INSERT INTO rank_runs (id, workspace_id, site_id, status, trigger, keywords_total)
         VALUES (gen_random_uuid(), $1, $2, 'running', 'manual', 2)",
    )
    .bind(w.workspace_id)
    .bind(w.site_id)
    .execute(&w.pool)
    .await
    .unwrap();
    let running = w.latest_run_id().await;
    sqlx::query(
        "INSERT INTO rank_checks (id, workspace_id, site_id, run_id, keyword_id, keyword, country,
             language, device, position, ai_overview)
         VALUES (gen_random_uuid(), $1, $2, $3, gen_random_uuid(), 'a', 'US', 'en', 'desktop', 3, 'absent')",
    )
    .bind(w.workspace_id)
    .bind(w.site_id)
    .bind(running)
    .execute(&w.pool)
    .await
    .unwrap();
    let mid = w.rankings().await;
    assert_eq!(
        mid["checked_at"],
        Value::Null,
        "a running run has not finished"
    );
    assert_eq!(mid["keywords"][0]["position"], 3, "its progress is visible");
    sqlx::query("UPDATE rank_runs SET status = 'completed', finished_at = now() WHERE id = $1")
        .bind(running)
        .execute(&w.pool)
        .await
        .unwrap();
    assert!(w.rankings().await["checked_at"].is_string());
}

#[sqlx::test]
async fn a_retried_job_resumes_its_run_and_skips_what_is_stored(pool: PgPool) {
    let w = world_with_login(pool, &["first", "second"]).await;
    w.mock_serp_for_all(json!([df::organic(6, "example.com")]))
        .await;
    let (status, _) = w.post(&w.rank_path("/check")).await;
    assert_eq!(status, 202);
    let run = w.latest_run_id().await;
    // The first attempt stored "first" and then died.
    sqlx::query("UPDATE rank_runs SET status = 'running', started_at = now() WHERE id = $1")
        .bind(run)
        .execute(&w.pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO rank_checks (id, workspace_id, site_id, run_id, keyword_id, keyword, country,
             language, device, position, ai_overview)
         VALUES (gen_random_uuid(), $1, $2, $3, gen_random_uuid(), 'first', 'US', 'en', 'desktop', 8, 'absent')",
    )
    .bind(w.workspace_id)
    .bind(w.site_id)
    .bind(run)
    .execute(&w.pool)
    .await
    .unwrap();
    w.rank_handler().run(&w.rank_job(Some(run))).await.unwrap();
    let asked: Vec<String> = w
        .serp_tasks()
        .await
        .iter()
        .map(|t| t["keyword"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(asked, vec!["second"]);
    assert_eq!(w.run_state(run).await.0, "completed");
    assert_eq!(w.ranking_of("first").await["position"], 8);
    assert_eq!(w.ranking_of("second").await["position"], 6);
}

#[sqlx::test]
async fn a_job_that_dies_for_good_fails_its_run_so_the_next_one_can_start(pool: PgPool) {
    let w = world_with_login(pool, &["a"]).await;
    let (status, _) = w.post(&w.rank_path("/check")).await;
    assert_eq!(status, 202);
    let run = w.latest_run_id().await;
    w.rank_handler()
        .on_dead(&w.rank_job(Some(run)), "boom")
        .await;
    let (state, error) = w.run_state(run).await;
    assert_eq!(state, "failed");
    assert_eq!(
        error.as_deref(),
        Some("The check stopped because of an internal error. Try again.")
    );
    let (status, _) = w.post(&w.rank_path("/check")).await;
    assert_eq!(status, 202);
}

#[sqlx::test]
async fn a_login_removed_before_the_job_runs_means_no_call(pool: PgPool) {
    let w = world_with_login(pool, &["a"]).await;
    let (status, _) = w.post(&w.rank_path("/check")).await;
    assert_eq!(status, 202);
    let run = w.latest_run_id().await;
    let (status, _) = w
        .send(
            "test-token",
            reqwest::Method::DELETE,
            "/api/v1/settings/dataforseo",
            None,
        )
        .await;
    assert_eq!(status, 200);
    let before = w.dataforseo_requests().await;
    w.rank_handler().run(&w.rank_job(Some(run))).await.unwrap();
    assert_eq!(w.dataforseo_requests().await, before);
    let (state, error) = w.run_state(run).await;
    assert_eq!(state, "failed");
    assert!(error.unwrap().contains("Settings"));
}
