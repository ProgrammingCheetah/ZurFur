use crate::pool::Pool;
use domain::organization_member::{
    OrganizationMember, OrganizationMemberError, OrganizationMemberRepository, Permissions,
};
use sqlx::Row;
use std::sync::Arc;
use uuid::Uuid;

pub struct SqlxOrganizationMemberRepository {
    pool: Pool,
}

impl SqlxOrganizationMemberRepository {
    pub fn new(pool: Pool) -> Self {
        Self { pool }
    }

    pub fn from_pool(pool: Pool) -> Arc<dyn OrganizationMemberRepository> {
        Arc::new(Self::new(pool))
    }
}

fn map_member(row: sqlx::postgres::PgRow) -> OrganizationMember {
    let permissions_raw: i64 = row.get("permissions");
    OrganizationMember {
        id: row.get("id"),
        org_id: row.get("org_id"),
        user_id: row.get("user_id"),
        role: row.get("role"),
        title: row.get("title"),
        is_owner: row.get("is_owner"),
        permissions: Permissions::new(permissions_raw as u64),
        joined_at: row.get("joined_at"),
        updated_at: row.get("updated_at"),
    }
}

const SELECT_COLS: &str =
    "id, org_id, user_id, role, title, is_owner, permissions, joined_at, updated_at";

#[async_trait::async_trait]
impl OrganizationMemberRepository for SqlxOrganizationMemberRepository {
    async fn add(
        &self,
        org_id: Uuid,
        user_id: Uuid,
        role: &str,
        title: Option<&str>,
        is_owner: bool,
        permissions: Permissions,
    ) -> Result<OrganizationMember, OrganizationMemberError> {
        let sql = format!(
            "INSERT INTO organization_members (org_id, user_id, role, title, is_owner, permissions) \
             VALUES ($1, $2, $3, $4, $5, $6) \
             RETURNING {SELECT_COLS}"
        );
        sqlx::query(&sql)
            .bind(org_id)
            .bind(user_id)
            .bind(role)
            .bind(title)
            .bind(is_owner)
            .bind(permissions.0 as i64)
            .fetch_one(&self.pool)
            .await
            .map(map_member)
            .map_err(|e| {
                if is_unique_violation(&e) {
                    OrganizationMemberError::AlreadyMember
                } else {
                    OrganizationMemberError::Database(e.to_string())
                }
            })
    }

    async fn find_by_org_and_user(
        &self,
        org_id: Uuid,
        user_id: Uuid,
    ) -> Result<Option<OrganizationMember>, OrganizationMemberError> {
        let sql = format!(
            "SELECT {SELECT_COLS} FROM organization_members WHERE org_id = $1 AND user_id = $2"
        );
        sqlx::query(&sql)
            .bind(org_id)
            .bind(user_id)
            .fetch_optional(&self.pool)
            .await
            .map(|opt| opt.map(map_member))
            .map_err(|e| OrganizationMemberError::Database(e.to_string()))
    }

    async fn list_by_org(
        &self,
        org_id: Uuid,
    ) -> Result<Vec<OrganizationMember>, OrganizationMemberError> {
        let sql = format!(
            "SELECT {SELECT_COLS} FROM organization_members WHERE org_id = $1 ORDER BY joined_at"
        );
        sqlx::query(&sql)
            .bind(org_id)
            .fetch_all(&self.pool)
            .await
            .map(|rows| rows.into_iter().map(map_member).collect())
            .map_err(|e| OrganizationMemberError::Database(e.to_string()))
    }

    async fn list_by_user(
        &self,
        user_id: Uuid,
    ) -> Result<Vec<OrganizationMember>, OrganizationMemberError> {
        let sql = format!(
            "SELECT {SELECT_COLS} FROM organization_members WHERE user_id = $1 ORDER BY joined_at"
        );
        sqlx::query(&sql)
            .bind(user_id)
            .fetch_all(&self.pool)
            .await
            .map(|rows| rows.into_iter().map(map_member).collect())
            .map_err(|e| OrganizationMemberError::Database(e.to_string()))
    }

    async fn update_role_and_title(
        &self,
        org_id: Uuid,
        user_id: Uuid,
        role: &str,
        title: Option<&str>,
    ) -> Result<OrganizationMember, OrganizationMemberError> {
        let sql = format!(
            "UPDATE organization_members SET role = $1, title = $2 \
             WHERE org_id = $3 AND user_id = $4 \
             RETURNING {SELECT_COLS}"
        );
        sqlx::query(&sql)
            .bind(role)
            .bind(title)
            .bind(org_id)
            .bind(user_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| OrganizationMemberError::Database(e.to_string()))?
            .map(map_member)
            .ok_or(OrganizationMemberError::NotFound)
    }

    async fn update_permissions(
        &self,
        org_id: Uuid,
        user_id: Uuid,
        permissions: Permissions,
    ) -> Result<OrganizationMember, OrganizationMemberError> {
        let sql = format!(
            "UPDATE organization_members SET permissions = $1 \
             WHERE org_id = $2 AND user_id = $3 \
             RETURNING {SELECT_COLS}"
        );
        sqlx::query(&sql)
            .bind(permissions.0 as i64)
            .bind(org_id)
            .bind(user_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| OrganizationMemberError::Database(e.to_string()))?
            .map(map_member)
            .ok_or(OrganizationMemberError::NotFound)
    }

    async fn remove(
        &self,
        org_id: Uuid,
        user_id: Uuid,
    ) -> Result<(), OrganizationMemberError> {
        let result = sqlx::query(
            "DELETE FROM organization_members WHERE org_id = $1 AND user_id = $2",
        )
        .bind(org_id)
        .bind(user_id)
        .execute(&self.pool)
        .await
        .map_err(|e| OrganizationMemberError::Database(e.to_string()))?;

        if result.rows_affected() == 0 {
            return Err(OrganizationMemberError::NotFound);
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
