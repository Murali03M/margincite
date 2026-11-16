use async_trait::async_trait;
use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use sqlx::PgPool;
use subtle::ConstantTimeEq;
use uuid::Uuid;

use crate::error::AppError;
use crate::workspaces;
use crate::AppState;

#[derive(Debug, Clone)]
pub struct Principal {
    pub user_id: Uuid,
    pub workspace_id: Uuid,
}

#[async_trait]
pub trait Authenticator: Send + Sync {
    async fn authenticate(&self, bearer: &str) -> Result<Principal, AppError>;
}

pub struct DevTokenAuthenticator {
    pool: PgPool,
    tokens: Vec<(String, String)>,
}

impl DevTokenAuthenticator {
    pub fn new(pool: PgPool, tokens: Vec<(String, String)>) -> Self {
        Self { pool, tokens }
    }

    fn email_for(&self, bearer: &str) -> Option<&str> {
        self.tokens.iter().find_map(|(token, email)| {
            if constant_time_eq(token, bearer) {
                Some(email.as_str())
            } else {
                None
            }
        })
    }
}

#[async_trait]
impl Authenticator for DevTokenAuthenticator {
    async fn authenticate(&self, bearer: &str) -> Result<Principal, AppError> {
        let Some(email) = self.email_for(bearer) else {
            return Err(AppError::Unauthorized);
        };
        let user = workspaces::find_or_create(&self.pool, email).await?;
        Ok(Principal {
            user_id: user.user_id,
            workspace_id: user.workspace_id,
        })
    }
}

fn constant_time_eq(stored: &str, offered: &str) -> bool {
    let stored = stored.as_bytes();
    let offered = offered.as_bytes();
    if stored.len() != offered.len() {
        let _ = stored.ct_eq(stored);
        return false;
    }
    stored.ct_eq(offered).into()
}

impl FromRequestParts<AppState> for Principal {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let header = parts
            .headers
            .get(axum::http::header::AUTHORIZATION)
            .and_then(|value| value.to_str().ok());
        let Some(bearer) = header.and_then(|value| value.strip_prefix("Bearer ")) else {
            return Err(AppError::Unauthorized);
        };
        if bearer.is_empty() {
            return Err(AppError::Unauthorized);
        }
        let principal = state.auth.authenticate(bearer).await?;
        if !state
            .workspace_limiter
            .allow(&principal.workspace_id.to_string())
        {
            return Err(AppError::RateLimited);
        }
        Ok(principal)
    }
}
