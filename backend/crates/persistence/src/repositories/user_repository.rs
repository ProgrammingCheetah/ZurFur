use crate::pool::Pool;
use domain::user::{User, UserError, UserRepository};
use sqlx::Row;
use std::sync::Arc;
use uuid::Uuid;

/// SQLx implementation of `UserRepository`.
pub struct SqlxUserRepository {
    pool: Pool,
}

impl SqlxUserRepository {
    /// Create a new repository instance.
    pub fn new(pool: Pool) -> Self {
        Self { pool }
    }

    /// Create a new repository instance wrapped as a trait object.
    pub fn from_pool(pool: Pool) -> Arc<dyn UserRepository> {
        Arc::new(Self::new(pool))
    }
}

fn map_user(row: sqlx::postgres::PgRow) -> User {
    User {
        id: row.get("id"),
        did: row.get("did"),
        handle: row.get("handle"),
        email: row.get("email"),
        username: row.get("username"),
        onboarding_completed_at: row.get("onboarding_completed_at"),
    }
}

#[async_trait::async_trait]
impl UserRepository for SqlxUserRepository {
    async fn find_by_email(&self, email: &str) -> Result<Option<User>, UserError> {
        sqlx::query("SELECT id, did, handle, email, username, onboarding_completed_at FROM users WHERE email = $1 AND deleted_at IS NULL")
            .bind(email)
            .fetch_optional(&self.pool)
            .await
            .map(|opt| opt.map(map_user))
            .map_err(|e| UserError::Database(e.to_string()))
    }

    async fn find_by_id(&self, id: Uuid) -> Result<Option<User>, UserError> {
        sqlx::query("SELECT id, did, handle, email, username, onboarding_completed_at FROM users WHERE id = $1 AND deleted_at IS NULL")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map(|opt| opt.map(map_user))
            .map_err(|e| UserError::Database(e.to_string()))
    }

    async fn find_by_did(&self, did: &str) -> Result<Option<User>, UserError> {
        sqlx::query("SELECT id, did, handle, email, username, onboarding_completed_at FROM users WHERE did = $1 AND deleted_at IS NULL")
            .bind(did)
            .fetch_optional(&self.pool)
            .await
            .map(|opt| opt.map(map_user))
            .map_err(|e| UserError::Database(e.to_string()))
    }

    async fn create_from_atproto(
        &self,
        did: &str,
        handle: Option<&str>,
        email: Option<&str>,
    ) -> Result<User, UserError> {
        let id = Uuid::new_v4();
        let username = handle
            .map(|h| h.strip_suffix(".bsky.social").unwrap_or(h).to_string())
            .unwrap_or_else(|| did.to_string());

        sqlx::query(
            "INSERT INTO users (id, did, handle, email, username) VALUES ($1, $2, $3, $4, $5) RETURNING id, did, handle, email, username, onboarding_completed_at",
        )
        .bind(id)
        .bind(did)
        .bind(handle)
        .bind(email)
        .bind(&username)
        .fetch_one(&self.pool)
        .await
        .map(map_user)
        .map_err(|e| UserError::Database(e.to_string()))
    }

    async fn update_handle(&self, user_id: Uuid, handle: &str) -> Result<(), UserError> {
        sqlx::query("UPDATE users SET handle = $1 WHERE id = $2 AND deleted_at IS NULL")
            .bind(handle)
            .bind(user_id)
            .execute(&self.pool)
            .await
            .map(|_| ())
            .map_err(|e| UserError::Database(e.to_string()))
    }

    async fn mark_onboarding_completed(&self, user_id: Uuid) -> Result<(), UserError> {
        // Idempotent: if already completed, treat as success.
        // Only return NotFound if the user truly doesn't exist.
        let result = sqlx::query(
            "UPDATE users SET onboarding_completed_at = now() \
             WHERE id = $1 AND deleted_at IS NULL AND onboarding_completed_at IS NULL",
        )
        .bind(user_id)
        .execute(&self.pool)
        .await
        .map_err(|e| UserError::Database(e.to_string()))?;

        if result.rows_affected() == 0 {
            // Check if user exists (may already be onboarded)
            let exists = sqlx::query(
                "SELECT id FROM users WHERE id = $1 AND deleted_at IS NULL",
            )
            .bind(user_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| UserError::Database(e.to_string()))?;

            if exists.is_none() {
                return Err(UserError::NotFound);
            }
        }
        Ok(())
    }
}
