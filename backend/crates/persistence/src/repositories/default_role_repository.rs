use crate::pool::Pool;
use domain::default_role::{DefaultRole, DefaultRoleError, DefaultRoleRepository};
use domain::organization_member::Permissions;
use sqlx::Row;
use std::sync::Arc;

pub struct SqlxDefaultRoleRepository {
    pool: Pool,
}

impl SqlxDefaultRoleRepository {
    pub fn new(pool: Pool) -> Self {
        Self { pool }
    }

    pub fn from_pool(pool: Pool) -> Arc<dyn DefaultRoleRepository> {
        Arc::new(Self::new(pool))
    }
}

/// ARCHITECTURE DECISIONS:
///   Permissions are stored as BIGINT (signed i64) in Postgres but
///   represented as u64 in Rust. The cast `i64 as u64` wraps correctly:
///   -1i64 becomes u64::MAX (all bits set) for the owner role. This matches
///   the existing pattern in organization_member_repository.rs.
fn map_default_role(row: sqlx::postgres::PgRow) -> DefaultRole {
    let perms_raw: i64 = row.get("default_permissions");

    DefaultRole {
        id: row.get("id"),
        name: row.get("name"),
        default_permissions: Permissions::new(perms_raw as u64),
        hierarchy_level: row.get("hierarchy_level"),
    }
}

#[async_trait::async_trait]
impl DefaultRoleRepository for SqlxDefaultRoleRepository {
    async fn find_by_name(&self, name: &str) -> Result<Option<DefaultRole>, DefaultRoleError> {
        let row = sqlx::query(
            "SELECT id, name, default_permissions, hierarchy_level \
             FROM default_role WHERE name = $1",
        )
        .bind(name)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| DefaultRoleError::Database(e.to_string()))?;

        let role = row.map(map_default_role);
        Ok(role)
    }

    async fn list_all(&self) -> Result<Vec<DefaultRole>, DefaultRoleError> {
        let rows = sqlx::query(
            "SELECT id, name, default_permissions, hierarchy_level \
             FROM default_role ORDER BY hierarchy_level ASC",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| DefaultRoleError::Database(e.to_string()))?;

        let roles = rows.into_iter().map(map_default_role).collect();
        Ok(roles)
    }
}
