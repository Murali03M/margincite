mod common;

use common::google::{location_of, GoogleWorld, ACCESS_TOKEN, NONCE, OAUTH_COOKIE, REFRESH_TOKEN};
use margincite::integrations::google::hash_nonce;
use serde_json::{json, Value};
use sqlx::PgPool;
use uuid::Uuid;
use wiremock::matchers::{method, path};
use wiremock::{Mock, ResponseTemplate};

const CALLBACK: &str = "http://localhost:5174/api/v1/oauth/google/callback";

fn query_of(url: &str) -> std::collections::HashMap<String, String> {
    url::Url::parse(url)
        .unwrap()
        .query_pairs()
        .map(|(k, v)| (k.into_owned(), v.into_owned()))
        .collect()
}

// ---------------------------------------------------------------- start

#[sqlx::test]
async fn start_returns_a_consent_url_with_every_required_parameter(pool: PgPool) {
    let w = GoogleWorld::new(pool).await;
    let (status, body) = w.post(&w.google_path("/start")).await;
    assert_eq!(status, 200, "{body}");
    let auth_url = body["data"]["auth_url"].as_str().unwrap();
    assert!(
        auth_url.starts_with(&format!("{}/auth?", w.mock.uri())),
        "{auth_url}"
    );
    let q = query_of(auth_url);
    assert_eq!(q["response_type"], "code");
    assert_eq!(
        q["scope"],
        "https://www.googleapis.com/auth/webmasters.readonly"
    );
    assert_eq!(q["access_type"], "offline");
    assert_eq!(q["prompt"], "consent");
    assert_eq!(q["client_id"], "test-client-id");
    assert_eq!(q["redirect_uri"], CALLBACK);
    let state: Uuid = q["state"].parse().unwrap();
    let stored: (Uuid, Uuid, String) =
        sqlx::query_as("SELECT workspace_id, site_id, provider FROM oauth_states WHERE id = $1")
            .bind(state)
            .fetch_one(&w.pool)
            .await
            .unwrap();
    assert_eq!(stored, (w.workspace_id, w.site_id, "google".to_string()));
    assert!(!auth_url.contains("test-client-secret"));
}

#[sqlx::test]
async fn start_is_422_without_google_credentials_or_the_secret_key(pool: PgPool) {
    for settings in [
        &[][..],
        &[("MARGINCITE_SECRET_KEY", common::SECRET_KEY)][..],
        &[
            ("GOOGLE_CLIENT_ID", "id"),
            ("GOOGLE_CLIENT_SECRET", "secret"),
        ][..],
    ] {
        let w = GoogleWorld::with_settings(pool.clone(), settings).await;
        let (status, body) = w.post(&w.google_path("/start")).await;
        assert_eq!(status, 422, "{body}");
        assert_eq!(w.count("oauth_states").await, 0);
        sqlx::query("DELETE FROM sites WHERE id = $1")
            .bind(w.site_id)
            .execute(&pool)
            .await
            .unwrap();
    }
}

#[sqlx::test]
async fn another_workspaces_site_is_not_found_on_every_endpoint(pool: PgPool) {
    let w = GoogleWorld::new(pool).await;
    for (method, tail) in [
        (reqwest::Method::GET, ""),
        (reqwest::Method::PUT, ""),
        (reqwest::Method::DELETE, ""),
        (reqwest::Method::POST, "/start"),
        (reqwest::Method::GET, "/properties"),
        (reqwest::Method::POST, "/sync"),
    ] {
        let (status, _) = w
            .send(
                "other-token",
                method.clone(),
                &w.google_path(tail),
                Some(json!({ "property": "https://example.com/" })),
            )
            .await;
        assert_eq!(status, 404, "{method} {tail}");
    }
    for tail in ["queries", "pages"] {
        let (status, _) = w
            .send(
                "other-token",
                reqwest::Method::GET,
                &format!("/api/v1/sites/{}/search/{tail}", w.site_id),
                None,
            )
            .await;
        assert_eq!(status, 404, "{tail}");
    }
}

// ---------------------------------------------------------------- browser binding

fn set_cookie(response: &reqwest::Response) -> String {
    response
        .headers()
        .get("set-cookie")
        .expect("a Set-Cookie header")
        .to_str()
        .unwrap()
        .to_string()
}

async fn start_response(w: &GoogleWorld) -> reqwest::Response {
    w.app
        .client
        .post(w.url(&w.google_path("/start")))
        .header("authorization", "Bearer test-token")
        .send()
        .await
        .unwrap()
}

#[sqlx::test]
async fn start_sets_an_http_only_lax_cookie_scoped_to_the_oauth_path(pool: PgPool) {
    let w = GoogleWorld::new(pool).await;
    let response = start_response(&w).await;
    assert_eq!(response.status(), 200);
    let cookie = set_cookie(&response);
    let (pair, attributes) = cookie.split_once(';').unwrap();
    let nonce = pair.strip_prefix("margincite_oauth=").unwrap();
    assert!(nonce.len() >= 43, "32 bytes in base64url: {nonce}");
    assert!(nonce
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'));
    for attribute in [
        "Path=/api/v1/oauth/google",
        "HttpOnly",
        "SameSite=Lax",
        "Max-Age=600",
    ] {
        assert!(attributes.contains(attribute), "{attribute} in {cookie}");
    }
    assert!(!attributes.contains("Secure"), "http base url: {cookie}");
}

#[sqlx::test]
async fn start_marks_the_cookie_secure_only_for_an_https_base_url(pool: PgPool) {
    let mut settings = common::GOOGLE_SETTINGS.to_vec();
    settings.push(("PUBLIC_BASE_URL", "https://app.example.com"));
    let w = GoogleWorld::with_settings(pool, &settings).await;
    let cookie = set_cookie(&start_response(&w).await);
    assert!(cookie.contains("; Secure"), "{cookie}");
}

#[sqlx::test]
async fn the_stored_value_is_a_hash_and_each_start_gets_a_new_nonce(pool: PgPool) {
    let w = GoogleWorld::new(pool).await;
    let first = set_cookie(&start_response(&w).await);
    let second = set_cookie(&start_response(&w).await);
    let nonce = |c: &str| {
        c.split(';')
            .next()
            .unwrap()
            .trim_start_matches("margincite_oauth=")
            .to_string()
    };
    assert_ne!(nonce(&first), nonce(&second));
    let hashes: Vec<String> =
        sqlx::query_scalar("SELECT nonce_hash FROM oauth_states ORDER BY created_at")
            .fetch_all(&w.pool)
            .await
            .unwrap();
    assert_eq!(hashes.len(), 2);
    assert_eq!(hashes[0], hash_nonce(&nonce(&first)));
    assert_eq!(hashes[1], hash_nonce(&nonce(&second)));
    for (hash, cookie) in hashes.iter().zip([&first, &second]) {
        assert_ne!(hash, &nonce(cookie));
        assert!(!hash.contains(&nonce(cookie)));
    }
}

#[sqlx::test]
async fn the_callback_with_the_start_cookie_connects(pool: PgPool) {
    let w = GoogleWorld::new(pool).await;
    w.mock_tokens().await;
    let started = start_response(&w).await;
    let cookie = set_cookie(&started);
    let pair = cookie.split(';').next().unwrap().to_string();
    let body: Value = started.json().await.unwrap();
    let state = query_of(body["data"]["auth_url"].as_str().unwrap())["state"].clone();
    let response = w
        .callback_response(&format!("code=c&state={state}"), Some(&pair))
        .await;
    assert_eq!(
        location_of(&response),
        w.connections_url("google=connected")
    );
    assert_eq!(w.count("google_connections").await, 1);
}

#[sqlx::test]
async fn a_callback_without_the_cookie_fails_and_stores_nothing(pool: PgPool) {
    let w = GoogleWorld::new(pool).await;
    w.mock_tokens().await;
    let state = w.new_state().await;
    let response = w
        .callback_response(&format!("code=c&state={state}"), None)
        .await;
    assert_eq!(response.status(), 302);
    assert!(location_of(&response).ends_with("google=error&reason=invalid_state"));
    assert_eq!(w.count("google_connections").await, 0);
    assert_eq!(w.token_calls().await, 0, "Google is not asked");
    assert_eq!(
        w.count("oauth_states").await,
        1,
        "not used up by a stranger"
    );
}

#[sqlx::test]
async fn a_callback_with_another_browsers_nonce_fails_and_stores_nothing(pool: PgPool) {
    let w = GoogleWorld::new(pool).await;
    w.mock_tokens().await;
    let state = w.new_state().await;
    for cookie in [
        format!("{OAUTH_COOKIE}=some-other-browsers-nonce"),
        format!("{OAUTH_COOKIE}="),
        format!("other=1; {OAUTH_COOKIE}=x"),
        format!("{OAUTH_COOKIE}={NONCE}x"),
    ] {
        let response = w
            .callback_response(&format!("code=c&state={state}"), Some(&cookie))
            .await;
        assert!(
            location_of(&response).ends_with("google=error&reason=invalid_state"),
            "{cookie}: {}",
            location_of(&response)
        );
    }
    assert_eq!(w.count("google_connections").await, 0);
    assert_eq!(w.token_calls().await, 0);
    // The victim's own browser still works afterwards.
    let ok = w
        .callback_response(
            &format!("code=c&state={state}"),
            Some(&format!("a=b; {OAUTH_COOKIE}={NONCE}; c=d")),
        )
        .await;
    assert_eq!(location_of(&ok), w.connections_url("google=connected"));
}

#[sqlx::test]
async fn a_state_with_no_stored_nonce_is_invalid(pool: PgPool) {
    let w = GoogleWorld::new(pool).await;
    w.mock_tokens().await;
    let state = w.new_state().await;
    sqlx::query("UPDATE oauth_states SET nonce_hash = NULL WHERE id = $1")
        .bind(state)
        .execute(&w.pool)
        .await
        .unwrap();
    let (_, location) = w.callback(&format!("code=c&state={state}")).await;
    assert!(location.ends_with("google=error&reason=invalid_state"));
    assert_eq!(w.count("google_connections").await, 0);
    assert_eq!(w.token_calls().await, 0);
}

#[sqlx::test]
async fn every_callback_response_clears_the_cookie(pool: PgPool) {
    let w = GoogleWorld::new(pool).await;
    w.mock_tokens().await;
    let state = w.new_state().await;
    let good = format!("{OAUTH_COOKIE}={NONCE}");
    let cases = [
        ("code=c&state=not-a-uuid".to_string(), None),
        (format!("code=c&state={state}"), None),
        (format!("state={state}"), Some(good.clone())),
        (format!("code=c&state={state}"), Some(good)),
    ];
    for (query, cookie) in cases {
        let response = w.callback_response(&query, cookie.as_deref()).await;
        let cleared = set_cookie(&response);
        assert!(
            cleared.starts_with("margincite_oauth=;"),
            "{query}: {cleared}"
        );
        for attribute in ["Max-Age=0", "Path=/api/v1/oauth/google", "HttpOnly"] {
            assert!(cleared.contains(attribute), "{attribute} in {cleared}");
        }
    }
}

// ---------------------------------------------------------------- callback

#[sqlx::test]
async fn the_callback_stores_a_sealed_token_and_redirects_to_connected(pool: PgPool) {
    let w = GoogleWorld::new(pool).await;
    w.mock_tokens().await;
    let state = w.new_state().await;
    let (status, location) = w.callback(&format!("code=the-code&state={state}")).await;
    assert_eq!(status, 302);
    assert_eq!(location, w.connections_url("google=connected"));

    let sealed: String = sqlx::query_scalar(
        "SELECT refresh_token_sealed FROM google_connections WHERE site_id = $1",
    )
    .bind(w.site_id)
    .fetch_one(&w.pool)
    .await
    .unwrap();
    assert!(!sealed.contains(REFRESH_TOKEN));
    assert_eq!(
        common::google::secrets().open(&sealed).unwrap(),
        REFRESH_TOKEN
    );
    assert_eq!(
        w.connection_status().await.as_deref(),
        Some("choose_property")
    );
    assert_eq!(w.count("oauth_states").await, 0, "the state is used up");
    assert_eq!(
        w.count("jobs").await,
        0,
        "no property yet, so nothing to sync"
    );

    let exchange = &w.requests_to("/token").await[0];
    let form: std::collections::HashMap<String, String> =
        url::form_urlencoded::parse(&exchange.body)
            .map(|(k, v)| (k.into_owned(), v.into_owned()))
            .collect();
    assert_eq!(form["grant_type"], "authorization_code");
    assert_eq!(form["code"], "the-code");
    assert_eq!(form["redirect_uri"], CALLBACK);
    assert_eq!(form["client_id"], "test-client-id");
    assert_eq!(form["client_secret"], "test-client-secret");
}

#[sqlx::test]
async fn a_replayed_state_is_refused_and_asks_google_nothing(pool: PgPool) {
    let w = GoogleWorld::new(pool).await;
    w.mock_tokens().await;
    let state = w.new_state().await;
    let (_, first) = w.callback(&format!("code=c1&state={state}")).await;
    assert_eq!(first, w.connections_url("google=connected"));
    let (status, second) = w.callback(&format!("code=c2&state={state}")).await;
    assert_eq!(status, 302);
    assert_eq!(
        second,
        "http://localhost:5174/?google=error&reason=invalid_state"
    );
    assert_eq!(w.token_calls().await, 1);
}

#[sqlx::test]
async fn an_expired_state_redirects_with_an_error_and_stores_nothing(pool: PgPool) {
    let w = GoogleWorld::new(pool).await;
    w.mock_tokens().await;
    let state = w.new_state().await;
    sqlx::query("UPDATE oauth_states SET created_at = now() - interval '11 minutes' WHERE id = $1")
        .bind(state)
        .execute(&w.pool)
        .await
        .unwrap();
    let (status, location) = w.callback(&format!("code=c&state={state}")).await;
    assert_eq!(status, 302);
    assert_eq!(location, w.connections_url("google=error&reason=expired"));
    assert_eq!(w.count("google_connections").await, 0);
    assert_eq!(w.token_calls().await, 0);
}

#[sqlx::test]
async fn a_state_that_does_not_match_its_workspace_is_refused(pool: PgPool) {
    let w = GoogleWorld::new(pool).await;
    w.mock_tokens().await;
    // Another workspace's site, with a state that names this workspace.
    let other = common::google::workspace_of(&w.app, "other-token").await;
    let foreign_site = common::google::add_site(&w.pool, other, "https://other.example/").await;
    let state = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO oauth_states (id, workspace_id, site_id, provider, nonce_hash)
         VALUES ($1, $2, $3, 'google', $4)",
    )
    .bind(state)
    .bind(w.workspace_id)
    .bind(foreign_site)
    .bind(hash_nonce(NONCE))
    .execute(&w.pool)
    .await
    .unwrap();
    let (status, location) = w.callback(&format!("code=c&state={state}")).await;
    assert_eq!(status, 302);
    assert_eq!(
        location,
        "http://localhost:5174/?google=error&reason=invalid_state"
    );
    assert!(!location.contains(&foreign_site.to_string()));
    assert_eq!(w.count("google_connections").await, 0);
    assert_eq!(w.token_calls().await, 0);
}

#[sqlx::test]
async fn a_state_for_another_provider_or_a_made_up_one_is_refused(pool: PgPool) {
    let w = GoogleWorld::new(pool).await;
    w.mock_tokens().await;
    let state = w.new_state().await;
    sqlx::query("UPDATE oauth_states SET provider = 'bing' WHERE id = $1")
        .bind(state)
        .execute(&w.pool)
        .await
        .unwrap();
    for query in [
        format!("code=c&state={state}"),
        format!("code=c&state={}", Uuid::new_v4()),
        "code=c&state=not-a-uuid".to_string(),
        "code=c".to_string(),
        String::new(),
    ] {
        let (status, location) = w.callback(&query).await;
        assert_eq!(status, 302, "{query}");
        assert_eq!(
            location, "http://localhost:5174/?google=error&reason=invalid_state",
            "{query}"
        );
    }
    assert_eq!(w.count("google_connections").await, 0);
    assert_eq!(w.token_calls().await, 0);
}

#[sqlx::test]
async fn a_missing_code_or_a_refusal_redirects_with_an_error(pool: PgPool) {
    let w = GoogleWorld::new(pool).await;
    w.mock_tokens().await;
    let state = w.new_state().await;
    let (_, location) = w.callback(&format!("state={state}")).await;
    assert_eq!(
        location,
        w.connections_url("google=error&reason=missing_code")
    );
    let (_, location) = w.callback(&format!("state={state}&code=")).await;
    assert_eq!(
        location,
        w.connections_url("google=error&reason=missing_code")
    );
    let (_, location) = w
        .callback(&format!("error=access_denied&state={state}"))
        .await;
    assert_eq!(location, w.connections_url("google=error&reason=denied"));
    let long = "x".repeat(5000);
    let (_, location) = w.callback(&format!("code={long}&state={state}")).await;
    assert_eq!(
        location,
        w.connections_url("google=error&reason=token_rejected")
    );
    assert_eq!(w.count("google_connections").await, 0);
    assert_eq!(w.token_calls().await, 0);
}

#[sqlx::test]
async fn request_text_never_reaches_the_redirect(pool: PgPool) {
    let w = GoogleWorld::new(pool).await;
    let state = w.new_state().await;
    let (_, location) = w
        .callback(&format!(
            "state={state}&error=http%3A%2F%2Fevil.example%2F&reason=evil&google=connected&redirect_uri=http%3A%2F%2Fevil.example"
        ))
        .await;
    assert_eq!(location, w.connections_url("google=error&reason=denied"));
    assert!(!location.contains("evil"));
}

#[sqlx::test]
async fn google_refusing_the_code_or_failing_stores_nothing(pool: PgPool) {
    for (answer, reason) in [
        (400, "token_rejected"),
        (401, "token_rejected"),
        (503, "google_unavailable"),
        (429, "google_unavailable"),
    ] {
        let w = GoogleWorld::new(pool.clone()).await;
        Mock::given(method("POST"))
            .and(path("/token"))
            .respond_with(
                ResponseTemplate::new(answer).set_body_json(json!({"error": "invalid_grant"})),
            )
            .mount(&w.mock)
            .await;
        let state = w.new_state().await;
        let (_, location) = w.callback(&format!("code=c&state={state}")).await;
        assert_eq!(
            location,
            w.connections_url(&format!("google=error&reason={reason}")),
            "{answer}"
        );
        assert_eq!(w.count("google_connections").await, 0, "{answer}");
        cleanup(&pool, &w).await;
    }
}

#[sqlx::test]
async fn wrong_server_credentials_at_the_code_exchange_are_a_server_error(pool: PgPool) {
    let w = GoogleWorld::new(pool).await;
    Mock::given(method("POST"))
        .and(path("/token"))
        .respond_with(ResponseTemplate::new(401).set_body_json(json!({"error": "invalid_client"})))
        .mount(&w.mock)
        .await;
    let state = w.new_state().await;
    let (_, location) = w.callback(&format!("code=c&state={state}")).await;
    assert_eq!(
        location,
        w.connections_url("google=error&reason=server_error")
    );
}

async fn cleanup(pool: &PgPool, w: &GoogleWorld) {
    sqlx::query("DELETE FROM sites WHERE id = $1")
        .bind(w.site_id)
        .execute(pool)
        .await
        .unwrap();
}

#[sqlx::test]
async fn an_answer_without_a_refresh_token_is_refused(pool: PgPool) {
    let w = GoogleWorld::new(pool).await;
    Mock::given(method("POST"))
        .and(path("/token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "access_token": ACCESS_TOKEN, "expires_in": 3599
        })))
        .mount(&w.mock)
        .await;
    let state = w.new_state().await;
    let (_, location) = w.callback(&format!("code=c&state={state}")).await;
    assert_eq!(
        location,
        w.connections_url("google=error&reason=no_refresh_token")
    );
    assert_eq!(w.count("google_connections").await, 0);
}

#[sqlx::test]
async fn the_callback_without_setup_redirects_with_not_configured(pool: PgPool) {
    let w = GoogleWorld::with_settings(pool, &[]).await;
    let state = w.new_state().await;
    let (_, location) = w.callback(&format!("code=c&state={state}")).await;
    assert_eq!(
        location,
        w.connections_url("google=error&reason=not_configured")
    );
}

#[sqlx::test]
async fn connecting_again_keeps_the_property_and_starts_a_sync(pool: PgPool) {
    let w = GoogleWorld::new(pool).await;
    w.mock_tokens().await;
    w.insert_connection(Some("https://example.com/"), "needs_reconnecting")
        .await;
    let state = w.new_state().await;
    let (_, location) = w.callback(&format!("code=c&state={state}")).await;
    assert_eq!(location, w.connections_url("google=connected"));
    assert_eq!(w.connection_status().await.as_deref(), Some("connected"));
    assert_eq!(w.count("jobs").await, 1);
}

// ---------------------------------------------------------------- status, properties, property choice

async fn mock_sites(w: &GoogleWorld) {
    Mock::given(method("GET"))
        .and(path("/webmasters/v3/sites"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "siteEntry": [
                {"siteUrl": "https://zebra.example/", "permissionLevel": "siteOwner"},
                {"siteUrl": "sc-domain:example.com", "permissionLevel": "siteFullUser"},
                {"siteUrl": "https://unverified.example.com/", "permissionLevel": "siteUnverifiedUser"}
            ]
        })))
        .mount(&w.mock)
        .await;
}

#[sqlx::test]
async fn status_walks_from_not_connected_to_connected(pool: PgPool) {
    let w = GoogleWorld::new(pool).await;
    let (_, body) = w.get(&w.google_path("")).await;
    assert_eq!(
        body["data"],
        json!({"status": "not_connected", "property": null, "synced_at": null, "error": null})
    );
    w.insert_connection(None, "choose_property").await;
    let (_, body) = w.get(&w.google_path("")).await;
    assert_eq!(body["data"]["status"], "choose_property");
    w.connect("https://example.com/").await;
    let (_, body) = w.get(&w.google_path("")).await;
    assert_eq!(body["data"]["status"], "connected");
    assert_eq!(body["data"]["property"], "https://example.com/");
}

#[sqlx::test]
async fn properties_are_listed_readable_ones_with_this_sites_first(pool: PgPool) {
    let w = GoogleWorld::new(pool).await;
    w.mock_tokens().await;
    mock_sites(&w).await;
    w.insert_connection(None, "choose_property").await;
    let (status, body) = w.get(&w.google_path("/properties")).await;
    assert_eq!(status, 200, "{body}");
    assert_eq!(
        body["data"],
        json!([
            {"property": "sc-domain:example.com", "permission": "siteFullUser"},
            {"property": "https://zebra.example/", "permission": "siteOwner"},
        ])
    );
    let asked = &w.requests_to("/webmasters/v3/sites").await[0];
    assert_eq!(
        asked
            .headers
            .get("authorization")
            .unwrap()
            .to_str()
            .unwrap(),
        format!("Bearer {ACCESS_TOKEN}")
    );
    let refresh = &w.requests_to("/token").await[0];
    let form: std::collections::HashMap<String, String> =
        url::form_urlencoded::parse(&refresh.body)
            .map(|(k, v)| (k.into_owned(), v.into_owned()))
            .collect();
    assert_eq!(form["grant_type"], "refresh_token");
    assert_eq!(form["refresh_token"], REFRESH_TOKEN);
}

#[sqlx::test]
async fn properties_without_a_connection_are_422(pool: PgPool) {
    let w = GoogleWorld::new(pool).await;
    let (status, _) = w.get(&w.google_path("/properties")).await;
    assert_eq!(status, 422);
}

#[sqlx::test]
async fn a_dead_refresh_token_marks_the_connection_needs_reconnecting(pool: PgPool) {
    let w = GoogleWorld::new(pool).await;
    Mock::given(method("POST"))
        .and(path("/token"))
        .respond_with(ResponseTemplate::new(400).set_body_json(json!({"error": "invalid_grant"})))
        .mount(&w.mock)
        .await;
    w.connect("https://example.com/").await;
    let (status, body) = w.get(&w.google_path("/properties")).await;
    assert_eq!(status, 422);
    assert!(body["error"]
        .as_str()
        .unwrap()
        .contains("Connect Google Search Console again"));
    let (_, body) = w.get(&w.google_path("")).await;
    assert_eq!(body["data"]["status"], "needs_reconnecting");
    assert!(body["data"]["error"].as_str().unwrap().contains("again"));
    // Once marked, no more calls to Google until the person connects again.
    let before = w.token_calls().await;
    let (status, _) = w.get(&w.google_path("/properties")).await;
    assert_eq!(status, 422);
    assert_eq!(w.token_calls().await, before);
}

#[sqlx::test]
async fn google_being_down_while_listing_is_a_502(pool: PgPool) {
    let w = GoogleWorld::new(pool).await;
    w.mock_tokens().await;
    Mock::given(method("GET"))
        .and(path("/webmasters/v3/sites"))
        .respond_with(ResponseTemplate::new(503))
        .mount(&w.mock)
        .await;
    w.insert_connection(None, "choose_property").await;
    let (status, _) = w.get(&w.google_path("/properties")).await;
    assert_eq!(status, 502);
    assert_eq!(
        w.connection_status().await.as_deref(),
        Some("choose_property")
    );
}

#[sqlx::test]
async fn choosing_a_property_connects_and_queues_one_sync(pool: PgPool) {
    let w = GoogleWorld::new(pool).await;
    w.mock_tokens().await;
    mock_sites(&w).await;
    w.insert_connection(None, "choose_property").await;
    let w = &w;
    let put = |property: &'static str| async move {
        w.send(
            "test-token",
            reqwest::Method::PUT,
            &w.google_path(""),
            Some(json!({ "property": property })),
        )
        .await
    };
    let (status, body) = put("sc-domain:example.com").await;
    assert_eq!(status, 200, "{body}");
    assert_eq!(body["data"]["status"], "connected");
    assert_eq!(body["data"]["property"], "sc-domain:example.com");
    assert_eq!(w.count("jobs").await, 1);
    let payload: Value = sqlx::query_scalar("SELECT payload FROM jobs WHERE kind = 'google_sync'")
        .fetch_one(&w.pool)
        .await
        .unwrap();
    assert_eq!(payload["site_id"], w.site_id.to_string());
    assert_eq!(payload["workspace_id"], w.workspace_id.to_string());
    // Choosing again does not queue a second one.
    put("sc-domain:example.com").await;
    assert_eq!(w.count("jobs").await, 1);
}

#[sqlx::test]
async fn a_property_the_person_cannot_read_is_refused(pool: PgPool) {
    let w = GoogleWorld::new(pool).await;
    w.mock_tokens().await;
    mock_sites(&w).await;
    w.insert_connection(None, "choose_property").await;
    for property in [
        "https://elsewhere.example/",
        "https://unverified.example.com/",
        "",
        "   ",
    ] {
        let (status, _) = w
            .send(
                "test-token",
                reqwest::Method::PUT,
                &w.google_path(""),
                Some(json!({ "property": property })),
            )
            .await;
        assert_eq!(status, 422, "{property:?}");
    }
    assert_eq!(
        w.connection_status().await.as_deref(),
        Some("choose_property")
    );
    assert_eq!(w.count("jobs").await, 0);
}

#[sqlx::test]
async fn changing_property_drops_the_old_propertys_rows(pool: PgPool) {
    let w = GoogleWorld::new(pool).await;
    w.mock_tokens().await;
    mock_sites(&w).await;
    w.connect("https://zebra.example/").await;
    w.seed_rows(&[("q", "https://example.com/a", 1, 10, 3.0)])
        .await;
    let (status, _) = w
        .send(
            "test-token",
            reqwest::Method::PUT,
            &w.google_path(""),
            Some(json!({ "property": "sc-domain:example.com" })),
        )
        .await;
    assert_eq!(status, 200);
    assert_eq!(w.count("search_rows").await, 0);
}

// ---------------------------------------------------------------- sync now, disconnect

#[sqlx::test]
async fn sync_now_is_202_and_queues_one_job(pool: PgPool) {
    let w = GoogleWorld::new(pool).await;
    w.connect("https://example.com/").await;
    for _ in 0..3 {
        let (status, body) = w.post(&w.google_path("/sync")).await;
        assert_eq!(status, 202, "{body}");
    }
    assert_eq!(w.count("jobs").await, 1);
}

#[sqlx::test]
async fn sync_now_needs_a_connected_property(pool: PgPool) {
    let w = GoogleWorld::new(pool).await;
    assert_eq!(w.post(&w.google_path("/sync")).await.0, 422);
    w.insert_connection(None, "choose_property").await;
    assert_eq!(w.post(&w.google_path("/sync")).await.0, 422);
    w.insert_connection(Some("https://example.com/"), "needs_reconnecting")
        .await;
    assert_eq!(w.post(&w.google_path("/sync")).await.0, 422);
    assert_eq!(w.count("jobs").await, 0);
}

#[sqlx::test]
async fn disconnecting_deletes_the_token_and_every_row(pool: PgPool) {
    let w = GoogleWorld::new(pool).await;
    w.connect("https://example.com/").await;
    w.seed_rows(&[("q", "https://example.com/a", 1, 10, 3.0)])
        .await;
    sqlx::query("INSERT INTO search_daily (workspace_id, site_id, date, clicks, impressions) VALUES ($1, $2, '2026-11-01', 1, 2)")
        .bind(w.workspace_id).bind(w.site_id).execute(&w.pool).await.unwrap();
    sqlx::query("INSERT INTO search_previous (site_id, workspace_id, clicks, impressions, keywords) VALUES ($1, $2, 1, 2, 3)")
        .bind(w.site_id).bind(w.workspace_id).execute(&w.pool).await.unwrap();
    let (status, body) = w
        .send(
            "test-token",
            reqwest::Method::DELETE,
            &w.google_path(""),
            None,
        )
        .await;
    assert_eq!(status, 200);
    assert_eq!(body["data"]["status"], "not_connected");
    for table in [
        "google_connections",
        "search_rows",
        "search_daily",
        "search_previous",
    ] {
        assert_eq!(w.count(table).await, 0, "{table}");
    }
    // Disconnecting when nothing is connected is fine too.
    let (status, _) = w
        .send(
            "test-token",
            reqwest::Method::DELETE,
            &w.google_path(""),
            None,
        )
        .await;
    assert_eq!(status, 200);
}

#[sqlx::test]
async fn the_token_never_appears_in_any_response(pool: PgPool) {
    let w = GoogleWorld::new(pool).await;
    w.mock_tokens().await;
    mock_sites(&w).await;
    let sealed_probe = common::google::secrets().seal(REFRESH_TOKEN);
    let mut seen = Vec::new();
    let (_, body) = w.post(&w.google_path("/start")).await;
    seen.push(body);
    let state = w.new_state().await;
    let (_, location) = w.callback(&format!("code=c&state={state}")).await;
    seen.push(json!(location));
    let (_, body) = w.get(&w.google_path("")).await;
    seen.push(body);
    let (_, body) = w.get(&w.google_path("/properties")).await;
    seen.push(body);
    let (_, body) = w
        .send(
            "test-token",
            reqwest::Method::PUT,
            &w.google_path(""),
            Some(json!({ "property": "sc-domain:example.com" })),
        )
        .await;
    seen.push(body);
    let stored: String = sqlx::query_scalar(
        "SELECT refresh_token_sealed FROM google_connections WHERE site_id = $1",
    )
    .bind(w.site_id)
    .fetch_one(&w.pool)
    .await
    .unwrap();
    for body in seen {
        let text = body.to_string();
        for secret in [
            REFRESH_TOKEN,
            ACCESS_TOKEN,
            "test-client-secret",
            stored.as_str(),
            sealed_probe.as_str(),
        ] {
            assert!(!text.contains(secret), "{secret} leaked in {text}");
        }
    }
}

// ---------------------------------------------------------------- search data endpoints

#[sqlx::test]
async fn queries_and_pages_are_paged_and_sorted(pool: PgPool) {
    let w = GoogleWorld::new(pool).await;
    w.connect("https://example.com/").await;
    w.seed_rows(&[
        ("blue widgets", "https://example.com/a", 10, 100, 4.0),
        ("blue widgets", "https://example.com/b", 0, 300, 8.0),
        ("red widgets", "https://example.com/a", 50, 200, 2.0),
        ("green", "https://example.com/c", 1, 10, 12.0),
    ])
    .await;
    let base = format!("/api/v1/sites/{}/search", w.site_id);
    let (status, body) = w.get(&format!("{base}/queries")).await;
    assert_eq!(status, 200);
    let names = |body: &Value| -> Vec<String> {
        body["data"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| r["query"].as_str().unwrap().to_string())
            .collect()
    };
    assert_eq!(names(&body), ["red widgets", "blue widgets", "green"]);
    assert_eq!(body["meta"]["total"], 3);
    let blue = &body["data"][1];
    assert_eq!(blue["clicks"], 10);
    assert_eq!(blue["impressions"], 400);
    assert_eq!(blue["top_page"], "https://example.com/b");
    assert!((blue["ctr"].as_f64().unwrap() - 0.025).abs() < 1e-9);
    assert!((blue["position"].as_f64().unwrap() - 7.0).abs() < 1e-9);

    let (_, body) = w
        .get(&format!("{base}/queries?sort=impressions&limit=2"))
        .await;
    assert_eq!(names(&body), ["blue widgets", "red widgets"]);
    let (_, body) = w
        .get(&format!("{base}/queries?sort=position&limit=1&page=2"))
        .await;
    assert_eq!(names(&body), ["blue widgets"]);
    assert_eq!(w.get(&format!("{base}/queries?sort=bogus")).await.0, 422);

    let (status, body) = w.get(&format!("{base}/pages")).await;
    assert_eq!(status, 200);
    assert_eq!(body["meta"]["total"], 3);
    let first = &body["data"][0];
    assert_eq!(first["url"], "https://example.com/a");
    assert_eq!(first["clicks"], 60);
    assert_eq!(first["impressions"], 300);
    let (_, body) = w.get(&format!("{base}/pages?limit=1&page=3")).await;
    assert_eq!(body["data"][0]["url"], "https://example.com/c");
}

#[sqlx::test]
async fn search_endpoints_are_empty_without_a_connection(pool: PgPool) {
    let w = GoogleWorld::new(pool).await;
    let base = format!("/api/v1/sites/{}/search", w.site_id);
    for tail in ["queries", "pages"] {
        let (status, body) = w.get(&format!("{base}/{tail}")).await;
        assert_eq!(status, 200);
        assert_eq!(body["data"], json!([]));
        assert_eq!(body["meta"]["total"], 0);
    }
}
