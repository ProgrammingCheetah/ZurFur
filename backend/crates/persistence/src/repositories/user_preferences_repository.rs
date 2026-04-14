use crate::pool::Pool;
use domain::user_preferences::{UserPreferences, UserPreferencesError, UserPreferencesRepository};
use sqlx::Row;
use std::sync::Arc;
use uuid::Uuid;

/// SQLx implementation of `UserPreferencesRepository`.
pub struct SqlxUserPreferencesRepository {
    pool: Pool,
}

impl SqlxUserPreferencesRepository {
    /// Create a new repository instance.
    pub fn new(pool: Pool) -> Self {
        Self { pool }
    }

    /// Create a new repository instance wrapped as a trait object.
    pub fn from_pool(pool: Pool) -> Arc<dyn UserPreferencesRepository> {
        Arc::new(Self::new(pool))
    }
}

fn map_preferences(row: sqlx::postgres::PgRow) -> UserPreferences {
    let settings: serde_json::Value = row.get("settings");
    UserPreferences {
        user_id: row.get("user_id"),
        settings: settings.to_string(),
    }
}

#[async_trait::async_trait]
impl UserPreferencesRepository for SqlxUserPreferencesRepository {
    async fn get(&self, user_id: Uuid) -> Result<UserPreferences, UserPreferencesError> {
        let row = sqlx::query(
            "SELECT user_id, settings FROM user_preference WHERE user_id = $1",
        )
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| UserPreferencesError::Database(e.to_string()))?;

        match row {
            Some(r) => Ok(map_preferences(r)),
            None => Ok(UserPreferences {
                user_id,
                settings: "{}".into(),
            }),
        }
    }

    async fn set(
        &self,
        user_id: Uuid,
        settings: &str,
    ) -> Result<UserPreferences, UserPreferencesError> {
        let json_val: serde_json::Value = serde_json::from_str(settings)
            .map_err(|e| UserPreferencesError::Database(format!("Invalid JSON: {e}")))?;

        let row = sqlx::query(
            "INSERT INTO user_preference (user_id, settings) \
             VALUES ($1, $2) \
             ON CONFLICT (user_id) DO UPDATE SET settings = EXCLUDED.settings \
             RETURNING user_id, settings",
        )
        .bind(user_id)
        .bind(&json_val)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| UserPreferencesError::Database(e.to_string()))?;

        Ok(map_preferences(row))
    }
}
