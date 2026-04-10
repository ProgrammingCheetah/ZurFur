use crate::pool::Pool;
use crate::sqlx_utils::is_unique_violation;
use domain::organization_member::{
    OrganizationMember, OrganizationMemberError, OrganizationMemberRepository, Permissions, Role,
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
    let role_str: String = row.get("role");
    let role = Role::from_str(&role_str).unwrap_or(Role::Member);
    OrganizationMember {
        id: row.get("id"),
        org_id: row.get("org_id"),
        user_id: row.get("user_id"),
        role,
        title: row.get("title"),
        permissions: Permissions::new(permissions_raw as u64),
        joined_at: row.get("joined_at"),
        updated_at: row.get("updated_at"),
    }
}

macro_rules! cols {
    () => {
        "id, org_id, user_id, role, title, permissions, joined_at, updated_at"
    };
}

#[async_trait::async_trait]
impl OrganizationMemberRepository for SqlxOrganizationMemberRepository {
    async fn add(
        &self,
        org_id: Uuid,
        user_id: Uuid,
        role: Role,
        title: Option<&str>,
        permissions: Permissions,
    ) -> Result<OrganizationMember, OrganizationMemberError> {
        sqlx::query(concat!(
            "INSERT INTO organization_members (org_id, user_id, role, title, permissions) ",
            "VALUES ($1, $2, $3, $4, $5) ",
            "RETURNING ", cols!()
        ))
        .bind(org_id)
        .bind(user_id)
        .bind(role.as_str())
        .bind(title)
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
        sqlx::query(concat!(
            "SELECT ", cols!(), " FROM organization_members WHERE org_id = $1 AND user_id = $2"
        ))
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
        sqlx::query(concat!(
            "SELECT ", cols!(), " FROM organization_members WHERE org_id = $1 ORDER BY joined_at"
        ))
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
        sqlx::query(concat!(
            "SELECT ", cols!(), " FROM organization_members WHERE user_id = $1 ORDER BY joined_at"
        ))
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
        role: Role,
        title: Option<&str>,
    ) -> Result<OrganizationMember, OrganizationMemberError> {
        sqlx::query(concat!(
            "UPDATE organization_members SET role = $1, title = $2 ",
            "WHERE org_id = $3 AND user_id = $4 ",
            "RETURNING ", cols!()
        ))
        .bind(role.as_str())
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
        sqlx::query(concat!(
            "UPDATE organization_members SET permissions = $1 ",
            "WHERE org_id = $2 AND user_id = $3 ",
            "RETURNING ", cols!()
        ))
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

