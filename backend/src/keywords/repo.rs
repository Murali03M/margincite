//! Target keywords. Every function takes the workspace and filters by it.

use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::checks::context::KeywordTarget;
use crate::error::AppError;

/// Keywords one site may hold.
pub const MAX_KEYWORDS: i64 = 50;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct KeywordRow {
    pub id: Uuid,
    pub site_id: Uuid,
    pub keyword: String,
    pub target_url: Option<String>,
    /// Monthly searches, from Google Ads data via DataForSEO. `None` until
    /// refreshed, and when Google has no data.
    pub search_volume: Option<i32>,
    pub competition: Option<String>,
    pub cpc: Option<f64>,
}

fn internal(err: sqlx::Error) -> AppError {
    AppError::Internal(err.into())
}

pub async fn list(
    pool: &PgPool,
    workspace_id: Uuid,
    site_id: Uuid,
) -> Result<Vec<KeywordRow>, AppError> {
    sqlx::query_as(
        "SELECT id, site_id, keyword, target_url, search_volume, competition, cpc FROM site_keywords
         WHERE workspace_id = $1 AND site_id = $2 ORDER BY created_at, id",
    )
    .bind(workspace_id)
    .bind(site_id)
    .fetch_all(pool)
    .await
    .map_err(internal)
}

/// The keywords as the checks read them, oldest first.
pub async fn targets(
    pool: &PgPool,
    workspace_id: Uuid,
    site_id: Uuid,
) -> Result<Vec<KeywordTarget>, AppError> {
    Ok(list(pool, workspace_id, site_id)
        .await?
        .into_iter()
        .map(|row| KeywordTarget {
            keyword: row.keyword,
            target_url: row.target_url,
        })
        .collect())
}

pub async fn get(pool: &PgPool, workspace_id: Uuid, id: Uuid) -> Result<KeywordRow, AppError> {
    sqlx::query_as(
        "SELECT id, site_id, keyword, target_url, search_volume, competition, cpc FROM site_keywords
         WHERE id = $1 AND workspace_id = $2",
    )
    .bind(id)
    .bind(workspace_id)
    .fetch_optional(pool)
    .await
    .map_err(internal)?
    .ok_or(AppError::NotFound)
}

/// Adds a keyword. The site row is locked while counting, so two requests at
/// once cannot both take the last place.
pub async fn create(
    pool: &PgPool,
    workspace_id: Uuid,
    site_id: Uuid,
    keyword: &str,
    target_url: Option<&str>,
) -> Result<KeywordRow, AppError> {
    let mut tx = pool.begin().await.map_err(internal)?;
    let locked = sqlx::query("SELECT id FROM sites WHERE id = $1 AND workspace_id = $2 FOR UPDATE")
        .bind(site_id)
        .bind(workspace_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(internal)?;
    if locked.is_none() {
        return Err(AppError::NotFound);
    }
    let count: i64 = sqlx::query("SELECT COUNT(*) FROM site_keywords WHERE site_id = $1")
        .bind(site_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(internal)?
        .get(0);
    if count >= MAX_KEYWORDS {
        return Err(AppError::Conflict(
            "A site can have at most 50 keywords.".into(),
        ));
    }
    let inserted = sqlx::query_as::<_, KeywordRow>(
        "INSERT INTO site_keywords (id, workspace_id, site_id, keyword, target_url)
         VALUES ($1, $2, $3, $4, $5) RETURNING id, site_id, keyword, target_url, search_volume, competition, cpc",
    )
    .bind(Uuid::new_v4())
    .bind(workspace_id)
    .bind(site_id)
    .bind(keyword)
    .bind(target_url)
    .fetch_one(&mut *tx)
    .await;
    match inserted {
        Ok(row) => {
            tx.commit().await.map_err(internal)?;
            Ok(row)
        }
        Err(err)
            if err
                .as_database_error()
                .is_some_and(|db| db.is_unique_violation()) =>
        {
            Err(AppError::Conflict(
                "This keyword is already on the list.".into(),
            ))
        }
        Err(err) => Err(internal(err)),
    }
}

pub async fn set_target(
    pool: &PgPool,
    workspace_id: Uuid,
    id: Uuid,
    target_url: Option<&str>,
) -> Result<KeywordRow, AppError> {
    sqlx::query_as(
        "UPDATE site_keywords SET target_url = $3 WHERE id = $1 AND workspace_id = $2
         RETURNING id, site_id, keyword, target_url, search_volume, competition, cpc",
    )
    .bind(id)
    .bind(workspace_id)
    .bind(target_url)
    .fetch_optional(pool)
    .await
    .map_err(internal)?
    .ok_or(AppError::NotFound)
}

/// Deletes the keyword and returns the site it belonged to.
pub async fn delete(pool: &PgPool, workspace_id: Uuid, id: Uuid) -> Result<Uuid, AppError> {
    sqlx::query_scalar(
        "DELETE FROM site_keywords WHERE id = $1 AND workspace_id = $2 RETURNING site_id",
    )
    .bind(id)
    .bind(workspace_id)
    .fetch_optional(pool)
    .await
    .map_err(internal)?
    .ok_or(AppError::NotFound)
}

/// Keyword ideas a workspace may ask for in a UTC day.
pub const MAX_IDEAS_PER_DAY: i64 = 20;

/// Counts a keyword idea search against the workspace's allowance for today
/// (UTC), or says it is used up. Counting and recording happen under one lock,
/// so two searches at once cannot both take the last place.
pub async fn reserve_idea_search(pool: &PgPool, workspace_id: Uuid) -> Result<(), AppError> {
    let mut tx = pool.begin().await.map_err(internal)?;
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))")
        .bind(format!("keyword_ideas:{workspace_id}"))
        .execute(&mut *tx)
        .await
        .map_err(internal)?;
    let used: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM keyword_idea_requests
         WHERE workspace_id = $1
           AND created_at >= (date_trunc('day', now() AT TIME ZONE 'UTC') AT TIME ZONE 'UTC')",
    )
    .bind(workspace_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(internal)?;
    if used >= MAX_IDEAS_PER_DAY {
        return Err(AppError::DailyLimit(
            "You have searched for keyword ideas 20 times today. Try again tomorrow.".into(),
        ));
    }
    sqlx::query("INSERT INTO keyword_idea_requests (workspace_id) VALUES ($1)")
        .bind(workspace_id)
        .execute(&mut *tx)
        .await
        .map_err(internal)?;
    tx.commit().await.map_err(internal)
}

/// Stores what the Google Ads data said about a site's keywords, matched
/// without regard to case (the provider lowercases). Every keyword in `asked`
/// is marked as checked: one the answer does not mention gets no numbers.
pub async fn save_metrics(
    pool: &PgPool,
    workspace_id: Uuid,
    site_id: Uuid,
    asked: &[String],
    found: &[crate::integrations::dataforseo::keywords::KeywordMetrics],
) -> Result<(), AppError> {
    let lowered: Vec<String> = asked.iter().map(|keyword| keyword.to_lowercase()).collect();
    let names: Vec<String> = found.iter().map(|m| m.keyword.to_lowercase()).collect();
    let volumes: Vec<Option<i32>> = found.iter().map(|m| m.search_volume).collect();
    let competition: Vec<Option<String>> = found.iter().map(|m| m.competition.clone()).collect();
    let cpc: Vec<Option<f64>> = found.iter().map(|m| m.cpc).collect();
    sqlx::query(
        "UPDATE site_keywords k
         SET search_volume = m.search_volume, competition = m.competition, cpc = m.cpc,
             metrics_checked_at = now()
         FROM (SELECT w.keyword, f.search_volume, f.competition, f.cpc
               FROM UNNEST($3::text[]) AS w(keyword)
               LEFT JOIN (SELECT DISTINCT ON (name) name, search_volume, competition, cpc
                          FROM UNNEST($4::text[], $5::int[], $6::text[], $7::float8[])
                               AS x(name, search_volume, competition, cpc)) f
                      ON f.name = w.keyword) m
         WHERE k.workspace_id = $1 AND k.site_id = $2 AND lower(k.keyword) = m.keyword",
    )
    .bind(workspace_id)
    .bind(site_id)
    .bind(&lowered)
    .bind(&names)
    .bind(&volumes)
    .bind(&competition)
    .bind(&cpc)
    .execute(pool)
    .await
    .map_err(internal)?;
    Ok(())
}
