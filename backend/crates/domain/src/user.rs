use chrono::{DateTime, Utc};
use uuid::Uuid;

/// User entity — atomic identity only.
///
/// ARCHITECTURE DECISIONS:
///   User holds only authentication identity data. All public-facing identity
///   (roles, bios, titles, capabilities) lives on Organizations. Never add
///   feature flags or role fields here — meaning comes from org membership.
#[derive(Debug, Clone)]
pub struct User {
    pub id: Uuid,
    pub did: Option<String>,
    pub handle: Option<String>,
    pub email: Option<String>,
    pub username: String,
    /// NULL means onboarding is pending. Set once during first-login wizard.
    pub onboarding_completed_at: Option<DateTime<Utc>>,
}

/// Errors that can occur when operating on users.
#[derive(Debug, thiserror::Error)]
pub enum UserError {
    #[error("User not found")]
    NotFound,
    #[error("Database error: {0}")]
    Database(String),
}

/// Repository trait for User persistence.
#[async_trait::async_trait]
pub trait UserRepository: Send + Sync {
    async fn find_by_email(&self, email: &str) -> Result<Option<User>, UserError>;
    async fn find_by_id(&self, id: Uuid) -> Result<Option<User>, UserError>;
    async fn find_by_did(&self, did: &str) -> Result<Option<User>, UserError>;
    async fn create_from_atproto(
        &self,
        did: &str,
        handle: Option<&str>,
        email: Option<&str>,
    ) -> Result<User, UserError>;
    async fn update_handle(&self, user_id: Uuid, handle: &str) -> Result<(), UserError>;
    async fn mark_onboarding_completed(&self, user_id: Uuid) -> Result<(), UserError>;
}
