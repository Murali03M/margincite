mod common;

use common::google::GoogleWorld;
use reqwest::Method;
use serde_json::json;
use sqlx::PgPool;

fn path(w: &GoogleWorld) -> String {
    format!("/api/v1/sites/{}/settings", w.site_id)
}

#[sqlx::test]
async fn a_new_site_has_the_defaults_and_the_choices(pool: PgPool) {
    let w = GoogleWorld::new(pool).await;
    let (status, body) = w.get(&path(&w)).await;
    assert_eq!(status, 200, "{body}");
    let data = &body["data"];
    assert_eq!(data["page_cap"], 500);
    assert_eq!(data["country"], "US");
    assert_eq!(data["language"], "en");
    assert_eq!(data["device"], "desktop");
    assert_eq!(data["track_rankings"], true);
    assert_eq!(data["ranking_frequency"], "weekly");
    assert!(data["countries"]
        .as_array()
        .unwrap()
        .iter()
        .any(|c| c["code"] == "GB" && c["name"] == "United Kingdom"));
    assert!(data["languages"]
        .as_array()
        .unwrap()
        .iter()
        .any(|c| c["code"] == "de" && c["name"] == "German"));
}

#[sqlx::test]
async fn a_partial_update_changes_only_what_it_names(pool: PgPool) {
    let w = GoogleWorld::new(pool).await;
    let (status, body) = w
        .send(
            "test-token",
            Method::PUT,
            &path(&w),
            Some(
                json!({ "page_cap": 1200, "country": "GB", "device": "mobile",
                         "track_rankings": false, "ranking_frequency": "daily",
                         "display_name": "  My shop " }),
            ),
        )
        .await;
    assert_eq!(status, 200, "{body}");
    let (_, again) = w
        .send(
            "test-token",
            Method::PUT,
            &path(&w),
            Some(json!({ "language": "de" })),
        )
        .await;
    let data = &again["data"];
    assert_eq!(data["page_cap"], 1200);
    assert_eq!(data["country"], "GB");
    assert_eq!(data["language"], "de");
    assert_eq!(data["device"], "mobile");
    assert_eq!(data["track_rankings"], false);
    assert_eq!(data["ranking_frequency"], "daily");
    assert_eq!(data["display_name"], "My shop");
    let (_, site) = w.get(&format!("/api/v1/sites/{}", w.site_id)).await;
    assert_eq!(site["data"]["page_cap"], 1200);
    assert_eq!(site["data"]["display_name"], "My shop");
}

#[sqlx::test]
async fn bad_values_are_refused_with_a_plain_reason(pool: PgPool) {
    let w = GoogleWorld::new(pool).await;
    let cases = [
        (json!({ "page_cap": 9 }), "page cap"),
        (json!({ "page_cap": 5001 }), "page cap"),
        (json!({ "country": "ZZ" }), "country"),
        (json!({ "language": "xx" }), "language"),
        (json!({ "device": "tablet" }), "device"),
        (json!({ "ranking_frequency": "hourly" }), "frequency"),
        (json!({ "display_name": "   " }), "name"),
    ];
    for (body, word) in cases {
        let (status, reply) = w
            .send("test-token", Method::PUT, &path(&w), Some(body.clone()))
            .await;
        assert_eq!(status, 422, "{body}");
        assert!(
            reply["error"]
                .as_str()
                .unwrap()
                .to_lowercase()
                .contains(word),
            "{body}: {reply}"
        );
    }
    let (_, now) = w.get(&path(&w)).await;
    assert_eq!(now["data"]["page_cap"], 500, "nothing was half saved");
}

#[sqlx::test]
async fn another_workspace_cannot_read_or_change_the_site(pool: PgPool) {
    let w = GoogleWorld::new(pool).await;
    let (status, _) = w.send("other-token", Method::GET, &path(&w), None).await;
    assert_eq!(status, 404);
    let (status, _) = w
        .send(
            "other-token",
            Method::PUT,
            &path(&w),
            Some(json!({ "page_cap": 50 })),
        )
        .await;
    assert_eq!(status, 404);
}
