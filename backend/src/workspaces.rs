use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppError;

pub struct UserRecord {
    pub user_id: Uuid,
    pub workspace_id: Uuid,
    pub email: String,
}

pub async fn find_or_create(pool: &PgPool, email: &str) -> Result<UserRecord, AppError> {
    if let Some(existing) = lookup(pool, email).await? {
        return Ok(existing);
    }
    insert_user(pool, email).await
}

pub async fn email_of(
    pool: &PgPool,
    user_id: Uuid,
    workspace_id: Uuid,
) -> Result<String, AppError> {
    sqlx::query_scalar("SELECT email FROM users WHERE id = $1 AND workspace_id = $2")
        .bind(user_id)
        .bind(workspace_id)
        .fetch_optional(pool)
        .await
        .map_err(internal)?
        .ok_or(AppError::NotFound)
}

async fn lookup(pool: &PgPool, email: &str) -> Result<Option<UserRecord>, AppError> {
    let row: Option<(Uuid, Uuid, String)> =
        sqlx::query_as("SELECT id, workspace_id, email FROM users WHERE email = $1")
            .bind(email)
            .fetch_optional(pool)
            .await
            .map_err(internal)?;
    Ok(row.map(|(user_id, workspace_id, email)| UserRecord {
        user_id,
        workspace_id,
        email,
    }))
}

async fn insert_user(pool: &PgPool, email: &str) -> Result<UserRecord, AppError> {
    let mut tx = pool.begin().await.map_err(internal)?;
    let workspace_id = Uuid::new_v4();
    let user_id = Uuid::new_v4();
    let name = email.split('@').next().unwrap_or("Workspace");
    sqlx::query("INSERT INTO workspaces (id, name) VALUES ($1, $2)")
        .bind(workspace_id)
        .bind(name)
        .execute(&mut *tx)
        .await
        .map_err(internal)?;
    let inserted = sqlx::query(
        "INSERT INTO users (id, workspace_id, email) VALUES ($1, $2, $3) ON CONFLICT (email) DO NOTHING",
    )
    .bind(user_id)
    .bind(workspace_id)
    .bind(email)
    .execute(&mut *tx)
    .await
    .map_err(internal)?;
    if inserted.rows_affected() == 0 {
        tx.rollback().await.map_err(internal)?;
        return lookup(pool, email)
            .await?
            .ok_or_else(|| AppError::Internal(anyhow::anyhow!("user vanished after conflict")));
    }
    tx.commit().await.map_err(internal)?;
    Ok(UserRecord {
        user_id,
        workspace_id,
        email: email.to_string(),
    })
}

fn internal(err: sqlx::Error) -> AppError {
    AppError::Internal(err.into())
}
