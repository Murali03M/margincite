//! Which worker loop takes which kind of job. Runs that take minutes each get
//! a loop of their own, so crawls and syncs (the last lane) never wait behind
//! them.

use super::queue::Lane;
use crate::ai::run::KIND as AI;
use crate::keywords::volumes::KIND as VOLUMES;
use crate::rankings::run::KIND as RANK;
use crate::speed::run::KIND as SPEED;

/// AI runs, rank runs, then page speed runs with search volume refreshes
/// (up to five pages at 90 seconds each, and one request of up to 130
/// seconds), then everything else.
pub fn lanes() -> [Lane; 4] {
    [
        Lane::Only(&[AI]),
        Lane::Only(&[RANK]),
        Lane::Only(&[SPEED, VOLUMES]),
        Lane::Except(&[AI, RANK, SPEED, VOLUMES]),
    ]
}
