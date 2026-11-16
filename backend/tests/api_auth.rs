mod common;

use sqlx::PgPool;

fn authed(app: &common::TestApp, token: &str) -> reqwest::RequestBuilder {
    app.client
        .get(format!("http://{}/api/v1/me", app.addr))
        .header("authorization", format!("Bearer {token}"))
}

#[sqlx::test]
async fn no_token_is_401(pool: PgPool) {
    let app = common::spawn_app(pool).await;
    let response = app
        .client
        .get(format!("http://{}/api/v1/me", app.addr))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 401);
    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["error"], "Sign in to continue.");
}

#[sqlx::test]
async fn unknown_token_is_401(pool: PgPool) {
    let app = common::spawn_app(pool).await;
    let response = authed(&app, "no-such-token").send().await.unwrap();
    assert_eq!(response.status(), 401);
}

#[sqlx::test]
async fn known_token_returns_its_user(pool: PgPool) {
    let app = common::spawn_app(pool).await;
    let response = authed(&app, "test-token").send().await.unwrap();
    assert_eq!(response.status(), 200);
    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["data"]["email"], "ada@example.com");
    assert!(body["data"]["user_id"].is_string());
    assert!(body["data"]["workspace_id"].is_string());
}

#[sqlx::test]
async fn the_same_token_keeps_the_same_workspace(pool: PgPool) {
    let app = common::spawn_app(pool).await;
    let first_res = authed(&app, "test-token").send().await.unwrap();
    assert_eq!(first_res.status(), 200);
    let first: serde_json::Value = first_res.json().await.unwrap();
    let second_res = authed(&app, "test-token").send().await.unwrap();
    assert_eq!(second_res.status(), 200);
    let second: serde_json::Value = second_res.json().await.unwrap();
    assert_eq!(
        first["data"]["workspace_id"],
        second["data"]["workspace_id"]
    );
    assert_eq!(first["data"]["user_id"], second["data"]["user_id"]);
}

#[sqlx::test]
async fn two_tokens_get_two_workspaces(pool: PgPool) {
    let app = common::spawn_app(pool).await;
    let ada_res = authed(&app, "test-token").send().await.unwrap();
    assert_eq!(ada_res.status(), 200);
    let ada: serde_json::Value = ada_res.json().await.unwrap();
    let bea_res = authed(&app, "other-token").send().await.unwrap();
    assert_eq!(bea_res.status(), 200);
    let bea: serde_json::Value = bea_res.json().await.unwrap();
    assert_ne!(ada["data"]["workspace_id"], bea["data"]["workspace_id"]);
}

#[sqlx::test]
async fn the_121st_request_in_a_minute_is_429(pool: PgPool) {
    let app = common::spawn_app(pool).await;
    for _ in 0..120 {
        let status = authed(&app, "test-token").send().await.unwrap().status();
        assert_eq!(status, 200);
    }
    let limited = authed(&app, "test-token").send().await.unwrap();
    assert_eq!(limited.status(), 429);
    let body: serde_json::Value = limited.json().await.unwrap();
    assert_eq!(body["success"], false);
}

#[sqlx::test]
async fn health_needs_no_token(pool: PgPool) {
    let app = common::spawn_app(pool).await;
    let response = app
        .client
        .get(format!("http://{}/healthz", app.addr))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
}

#[sqlx::test]
async fn many_bogus_tokens_from_one_address_share_one_allowance(pool: PgPool) {
    let app = common::spawn_app(pool).await;
    for n in 0..120 {
        let status = authed(&app, &format!("bogus-{n}"))
            .send()
            .await
            .unwrap()
            .status();
        assert_eq!(status, 401, "request {n}");
    }
    let limited = authed(&app, "bogus-another").send().await.unwrap();
    assert_eq!(limited.status(), 429);
}

#[sqlx::test]
async fn requests_without_a_token_share_the_same_address_allowance(pool: PgPool) {
    let app = common::spawn_app(pool).await;
    let url = format!("http://{}/api/v1/me", app.addr);
    for _ in 0..120 {
        assert_eq!(app.client.get(&url).send().await.unwrap().status(), 401);
    }
    assert_eq!(app.client.get(&url).send().await.unwrap().status(), 429);
}

#[sqlx::test]
async fn health_is_never_rate_limited(pool: PgPool) {
    let app = common::spawn_app(pool).await;
    let url = format!("http://{}/healthz", app.addr);
    for _ in 0..150 {
        assert_eq!(app.client.get(&url).send().await.unwrap().status(), 200);
    }
    let me = authed(&app, "test-token").send().await.unwrap();
    assert_eq!(me.status(), 200);
}
