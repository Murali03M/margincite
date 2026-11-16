mod common;

use common::ai::Q1;
use common::google::GoogleWorld;
use common::settings::*;
use common::SECRET_KEY;
use reqwest::Method;
use serde_json::{json, Value};
use sqlx::PgPool;

// ------------------------------------------------------------ safe handling

#[sqlx::test]
async fn a_bad_body_never_quotes_what_was_sent(pool: PgPool) {
    let w = world(pool).await;
    // The secret sits in a field of the wrong type's neighbour.
    for (p, body) in [
        (
            "/api/v1/settings/providers/chatgpt",
            json!({ "api_key": OPENAI_SECRET, "model": 5 }),
        ),
        (
            "/api/v1/settings/google",
            json!({ "client_secret": GOOGLE_SECRET, "client_id": ["x"] }),
        ),
        (
            "/api/v1/settings/pagespeed",
            json!({ "api_key": PSI_SECRET, "extra": [1] , "x": null}),
        ),
        (
            "/api/v1/settings/dataforseo",
            json!({ "password": SEO_PASSWORD }),
        ),
    ] {
        let (s, _, raw) = put(&w, p, body).await;
        assert!(matches!(s, 422 | 200), "{p} {s}");
        assert_no_secret(&raw);
    }
    let response = w
        .app
        .client
        .put(w.url("/api/v1/settings/pagespeed"))
        .header("authorization", "Bearer test-token")
        .header("content-type", "application/json")
        .body(format!("{{\"api_key\": \"{PSI_SECRET}"))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 422);
    assert_no_secret(&response.text().await.unwrap());
}

#[sqlx::test]
async fn values_are_validated(pool: PgPool) {
    let w = world(pool).await;
    let bad = [
        ("/api/v1/settings/providers/chatgpt", json!({})),
        (
            "/api/v1/settings/providers/chatgpt",
            json!({ "api_key": "has space" }),
        ),
        (
            "/api/v1/settings/providers/chatgpt",
            json!({ "api_key": "" }),
        ),
        (
            "/api/v1/settings/providers/chatgpt",
            json!({ "model": "x/../y" }),
        ),
        (
            "/api/v1/settings/providers/gemini",
            json!({ "model": "a b" }),
        ),
        (
            "/api/v1/settings/crawler",
            json!({ "contact": "not a contact" }),
        ),
        (
            "/api/v1/settings/crawler",
            json!({ "contact": "ftp://example.com" }),
        ),
        ("/api/v1/settings/crawler", json!({ "contact": "a@b" })),
        ("/api/v1/settings/crawler", json!({ "contact": "" })),
        (
            "/api/v1/settings/google",
            json!({ "client_id": "", "client_secret": "x" }),
        ),
        (
            "/api/v1/settings/dataforseo",
            json!({ "login": "a", "password": "" }),
        ),
    ];
    for (p, body) in bad {
        let (s, reply, _) = put(&w, p, body.clone()).await;
        assert_eq!(s, 422, "{p} {body}: {reply}");
    }
    for good in [
        "a@example.com",
        "https://example.com/bot",
        "http://example.com/x?y=1",
    ] {
        let (s, _, raw) = put(&w, "/api/v1/settings/crawler", json!({ "contact": good })).await;
        assert_eq!(s, 200, "{good}: {raw}");
    }
    let (s, _, _) = put(
        &w,
        "/api/v1/settings/providers/nobody",
        json!({ "model": "x" }),
    )
    .await;
    assert_eq!(s, 404);
}

#[sqlx::test]
async fn secrets_cannot_be_saved_without_a_secret_key(pool: PgPool) {
    let w = GoogleWorld::with_settings(pool.clone(), &[]).await;
    let (_, data) = {
        let (_, body, _) = call(&w, "test-token", Method::GET, "/api/v1/settings", None).await;
        ((), body["data"].clone())
    };
    assert_eq!(data["secret_key_ready"], false);
    let (s, body, raw) = put(
        &w,
        "/api/v1/settings/providers/chatgpt",
        json!({ "api_key": OPENAI_SECRET }),
    )
    .await;
    assert_eq!(s, 422, "{raw}");
    assert!(body["error"].as_str().unwrap().contains("secret key"));
    assert_no_secret(&raw);
    let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM workspace_settings")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(n, 0, "nothing was stored, in plain or any other form");
    // Options that are not secrets still work.
    let (s, _, _) = put(
        &w,
        "/api/v1/settings/crawler",
        json!({ "contact": "me@example.org" }),
    )
    .await;
    assert_eq!(s, 200);
}

#[sqlx::test]
async fn a_saved_secret_that_cannot_be_opened_counts_as_not_saved(pool: PgPool) {
    let w = GoogleWorld::with_settings(
        pool.clone(),
        &[
            ("MARGINCITE_SECRET_KEY", SECRET_KEY),
            ("OPENAI_API_KEY", "sk-env-fallback"),
        ],
    )
    .await;
    mock_models_ok(&w).await;
    put(
        &w,
        "/api/v1/settings/providers/chatgpt",
        json!({ "api_key": OPENAI_SECRET }),
    )
    .await;
    sqlx::query("UPDATE workspace_settings SET value_sealed = 'bm90IGEgcmVhbCBzZWFsZWQgdmFsdWU=' WHERE key LIKE 'provider.chatgpt.api_key'")
        .execute(&pool)
        .await
        .unwrap();
    let (data, raw) = settings(&w).await;
    assert_eq!(provider(&data, "chatgpt")["source"], "environment");
    assert_no_secret(&raw);
}

// ------------------------------------------------------------ crawler contact

async fn no_contact_world(pool: PgPool) -> GoogleWorld {
    // An empty value counts as unset, which beats the test config's default.
    GoogleWorld::with_settings(
        pool,
        &[
            ("MARGINCITE_SECRET_KEY", SECRET_KEY),
            ("CRAWLER_CONTACT", ""),
        ],
    )
    .await
}

#[sqlx::test]
async fn adding_a_site_or_crawling_needs_a_contact_address(pool: PgPool) {
    let w = no_contact_world(pool).await;
    let message = "Add a contact address in Settings before crawling. It is sent with every request the crawler makes.";
    let (s, body, _) = call(
        &w,
        "test-token",
        Method::POST,
        "/api/v1/sites",
        Some(json!({ "url": "https://new-site.example.net/" })),
    )
    .await;
    assert_eq!(s, 422, "{body}");
    assert_eq!(body["error"], message);
    let n: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM sites WHERE host = 'new-site.example.net'")
            .fetch_one(&w.pool)
            .await
            .unwrap();
    assert_eq!(n, 0, "a refused site leaves nothing behind");
    let (s, body, _) = call(
        &w,
        "test-token",
        Method::POST,
        &format!("/api/v1/sites/{}/crawls", w.site_id),
        None,
    )
    .await;
    assert_eq!(s, 422, "{body}");
    assert_eq!(body["error"], message);

    let (s, _, _) = put(
        &w,
        "/api/v1/settings/crawler",
        json!({ "contact": "bot@example.org" }),
    )
    .await;
    assert_eq!(s, 200);
    let (s, body, _) = call(
        &w,
        "test-token",
        Method::POST,
        &format!("/api/v1/sites/{}/crawls", w.site_id),
        None,
    )
    .await;
    assert_eq!(s, 202, "{body}");
    let (s, body, _) = call(
        &w,
        "test-token",
        Method::POST,
        "/api/v1/sites",
        Some(json!({ "url": "https://new-site.example.net/" })),
    )
    .await;
    assert_eq!(s, 201, "{body}");
}

// -------------------------------------------------------------------- usage

#[sqlx::test]
async fn usage_is_recorded_and_listed_per_day(pool: PgPool) {
    let w = world(pool).await;
    for (calls, cost) in [(2, Some(0.5)), (3, Some(0.25))] {
        margincite::usage::record(&w.pool, w.workspace_id, "dataforseo", "serp", calls, cost)
            .await
            .unwrap();
    }
    margincite::usage::record(&w.pool, w.workspace_id, "pagespeed", "run", 1, None)
        .await
        .unwrap();
    sqlx::query("INSERT INTO usage_events (workspace_id, source, kind, calls, created_at) VALUES ($1, 'dataforseo', 'serp', 7, now() - interval '3 days')")
        .bind(w.workspace_id)
        .execute(&w.pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO usage_events (workspace_id, source, kind, calls, created_at) VALUES ($1, 'dataforseo', 'serp', 9, now() - interval '60 days')")
        .bind(w.workspace_id)
        .execute(&w.pool)
        .await
        .unwrap();
    let (s, body, _) = call(&w, "test-token", Method::GET, "/api/v1/usage", None).await;
    assert_eq!(s, 200);
    let rows = body["data"].as_array().unwrap();
    assert_eq!(rows.len(), 3, "{rows:?}");
    let today = &rows[0];
    assert_eq!(today["source"], "dataforseo");
    assert_eq!(today["kind"], "serp");
    assert_eq!(today["calls"], 5);
    assert_eq!(today["estimated_cost_usd"], 0.75);
    assert_eq!(rows[1]["source"], "pagespeed");
    assert_eq!(rows[1]["estimated_cost_usd"], Value::Null);
    assert_eq!(rows[2]["calls"], 7);
    assert!(rows[0]["day"].as_str().unwrap().len() == 10);
    let (_, body, _) = call(&w, "test-token", Method::GET, "/api/v1/usage?days=1", None).await;
    assert_eq!(body["data"].as_array().unwrap().len(), 2);
    let (_, body, _) = call(&w, "test-token", Method::GET, "/api/v1/usage?days=90", None).await;
    assert_eq!(body["data"].as_array().unwrap().len(), 4);
    let (_, other, _) = call(&w, "other-token", Method::GET, "/api/v1/usage", None).await;
    assert_eq!(other["data"], json!([]));
}

#[sqlx::test]
async fn an_ai_answer_is_counted_in_the_usage(pool: PgPool) {
    let w = GoogleWorld::with_ai(pool).await;
    w.mock_engines("Nothing special.", &[], None).await;
    w.add_prompt(Q1).await;
    w.run_to_end().await;
    let (_, body) = w.get("/api/v1/usage").await;
    let rows = body["data"].as_array().unwrap();
    let sources: Vec<_> = rows.iter().map(|r| r["source"].as_str().unwrap()).collect();
    assert_eq!(
        sources,
        ["chatgpt", "claude", "gemini", "grok", "perplexity"]
    );
    assert!(rows
        .iter()
        .all(|r| r["calls"] == 1 && r["kind"] == "answer"));
}
