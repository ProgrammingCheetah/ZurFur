//! SQLx PostgreSQL implementation of `TagRepository`.

use crate::pool::Pool;
use crate::sqlx_utils::is_unique_violation;
use domain::entity_tag::{EntityTag, EntityTagError, TaggableEntityType};
use domain::tag::{Tag, TagCategory, TagError, TagRepository};
use sqlx::Row;
use std::sync::Arc;
use uuid::Uuid;

/// PostgreSQL implementation of `TagRepository`.
///
/// Uses the `tag` table with `tag_category` PG ENUM. The `category` column
/// is cast to `::text` on SELECT and from `::tag_category` on INSERT/WHERE
/// to bridge between the PG ENUM and Rust's `TagCategory` enum.
pub struct SqlxTagRepository {
    pool: Pool,
}

impl SqlxTagRepository {
    /// Create a new repository instance.
    pub fn new(pool: Pool) -> Self {
        Self { pool }
    }

    /// Create a new repository instance wrapped as a trait object.
    pub fn from_pool(pool: Pool) -> Arc<dyn TagRepository> {
        Arc::new(Self::new(pool))
    }
}

/// Map a PostgreSQL row to a `Tag` domain entity. The `category` column is
/// read as text (via `::text` cast in SQL) and parsed into `TagCategory`.
fn map_tag(row: sqlx::postgres::PgRow) -> Result<Tag, TagError> {
    let category_str: String = row.get("category");
    let category = TagCategory::from_str(&category_str)
        .ok_or_else(|| TagError::Database(format!("Unknown tag category: {category_str}")))?;

    Ok(Tag {
        id: row.get("id"),
        category,
        name: row.get("name"),
        usage_count: row.get("usage_count"),
        is_approved: row.get("is_approved"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}

macro_rules! cols {
    () => {
        "id, category::text, name, usage_count, is_approved, created_at, updated_at"
    };
}

// --- Executor-generic helpers ------------------------------------------------
// These accept any sqlx::Executor (pool connection OR transaction), allowing
// reuse between standalone trait methods and transactional create_and_attach.

async fn create_tag<'e>(
    executor: impl sqlx::Executor<'e, Database = sqlx::Postgres>,
    category: TagCategory,
    name: &str,
    is_approved: bool,
) -> Result<Tag, TagError> {
    let row = sqlx::query(concat!(
        "INSERT INTO tag (category, name, is_approved) ",
        "VALUES ($1::tag_category, $2, $3) ",
        "RETURNING ", cols!()
    ))
    .bind(category.as_str())
    .bind(name)
    .bind(is_approved)
    .fetch_one(executor)
    .await
    .map_err(|e| {
        if is_unique_violation(&e) {
            TagError::NameTaken(name.to_string())
        } else {
            TagError::Database(e.to_string())
        }
    })?;

    map_tag(row)
}


async fn increment_usage_count<'e>(
    executor: impl sqlx::Executor<'e, Database = sqlx::Postgres>,
    id: Uuid,
) -> Result<(), TagError> {
    sqlx::query("UPDATE tag SET usage_count = usage_count + 1 WHERE id = $1")
        .bind(id)
        .execute(executor)
        .await
        .map_err(|e| TagError::Database(e.to_string()))?;
    Ok(())
}

async fn decrement_usage_count<'e>(
    executor: impl sqlx::Executor<'e, Database = sqlx::Postgres>,
    id: Uuid,
) -> Result<(), TagError> {
    sqlx::query(
        "UPDATE tag SET usage_count = GREATEST(usage_count - 1, 0) WHERE id = $1",
    )
    .bind(id)
    .execute(executor)
    .await
    .map_err(|e| TagError::Database(e.to_string()))?;
    Ok(())
}

// --- Trait implementation ----------------------------------------------------

#[async_trait::async_trait]
impl TagRepository for SqlxTagRepository {
    async fn create(
        &self,
        category: TagCategory,
        name: &str,
        is_approved: bool,
    ) -> Result<Tag, TagError> {
        create_tag(&self.pool, category, name, is_approved).await
    }

    async fn find_by_id(&self, id: Uuid) -> Result<Option<Tag>, TagError> {
        let row = sqlx::query(concat!(
            "SELECT ", cols!(), " FROM tag WHERE id = $1"
        ))
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| TagError::Database(e.to_string()))?;

        match row {
            Some(r) => Ok(Some(map_tag(r)?)),
            None => Ok(None),
        }
    }

    async fn find_by_name_and_category(
        &self,
        name: &str,
        category: TagCategory,
    ) -> Result<Option<Tag>, TagError> {
        let row = sqlx::query(concat!(
            "SELECT ", cols!(), " FROM tag ",
            "WHERE name = $1 AND category = $2::tag_category"
        ))
        .bind(name)
        .bind(category.as_str())
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| TagError::Database(e.to_string()))?;

        match row {
            Some(r) => Ok(Some(map_tag(r)?)),
            None => Ok(None),
        }
    }

    async fn list_by_category(
        &self,
        category: TagCategory,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<Tag>, TagError> {
        let rows = sqlx::query(concat!(
            "SELECT ", cols!(), " FROM tag ",
            "WHERE category = $1::tag_category ",
            "ORDER BY usage_count DESC, name ASC ",
            "LIMIT $2 OFFSET $3"
        ))
        .bind(category.as_str())
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| TagError::Database(e.to_string()))?;

        let mut tags = Vec::with_capacity(rows.len());
        for row in rows {
            tags.push(map_tag(row)?);
        }
        Ok(tags)
    }

    async fn list_by_ids(&self, ids: &[Uuid]) -> Result<Vec<Tag>, TagError> {
        let rows = sqlx::query(concat!(
            "SELECT ", cols!(), " FROM tag WHERE id = ANY($1)"
        ))
        .bind(ids)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| TagError::Database(e.to_string()))?;

        let mut tags = Vec::with_capacity(rows.len());
        for row in rows {
            tags.push(map_tag(row)?);
        }
        Ok(tags)
    }

    async fn search_by_name(
        &self,
        query: &str,
        limit: i64,
    ) -> Result<Vec<Tag>, TagError> {
        let pattern = format!("{}%", query.to_lowercase());
        let rows = sqlx::query(concat!(
            "SELECT ", cols!(), " FROM tag ",
            "WHERE lower(name) LIKE $1 ",
            "ORDER BY usage_count DESC, name ASC ",
            "LIMIT $2"
        ))
        .bind(&pattern)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| TagError::Database(e.to_string()))?;

        let mut tags = Vec::with_capacity(rows.len());
        for row in rows {
            tags.push(map_tag(row)?);
        }
        Ok(tags)
    }

    async fn update(
        &self,
        id: Uuid,
        name: &str,
        is_approved: bool,
    ) -> Result<Tag, TagError> {
        let row = sqlx::query(concat!(
            "UPDATE tag SET name = $1, is_approved = $2 ",
            "WHERE id = $3 ",
            "RETURNING ", cols!()
        ))
        .bind(name)
        .bind(is_approved)
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| {
            if is_unique_violation(&e) {
                TagError::NameTaken(name.to_string())
            } else {
                TagError::Database(e.to_string())
            }
        })?
        .ok_or(TagError::NotFound)?;

        map_tag(row)
    }

    async fn increment_usage_count(&self, id: Uuid) -> Result<(), TagError> {
        increment_usage_count(&self.pool, id).await
    }

    async fn decrement_usage_count(&self, id: Uuid) -> Result<(), TagError> {
        decrement_usage_count(&self.pool, id).await
    }

    async fn delete(&self, id: Uuid) -> Result<(), TagError> {
        let result = sqlx::query("DELETE FROM tag WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| TagError::Database(e.to_string()))?;

        if result.rows_affected() == 0 {
            return Err(TagError::NotFound);
        }
        Ok(())
    }

    async fn attach_and_increment(
        &self,
        entity_type: TaggableEntityType,
        entity_id: Uuid,
        tag_id: Uuid,
    ) -> Result<EntityTag, TagError> {
        let mut tx = self.pool.begin().await
            .map_err(|e| TagError::Database(e.to_string()))?;

        let entity_tag = super::entity_tag_repository::attach_entity_tag(
            &mut *tx, entity_type, entity_id, tag_id,
        )
        .await
        .map_err(|e| match e {
            EntityTagError::AlreadyAttached => TagError::AlreadyAttached,
            EntityTagError::Database(msg) => TagError::Database(msg),
            other => TagError::Database(other.to_string()),
        })?;

        increment_usage_count(&mut *tx, tag_id).await?;

        tx.commit().await
            .map_err(|e| TagError::Database(e.to_string()))?;

        Ok(entity_tag)
    }

    async fn detach_and_decrement(
        &self,
        entity_type: TaggableEntityType,
        entity_id: Uuid,
        tag_id: Uuid,
    ) -> Result<(), TagError> {
        let mut tx = self.pool.begin().await
            .map_err(|e| TagError::Database(e.to_string()))?;

        super::entity_tag_repository::detach_entity_tag(
            &mut *tx, entity_type, entity_id, tag_id,
        )
        .await
        .map_err(|e| match e {
            EntityTagError::NotFound => TagError::NotAttached,
            EntityTagError::Database(msg) => TagError::Database(msg),
            other => TagError::Database(other.to_string()),
        })?;

        decrement_usage_count(&mut *tx, tag_id).await?;

        tx.commit().await
            .map_err(|e| TagError::Database(e.to_string()))?;

        Ok(())
    }

    async fn create_and_attach(
        &self,
        category: TagCategory,
        name: &str,
        is_approved: bool,
        entity_type: TaggableEntityType,
        entity_id: Uuid,
    ) -> Result<Tag, TagError> {
        let mut tx = self.pool.begin().await
            .map_err(|e| TagError::Database(e.to_string()))?;

        let mut tag = create_tag(&mut *tx, category, name, is_approved).await?;
        super::entity_tag_repository::attach_entity_tag(&mut *tx, entity_type, entity_id, tag.id)
            .await
            .map_err(|e| TagError::Database(e.to_string()))?;
        increment_usage_count(&mut *tx, tag.id).await?;

        tx.commit().await
            .map_err(|e| TagError::Database(e.to_string()))?;

        tag.usage_count = 1;
        Ok(tag)
    }
}
