use uuid::Uuid;

use crate::content_rating::ContentRating;

/// Per-user content filter preference. Stored separately from User to keep the
/// core user entity atomic and allow independent updates.
#[derive(Debug, Clone)]
pub struct UserPreferences {
    pub user_id: Uuid,
    pub max_content_rating: ContentRating,
}

#[derive(Debug, thiserror::Error)]
pub enum UserPreferencesError {
    #[error("Invalid content rating value: {0}")]
    InvalidRating(String),
    #[error("Database error: {0}")]
    Database(String),
}

/// Repository trait for user preferences.
///
/// ARCHITECTURE DECISIONS:
///   `get` returns a `UserPreferences` with the SFW default when no row exists,
///   rather than `Option<UserPreferences>`. This simplifies all callers — they
///   never need to handle the "no preferences yet" case specially.
#[async_trait::async_trait]
pub trait UserPreferencesRepository: Send + Sync {
    async fn get(&self, user_id: Uuid) -> Result<UserPreferences, UserPreferencesError>;

    async fn set_max_content_rating(
        &self,
        user_id: Uuid,
        rating: ContentRating,
    ) -> Result<UserPreferences, UserPreferencesError>;
}
