//! Google Search Console: sign-in, the property choice, the sync job and the
//! numbers it brings in.

pub mod client;
pub mod oauth;
pub mod repo;
pub mod routes;
pub mod search;
pub mod sync;

pub use client::{GoogleClient, GoogleHosts};
pub use oauth::hash_nonce;
