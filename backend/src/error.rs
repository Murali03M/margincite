use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;

use crate::envelope::Envelope;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("Not found")]
    NotFound,
    #[error("{0}")]
    Invalid(String),
    #[error("Sign in to continue.")]
    Unauthorized,
    #[error("{0}")]
    Conflict(String),
    #[error("{0}")]
    BadGateway(String),
    #[error("Too many requests.")]
    RateLimited,
    #[error("{0}")]
    DailyLimit(String),
    #[error(transparent)]
    Internal(#[from] anyhow::Error),
}

impl AppError {
    fn status(&self) -> StatusCode {
        match self {
            AppError::NotFound => StatusCode::NOT_FOUND,
            AppError::Invalid(_) => StatusCode::UNPROCESSABLE_ENTITY,
            AppError::Unauthorized => StatusCode::UNAUTHORIZED,
            AppError::Conflict(_) => StatusCode::CONFLICT,
            AppError::RateLimited | AppError::DailyLimit(_) => StatusCode::TOO_MANY_REQUESTS,
            AppError::BadGateway(_) => StatusCode::BAD_GATEWAY,
            AppError::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    fn public_message(&self) -> String {
        match self {
            AppError::Internal(err) => {
                tracing::error!(error = %err, "request failed");
                "Something went wrong on our side.".to_string()
            }
            other => other.to_string(),
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let status = self.status();
        let body = Envelope::<()>::err(self.public_message());
        (status, Json(body)).into_response()
    }
}
