mod common;

use sqlx::PgPool;

#[sqlx::test]
async fn health_answers_with_the_envelope(pool: PgPool) {
    let app = common::spawn_app(pool).await;
    let response = app
        .client
        .get(format!("http://{}/healthz", app.addr))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    assert_eq!(
        response.text().await.unwrap(),
        r#"{"success":true,"data":{"status":"ok"},"error":null,"meta":null}"#
    );
}

#[sqlx::test]
async fn unknown_route_is_a_404_envelope(pool: PgPool) {
    let app = common::spawn_app(pool).await;
    let response = app
        .client
        .get(format!("http://{}/nope", app.addr))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 404);
    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["success"], false);
    assert_eq!(body["error"], "Not found");
    assert!(body["data"].is_null());
}
