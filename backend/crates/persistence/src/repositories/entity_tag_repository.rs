//! SQLx PostgreSQL implementation of `EntityTagRepository`.

use crate::pool::Pool;
use crate::sqlx_utils::is_unique_violation;
use domain::entity_tag::{EntityTag, EntityTagError, EntityTagRepository, TaggableEntityType};
use sqlx::Row;
use std::sync::Arc;
use uuid::Uuid;

/// PostgreSQL implementation of `EntityTagRepository`.
///
/// Operates on the `entity_tag` table — a polymorphic junction with composite
/// PK (entity_type, entity_id, tag_id). Follows the same pattern as
/// `SqlxEntityFeedRepository`.
pub struct SqlxEntityTagRepository {
    pool: Pool,
}

impl SqlxEntityTagRepository {
    /// Create a new repository instance.
    pub fn new(pool: Pool) -> Self {
        Self { pool }
    }

    /// Create a new repository instance wrapped as a trait object.
    pub fn from_pool(pool: Pool) -> Arc<dyn EntityTagRepository> {
        Arc::new(Self::new(pool))
    }
}

/// Map a PostgreSQL row to an `EntityTag` domain entity.
fn map_entity_tag(row: sqlx::postgres::PgRow) -> Result<EntityTag, EntityTagError> {
    let entity_type_str: String = row.get("entity_type");
    let entity_type = TaggableEntityType::from_str(&entity_type_str).ok_or_else(|| {
        EntityTagError::Database(format!("Unknown entity type: {entity_type_str}"))
    })?;

    Ok(EntityTag {
        entity_type,
        entity_id: row.get("entity_id"),
        tag_id: row.get("tag_id"),
    })
}

// --- Executor-generic helpers ------------------------------------------------

pub(super) async fn attach_entity_tag<'e>(
    executor: impl sqlx::Executor<'e, Database = sqlx::Postgres>,
    entity_type: TaggableEntityType,
    entity_id: Uuid,
    tag_id: Uuid,
) -> Result<EntityTag, EntityTagError> {
    let row = sqlx::query(
        "INSERT INTO entity_tag (entity_type, entity_id, tag_id) \
         VALUES ($1, $2, $3) \
         RETURNING entity_type, entity_id, tag_id",
    )
    .bind(entity_type.as_str())
    .bind(entity_id)
    .bind(tag_id)
    .fetch_one(executor)
    .await
    .map_err(|e| {
        if is_unique_violation(&e) {
            EntityTagError::AlreadyAttached
        } else {
            EntityTagError::Database(e.to_string())
        }
    })?;

    map_entity_tag(row)
}

// --- Trait implementation ----------------------------------------------------

#[async_trait::async_trait]
impl EntityTagRepository for SqlxEntityTagRepository {
    async fn attach(
        &self,
        entity_type: TaggableEntityType,
        entity_id: Uuid,
        tag_id: Uuid,
    ) -> Result<EntityTag, EntityTagError> {
        attach_entity_tag(&self.pool, entity_type, entity_id, tag_id).await
    }

    async fn detach(
        &self,
        entity_type: TaggableEntityType,
        entity_id: Uuid,
        tag_id: Uuid,
    ) -> Result<(), EntityTagError> {
        let result = sqlx::query(
            "DELETE FROM entity_tag \
             WHERE entity_type = $1 AND entity_id = $2 AND tag_id = $3",
        )
        .bind(entity_type.as_str())
        .bind(entity_id)
        .bind(tag_id)
        .execute(&self.pool)
        .await
        .map_err(|e| EntityTagError::Database(e.to_string()))?;

        if result.rows_affected() == 0 {
            return Err(EntityTagError::NotFound);
        }
        Ok(())
    }

    async fn list_by_entity(
        &self,
        entity_type: TaggableEntityType,
        entity_id: Uuid,
    ) -> Result<Vec<EntityTag>, EntityTagError> {
        let rows = sqlx::query(
            "SELECT entity_type, entity_id, tag_id \
             FROM entity_tag WHERE entity_type = $1 AND entity_id = $2",
        )
        .bind(entity_type.as_str())
        .bind(entity_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| EntityTagError::Database(e.to_string()))?;

        let mut results = Vec::with_capacity(rows.len());
        for row in rows {
            results.push(map_entity_tag(row)?);
        }
        Ok(results)
    }

    async fn list_by_tag(
        &self,
        tag_id: Uuid,
    ) -> Result<Vec<EntityTag>, EntityTagError> {
        let rows = sqlx::query(
            "SELECT entity_type, entity_id, tag_id \
             FROM entity_tag WHERE tag_id = $1",
        )
        .bind(tag_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| EntityTagError::Database(e.to_string()))?;

        let mut results = Vec::with_capacity(rows.len());
        for row in rows {
            results.push(map_entity_tag(row)?);
        }
        Ok(results)
    }
}
