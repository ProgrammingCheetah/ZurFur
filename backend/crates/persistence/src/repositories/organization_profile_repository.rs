use crate::pool::Pool;
use domain::organization_profile::{
    CommissionStatus, OrganizationProfile, OrganizationProfileError, OrganizationProfileRepository,
};
use sqlx::Row;
use std::sync::Arc;
use uuid::Uuid;

pub struct SqlxOrganizationProfileRepository {
    pool: Pool,
}

impl SqlxOrganizationProfileRepository {
    pub fn new(pool: Pool) -> Self {
        Self { pool }
    }

    pub fn from_pool(pool: Pool) -> Arc<dyn OrganizationProfileRepository> {
        Arc::new(Self::new(pool))
    }
}

fn map_profile(
    row: sqlx::postgres::PgRow,
) -> Result<OrganizationProfile, OrganizationProfileError> {
    let status_str: String = row.get("commission_status");
    let commission_status = CommissionStatus::from_str(&status_str)
        .ok_or_else(|| OrganizationProfileError::InvalidStatus(status_str))?;
    Ok(OrganizationProfile {
        org_id: row.get("org_id"),
        bio: row.get("bio"),
        commission_status,
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}

#[async_trait::async_trait]
impl OrganizationProfileRepository for SqlxOrganizationProfileRepository {
    async fn upsert(
        &self,
        org_id: Uuid,
        bio: Option<&str>,
        commission_status: CommissionStatus,
    ) -> Result<OrganizationProfile, OrganizationProfileError> {
        sqlx::query(
            "INSERT INTO organization_profiles (org_id, bio, commission_status) \
             VALUES ($1, $2, $3) \
             ON CONFLICT (org_id) DO UPDATE \
                SET bio = EXCLUDED.bio, commission_status = EXCLUDED.commission_status \
             RETURNING org_id, bio, commission_status, created_at, updated_at",
        )
        .bind(org_id)
        .bind(bio)
        .bind(commission_status.as_str())
        .fetch_one(&self.pool)
        .await
        .map_err(|e| OrganizationProfileError::Database(e.to_string()))
        .and_then(map_profile)
    }

    async fn find_by_org_id(
        &self,
        org_id: Uuid,
    ) -> Result<Option<OrganizationProfile>, OrganizationProfileError> {
        sqlx::query(
            "SELECT org_id, bio, commission_status, created_at, updated_at \
             FROM organization_profiles WHERE org_id = $1",
        )
        .bind(org_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| OrganizationProfileError::Database(e.to_string()))?
        .map(map_profile)
        .transpose()
    }
}
