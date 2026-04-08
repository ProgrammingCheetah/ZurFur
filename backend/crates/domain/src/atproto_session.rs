use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::user::UserError;

/// Stores AT Protocol OAuth tokens for making Bluesky API calls on behalf of a user.
/// This is separate from the Zurfur platform session (JWT-based).
#[derive(Debug, Clone)]
pub struct AtprotoSessionEntity {
    pub id: Uuid,
    pub user_id: Uuid,
    pub did: String,
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_at: DateTime<Utc>,
    pub pds_url: Option<String>,
}

/// Repository trait for AT Protocol session persistence.
#[async_trait::async_trait]
pub trait AtprotoSessionRepository: Send + Sync {
    async fn upsert(&self, session: &AtprotoSessionEntity) -> Result<(), UserError>;
    async fn find_by_user_id(&self, user_id: Uuid) -> Result<Option<AtprotoSessionEntity>, UserError>;
    async fn find_by_did(&self, did: &str) -> Result<Option<AtprotoSessionEntity>, UserError>;
    async fn delete_by_user_id(&self, user_id: Uuid) -> Result<(), UserError>;
}
