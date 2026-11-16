mod common;

use common::bing::{BING_KEY, SITE_URL};
use common::google::GoogleWorld;
use serde_json::{json, Value};
use sqlx::PgPool;

const NOT_CONNECTED: &str = "Connect Bing Webmaster Tools to see backlinks.";

async fn metrics(w: &GoogleWorld) -> Value {
    let (status, body) = w.get(&format!("/api/v1/sites/{}/metrics", w.site_id)).await;
    assert_eq!(status, 200, "{body}");
    body["data"]["backlinks"].clone()
}

async fn seed_targets(w: &GoogleWorld, rows: &[(&str, i64)]) {
    for (url, count) in rows {
        sqlx::query(
            "INSERT INTO backlink_targets (workspace_id, site_id, target_url, count) VALUES ($1, $2, $3, $4)",
        )
        .bind(w.workspace_id)
        .bind(w.site_id)
        .bind(url)
        .bind(count)
        .execute(&w.pool)
        .await
        .unwrap();
    }
}

async fn seed_source(w: &GoogleWorld, target: &str, source: &str, anchor: &str) {
    sqlx::query(
        "INSERT INTO backlink_sources (workspace_id, site_id, target_url, source_url, anchor_text) VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(w.workspace_id)
    .bind(w.site_id)
    .bind(target)
    .bind(source)
    .bind(anchor)
    .execute(&w.pool)
    .await
    .unwrap();
}

// ---------------------------------------------------------------- connecting

#[sqlx::test]
async fn a_key_whose_account_lacks_the_site_is_422_with_a_plain_reason(pool: PgPool) {
    let w = GoogleWorld::new(pool).await;
    w.mock_bing_sites(&["https://other.example/"]).await;
    let (status, body) = w.put_bing(BING_KEY, SITE_URL).await;
    assert_eq!(status, 422, "{body}");
    assert_eq!(
        body["error"],
        "That site is not in this Bing Webmaster Tools account. Add and verify it there first."
    );
    assert_eq!(w.bing_count("bing_connections").await, 0);
    assert_eq!(w.bing_count("jobs").await, 0);
}

#[sqlx::test]
async fn a_key_bing_refuses_is_422_and_stores_nothing(pool: PgPool) {
    let w = GoogleWorld::new(pool).await;
    for status in [401, 403] {
        w.mock.reset().await;
        w.mock_bing("GetUserSites", &[], status, json!({})).await;
        let (code, body) = w.put_bing(BING_KEY, SITE_URL).await;
        assert_eq!(code, 422, "{status}: {body}");
        assert!(body["error"].as_str().unwrap().contains("API key"));
    }
    w.mock.reset().await;
    w.mock_bing(
        "GetUserSites",
        &[],
        400,
        json!({ "ErrorCode": 14, "Message": "ERROR!!! InvalidApiKey" }),
    )
    .await;
    let (code, _) = w.put_bing(BING_KEY, SITE_URL).await;
    assert_eq!(code, 422);
    assert_eq!(w.bing_count("bing_connections").await, 0);
}

#[sqlx::test]
async fn bing_being_down_while_connecting_is_a_502(pool: PgPool) {
    let w = GoogleWorld::new(pool).await;
    w.mock_bing("GetUserSites", &[], 503, json!({})).await;
    let (status, _) = w.put_bing(BING_KEY, SITE_URL).await;
    assert_eq!(status, 502);
    assert_eq!(w.bing_count("bing_connections").await, 0);
}

#[sqlx::test]
async fn a_good_key_stores_a_sealed_key_and_queues_a_sync(pool: PgPool) {
    let w = GoogleWorld::new(pool).await;
    w.mock_bing_sites(&["https://example.com/"]).await;
    let (status, body) = w.put_bing(BING_KEY, "https://example.com/").await;
    assert_eq!(status, 200, "{body}");
    assert_eq!(
        body["data"],
        json!({ "status": "connected", "site_url": "https://example.com/", "synced_at": null, "error": null })
    );
    assert!(!body.to_string().contains(BING_KEY));
    let sealed: String =
        sqlx::query_scalar("SELECT api_key_sealed FROM bing_connections WHERE site_id = $1")
            .bind(w.site_id)
            .fetch_one(&w.pool)
            .await
            .unwrap();
    assert!(!sealed.contains(BING_KEY));
    assert_eq!(common::google::secrets().open(&sealed).unwrap(), BING_KEY);
    assert_eq!(w.bing_count("jobs").await, 1);

    let requests = w.bing_requests().await;
    assert_eq!(requests[0].url.path(), "/bing/GetUserSites");
    assert_eq!(
        requests[0]
            .url
            .query_pairs()
            .find(|(k, _)| k == "apikey")
            .map(|(_, v)| v.into_owned()),
        Some(BING_KEY.to_string())
    );

    let (status, body) = w.get(&w.bing_path("")).await;
    assert_eq!(status, 200);
    assert_eq!(body["data"]["status"], "connected");
    assert!(!body.to_string().contains(BING_KEY));
}

#[sqlx::test]
async fn connecting_again_replaces_the_key_and_drops_rows_of_another_site_url(pool: PgPool) {
    let w = GoogleWorld::new(pool).await;
    w.insert_bing("needs_reconnecting").await;
    seed_targets(&w, &[("https://example.com/a", 4)]).await;
    w.mock_bing_sites(&["https://www.example.com/", "https://example.com/"])
        .await;
    let (status, body) = w.put_bing("another-key", "https://www.example.com/").await;
    assert_eq!(status, 200, "{body}");
    assert_eq!(body["data"]["site_url"], "https://www.example.com/");
    assert_eq!(body["data"]["status"], "connected");
    assert_eq!(w.bing_count("bing_connections").await, 1);
    assert_eq!(w.bing_count("backlink_targets").await, 0);
}

#[sqlx::test]
async fn the_site_url_must_be_http_and_on_the_sites_host(pool: PgPool) {
    let w = GoogleWorld::new(pool).await;
    w.mock_bing_sites(&["https://example.com/", "https://evil.example/"])
        .await;
    for bad in [
        "",
        "not a url",
        "ftp://example.com/",
        "javascript:alert(1)",
        "https://evil.example/",
        "https://example.com.evil.example/",
        "https://user:pass@example.com/",
        "https://sub.example.com/",
    ] {
        let (status, body) = w.put_bing(BING_KEY, bad).await;
        assert_eq!(status, 422, "{bad}: {body}");
    }
    assert_eq!(w.bing_count("bing_connections").await, 0);
    assert!(
        w.bing_requests().await.is_empty(),
        "Bing is not asked about a bad address"
    );
    // A leading www. is ignored when comparing hosts.
    w.mock.reset().await;
    w.mock_bing_sites(&["https://www.example.com/"]).await;
    let (status, body) = w.put_bing(BING_KEY, "https://www.example.com/").await;
    assert_eq!(status, 200, "{body}");
}

#[sqlx::test]
async fn an_empty_key_or_one_that_is_too_long_is_422(pool: PgPool) {
    let w = GoogleWorld::new(pool).await;
    for key in ["", "   ", &"k".repeat(300)] {
        let (status, _) = w.put_bing(key, SITE_URL).await;
        assert_eq!(status, 422);
    }
    assert!(w.bing_requests().await.is_empty());
}

#[sqlx::test]
async fn connecting_is_422_without_the_secret_key(pool: PgPool) {
    let w = GoogleWorld::with_settings(pool, &[]).await;
    let (status, body) = w.put_bing(BING_KEY, SITE_URL).await;
    assert_eq!(status, 422, "{body}");
    assert!(body["error"]
        .as_str()
        .unwrap()
        .contains("MARGINCITE_SECRET_KEY"));
    assert_eq!(w.bing_count("bing_connections").await, 0);
    assert!(w.bing_requests().await.is_empty());
}

// ---------------------------------------------------------------- status, sync, disconnect

#[sqlx::test]
async fn status_walks_from_not_connected_to_needs_reconnecting(pool: PgPool) {
    let w = GoogleWorld::new(pool).await;
    let (_, body) = w.get(&w.bing_path("")).await;
    assert_eq!(
        body["data"],
        json!({ "status": "not_connected", "site_url": null, "synced_at": null, "error": null })
    );
    w.insert_bing("needs_reconnecting").await;
    sqlx::query("UPDATE bing_connections SET error = 'Bing no longer accepts this API key.'")
        .execute(&w.pool)
        .await
        .unwrap();
    let (_, body) = w.get(&w.bing_path("")).await;
    assert_eq!(body["data"]["status"], "needs_reconnecting");
    assert_eq!(
        body["data"]["error"],
        "Bing no longer accepts this API key."
    );
}

#[sqlx::test]
async fn sync_now_is_202_and_queues_one_job(pool: PgPool) {
    let w = GoogleWorld::new(pool).await;
    w.insert_bing("connected").await;
    for _ in 0..2 {
        let (status, body) = w.post(&w.bing_path("/sync")).await;
        assert_eq!(status, 202, "{body}");
        assert_eq!(body["data"]["status"], "connected");
    }
    assert_eq!(w.bing_count("jobs").await, 1);
}

#[sqlx::test]
async fn sync_now_needs_a_working_connection(pool: PgPool) {
    let w = GoogleWorld::new(pool).await;
    let (status, _) = w.post(&w.bing_path("/sync")).await;
    assert_eq!(status, 422);
    w.insert_bing("needs_reconnecting").await;
    let (status, _) = w.post(&w.bing_path("/sync")).await;
    assert_eq!(status, 422);
    assert_eq!(w.bing_count("jobs").await, 0);
}

#[sqlx::test]
async fn disconnecting_deletes_the_key_and_every_row(pool: PgPool) {
    let w = GoogleWorld::new(pool).await;
    w.insert_bing("connected").await;
    seed_targets(&w, &[("https://example.com/a", 4)]).await;
    seed_source(&w, "https://example.com/a", "https://blog.example/x", "a").await;
    let (status, body) = w
        .send(
            "test-token",
            reqwest::Method::DELETE,
            &w.bing_path(""),
            None,
        )
        .await;
    assert_eq!(status, 200);
    assert_eq!(body["data"]["status"], "not_connected");
    for table in ["bing_connections", "backlink_targets", "backlink_sources"] {
        assert_eq!(w.bing_count(table).await, 0, "{table}");
    }
    assert_eq!(metrics(&w).await["reason"], NOT_CONNECTED);
}

// ---------------------------------------------------------------- reading backlinks

#[sqlx::test]
async fn backlinks_are_paged_most_linked_first(pool: PgPool) {
    let w = GoogleWorld::new(pool).await;
    seed_targets(
        &w,
        &[
            ("https://example.com/a", 4),
            ("https://example.com/b", 9),
            ("https://example.com/c", 1),
        ],
    )
    .await;
    let path = format!("/api/v1/sites/{}/backlinks?page=1&limit=2", w.site_id);
    let (status, body) = w.get(&path).await;
    assert_eq!(status, 200);
    assert_eq!(
        body["data"],
        json!([
            { "target_url": "https://example.com/b", "count": 9 },
            { "target_url": "https://example.com/a", "count": 4 }
        ])
    );
    assert_eq!(body["meta"], json!({ "total": 3, "page": 1, "limit": 2 }));
    let (_, second) = w
        .get(&format!(
            "/api/v1/sites/{}/backlinks?page=2&limit=2",
            w.site_id
        ))
        .await;
    assert_eq!(second["data"][0]["target_url"], "https://example.com/c");
}

#[sqlx::test]
async fn sources_are_listed_for_one_target(pool: PgPool) {
    let w = GoogleWorld::new(pool).await;
    seed_source(
        &w,
        "https://example.com/a",
        "https://blog.example/x",
        "widgets",
    )
    .await;
    seed_source(&w, "https://example.com/a", "https://blog.example/y", "").await;
    seed_source(
        &w,
        "https://example.com/b",
        "https://news.example/z",
        "other",
    )
    .await;
    let base = format!("/api/v1/sites/{}/backlinks/sources", w.site_id);
    let (status, body) = w
        .get(&format!("{base}?target_url=https%3A%2F%2Fexample.com%2Fa"))
        .await;
    assert_eq!(status, 200, "{body}");
    assert_eq!(
        body["data"],
        json!([
            { "source_url": "https://blog.example/x", "anchor_text": "widgets" },
            { "source_url": "https://blog.example/y", "anchor_text": "" }
        ])
    );
    let (_, none) = w
        .get(&format!(
            "{base}?target_url=https%3A%2F%2Fexample.com%2Fnope"
        ))
        .await;
    assert_eq!(none["data"], json!([]));
    let (status, _) = w.get(&base).await;
    assert_eq!(status, 422, "target_url is required");
}

#[sqlx::test]
async fn backlink_endpoints_are_empty_without_a_connection(pool: PgPool) {
    let w = GoogleWorld::new(pool).await;
    let (_, body) = w
        .get(&format!("/api/v1/sites/{}/backlinks", w.site_id))
        .await;
    assert_eq!(body["data"], json!([]));
    assert_eq!(body["meta"]["total"], 0);
}

#[sqlx::test]
async fn another_workspace_cannot_use_any_bing_endpoint(pool: PgPool) {
    let w = GoogleWorld::new(pool).await;
    w.insert_bing("connected").await;
    seed_targets(&w, &[("https://example.com/a", 4)]).await;
    seed_source(&w, "https://example.com/a", "https://blog.example/x", "a").await;
    for (method, path) in [
        (reqwest::Method::GET, w.bing_path("")),
        (reqwest::Method::PUT, w.bing_path("")),
        (reqwest::Method::DELETE, w.bing_path("")),
        (reqwest::Method::POST, w.bing_path("/sync")),
        (
            reqwest::Method::GET,
            format!("/api/v1/sites/{}/backlinks", w.site_id),
        ),
        (
            reqwest::Method::GET,
            format!(
                "/api/v1/sites/{}/backlinks/sources?target_url=https%3A%2F%2Fexample.com%2Fa",
                w.site_id
            ),
        ),
    ] {
        let (status, body) = w
            .send(
                "other-token",
                method.clone(),
                &path,
                Some(json!({ "api_key": BING_KEY, "site_url": SITE_URL })),
            )
            .await;
        assert_eq!(status, 404, "{method} {path}: {body}");
    }
    assert_eq!(w.bing_count("bing_connections").await, 1);
    assert_eq!(w.bing_count("backlink_targets").await, 1);
    let (_, other) = w
        .send("other-token", reqwest::Method::GET, "/api/v1/sites", None)
        .await;
    assert_eq!(other["data"], json!([]));
}

// ---------------------------------------------------------------- metrics

#[sqlx::test]
async fn backlinks_metrics_say_why_without_a_connection(pool: PgPool) {
    let w = GoogleWorld::new(pool).await;
    let block = metrics(&w).await;
    assert_eq!(block["total"], Value::Null);
    assert_eq!(block["linked_pages"], Value::Null);
    assert_eq!(block["top_pages"], json!([]));
    assert_eq!(block["source"], "bing");
    assert_eq!(block["reason"], NOT_CONNECTED);
}

#[sqlx::test]
async fn backlinks_metrics_wait_for_the_first_sync(pool: PgPool) {
    let w = GoogleWorld::new(pool).await;
    w.insert_bing("connected").await;
    let block = metrics(&w).await;
    assert_eq!(block["total"], Value::Null);
    assert_eq!(block["synced_at"], Value::Null);
    assert_eq!(
        block["reason"],
        "Bing Webmaster Tools is connected. The first sync has not finished yet."
    );
}

#[sqlx::test]
async fn backlinks_metrics_sum_counts_and_list_the_top_five(pool: PgPool) {
    let w = GoogleWorld::new(pool).await;
    w.insert_bing("connected").await;
    sqlx::query("UPDATE bing_connections SET synced_at = now()")
        .execute(&w.pool)
        .await
        .unwrap();
    let rows: Vec<(String, i64)> = (1..=7)
        .map(|n| (format!("https://example.com/p{n}"), n * 10))
        .collect();
    let borrowed: Vec<(&str, i64)> = rows.iter().map(|(u, c)| (u.as_str(), *c)).collect();
    seed_targets(&w, &borrowed).await;
    let block = metrics(&w).await;
    assert_eq!(block["total"], 280);
    assert_eq!(block["linked_pages"], 7);
    assert_eq!(block["reason"], Value::Null);
    assert!(block["synced_at"].as_str().unwrap().contains('T'));
    let top = block["top_pages"].as_array().unwrap();
    assert_eq!(top.len(), 5);
    assert_eq!(
        top[0],
        json!({ "url": "https://example.com/p7", "count": 70 })
    );
    assert_eq!(top[4]["count"], 30);
}

#[sqlx::test]
async fn a_synced_site_with_no_links_has_zero_not_null(pool: PgPool) {
    let w = GoogleWorld::new(pool).await;
    w.insert_bing("connected").await;
    sqlx::query("UPDATE bing_connections SET synced_at = now()")
        .execute(&w.pool)
        .await
        .unwrap();
    let block = metrics(&w).await;
    assert_eq!(block["total"], 0);
    assert_eq!(block["linked_pages"], 0);
    assert_eq!(block["top_pages"], json!([]));
}

#[sqlx::test]
async fn backlinks_metrics_ask_to_reconnect_when_bing_refused_the_key(pool: PgPool) {
    let w = GoogleWorld::new(pool).await;
    w.insert_bing("needs_reconnecting").await;
    seed_targets(&w, &[("https://example.com/a", 4)]).await;
    let block = metrics(&w).await;
    assert_eq!(block["total"], Value::Null);
    assert_eq!(
        block["reason"],
        "Bing no longer accepts this API key. Connect Bing Webmaster Tools again."
    );
}
