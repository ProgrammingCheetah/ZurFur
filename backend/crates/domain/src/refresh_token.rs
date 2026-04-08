use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::user::UserError;

/// A hashed refresh token for Zurfur platform session management.
/// The raw token is never stored — only its SHA-256 hash.
#[derive(Debug, Clone)]
pub struct RefreshTokenEntity {
    pub id: Uuid,
    pub user_id: Uuid,
    pub token_hash: String,
    pub expires_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

/// Repository trait for refresh token persistence.
#[async_trait::async_trait]
pub trait RefreshTokenRepository: Send + Sync {
    async fn create(
        &self,
        user_id: Uuid,
        token_hash: &str,
        expires_at: DateTime<Utc>,
    ) -> Result<(), UserError>;

    async fn find_by_hash(
        &self,
        token_hash: &str,
    ) -> Result<Option<RefreshTokenEntity>, UserError>;

    /// Delete a single token by hash (single-use rotation).
    async fn delete_by_hash(&self, token_hash: &str) -> Result<(), UserError>;

    /// Delete all refresh tokens for a user (logout from all devices).
    async fn delete_all_for_user(&self, user_id: Uuid) -> Result<(), UserError>;
}
