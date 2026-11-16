//! DataForSEO: Google search results (rank tracking, AI Overviews) and Google
//! Ads keyword data (search volume, keyword ideas). Paid per request, and only
//! ever called with the workspace's own login.

pub mod client;
pub mod keywords;
pub mod locations;
pub mod pricing;
pub mod serp;

pub use client::{Credentials, DataForSeoClient, DataForSeoError};
