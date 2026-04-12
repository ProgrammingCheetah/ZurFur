use uuid::Uuid;

/// Per-user settings. Stored as JSONB in the database for extensibility
/// without migration. The `settings` field is a JSON string (same pattern
/// as `content_json` on FeedElement — keeps domain crate dependency-free).
#[derive(Debug, Clone)]
pub struct UserPreferences {
    pub user_id: Uuid,
    pub settings: String,
}

#[derive(Debug, thiserror::Error)]
pub enum UserPreferencesError {
    #[error("Database error: {0}")]
    Database(String),
}

/// Repository trait for user preferences.
///
/// ARCHITECTURE DECISIONS:
///   `get` returns a `UserPreferences` with empty settings `"{}"` when no row
///   exists, rather than `Option<UserPreferences>`. This simplifies all callers.
#[async_trait::async_trait]
pub trait UserPreferencesRepository: Send + Sync {
    async fn get(&self, user_id: Uuid) -> Result<UserPreferences, UserPreferencesError>;

    async fn set(
        &self,
        user_id: Uuid,
        settings: &str,
    ) -> Result<UserPreferences, UserPreferencesError>;
}
