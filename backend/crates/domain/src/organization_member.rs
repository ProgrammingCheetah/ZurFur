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

    /// Create a new permission set from raw bits.
    pub fn new(bits: u64) -> Self {
        Self(bits)
    }

    /// Check whether the given permission bit(s) are all set.
    pub fn has(&self, permission: u64) -> bool {
        self.0 & permission == permission
    }

    /// Return a new permission set with the given bit(s) added.
    pub fn add(&self, permission: u64) -> Self {
        Self(self.0 | permission)
    }

    /// Return a new permission set with the given bit(s) removed.
    pub fn remove(&self, permission: u64) -> Self {
        Self(self.0 & !permission)
    }
}

impl Default for Permissions {
    fn default() -> Self {
        Self(Self::NONE)
    }
}

/// Administrative role within an organization.
///
/// Roles are structural positions that determine what a member can do.
/// Cosmetic identifiers like "Artist" or "Furry Illustrator" belong in `title`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Owner,
    Admin,
    Mod,
    Member,
}

impl Role {
    /// Returns the string representation matching the database value.
    pub fn as_str(&self) -> &'static str {
        match self {
            Role::Owner => "owner",
            Role::Admin => "admin",
            Role::Mod => "mod",
            Role::Member => "member",
        }
    }

    /// Parse from a database string value. Returns `None` for unknown values.
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "owner" => Some(Role::Owner),
            "admin" => Some(Role::Admin),
            "mod" => Some(Role::Mod),
            "member" => Some(Role::Member),
            _ => None,
        }
    }
}

impl From<Role> for &'static str {
    fn from(role: Role) -> Self {
        role.as_str()
    }
}

impl TryFrom<&str> for Role {
    type Error = String;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        Role::from_str(s).ok_or_else(|| format!("Unknown role: {s}"))
    }
}

/// A membership linking a user to an organization with a role, title, and permissions.
///
/// ARCHITECTURE DECISIONS:
///   `role` is an enum of administrative positions (Owner, Admin, Mod, Member).
///   Cosmetic identifiers belong in `title`, which is a self-given display string
///   (like "Furry Artist", "Code Breaker", etc.).
///
///   Ownership is derived from `role == Role::Owner` via the `is_owner()` method.
///   Domain rule: personal org owners are immutable — their role cannot be changed.
#[derive(Debug, Clone)]
pub struct OrganizationMember {
    pub id: Uuid,
    pub org_id: Uuid,
    pub user_id: Uuid,
    pub role: Role,
    pub title: Option<String>,
    pub permissions: Permissions,
    pub joined_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl OrganizationMember {
    /// Whether this member has the Owner role.
    pub fn is_owner(&self) -> bool {
        self.role == Role::Owner
    }
}

/// Errors from organization member operations.
#[derive(Debug, thiserror::Error)]
pub enum OrganizationMemberError {
    #[error("Member not found")]
    NotFound,
    #[error("User is already a member of this organization")]
    AlreadyMember,
    #[error("Database error: {0}")]
    Database(String),
}

/// Repository trait for organization member persistence.
#[async_trait::async_trait]
pub trait OrganizationMemberRepository: Send + Sync {
    async fn add(
        &self,
        org_id: Uuid,
        user_id: Uuid,
        role: Role,
        title: Option<&str>,
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
        role: Role,
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

    #[test]
    fn role_round_trip() {
        let variants = [
            (Role::Owner, "owner"),
            (Role::Admin, "admin"),
            (Role::Mod, "mod"),
            (Role::Member, "member"),
        ];
        for (variant, s) in variants {
            assert_eq!(variant.as_str(), s);
            assert_eq!(Role::from_str(s), Some(variant));
        }
    }

    #[test]
    fn role_from_str_returns_none_for_unknown() {
        assert_eq!(Role::from_str("artist"), None);
        assert_eq!(Role::from_str(""), None);
    }

    #[test]
    fn is_owner_returns_true_only_for_owner_role() {
        let make_member = |role: Role| OrganizationMember {
            id: Uuid::new_v4(),
            org_id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            role,
            title: None,
            permissions: Permissions::default(),
            joined_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };

        assert!(make_member(Role::Owner).is_owner());
        assert!(!make_member(Role::Admin).is_owner());
        assert!(!make_member(Role::Mod).is_owner());
        assert!(!make_member(Role::Member).is_owner());
    }
}
