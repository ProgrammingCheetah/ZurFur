use crate::pool::Pool;
use chrono::{DateTime, Utc};
use domain::refresh_token::{RefreshTokenEntity, RefreshTokenRepository};
use domain::user::UserError;
use sqlx::Row;
use std::sync::Arc;
use uuid::Uuid;

/// SQLx implementation of `RefreshTokenRepository`.
pub struct SqlxRefreshTokenRepository {
    pool: Pool,
}

impl SqlxRefreshTokenRepository {
    /// Create a new repository instance.
    pub fn new(pool: Pool) -> Self {
        Self { pool }
    }

    /// Create a new repository instance wrapped as a trait object.
    pub fn from_pool(pool: Pool) -> Arc<dyn RefreshTokenRepository> {
        Arc::new(Self::new(pool))
    }
}

fn map_refresh_token(row: sqlx::postgres::PgRow) -> RefreshTokenEntity {
    RefreshTokenEntity {
        id: row.get("id"),
        user_id: row.get("user_id"),
        token_hash: row.get("token_hash"),
        expires_at: row.get("expires_at"),
        created_at: row.get("created_at"),
    }
}

#[async_trait::async_trait]
impl RefreshTokenRepository for SqlxRefreshTokenRepository {
    async fn create(
        &self,
        user_id: Uuid,
        token_hash: &str,
        expires_at: DateTime<Utc>,
    ) -> Result<(), UserError> {
        sqlx::query("INSERT INTO refresh_token (user_id, token_hash, expires_at) VALUES ($1, $2, $3)")
            .bind(user_id)
            .bind(token_hash)
            .bind(expires_at)
            .execute(&self.pool)
            .await
            .map(|_| ())
            .map_err(|e| UserError::Database(e.to_string()))
    }

    async fn find_by_hash(
        &self,
        token_hash: &str,
    ) -> Result<Option<RefreshTokenEntity>, UserError> {
        sqlx::query("SELECT id, user_id, token_hash, expires_at, created_at FROM refresh_token WHERE token_hash = $1")
            .bind(token_hash)
            .fetch_optional(&self.pool)
            .await
            .map(|opt| opt.map(map_refresh_token))
            .map_err(|e| UserError::Database(e.to_string()))
    }

    async fn take_by_hash(
        &self,
        token_hash: &str,
    ) -> Result<Option<RefreshTokenEntity>, UserError> {
        sqlx::query("DELETE FROM refresh_token WHERE token_hash = $1 RETURNING id, user_id, token_hash, expires_at, created_at")
            .bind(token_hash)
            .fetch_optional(&self.pool)
            .await
            .map(|opt| opt.map(map_refresh_token))
            .map_err(|e| UserError::Database(e.to_string()))
    }

    async fn delete_all_for_user(&self, user_id: Uuid) -> Result<(), UserError> {
        sqlx::query("DELETE FROM refresh_token WHERE user_id = $1")
            .bind(user_id)
            .execute(&self.pool)
            .await
            .map(|_| ())
            .map_err(|e| UserError::Database(e.to_string()))
    }
}
