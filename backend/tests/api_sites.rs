mod common;

use sqlx::PgPool;

fn auth(builder: reqwest::RequestBuilder, token: &str) -> reqwest::RequestBuilder {
    builder.header("authorization", format!("Bearer {token}"))
}

async fn add_site(app: &common::TestApp, token: &str, url: &str) -> reqwest::Response {
    auth(
        app.client.post(format!("http://{}/api/v1/sites", app.addr)),
        token,
    )
    .json(&serde_json::json!({ "url": url }))
    .send()
    .await
    .unwrap()
}

#[sqlx::test]
async fn adding_a_site_returns_it(pool: PgPool) {
    let app = common::spawn_app(pool).await;
    let response = add_site(&app, "test-token", "example.com").await;
    assert_eq!(response.status(), 201);
    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["data"]["host"], "example.com");
    assert_eq!(body["data"]["page_cap"], 500);
    assert_eq!(body["data"]["root_url"], "https://example.com/");
}

#[sqlx::test]
async fn an_invalid_address_is_422_with_a_reason(pool: PgPool) {
    let app = common::spawn_app(pool).await;
    let response = add_site(&app, "test-token", "ftp://files.example").await;
    assert_eq!(response.status(), 422);
    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["success"], false);
    assert!(body["error"].as_str().unwrap().len() > 3);
}

#[sqlx::test]
async fn adding_the_same_site_twice_is_409(pool: PgPool) {
    let app = common::spawn_app(pool).await;
    assert_eq!(
        add_site(&app, "test-token", "example.com").await.status(),
        201
    );
    let again = add_site(&app, "test-token", "https://www.example.com/").await;
    assert_eq!(again.status(), 409);
}

#[sqlx::test]
async fn the_list_shows_only_my_sites(pool: PgPool) {
    let app = common::spawn_app(pool).await;
    add_site(&app, "test-token", "ada.example").await;
    add_site(&app, "other-token", "bea.example").await;
    let response = auth(
        app.client.get(format!("http://{}/api/v1/sites", app.addr)),
        "test-token",
    )
    .send()
    .await
    .unwrap();
    let body: serde_json::Value = response.json().await.unwrap();
    let hosts: Vec<_> = body["data"]
        .as_array()
        .unwrap()
        .iter()
        .map(|site| site["host"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(hosts, vec!["ada.example"]);
}

#[sqlx::test]
async fn another_workspaces_site_is_404_to_read_and_delete(pool: PgPool) {
    let app = common::spawn_app(pool).await;
    let created: serde_json::Value = add_site(&app, "test-token", "example.com")
        .await
        .json()
        .await
        .unwrap();
    let id = created["data"]["id"].as_str().unwrap();
    let read = auth(
        app.client
            .get(format!("http://{}/api/v1/sites/{id}", app.addr)),
        "other-token",
    )
    .send()
    .await
    .unwrap();
    assert_eq!(read.status(), 404);
    let remove = auth(
        app.client
            .delete(format!("http://{}/api/v1/sites/{id}", app.addr)),
        "other-token",
    )
    .send()
    .await
    .unwrap();
    assert_eq!(remove.status(), 404);
}

#[sqlx::test]
async fn two_workspaces_can_add_the_same_site(pool: PgPool) {
    let app = common::spawn_app(pool).await;
    let ada: serde_json::Value = add_site(&app, "test-token", "example.com")
        .await
        .json()
        .await
        .unwrap();
    let bea_res = add_site(&app, "other-token", "example.com").await;
    assert_eq!(bea_res.status(), 201);
    let bea: serde_json::Value = bea_res.json().await.unwrap();
    assert_ne!(ada["data"]["id"], bea["data"]["id"]);
}

#[sqlx::test]
async fn deleting_a_site_removes_it(pool: PgPool) {
    let app = common::spawn_app(pool).await;
    let created: serde_json::Value = add_site(&app, "test-token", "example.com")
        .await
        .json()
        .await
        .unwrap();
    let id = created["data"]["id"].as_str().unwrap();
    let removed = auth(
        app.client
            .delete(format!("http://{}/api/v1/sites/{id}", app.addr)),
        "test-token",
    )
    .send()
    .await
    .unwrap();
    assert_eq!(removed.status(), 200);
    let read = auth(
        app.client
            .get(format!("http://{}/api/v1/sites/{id}", app.addr)),
        "test-token",
    )
    .send()
    .await
    .unwrap();
    assert_eq!(read.status(), 404);
}

#[sqlx::test]
async fn the_26th_site_is_a_409(pool: PgPool) {
    let app = common::spawn_app(pool.clone()).await;
    let first = add_site(&app, "test-token", "site0.example").await;
    let body: serde_json::Value = first.json().await.unwrap();
    let workspace_id: uuid::Uuid =
        sqlx::query_scalar("SELECT workspace_id FROM sites WHERE id = $1")
            .bind(uuid::Uuid::parse_str(body["data"]["id"].as_str().unwrap()).unwrap())
            .fetch_one(&pool)
            .await
            .unwrap();
    for n in 1..25 {
        sqlx::query(
            "INSERT INTO sites (id, workspace_id, root_url, host, path_prefix, display_name)
             VALUES ($1, $2, $3, $4, '/', $4)",
        )
        .bind(uuid::Uuid::new_v4())
        .bind(workspace_id)
        .bind(format!("https://site{n}.example/"))
        .bind(format!("site{n}.example"))
        .execute(&pool)
        .await
        .unwrap();
    }
    let response = add_site(&app, "test-token", "one-too-many.example").await;
    assert_eq!(response.status(), 409);
    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(
        body["error"],
        "You have reached the limit of 25 sites. Delete one to add another."
    );
    let other = add_site(&app, "other-token", "one-too-many.example").await;
    assert_eq!(other.status(), 201);
}
