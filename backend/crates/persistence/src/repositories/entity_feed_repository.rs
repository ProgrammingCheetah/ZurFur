use crate::pool::Pool;
use crate::sqlx_utils::is_unique_violation;
use domain::entity::EntityKind;
use domain::entity_feed::{EntityFeed, EntityFeedError, EntityFeedRepository};
use sqlx::Row;
use std::sync::Arc;
use uuid::Uuid;

/// SQLx implementation of `EntityFeedRepository`.
pub struct SqlxEntityFeedRepository {
    pool: Pool,
}

impl SqlxEntityFeedRepository {
    /// Create a new repository instance.
    pub fn new(pool: Pool) -> Self {
        Self { pool }
    }

    /// Create a new repository instance wrapped as a trait object.
    pub fn from_pool(pool: Pool) -> Arc<dyn EntityFeedRepository> {
        Arc::new(Self::new(pool))
    }
}

fn map_entity_feed(row: sqlx::postgres::PgRow) -> Result<EntityFeed, EntityFeedError> {
    let entity_type_str: String = row.get("entity_type");
    let entity_type = EntityKind::from_str(&entity_type_str)
        .ok_or_else(|| EntityFeedError::Database(format!("Unknown entity type: {entity_type_str}")))?;

    let entity_feed = EntityFeed {
        feed_id: row.get("feed_id"),
        entity_type,
        entity_id: row.get("entity_id"),
    };
    Ok(entity_feed)
}

// --- Executor-generic helpers ------------------------------------------------

pub(super) async fn attach_entity_feed<'e>(
    executor: impl sqlx::Executor<'e, Database = sqlx::Postgres>,
    feed_id: Uuid,
    entity_type: EntityKind,
    entity_id: Uuid,
) -> Result<EntityFeed, EntityFeedError> {
    let row = sqlx::query(
        "INSERT INTO entity_feed (feed_id, entity_type, entity_id) \
         VALUES ($1, $2, $3) \
         RETURNING feed_id, entity_type, entity_id",
    )
    .bind(feed_id)
    .bind(entity_type.as_str())
    .bind(entity_id)
    .fetch_one(executor)
    .await
    .map_err(|e| {
        if is_unique_violation(&e) {
            EntityFeedError::AlreadyAttached
        } else {
            EntityFeedError::Database(e.to_string())
        }
    })?;

    map_entity_feed(row)
}

// --- Trait implementation ----------------------------------------------------

#[async_trait::async_trait]
impl EntityFeedRepository for SqlxEntityFeedRepository {
    async fn attach(
        &self,
        feed_id: Uuid,
        entity_type: EntityKind,
        entity_id: Uuid,
    ) -> Result<EntityFeed, EntityFeedError> {
        attach_entity_feed(&self.pool, feed_id, entity_type, entity_id).await
    }

    async fn find_by_feed_id(&self, feed_id: Uuid) -> Result<Option<EntityFeed>, EntityFeedError> {
        let row = sqlx::query(
            "SELECT feed_id, entity_type, entity_id FROM entity_feed WHERE feed_id = $1",
        )
        .bind(feed_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| EntityFeedError::Database(e.to_string()))?;

        match row {
            Some(r) => Ok(Some(map_entity_feed(r)?)),
            None => Ok(None),
        }
    }

    async fn list_by_entity(
        &self,
        entity_type: EntityKind,
        entity_id: Uuid,
    ) -> Result<Vec<EntityFeed>, EntityFeedError> {
        let rows = sqlx::query(
            "SELECT feed_id, entity_type, entity_id \
             FROM entity_feed WHERE entity_type = $1 AND entity_id = $2",
        )
        .bind(entity_type.as_str())
        .bind(entity_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| EntityFeedError::Database(e.to_string()))?;

        let mut results = Vec::with_capacity(rows.len());
        for row in rows {
            results.push(map_entity_feed(row)?);
        }
        Ok(results)
    }

    async fn detach(&self, feed_id: Uuid) -> Result<(), EntityFeedError> {
        let result = sqlx::query("DELETE FROM entity_feed WHERE feed_id = $1")
            .bind(feed_id)
            .execute(&self.pool)
            .await
            .map_err(|e| EntityFeedError::Database(e.to_string()))?;

        if result.rows_affected() == 0 {
            return Err(EntityFeedError::NotFound);
        }
        Ok(())
    }
}
