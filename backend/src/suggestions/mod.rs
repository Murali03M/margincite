//! Suggestions: findings grouped by check, kept across crawls, ranked.

pub mod rank;
pub mod reach;
pub mod reconcile;
pub mod repo;
pub mod routes;

use serde::{Deserialize, Serialize};

/// Where one item (one page or the site) stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Open,
    Done,
    Dismissed,
}

impl Status {
    pub fn as_str(self) -> &'static str {
        match self {
            Status::Open => "open",
            Status::Done => "done",
            Status::Dismissed => "dismissed",
        }
    }

    pub fn parse(text: &str) -> Option<Status> {
        [Status::Open, Status::Done, Status::Dismissed]
            .into_iter()
            .find(|status| status.as_str() == text)
    }
}
