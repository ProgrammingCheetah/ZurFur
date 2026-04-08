use chrono::{DateTime, Utc};
use uuid::Uuid;

/// Bitfield permissions for organization members.
///
/// ARCHITECTURE DECISIONS:
///   BIGINT (i64 in Postgres, u64 in Rust) bitfield chosen over JSONB for:
///   - Faster permission checks (bitwise AND vs JSON parsing)
///   - Compact storage (8 bytes vs variable-length JSON)
///   - Easy combination with `|` and checking with `&`
///   - Extensible without migration: just define new bit positions
///
///   `ALL = u64::MAX` ensures that every current and future bit position is set,
///   so adding new permissions automatically grants them to owners.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Permissions(pub u64);

impl Permissions {
    pub const NONE: u64 = 0;
    pub const MANAGE_PROFILE: u64 = 1 << 0;
    pub const MANAGE_MEMBERS: u64 = 1 << 1;
    pub const MANAGE_COMMISSIONS: u64 = 1 << 2;
    pub const CHAT: u64 = 1 << 3;
    pub const MANAGE_TOS: u64 = 1 << 4;
    pub const MANAGE_PAYMENTS: u64 = 1 << 5;
    pub const ALL: u64 = u64::MAX;

    pub fn new(bits: u64) -> Self {
        Self(bits)
    }

    pub fn has(&self, permission: u64) -> bool {
        self.0 & permission == permission
    }

    pub fn add(&self, permission: u64) -> Self {
        Self(self.0 | permission)
    }

    pub fn remove(&self, permission: u64) -> Self {
        Self(self.0 & !permission)
    }
}

impl Default for Permissions {
    fn default() -> Self {
        Self(Self::NONE)
    }
}

/// A membership linking a user to an organization with a role, title, and permissions.
///
/// ARCHITECTURE DECISIONS:
///   `role` is free-text (not an enum) because orgs can define whatever roles make
///   sense for their context. The platform suggests common roles (Artist, Manager,
///   Member) but doesn't restrict them.
///
///   `title` is purely cosmetic — a self-given display string shown on the org
///   page and profile (like GitHub's "Contributor", "Maintainer" labels, but
///   fully customizable: "Furry Artist", "Code Breaker", etc.).
///
///   `is_owner` is separate from permissions because ownership is a structural
///   property (can't be removed, gets ALL permissions) not just a permission level.
#[derive(Debug, Clone)]
pub struct OrganizationMember {
    pub id: Uuid,
    pub org_id: Uuid,
    pub user_id: Uuid,
    pub role: String,
    pub title: Option<String>,
    pub is_owner: bool,
    pub permissions: Permissions,
    pub joined_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, thiserror::Error)]
pub enum OrganizationMemberError {
    #[error("Member not found")]
    NotFound,
    #[error("User is already a member of this organization")]
    AlreadyMember,
    #[error("Database error: {0}")]
    Database(String),
}

#[async_trait::async_trait]
pub trait OrganizationMemberRepository: Send + Sync {
    async fn add(
        &self,
        org_id: Uuid,
        user_id: Uuid,
        role: &str,
        title: Option<&str>,
        is_owner: bool,
        permissions: Permissions,
    ) -> Result<OrganizationMember, OrganizationMemberError>;

    async fn find_by_org_and_user(
        &self,
        org_id: Uuid,
        user_id: Uuid,
    ) -> Result<Option<OrganizationMember>, OrganizationMemberError>;

    async fn list_by_org(
        &self,
        org_id: Uuid,
    ) -> Result<Vec<OrganizationMember>, OrganizationMemberError>;

    async fn list_by_user(
        &self,
        user_id: Uuid,
    ) -> Result<Vec<OrganizationMember>, OrganizationMemberError>;

    async fn update_role_and_title(
        &self,
        org_id: Uuid,
        user_id: Uuid,
        role: &str,
        title: Option<&str>,
    ) -> Result<OrganizationMember, OrganizationMemberError>;

    async fn update_permissions(
        &self,
        org_id: Uuid,
        user_id: Uuid,
        permissions: Permissions,
    ) -> Result<OrganizationMember, OrganizationMemberError>;

    async fn remove(
        &self,
        org_id: Uuid,
        user_id: Uuid,
    ) -> Result<(), OrganizationMemberError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_permissions_is_none() {
        let perms = Permissions::default();
        assert_eq!(perms.0, Permissions::NONE);
        assert!(!perms.has(Permissions::MANAGE_PROFILE));
        assert!(!perms.has(Permissions::CHAT));
    }

    #[test]
    fn has_checks_single_permission() {
        let perms = Permissions::new(Permissions::MANAGE_PROFILE | Permissions::CHAT);
        assert!(perms.has(Permissions::MANAGE_PROFILE));
        assert!(perms.has(Permissions::CHAT));
        assert!(!perms.has(Permissions::MANAGE_MEMBERS));
    }

    #[test]
    fn has_checks_combined_permissions() {
        let perms = Permissions::new(Permissions::MANAGE_PROFILE | Permissions::CHAT);
        assert!(perms.has(Permissions::MANAGE_PROFILE | Permissions::CHAT));
        assert!(!perms.has(Permissions::MANAGE_PROFILE | Permissions::MANAGE_MEMBERS));
    }

    #[test]
    fn add_sets_new_bits() {
        let perms = Permissions::default()
            .add(Permissions::MANAGE_PROFILE)
            .add(Permissions::CHAT);
        assert!(perms.has(Permissions::MANAGE_PROFILE));
        assert!(perms.has(Permissions::CHAT));
        assert!(!perms.has(Permissions::MANAGE_MEMBERS));
    }

    #[test]
    fn remove_clears_bits() {
        let perms = Permissions::new(Permissions::MANAGE_PROFILE | Permissions::CHAT)
            .remove(Permissions::CHAT);
        assert!(perms.has(Permissions::MANAGE_PROFILE));
        assert!(!perms.has(Permissions::CHAT));
    }

    #[test]
    fn all_contains_every_defined_permission() {
        let all = Permissions::new(Permissions::ALL);
        assert!(all.has(Permissions::MANAGE_PROFILE));
        assert!(all.has(Permissions::MANAGE_MEMBERS));
        assert!(all.has(Permissions::MANAGE_COMMISSIONS));
        assert!(all.has(Permissions::CHAT));
        assert!(all.has(Permissions::MANAGE_TOS));
        assert!(all.has(Permissions::MANAGE_PAYMENTS));
    }

    #[test]
    fn all_contains_hypothetical_future_permission() {
        let future_bit: u64 = 1 << 42;
        let all = Permissions::new(Permissions::ALL);
        assert!(all.has(future_bit));
    }
}
