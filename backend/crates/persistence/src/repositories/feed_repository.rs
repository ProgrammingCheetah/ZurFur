use crate::pool::Pool;
use crate::sqlx_utils::is_unique_violation;
use domain::feed::{Feed, FeedError, FeedRepository, FeedType};
use sqlx::Row;
use std::sync::Arc;
use uuid::Uuid;

pub struct SqlxFeedRepository {
    pool: Pool,
}

impl SqlxFeedRepository {
    pub fn new(pool: Pool) -> Self {
        Self { pool }
    }

    pub fn from_pool(pool: Pool) -> Arc<dyn FeedRepository> {
        Arc::new(Self::new(pool))
    }
}

fn map_feed(row: sqlx::postgres::PgRow) -> Result<Feed, FeedError> {
    let feed_type_str: String = row.get("feed_type");
    let feed_type = FeedType::from_str(&feed_type_str)
        .ok_or_else(|| FeedError::Database(format!("Unknown feed type: {feed_type_str}")))?;

    let feed = Feed {
        id: row.get("id"),
        slug: row.get("slug"),
        display_name: row.get("display_name"),
        description: row.get("description"),
        feed_type,
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
        deleted_at: row.get("deleted_at"),
    };
    Ok(feed)
}

#[async_trait::async_trait]
impl FeedRepository for SqlxFeedRepository {
    async fn create(
        &self,
        slug: &str,
        display_name: &str,
        description: Option<&str>,
        feed_type: FeedType,
    ) -> Result<Feed, FeedError> {
        let row = sqlx::query(
            "INSERT INTO feed (slug, display_name, description, feed_type) \
             VALUES ($1, $2, $3, $4) \
             RETURNING id, slug, display_name, description, feed_type, created_at, updated_at, deleted_at",
        )
        .bind(slug)
        .bind(display_name)
        .bind(description)
        .bind(feed_type.as_str())
        .fetch_one(&self.pool)
        .await
        .map_err(|e| {
            if is_unique_violation(&e) {
                FeedError::SlugTaken(slug.to_string())
            } else {
                FeedError::Database(e.to_string())
            }
        })?;

        map_feed(row)
    }

    async fn find_by_id(&self, id: Uuid) -> Result<Option<Feed>, FeedError> {
        let row = sqlx::query(
            "SELECT id, slug, display_name, description, feed_type, created_at, updated_at, deleted_at \
             FROM feed WHERE id = $1 AND deleted_at IS NULL",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| FeedError::Database(e.to_string()))?;

        match row {
            Some(r) => Ok(Some(map_feed(r)?)),
            None => Ok(None),
        }
    }

    async fn update(
        &self,
        id: Uuid,
        display_name: &str,
        description: Option<&str>,
    ) -> Result<Feed, FeedError> {
        let row = sqlx::query(
            "UPDATE feed SET display_name = $1, description = $2 \
             WHERE id = $3 AND deleted_at IS NULL \
             RETURNING id, slug, display_name, description, feed_type, created_at, updated_at, deleted_at",
        )
        .bind(display_name)
        .bind(description)
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| FeedError::Database(e.to_string()))?
        .ok_or(FeedError::NotFound)?;

        map_feed(row)
    }

    async fn soft_delete(&self, id: Uuid) -> Result<(), FeedError> {
        // Single CTE query: atomically checks existence, feed type, and deletes.
        // No TOCTOU race — all in one statement.
        let row = sqlx::query(
            "WITH target AS ( \
                 SELECT feed_type FROM feed WHERE id = $1 AND deleted_at IS NULL \
             ), deleted AS ( \
                 UPDATE feed SET deleted_at = now() \
                 WHERE id = $1 AND deleted_at IS NULL AND feed_type = 'custom' \
                 RETURNING 1 \
             ) \
             SELECT CASE \
                 WHEN EXISTS (SELECT 1 FROM deleted) THEN 'deleted' \
                 WHEN EXISTS (SELECT 1 FROM target) THEN 'system' \
                 ELSE 'not_found' \
             END AS outcome",
        )
        .bind(id)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| FeedError::Database(e.to_string()))?;

        let outcome: String = row.get("outcome");
        match outcome.as_str() {
            "deleted" => Ok(()),
            "system" => Err(FeedError::SystemFeedUndeletable),
            "not_found" => Err(FeedError::NotFound),
            other => Err(FeedError::Database(format!("Unexpected soft_delete outcome: {other}"))),
        }
    }

    async fn list_by_ids(&self, ids: &[Uuid]) -> Result<Vec<Feed>, FeedError> {
        let rows = sqlx::query(
            "SELECT id, slug, display_name, description, feed_type, created_at, updated_at, deleted_at \
             FROM feed WHERE id = ANY($1) AND deleted_at IS NULL",
        )
        .bind(ids)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| FeedError::Database(e.to_string()))?;

        let mut feeds = Vec::with_capacity(rows.len());
        for row in rows {
            feeds.push(map_feed(row)?);
        }
        Ok(feeds)
    }
}
