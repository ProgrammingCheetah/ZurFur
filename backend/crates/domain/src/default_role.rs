//! DefaultRole — system-defined role templates with preset permissions.
//!
//! ARCHITECTURE DECISIONS:
//!   Default roles are seeded in the migration (owner/admin/artist/mod/member).
//!   When assigning a role to a member, the application layer looks up the
//!   default role to get initial permissions. Per-member permissions can then
//!   be overridden individually without affecting the default role definition.
//!   `hierarchy_level` determines who can modify whom: lower level = higher rank
//!   (owner=0, admin=1, artist=2, mod=3, member=4).
//!
//! TODO: Evolve into a full `Role` entity where default roles are marked with
//!   `is_default = true`. Currently org_members.role is a free-text string —
//!   it should eventually become a FK to a roles table. Deferred until the
//!   commission engine needs custom role definitions.

use uuid::Uuid;

use crate::organization_member::Permissions;

#[derive(Debug, Clone)]
pub struct DefaultRole {
    pub id: Uuid,
    pub name: String,
    pub default_permissions: Permissions,
    pub hierarchy_level: i32,
}

#[derive(Debug, thiserror::Error)]
pub enum DefaultRoleError {
    #[error("Default role not found: {0}")]
    NotFound(String),
    #[error("Database error: {0}")]
    Database(String),
}

#[async_trait::async_trait]
pub trait DefaultRoleRepository: Send + Sync {
    async fn find_by_name(&self, name: &str) -> Result<Option<DefaultRole>, DefaultRoleError>;

    async fn list_all(&self) -> Result<Vec<DefaultRole>, DefaultRoleError>;
}
