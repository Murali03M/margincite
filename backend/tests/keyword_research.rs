//! Search volume and keyword ideas over the API and the volumes job.

mod common;

use common::dataforseo::{self as df, ads_item, IDEAS_PATH, VOLUME_PATH};
use common::google::{add_site, workspace_of, GoogleWorld};
use margincite::integrations::dataforseo::DataForSeoClient;
use margincite::jobs::queue::Job;
use margincite::jobs::worker::JobHandler;
use margincite::keywords::volumes::KeywordVolumesJob;
use serde_json::{json, Value};
use sqlx::PgPool;
use std::sync::Arc;
use uuid::Uuid;
use wiremock::matchers::{method, path};
use wiremock::{Mock, ResponseTemplate};

async fn world_with_login(pool: PgPool, keywords: &[&str]) -> GoogleWorld {
    let w = GoogleWorld::new(pool).await;
    w.save_dataforseo().await;
    for keyword in keywords {
        w.add_keyword(keyword).await;
    }
    w
}

async fn mock(w: &GoogleWorld, at: &str, status: u16, body: Value) {
    Mock::given(method("POST"))
        .and(path(at))
        .respond_with(ResponseTemplate::new(status).set_body_json(body))
        .mount(&w.mock)
        .await;
}

fn volumes_job(w: &GoogleWorld) -> (KeywordVolumesJob, Job) {
    let config = Arc::new(common::test_config(common::GOOGLE_SETTINGS));
    let client = Arc::new(DataForSeoClient::new(&common::rankings::dataforseo_base(
        &w.mock,
    )));
    let job = Job {
        id: Uuid::new_v4(),
        kind: "keyword_volumes".into(),
        payload: json!({ "workspace_id": w.workspace_id, "site_id": w.site_id }),
        attempts: 1,
    };
    (KeywordVolumesJob::new(w.pool.clone(), config, client), job)
}

async fn keywords(w: &GoogleWorld) -> Vec<Value> {
    let (status, body) = w
        .get(&format!("/api/v1/sites/{}/keywords", w.site_id))
        .await;
    assert_eq!(status, 200, "{body}");
    body["data"].as_array().unwrap().clone()
}

fn volume_path(w: &GoogleWorld) -> String {
    format!("/api/v1/sites/{}/keywords/volumes", w.site_id)
}

fn ideas_path(w: &GoogleWorld) -> String {
    format!("/api/v1/sites/{}/keywords/ideas", w.site_id)
}

async fn ideas(w: &GoogleWorld, seed: &str) -> (u16, Value) {
    w.send(
        "test-token",
        reqwest::Method::POST,
        &ideas_path(w),
        Some(json!({ "seed": seed })),
    )
    .await
}

async fn requests(w: &GoogleWorld, at: &str) -> Vec<Value> {
    w.requests_to(at)
        .await
        .iter()
        .map(|r| serde_json::from_slice::<Value>(&r.body).unwrap()[0].clone())
        .collect()
}

// ----------------------------------------------------------------- volumes

#[sqlx::test]
async fn volumes_are_stored_and_shown_on_the_keywords_list(pool: PgPool) {
    let w = world_with_login(pool, &["Blue Widgets", "rare thing", "never answered"]).await;
    // The provider lowercases what it sends back.
    mock(
        &w,
        VOLUME_PATH,
        200,
        df::answer(json!([
            ads_item("blue widgets", Some(1900), Some("HIGH"), Some(1.25)),
            ads_item("rare thing", None, None, None),
        ])),
    )
    .await;
    let before = keywords(&w).await;
    assert!(before
        .iter()
        .all(|k| k["search_volume"].is_null() && k["competition"].is_null() && k["cpc"].is_null()));
    let (job, queued) = volumes_job(&w);
    job.run(&queued).await.unwrap();
    let after = keywords(&w).await;
    let by = |name: &str| after.iter().find(|k| k["keyword"] == name).unwrap().clone();
    assert_eq!(by("Blue Widgets")["search_volume"], 1900);
    assert_eq!(by("Blue Widgets")["competition"], "HIGH");
    assert_eq!(by("Blue Widgets")["cpc"], 1.25);
    assert_eq!(
        by("rare thing")["search_volume"],
        Value::Null,
        "no data stays null, not zero"
    );
    assert_eq!(by("rare thing")["cpc"], Value::Null);
    assert_eq!(by("never answered")["search_volume"], Value::Null);
    let asked = requests(&w, VOLUME_PATH).await;
    assert_eq!(asked.len(), 1, "one request refreshes every keyword");
    assert_eq!(
        asked[0],
        json!({ "keywords": ["Blue Widgets", "rare thing", "never answered"],
                "location_code": 2840, "language_code": "en" })
    );
}

#[sqlx::test]
async fn volumes_use_the_sites_country_and_language(pool: PgPool) {
    let w = world_with_login(pool, &["widgets"]).await;
    w.set_site(json!({ "country": "IN", "language": "hi" }))
        .await;
    mock(&w, VOLUME_PATH, 200, df::answer(json!([]))).await;
    let (job, queued) = volumes_job(&w);
    job.run(&queued).await.unwrap();
    let asked = requests(&w, VOLUME_PATH).await;
    assert_eq!(asked[0]["location_code"], 2356);
    assert_eq!(asked[0]["language_code"], "hi");
}

#[sqlx::test]
async fn a_keyword_the_provider_would_refuse_is_left_out_and_does_not_fail_the_rest(pool: PgPool) {
    let long = "x".repeat(90);
    let w = world_with_login(pool, &["fine", &long]).await;
    mock(
        &w,
        VOLUME_PATH,
        200,
        df::answer(json!([ads_item("fine", Some(5), Some("LOW"), Some(0.2))])),
    )
    .await;
    let (job, queued) = volumes_job(&w);
    job.run(&queued).await.unwrap();
    let asked = requests(&w, VOLUME_PATH).await;
    assert_eq!(asked[0]["keywords"], json!(["fine"]));
    let all = keywords(&w).await;
    assert_eq!(
        all.iter().find(|k| k["keyword"] == "fine").unwrap()["search_volume"],
        5
    );
}

#[sqlx::test]
async fn one_volume_request_is_counted_as_one_estimated_paid_call(pool: PgPool) {
    let w = world_with_login(pool, &["a", "b", "c"]).await;
    mock(
        &w,
        VOLUME_PATH,
        200,
        df::answer(json!([ads_item("a", Some(1), None, None)])),
    )
    .await;
    let (job, queued) = volumes_job(&w);
    job.run(&queued).await.unwrap();
    let rows = w.usage_rows().await;
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].0, 1);
    assert!((rows[0].1.unwrap() - 0.09).abs() < 1e-9);
    let (_, usage) = w.get("/api/v1/usage?days=30").await;
    assert_eq!(usage["data"][0]["kind"], "keyword_volume");
}

#[sqlx::test]
async fn a_rate_limited_request_fails_the_job_so_the_queue_retries_it(pool: PgPool) {
    let w = world_with_login(pool, &["a"]).await;
    for (status, body) in [
        (429, json!({})),
        (
            200,
            df::task_failed(40202, "the rate-limit per minute has been exceeded."),
        ),
        (500, json!({})),
    ] {
        w.mock.reset().await;
        mock(&w, VOLUME_PATH, status, body).await;
        let (job, queued) = volumes_job(&w);
        assert!(job.run(&queued).await.is_err(), "{status}");
    }
    assert!(keywords(&w).await[0]["search_volume"].is_null());
    assert_eq!(
        w.usage_rows().await.len(),
        0,
        "nothing was answered, nothing is counted"
    );
}

#[sqlx::test]
async fn a_balance_that_ran_out_ends_the_job_and_marks_the_setting(pool: PgPool) {
    let w = world_with_login(pool, &["a"]).await;
    sqlx::query("UPDATE site_keywords SET search_volume = 77, competition = 'LOW', cpc = 0.3")
        .execute(&w.pool)
        .await
        .unwrap();
    mock(&w, VOLUME_PATH, 402, json!({ "x": 1 })).await;
    let (job, queued) = volumes_job(&w);
    job.run(&queued).await.unwrap();
    let kept = keywords(&w).await;
    assert_eq!(kept[0]["search_volume"], 77, "old data is kept");
    let (_, settings) = w.get("/api/v1/settings").await;
    assert_eq!(settings["data"]["dataforseo"]["status"], "error");
    assert_eq!(
        settings["data"]["dataforseo"]["error"],
        "Your DataForSEO balance has run out."
    );
}

#[sqlx::test]
async fn a_job_without_a_login_makes_no_call(pool: PgPool) {
    let w = GoogleWorld::new(pool).await;
    w.add_keyword("a").await;
    mock(
        &w,
        VOLUME_PATH,
        200,
        df::answer(json!([ads_item("a", Some(1), None, None)])),
    )
    .await;
    let (job, queued) = volumes_job(&w);
    job.run(&queued).await.unwrap();
    assert!(requests(&w, VOLUME_PATH).await.is_empty());
    assert!(keywords(&w).await[0]["search_volume"].is_null());
}

#[sqlx::test]
async fn refreshing_volumes_is_accepted_queued_once_and_needs_a_login(pool: PgPool) {
    let w = GoogleWorld::new(pool).await;
    w.add_keyword("a").await;
    let (status, body) = w.post(&volume_path(&w)).await;
    assert_eq!(status, 422);
    assert_eq!(
        body["error"],
        "Search volume needs a DataForSEO login. Add it in Settings, then try again."
    );
    w.save_dataforseo().await;
    sqlx::query("DELETE FROM jobs")
        .execute(&w.pool)
        .await
        .unwrap();
    for _ in 0..3 {
        let (status, body) = w.post(&volume_path(&w)).await;
        assert_eq!(status, 202, "{body}");
        assert_eq!(body["data"]["queued"], true);
    }
    let queued: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM jobs WHERE kind = 'keyword_volumes' AND status = 'queued'",
    )
    .fetch_one(&w.pool)
    .await
    .unwrap();
    assert_eq!(queued, 1);
    assert!(
        requests(&w, VOLUME_PATH).await.is_empty(),
        "nothing is asked until the job runs"
    );
}

#[sqlx::test]
async fn refreshing_volumes_without_keywords_is_refused(pool: PgPool) {
    let w = world_with_login(pool, &[]).await;
    let (status, body) = w.post(&volume_path(&w)).await;
    assert_eq!(status, 422);
    assert_eq!(
        body["error"],
        "Add at least one keyword before refreshing search volume."
    );
}

#[sqlx::test]
async fn another_workspace_cannot_refresh_or_search(pool: PgPool) {
    let w = world_with_login(pool, &["a"]).await;
    for (path, body) in [
        (volume_path(&w), None),
        (ideas_path(&w), Some(json!({ "seed": "x" }))),
    ] {
        let (status, _) = w
            .send("other-token", reqwest::Method::POST, &path, body)
            .await;
        assert_eq!(status, 404, "{path}");
    }
    assert_eq!(w.dataforseo_requests().await, 1, "only the login test");
}

#[sqlx::test]
async fn the_volumes_job_never_touches_another_workspaces_keywords(pool: PgPool) {
    let w = world_with_login(pool, &["mine"]).await;
    let other_ws = workspace_of(&w.app, "other-token").await;
    let other_site = add_site(&w.pool, other_ws, "https://other.example.org/").await;
    sqlx::query("INSERT INTO site_keywords (id, workspace_id, site_id, keyword) VALUES (gen_random_uuid(), $1, $2, 'mine')")
        .bind(other_ws)
        .bind(other_site)
        .execute(&w.pool)
        .await
        .unwrap();
    mock(
        &w,
        VOLUME_PATH,
        200,
        df::answer(json!([ads_item("mine", Some(9), Some("LOW"), Some(1.0))])),
    )
    .await;
    let (job, queued) = volumes_job(&w);
    job.run(&queued).await.unwrap();
    let theirs: Option<i32> =
        sqlx::query_scalar("SELECT search_volume FROM site_keywords WHERE site_id = $1")
            .bind(other_site)
            .fetch_one(&w.pool)
            .await
            .unwrap();
    assert_eq!(theirs, None);
}

// ------------------------------------------------------------------- ideas

#[sqlx::test]
async fn ideas_come_back_sorted_by_volume_with_unknowns_last_and_nothing_is_saved(pool: PgPool) {
    let w = world_with_login(pool, &["existing"]).await;
    mock(
        &w,
        IDEAS_PATH,
        200,
        df::answer(json!([
            ads_item("small", Some(10), Some("LOW"), Some(0.1)),
            ads_item("unknown", None, None, None),
            ads_item("big", Some(9000), Some("HIGH"), Some(2.5)),
            ads_item("Big", Some(9000), Some("HIGH"), Some(2.5)),
            ads_item("medium", Some(500), Some("MEDIUM"), None),
        ])),
    )
    .await;
    let (status, body) = ideas(&w, "  widgets  ").await;
    assert_eq!(status, 200, "{body}");
    let found = body["data"]["ideas"].as_array().unwrap();
    let names: Vec<&str> = found
        .iter()
        .map(|i| i["keyword"].as_str().unwrap())
        .collect();
    assert_eq!(names, vec!["big", "medium", "small", "unknown"]);
    assert_eq!(
        found[0],
        json!({ "keyword": "big", "search_volume": 9000, "competition": "HIGH", "cpc": 2.5 })
    );
    assert_eq!(
        found[3],
        json!({ "keyword": "unknown", "search_volume": null, "competition": null, "cpc": null })
    );
    let asked = requests(&w, IDEAS_PATH).await;
    assert_eq!(asked[0]["keywords"], json!(["widgets"]));
    assert_eq!(asked[0]["location_code"], 2840);
    assert_eq!(keywords(&w).await.len(), 1, "nothing is saved");
}

#[sqlx::test]
async fn ideas_are_cut_to_fifty(pool: PgPool) {
    let w = world_with_login(pool, &[]).await;
    let items: Vec<Value> = (0..80)
        .map(|n| {
            ads_item(
                &format!("idea {n:02}"),
                Some(1000 - n),
                Some("LOW"),
                Some(0.5),
            )
        })
        .collect();
    mock(&w, IDEAS_PATH, 200, df::answer(json!(items))).await;
    let (_, body) = ideas(&w, "idea").await;
    let found = body["data"]["ideas"].as_array().unwrap();
    assert_eq!(found.len(), 50);
    assert_eq!(found[0]["keyword"], "idea 00");
    assert_eq!(found[49]["search_volume"], 951);
}

#[sqlx::test]
async fn the_seed_is_validated_before_anything_is_spent(pool: PgPool) {
    let w = world_with_login(pool, &[]).await;
    mock(&w, IDEAS_PATH, 200, df::answer(json!([]))).await;
    for seed in ["", "   ", "!!!", &"x".repeat(81), "a b c d e f g h i j k"] {
        let (status, _) = ideas(&w, seed).await;
        assert_eq!(status, 422, "{seed:?}");
    }
    let (status, _) = w
        .send(
            "test-token",
            reqwest::Method::POST,
            &ideas_path(&w),
            Some(json!({})),
        )
        .await;
    assert_eq!(status, 422);
    assert!(requests(&w, IDEAS_PATH).await.is_empty());
    let used: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM keyword_idea_requests")
        .fetch_one(&w.pool)
        .await
        .unwrap();
    assert_eq!(used, 0);
}

#[sqlx::test]
async fn ideas_need_a_login_and_ask_nothing_without_one(pool: PgPool) {
    let w = GoogleWorld::new(pool).await;
    mock(&w, IDEAS_PATH, 200, df::answer(json!([]))).await;
    let (status, body) = ideas(&w, "widgets").await;
    assert_eq!(status, 422);
    assert_eq!(
        body["error"],
        "Keyword ideas need a DataForSEO login. Add it in Settings, then try again."
    );
    assert!(requests(&w, IDEAS_PATH).await.is_empty());
    let used: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM keyword_idea_requests")
        .fetch_one(&w.pool)
        .await
        .unwrap();
    assert_eq!(used, 0, "a refusal uses up nothing");
}

#[sqlx::test]
async fn the_twenty_first_idea_search_of_a_day_is_429_for_the_whole_workspace(pool: PgPool) {
    let w = world_with_login(pool, &[]).await;
    mock(
        &w,
        IDEAS_PATH,
        200,
        df::answer(json!([ads_item("a", Some(1), None, None)])),
    )
    .await;
    for n in 0..20 {
        let (status, body) = ideas(&w, "widgets").await;
        assert_eq!(status, 200, "{n}: {body}");
    }
    let (status, body) = ideas(&w, "widgets").await;
    assert_eq!(status, 429);
    assert_eq!(
        body["error"],
        "You have searched for keyword ideas 20 times today. Try again tomorrow."
    );
    assert_eq!(requests(&w, IDEAS_PATH).await.len(), 20);
    // Across the workspace's sites, and not given back by deleting a site.
    let second = add_site(&w.pool, w.workspace_id, "https://second.example.net/").await;
    let (status, _) = w
        .send(
            "test-token",
            reqwest::Method::POST,
            &format!("/api/v1/sites/{second}/keywords/ideas"),
            Some(json!({ "seed": "x" })),
        )
        .await;
    assert_eq!(status, 429);
    sqlx::query("DELETE FROM sites WHERE id = $1")
        .bind(w.site_id)
        .execute(&w.pool)
        .await
        .unwrap();
    let (status, _) = w
        .send(
            "test-token",
            reqwest::Method::POST,
            &format!("/api/v1/sites/{second}/keywords/ideas"),
            Some(json!({ "seed": "x" })),
        )
        .await;
    assert_eq!(status, 429);
    // Yesterday's searches do not count.
    sqlx::query("UPDATE keyword_idea_requests SET created_at = now() - interval '2 days'")
        .execute(&w.pool)
        .await
        .unwrap();
    let (status, _) = ideas_for(&w, second).await;
    assert_eq!(status, 200);
}

async fn ideas_for(w: &GoogleWorld, site: Uuid) -> (u16, Value) {
    w.send(
        "test-token",
        reqwest::Method::POST,
        &format!("/api/v1/sites/{site}/keywords/ideas"),
        Some(json!({ "seed": "x" })),
    )
    .await
}

#[sqlx::test]
async fn an_idea_search_is_counted_as_one_estimated_paid_call(pool: PgPool) {
    let w = world_with_login(pool, &[]).await;
    mock(
        &w,
        IDEAS_PATH,
        200,
        df::answer(json!([ads_item("a", Some(1), None, None)])),
    )
    .await;
    ideas(&w, "widgets").await;
    let rows = w.usage_rows().await;
    assert_eq!(rows.len(), 1);
    assert!((rows[0].1.unwrap() - 0.09).abs() < 1e-9);
    let (_, usage) = w.get("/api/v1/usage?days=30").await;
    assert_eq!(usage["data"][0]["kind"], "keyword_ideas");
}

#[sqlx::test]
async fn provider_trouble_becomes_a_plain_sentence_and_marks_a_dead_login(pool: PgPool) {
    let w = world_with_login(pool, &[]).await;
    for (status, body, expected) in [
        (
            429,
            json!({}),
            "DataForSEO is limiting requests right now. Try again later.",
        ),
        (500, json!({}), "DataForSEO could not answer this request."),
        (
            200,
            df::task_failed(40200, "payment required."),
            "Your DataForSEO balance has run out.",
        ),
        (
            200,
            json!({ "nope": 1 }),
            "DataForSEO replied in a format Margincite does not recognise.",
        ),
    ] {
        w.mock.reset().await;
        mock(&w, IDEAS_PATH, status, body).await;
        let (code, reply) = ideas(&w, "widgets").await;
        assert_eq!(code, 502);
        assert_eq!(reply["error"], expected);
    }
    let (_, settings) = w.get("/api/v1/settings").await;
    assert_eq!(settings["data"]["dataforseo"]["status"], "error");
    assert_eq!(
        settings["data"]["dataforseo"]["error"],
        "Your DataForSEO balance has run out."
    );
}
