//! Request readers that answer a request they cannot read in the usual
//! envelope, with a plain message that never quotes what was sent. Axum's own
//! readers answer with plain text that repeats the parser's complaint.

use axum::body::Bytes;
use axum::extract::{FromRequest, FromRequestParts, Query, Request};
use axum::http::request::Parts;
use serde::de::DeserializeOwned;

use crate::error::AppError;
use crate::AppState;

const BAD_BODY: &str = "The request could not be read. Check the fields and try again.";
const BAD_QUERY: &str = "The address could not be read. Check the values and try again.";

/// A JSON body that is parsed here, so a failure says nothing about what was
/// sent.
pub struct Body<T>(pub T);

impl<T: DeserializeOwned> FromRequest<AppState> for Body<T> {
    type Rejection = AppError;

    async fn from_request(req: Request, state: &AppState) -> Result<Self, Self::Rejection> {
        let bytes = Bytes::from_request(req, state)
            .await
            .map_err(|_| AppError::Invalid(BAD_BODY.into()))?;
        serde_json::from_slice(&bytes)
            .map(Body)
            .map_err(|_| AppError::Invalid(BAD_BODY.into()))
    }
}

/// A query string that is parsed here, for the same reason.
pub struct Params<T>(pub T);

impl<T: DeserializeOwned> FromRequestParts<AppState> for Params<T> {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        Query::<T>::from_request_parts(parts, state)
            .await
            .map(|Query(value)| Params(value))
            .map_err(|_| AppError::Invalid(BAD_QUERY.into()))
    }
}
