use crate::pool::Pool;
use domain::content_rating::ContentRating;
use domain::user_preferences::{UserPreferences, UserPreferencesError, UserPreferencesRepository};
use sqlx::Row;
use std::sync::Arc;
use uuid::Uuid;

pub struct SqlxUserPreferencesRepository {
    pool: Pool,
}

impl SqlxUserPreferencesRepository {
    pub fn new(pool: Pool) -> Self {
        Self { pool }
    }

    pub fn from_pool(pool: Pool) -> Arc<dyn UserPreferencesRepository> {
        Arc::new(Self::new(pool))
    }
}

fn map_preferences(row: sqlx::postgres::PgRow) -> Result<UserPreferences, UserPreferencesError> {
    // PG enum is read as a String by sqlx
    let rating_str: String = row.get("max_content_rating");
    let max_content_rating = ContentRating::from_str(&rating_str)
        .ok_or_else(|| UserPreferencesError::InvalidRating(rating_str))?;
    Ok(UserPreferences {
        user_id: row.get("user_id"),
        max_content_rating,
    })
}

#[async_trait::async_trait]
impl UserPreferencesRepository for SqlxUserPreferencesRepository {
    async fn get(&self, user_id: Uuid) -> Result<UserPreferences, UserPreferencesError> {
        let row = sqlx::query(
            "SELECT user_id, max_content_rating::text FROM user_preferences WHERE user_id = $1",
        )
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| UserPreferencesError::Database(e.to_string()))?;

        match row {
            Some(r) => map_preferences(r),
            None => Ok(UserPreferences {
                user_id,
                max_content_rating: ContentRating::Sfw,
            }),
        }
    }

    async fn set_max_content_rating(
        &self,
        user_id: Uuid,
        rating: ContentRating,
    ) -> Result<UserPreferences, UserPreferencesError> {
        // Cast the text value to the PG content_rating enum type
        sqlx::query(
            "INSERT INTO user_preferences (user_id, max_content_rating) \
             VALUES ($1, $2::content_rating) \
             ON CONFLICT (user_id) DO UPDATE SET max_content_rating = EXCLUDED.max_content_rating \
             RETURNING user_id, max_content_rating::text",
        )
        .bind(user_id)
        .bind(rating.as_str())
        .fetch_one(&self.pool)
        .await
        .map_err(|e| UserPreferencesError::Database(e.to_string()))
        .and_then(map_preferences)
    }
}
