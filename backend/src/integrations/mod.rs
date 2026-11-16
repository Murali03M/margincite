//! Outside services Margincite connects to. Each one reports whether it is
//! set up without ever exposing a key.

pub mod bing;
pub mod dataforseo;
pub mod google;
pub mod pagespeed;
pub mod status;

use axum::Router;

use crate::AppState;

pub fn routes() -> Router<AppState> {
    status::routes()
        .merge(google::routes::routes())
        .merge(bing::routes::routes())
}
