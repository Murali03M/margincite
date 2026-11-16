//! Bing Webmaster Tools: the API key, the weekly sync and the backlinks it
//! brings in.

pub mod client;
pub mod parse;
pub mod repo;
pub mod routes;
pub mod sync;

pub use client::BingClient;
