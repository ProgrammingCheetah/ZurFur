use crate::pool::Pool;
use domain::feed_item::{AuthorType, FeedItem, FeedItemError, FeedItemRepository};
use sqlx::Row;
use std::sync::Arc;
use uuid::Uuid;

/// SQLx implementation of `FeedItemRepository`.
pub struct SqlxFeedItemRepository {
    pool: Pool,
}

impl SqlxFeedItemRepository {
    /// Create a new repository instance.
    pub fn new(pool: Pool) -> Self {
        Self { pool }
    }

    /// Create a new repository instance wrapped as a trait object.
    pub fn from_pool(pool: Pool) -> Arc<dyn FeedItemRepository> {
        Arc::new(Self::new(pool))
    }
}

fn map_feed_item(row: sqlx::postgres::PgRow) -> Result<FeedItem, FeedItemError> {
    let author_type_str: String = row.get("author_type");
    let author_type = AuthorType::from_str(&author_type_str)
        .ok_or_else(|| FeedItemError::Database(format!("Unknown author type: {author_type_str}")))?;

    let item = FeedItem {
        id: row.get("id"),
        feed_id: row.get("feed_id"),
        author_type,
        author_id: row.get("author_id"),
        created_at: row.get("created_at"),
    };
    Ok(item)
}

// --- Executor-generic helpers ------------------------------------------------

pub(super) async fn create_feed_item<'e>(
    executor: impl sqlx::Executor<'e, Database = sqlx::Postgres>,
    feed_id: Uuid,
    author_type: AuthorType,
    author_id: Uuid,
) -> Result<FeedItem, FeedItemError> {
    let row = sqlx::query(
        "INSERT INTO feed_item (feed_id, author_type, author_id) \
         VALUES ($1, $2, $3) \
         RETURNING id, feed_id, author_type, author_id, created_at",
    )
    .bind(feed_id)
    .bind(author_type.as_str())
    .bind(author_id)
    .fetch_one(executor)
    .await
    .map_err(|e| FeedItemError::Database(e.to_string()))?;

    map_feed_item(row)
}

// --- Trait implementation ----------------------------------------------------

#[async_trait::async_trait]
impl FeedItemRepository for SqlxFeedItemRepository {
    async fn create(
        &self,
        feed_id: Uuid,
        author_type: AuthorType,
        author_id: Uuid,
    ) -> Result<FeedItem, FeedItemError> {
        create_feed_item(&self.pool, feed_id, author_type, author_id).await
    }

    async fn find_by_id(&self, id: Uuid) -> Result<Option<FeedItem>, FeedItemError> {
        let row = sqlx::query(
            "SELECT id, feed_id, author_type, author_id, created_at \
             FROM feed_item WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| FeedItemError::Database(e.to_string()))?;

        match row {
            Some(r) => Ok(Some(map_feed_item(r)?)),
            None => Ok(None),
        }
    }

    async fn list_by_feed(
        &self,
        feed_id: Uuid,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<FeedItem>, FeedItemError> {
        let rows = sqlx::query(
            "SELECT id, feed_id, author_type, author_id, created_at \
             FROM feed_item WHERE feed_id = $1 \
             ORDER BY created_at DESC \
             LIMIT $2 OFFSET $3",
        )
        .bind(feed_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| FeedItemError::Database(e.to_string()))?;

        let mut items = Vec::with_capacity(rows.len());
        for row in rows {
            items.push(map_feed_item(row)?);
        }
        Ok(items)
    }

    async fn delete(&self, id: Uuid) -> Result<(), FeedItemError> {
        let result = sqlx::query("DELETE FROM feed_item WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| FeedItemError::Database(e.to_string()))?;

        if result.rows_affected() == 0 {
            return Err(FeedItemError::NotFound);
        }
        Ok(())
    }
}
