use crate::pool::Pool;
use domain::organization::{Organization, OrganizationError, OrganizationRepository};
use sqlx::Row;
use std::sync::Arc;
use uuid::Uuid;

pub struct SqlxOrganizationRepository {
    pool: Pool,
}

impl SqlxOrganizationRepository {
    pub fn new(pool: Pool) -> Self {
        Self { pool }
    }

    pub fn from_pool(pool: Pool) -> Arc<dyn OrganizationRepository> {
        Arc::new(Self::new(pool))
    }
}

fn map_organization(row: sqlx::postgres::PgRow) -> Organization {
    Organization {
        id: row.get("id"),
        slug: row.get("slug"),
        display_name: row.get("display_name"),
        is_personal: row.get("is_personal"),
        created_by: row.get("created_by"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
}

#[async_trait::async_trait]
impl OrganizationRepository for SqlxOrganizationRepository {
    async fn create(
        &self,
        slug: &str,
        display_name: Option<&str>,
        is_personal: bool,
        created_by: Uuid,
    ) -> Result<Organization, OrganizationError> {
        sqlx::query(
            "INSERT INTO organizations (slug, display_name, is_personal, created_by) \
             VALUES ($1, $2, $3, $4) \
             RETURNING id, slug, display_name, is_personal, created_by, created_at, updated_at",
        )
        .bind(slug)
        .bind(display_name)
        .bind(is_personal)
        .bind(created_by)
        .fetch_one(&self.pool)
        .await
        .map(map_organization)
        .map_err(|e| {
            if is_unique_violation(&e) {
                OrganizationError::SlugTaken(slug.to_string())
            } else {
                OrganizationError::Database(e.to_string())
            }
        })
    }

    async fn find_by_id(&self, id: Uuid) -> Result<Option<Organization>, OrganizationError> {
        sqlx::query(
            "SELECT id, slug, display_name, is_personal, created_by, created_at, updated_at \
             FROM organizations WHERE id = $1 AND deleted_at IS NULL",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map(|opt| opt.map(map_organization))
        .map_err(|e| OrganizationError::Database(e.to_string()))
    }

    async fn find_by_slug(&self, slug: &str) -> Result<Option<Organization>, OrganizationError> {
        sqlx::query(
            "SELECT id, slug, display_name, is_personal, created_by, created_at, updated_at \
             FROM organizations WHERE slug = $1 AND deleted_at IS NULL",
        )
        .bind(slug)
        .fetch_optional(&self.pool)
        .await
        .map(|opt| opt.map(map_organization))
        .map_err(|e| OrganizationError::Database(e.to_string()))
    }

    async fn find_personal_org(
        &self,
        user_id: Uuid,
    ) -> Result<Option<Organization>, OrganizationError> {
        sqlx::query(
            "SELECT id, slug, display_name, is_personal, created_by, created_at, updated_at \
             FROM organizations \
             WHERE created_by = $1 AND is_personal = true AND deleted_at IS NULL",
        )
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await
        .map(|opt| opt.map(map_organization))
        .map_err(|e| OrganizationError::Database(e.to_string()))
    }

    async fn update_display_name(
        &self,
        id: Uuid,
        display_name: Option<&str>,
    ) -> Result<Organization, OrganizationError> {
        sqlx::query(
            "UPDATE organizations SET display_name = $1 \
             WHERE id = $2 AND deleted_at IS NULL \
             RETURNING id, slug, display_name, is_personal, created_by, created_at, updated_at",
        )
        .bind(display_name)
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| OrganizationError::Database(e.to_string()))?
        .map(map_organization)
        .ok_or(OrganizationError::NotFound)
    }

    async fn soft_delete(&self, id: Uuid) -> Result<(), OrganizationError> {
        let result = sqlx::query(
            "UPDATE organizations SET deleted_at = now() \
             WHERE id = $1 AND deleted_at IS NULL",
        )
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(|e| OrganizationError::Database(e.to_string()))?;

        if result.rows_affected() == 0 {
            return Err(OrganizationError::NotFound);
        }
        Ok(())
    }
}

fn is_unique_violation(e: &sqlx::Error) -> bool {
    if let sqlx::Error::Database(db_err) = e {
        db_err.code().as_deref() == Some("23505")
    } else {
        false
    }
}
