use std::collections::{HashMap, VecDeque};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use std::net::SocketAddr;

use axum::extract::{ConnectInfo, State};
use axum::http::Request;
use axum::middleware::Next;
use axum::response::Response;
use axum::routing::get;
use axum::{middleware, Json, Router};
use serde::Serialize;
use uuid::Uuid;

use crate::auth::Principal;
use crate::envelope::Envelope;
use crate::error::AppError;
use crate::workspaces;
use crate::AppState;

#[derive(Serialize)]
struct Health {
    status: &'static str,
}

#[derive(Serialize)]
struct Me {
    user_id: Uuid,
    workspace_id: Uuid,
    email: String,
}

/// A sliding window of hits per key. Keys whose window has fully expired are
/// dropped, so memory follows the callers seen in the last window, not all
/// callers ever seen.
pub struct RateLimiter {
    max: usize,
    window: Duration,
    state: Mutex<Windows>,
}

struct Windows {
    hits: HashMap<String, VecDeque<Instant>>,
    last_sweep: Instant,
}

impl RateLimiter {
    pub fn new(max: usize) -> Self {
        Self::with_window(max, Duration::from_secs(60))
    }

    pub fn with_window(max: usize, window: Duration) -> Self {
        Self {
            max,
            window,
            state: Mutex::new(Windows {
                hits: HashMap::new(),
                last_sweep: Instant::now(),
            }),
        }
    }

    pub fn allow(&self, key: &str) -> bool {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let now = Instant::now();
        if now.duration_since(state.last_sweep) >= self.window {
            let window = self.window;
            state.hits.retain(|_, hits| {
                hits.back()
                    .is_some_and(|at| now.duration_since(*at) <= window)
            });
            state.last_sweep = now;
        }
        let hits = state.hits.entry(key.to_string()).or_default();
        while hits
            .front()
            .is_some_and(|at| now.duration_since(*at) > self.window)
        {
            hits.pop_front();
        }
        if hits.len() >= self.max {
            return false;
        }
        hits.push_back(now);
        true
    }

    /// How many keys are being remembered.
    #[cfg(test)]
    fn tracked(&self) -> usize {
        self.state
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .hits
            .len()
    }
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/healthz", get(health))
        .route("/api/v1/me", get(me))
        .merge(crate::sites::routes::routes())
        .merge(crate::crawl::routes::routes())
        .merge(crate::suggestions::routes::routes())
        .merge(crate::integrations::routes())
        .merge(crate::keywords::routes::routes())
        .merge(crate::metrics::routes::routes())
        .merge(crate::ai::routes::routes())
        .merge(crate::speed::routes::routes())
        .merge(crate::rankings::routes::routes())
        .merge(crate::settings::routes::routes())
        .fallback(unknown)
        .layer(middleware::from_fn_with_state(
            state.clone(),
            limit_requests,
        ))
        .with_state(state)
}

async fn health() -> Json<Envelope<Health>> {
    Json(Envelope::ok(Health { status: "ok" }))
}

async fn me(
    State(state): State<AppState>,
    principal: Principal,
) -> Result<Json<Envelope<Me>>, AppError> {
    let email =
        workspaces::email_of(&state.pool, principal.user_id, principal.workspace_id).await?;
    Ok(Json(Envelope::ok(Me {
        user_id: principal.user_id,
        workspace_id: principal.workspace_id,
        email,
    })))
}

async fn unknown() -> AppError {
    AppError::NotFound
}

/// Every request except `/healthz` counts against its caller's address. The
/// bearer token is never part of the key, so inventing tokens buys nothing.
/// Signed-in callers also count against their workspace, see `Principal`.
async fn limit_requests(
    State(state): State<AppState>,
    request: Request<axum::body::Body>,
    next: Next,
) -> Result<Response, AppError> {
    if request.uri().path() != "/healthz" && !state.limiter.allow(&limit_key(&request)) {
        return Err(AppError::RateLimited);
    }
    Ok(next.run(request).await)
}

fn limit_key(request: &Request<axum::body::Body>) -> String {
    request
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map(|ConnectInfo(addr)| format!("ip:{}", addr.ip()))
        .unwrap_or_else(|| "ip:unknown".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_key_is_held_to_its_allowance_until_the_window_passes() {
        let limiter = RateLimiter::with_window(2, Duration::from_millis(60));
        assert!(limiter.allow("a"));
        assert!(limiter.allow("a"));
        assert!(!limiter.allow("a"));
        assert!(limiter.allow("b"));
        std::thread::sleep(Duration::from_millis(90));
        assert!(limiter.allow("a"));
    }

    #[test]
    fn expired_windows_are_removed() {
        let limiter = RateLimiter::with_window(5, Duration::from_millis(40));
        for n in 0..100 {
            limiter.allow(&format!("caller-{n}"));
        }
        assert_eq!(limiter.tracked(), 100);
        std::thread::sleep(Duration::from_millis(80));
        limiter.allow("fresh");
        assert_eq!(limiter.tracked(), 1);
    }

    #[test]
    fn a_window_still_in_use_is_kept() {
        let limiter = RateLimiter::with_window(5, Duration::from_millis(80));
        limiter.allow("old");
        std::thread::sleep(Duration::from_millis(50));
        limiter.allow("busy");
        std::thread::sleep(Duration::from_millis(50));
        limiter.allow("fresh");
        assert_eq!(limiter.tracked(), 2);
    }

    #[test]
    fn the_key_is_the_callers_address_and_ignores_the_token() {
        let mut request = Request::builder()
            .header("authorization", "Bearer made-up-1")
            .body(axum::body::Body::empty())
            .unwrap();
        request.extensions_mut().insert(ConnectInfo(
            "203.0.113.9:5000".parse::<SocketAddr>().unwrap(),
        ));
        let first = limit_key(&request);
        request
            .headers_mut()
            .insert("authorization", "Bearer made-up-2".parse().unwrap());
        assert_eq!(first, "ip:203.0.113.9");
        assert_eq!(limit_key(&request), first);
    }
}
