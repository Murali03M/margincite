//! The numbers on a site's project card. Each block is computed on its own:
//! one that cannot be loaded says so and never takes the others with it.
//!
//! To fill a block, replace the body of its `load` function with the real
//! query and return the block struct with its values set and `reason: None`.

pub mod ai;
pub mod ai_overviews;
pub mod backlinks;
pub mod organic;
pub mod rankings;
pub mod routes;
pub mod site_health;
pub mod speed;
pub mod visibility;

use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppError;

/// What a block says when its query failed.
pub const COULD_NOT_LOAD: &str = "Could not load this number right now.";

/// A block that can stand in for itself when it could not be loaded.
pub trait Block {
    fn unavailable(reason: &str) -> Self;
}

/// The block, or the "could not load" block with the cause logged.
pub fn settle<T: Block>(name: &str, site_id: Uuid, loaded: Result<T, AppError>) -> T {
    loaded.unwrap_or_else(|err| {
        tracing::error!(block = name, %site_id, error = %err, "could not load a metrics block");
        T::unavailable(COULD_NOT_LOAD)
    })
}

#[derive(Debug, Serialize)]
pub struct Metrics {
    pub site_health: site_health::SiteHealth,
    pub ai_visibility: ai::AiVisibility,
    pub visibility: visibility::Visibility,
    pub organic: organic::Organic,
    pub backlinks: backlinks::Backlinks,
    pub speed: speed::Speed,
    pub rankings: rankings::Rankings,
    pub ai_overviews: ai_overviews::AiOverviews,
}

pub async fn load(pool: &PgPool, workspace_id: Uuid, site_id: Uuid) -> Metrics {
    Metrics {
        site_health: settle(
            "site_health",
            site_id,
            site_health::load(pool, workspace_id, site_id).await,
        ),
        ai_visibility: settle(
            "ai_visibility",
            site_id,
            ai::load(pool, workspace_id, site_id).await,
        ),
        visibility: settle(
            "visibility",
            site_id,
            visibility::load(pool, workspace_id, site_id).await,
        ),
        organic: settle(
            "organic",
            site_id,
            organic::load(pool, workspace_id, site_id).await,
        ),
        backlinks: settle(
            "backlinks",
            site_id,
            backlinks::load(pool, workspace_id, site_id).await,
        ),
        speed: settle(
            "speed",
            site_id,
            speed::load(pool, workspace_id, site_id).await,
        ),
        rankings: settle(
            "rankings",
            site_id,
            rankings::load(pool, workspace_id, site_id).await,
        ),
        ai_overviews: settle(
            "ai_overviews",
            site_id,
            ai_overviews::load(pool, workspace_id, site_id).await,
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, PartialEq)]
    struct Fake(Option<String>);

    impl Block for Fake {
        fn unavailable(reason: &str) -> Self {
            Fake(Some(reason.to_string()))
        }
    }

    #[test]
    fn a_failed_block_becomes_its_could_not_load_form() {
        let failed: Result<Fake, AppError> = Err(AppError::Internal(anyhow::anyhow!("boom")));
        assert_eq!(
            settle("fake", Uuid::nil(), failed),
            Fake(Some(COULD_NOT_LOAD.into()))
        );
        assert_eq!(settle("fake", Uuid::nil(), Ok(Fake(None))), Fake(None));
    }
}
