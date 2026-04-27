use crate::pool::Pool;
use domain::character::{Character, CharacterError, CharacterRepository, CharacterVisibility};
use domain::content_rating::ContentRating;
use sqlx::Row;
use std::sync::Arc;
use uuid::Uuid;

/// SQLx implementation of `CharacterRepository`.
pub struct SqlxCharacterRepository {
    pool: Pool,
}

impl SqlxCharacterRepository {
    /// Create a new repository instance.
    pub fn new(pool: Pool) -> Self {
        Self { pool }
    }

    /// Create a new repository instance wrapped as a trait object.
    pub fn from_pool(pool: Pool) -> Arc<dyn CharacterRepository> {
        Arc::new(Self::new(pool))
    }
}

/// Map a PostgreSQL row to a `Character` domain entity. PG ENUM columns are
/// cast to `::text` in SQL and parsed back into Rust enums here.
fn map_character(row: sqlx::postgres::PgRow) -> Result<Character, CharacterError> {
    let content_rating_str: String = row.get("content_rating");
    let visibility_str: String = row.get("visibility");

    let content_rating = ContentRating::from_str(&content_rating_str).ok_or_else(|| {
        CharacterError::Database(format!("Unknown content_rating: {content_rating_str}"))
    })?;
    let visibility = CharacterVisibility::from_str(&visibility_str).ok_or_else(|| {
        CharacterError::Database(format!("Unknown character_visibility: {visibility_str}"))
    })?;

    Ok(Character {
        id: row.get("id"),
        org_id: row.get("org_id"),
        name: row.get("name"),
        description: row.get("description"),
        content_rating,
        visibility,
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
        deleted_at: row.get("deleted_at"),
    })
}

macro_rules! cols {
    () => {
        "id, org_id, name, description, content_rating::text, visibility::text, \
         created_at, updated_at, deleted_at"
    };
}

#[async_trait::async_trait]
impl CharacterRepository for SqlxCharacterRepository {
    async fn create(
        &self,
        org_id: Uuid,
        name: &str,
        description: Option<&str>,
        content_rating: ContentRating,
        visibility: CharacterVisibility,
    ) -> Result<Character, CharacterError> {
        sqlx::query(
            concat!(
                "INSERT INTO character (org_id, name, description, content_rating, visibility) \
                 VALUES ($1, $2, $3, $4::content_rating, $5::character_visibility) \
                 RETURNING ", cols!()
            ),
        )
        .bind(org_id)
        .bind(name)
        .bind(description)
        .bind(content_rating.as_str())
        .bind(visibility.as_str())
        .fetch_one(&self.pool)
        .await
        .map_err(|e| CharacterError::Database(e.to_string()))
        .and_then(map_character)
    }

    async fn find_by_id(&self, id: Uuid) -> Result<Option<Character>, CharacterError> {
        sqlx::query(
            concat!("SELECT ", cols!(), " FROM character WHERE id = $1 AND deleted_at IS NULL"),
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| CharacterError::Database(e.to_string()))?
        .map(map_character)
        .transpose()
    }

    async fn list_by_org(
        &self,
        org_id: Uuid,
        limit: i64,
        offset: i64,
        content_rating: Option<ContentRating>,
    ) -> Result<Vec<Character>, CharacterError> {
        // Dynamic SQL: optional filter changes the parameter count, so the query
        // must be built at runtime rather than using a static string.
        let mut sql = String::from(concat!(
            "SELECT ", cols!(), " FROM character WHERE org_id = $1 AND deleted_at IS NULL"
        ));

        let mut param_idx = 2u32;

        if content_rating.is_some() {
            sql.push_str(&format!(" AND content_rating = ${param_idx}::content_rating"));
            param_idx += 1;
        }

        sql.push_str(&format!(
            " ORDER BY created_at DESC LIMIT ${param_idx} OFFSET ${}",
            param_idx + 1
        ));

        let mut query = sqlx::query(&sql).bind(org_id);

        if let Some(cr) = content_rating {
            query = query.bind(cr.as_str());
        }

        query = query.bind(limit).bind(offset);

        query
            .fetch_all(&self.pool)
            .await
            .map_err(|e| CharacterError::Database(e.to_string()))?
            .into_iter()
            .map(map_character)
            .collect()
    }

    async fn update(
        &self,
        id: Uuid,
        name: &str,
        description: Option<&str>,
        content_rating: ContentRating,
        visibility: CharacterVisibility,
    ) -> Result<Character, CharacterError> {
        sqlx::query(
            concat!(
                "UPDATE character \
                 SET name = $1, description = $2, content_rating = $3::content_rating, \
                     visibility = $4::character_visibility, updated_at = now() \
                 WHERE id = $5 AND deleted_at IS NULL \
                 RETURNING ", cols!()
            ),
        )
        .bind(name)
        .bind(description)
        .bind(content_rating.as_str())
        .bind(visibility.as_str())
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| CharacterError::Database(e.to_string()))?
        .map(map_character)
        .transpose()?
        .ok_or(CharacterError::NotFound)
    }

    async fn soft_delete(&self, id: Uuid) -> Result<(), CharacterError> {
        let result = sqlx::query(
            "UPDATE character SET deleted_at = now() \
             WHERE id = $1 AND deleted_at IS NULL",
        )
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(|e| CharacterError::Database(e.to_string()))?;

        if result.rows_affected() == 0 {
            return Err(CharacterError::NotFound);
        }
        Ok(())
    }
}
