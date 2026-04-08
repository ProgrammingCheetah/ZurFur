use uuid::Uuid;

/// User entity.
#[derive(Debug, Clone)]
pub struct User {
    pub id: Uuid,
    pub did: Option<String>,
    pub handle: Option<String>,
    pub email: Option<String>,
    pub username: String,
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
}
