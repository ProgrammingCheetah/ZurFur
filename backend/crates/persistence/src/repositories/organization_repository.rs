use crate::pool::Pool;
use crate::sqlx_utils::{is_unique_violation, violated_constraint};
use domain::organization::{Organization, OrganizationError, OrganizationRepository};
use sqlx::Row;
use std::sync::Arc;
use uuid::Uuid;

/// SQLx implementation of `OrganizationRepository`.
pub struct SqlxOrganizationRepository {
    pool: Pool,
}

impl SqlxOrganizationRepository {
    /// Create a new repository instance.
    pub fn new(pool: Pool) -> Self {
        Self { pool }
    }

    /// Create a new repository instance wrapped as a trait object.
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
    ) -> Result<Organization, OrganizationError> {
        sqlx::query(
            "INSERT INTO organization (slug, display_name, is_personal) \
             VALUES ($1, $2, $3) \
             RETURNING id, slug, display_name, is_personal, created_at, updated_at",
        )
        .bind(slug)
        .bind(display_name)
        .bind(is_personal)
        .fetch_one(&self.pool)
        .await
        .map(map_organization)
        .map_err(|e| {
            if is_unique_violation(&e) {
                match violated_constraint(&e) {
                    Some("uq_organizations_slug") => {
                        OrganizationError::SlugTaken(slug.to_string())
                    }
                    _ => OrganizationError::Database(e.to_string()),
                }
            } else {
                OrganizationError::Database(e.to_string())
            }
        })
    }

    async fn find_by_id(&self, id: Uuid) -> Result<Option<Organization>, OrganizationError> {
        sqlx::query(
            "SELECT id, slug, display_name, is_personal, created_at, updated_at \
             FROM organization WHERE id = $1 AND deleted_at IS NULL",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map(|opt| opt.map(map_organization))
        .map_err(|e| OrganizationError::Database(e.to_string()))
    }

    async fn find_by_slug(&self, slug: &str) -> Result<Option<Organization>, OrganizationError> {
        sqlx::query(
            "SELECT id, slug, display_name, is_personal, created_at, updated_at \
             FROM organization WHERE slug = $1 AND deleted_at IS NULL",
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
            "SELECT o.id, o.slug, o.display_name, o.is_personal, o.created_at, o.updated_at \
             FROM organization o \
             JOIN organization_member om ON om.org_id = o.id \
             WHERE om.user_id = $1 AND om.role = 'owner' \
             AND o.is_personal = true AND o.deleted_at IS NULL",
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
            "UPDATE organization SET display_name = $1 \
             WHERE id = $2 AND deleted_at IS NULL \
             RETURNING id, slug, display_name, is_personal, created_at, updated_at",
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
            "UPDATE organization SET deleted_at = now() \
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
