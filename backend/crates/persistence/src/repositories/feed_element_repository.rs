use crate::pool::Pool;
use domain::feed_element::{FeedElement, FeedElementError, FeedElementRepository, FeedElementType};
use sqlx::Row;
use std::sync::Arc;
use uuid::Uuid;

/// SQLx implementation of `FeedElementRepository`.
pub struct SqlxFeedElementRepository {
    pool: Pool,
}

impl SqlxFeedElementRepository {
    /// Create a new repository instance.
    pub fn new(pool: Pool) -> Self {
        Self { pool }
    }

    /// Create a new repository instance wrapped as a trait object.
    pub fn from_pool(pool: Pool) -> Arc<dyn FeedElementRepository> {
        Arc::new(Self::new(pool))
    }
}

fn map_feed_element(row: sqlx::postgres::PgRow) -> Result<FeedElement, FeedElementError> {
    let element_type_str: String = row.get("element_type");
    let element_type = FeedElementType::from_str(&element_type_str)
        .ok_or_else(|| FeedElementError::Database(format!("Unknown element type: {element_type_str}")))?;

    let element = FeedElement {
        id: row.get("id"),
        feed_item_id: row.get("feed_item_id"),
        element_type,
        content_json: row.get("content_json"),
        position: row.get("position"),
    };
    Ok(element)
}

// --- Executor-generic helpers ------------------------------------------------

pub(super) async fn create_feed_element<'e>(
    executor: impl sqlx::Executor<'e, Database = sqlx::Postgres>,
    feed_item_id: Uuid,
    element_type: FeedElementType,
    content_json: &str,
    position: i32,
) -> Result<FeedElement, FeedElementError> {
    let row = sqlx::query(
        "INSERT INTO feed_element (feed_item_id, element_type, content_json, position) \
         VALUES ($1, $2, $3, $4) \
         RETURNING id, feed_item_id, element_type, content_json, position",
    )
    .bind(feed_item_id)
    .bind(element_type.as_str())
    .bind(content_json)
    .bind(position)
    .fetch_one(executor)
    .await
    .map_err(|e| FeedElementError::Database(e.to_string()))?;

    map_feed_element(row)
}

// --- Trait implementation ----------------------------------------------------

#[async_trait::async_trait]
impl FeedElementRepository for SqlxFeedElementRepository {
    async fn create(
        &self,
        feed_item_id: Uuid,
        element_type: FeedElementType,
        content_json: &str,
        position: i32,
    ) -> Result<FeedElement, FeedElementError> {
        create_feed_element(&self.pool, feed_item_id, element_type, content_json, position).await
    }

    async fn find_by_id(&self, id: Uuid) -> Result<Option<FeedElement>, FeedElementError> {
        let row = sqlx::query(
            "SELECT id, feed_item_id, element_type, content_json, position \
             FROM feed_element WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| FeedElementError::Database(e.to_string()))?;

        match row {
            Some(r) => Ok(Some(map_feed_element(r)?)),
            None => Ok(None),
        }
    }

    async fn list_by_feed_item(
        &self,
        feed_item_id: Uuid,
    ) -> Result<Vec<FeedElement>, FeedElementError> {
        let rows = sqlx::query(
            "SELECT id, feed_item_id, element_type, content_json, position \
             FROM feed_element WHERE feed_item_id = $1 \
             ORDER BY position ASC",
        )
        .bind(feed_item_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| FeedElementError::Database(e.to_string()))?;

        let mut elements = Vec::with_capacity(rows.len());
        for row in rows {
            elements.push(map_feed_element(row)?);
        }
        Ok(elements)
    }

    async fn update_content(
        &self,
        id: Uuid,
        content_json: &str,
    ) -> Result<FeedElement, FeedElementError> {
        let row = sqlx::query(
            "UPDATE feed_element SET content_json = $1 \
             WHERE id = $2 \
             RETURNING id, feed_item_id, element_type, content_json, position",
        )
        .bind(content_json)
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| FeedElementError::Database(e.to_string()))?
        .ok_or(FeedElementError::NotFound)?;

        map_feed_element(row)
    }

    async fn delete(&self, id: Uuid) -> Result<(), FeedElementError> {
        let result = sqlx::query("DELETE FROM feed_element WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| FeedElementError::Database(e.to_string()))?;

        if result.rows_affected() == 0 {
            return Err(FeedElementError::NotFound);
        }
        Ok(())
    }
}
