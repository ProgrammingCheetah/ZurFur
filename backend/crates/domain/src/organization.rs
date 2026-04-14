use chrono::{DateTime, Utc};
use uuid::Uuid;

/// An organization — the universal container for identity, profiles, commissions,
/// TOS, and payments on the platform.
///
/// ARCHITECTURE DECISIONS:
///   Every user gets a personal org on signup (`is_personal = true`). The personal
///   org IS the user's public-facing profile. Users can also create additional orgs
///   freely (studios, groups, SFW/NSFW separation, etc.).
///
///   `display_name` is nullable. For personal orgs, NULL means "resolve from the
///   owner's username/handle" at the API layer. This avoids duplicating the user's
///   handle (which syncs from Bluesky) and prevents stale-data drift.
///
///   No `created_by` — creator is the owner member in organization_member.
///   Aggregates never reference each other in the schema.
#[derive(Debug, Clone)]
pub struct Organization {
    pub id: Uuid,
    pub slug: String,
    pub display_name: Option<String>,
    pub is_personal: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Errors from organization operations.
#[derive(Debug, thiserror::Error)]
pub enum OrganizationError {
    #[error("Organization not found")]
    NotFound,
    #[error("Slug already taken: {0}")]
    SlugTaken(String),
    #[error("Database error: {0}")]
    Database(String),
}

/// Repository trait for organization persistence.
#[async_trait::async_trait]
pub trait OrganizationRepository: Send + Sync {
    async fn create(
        &self,
        slug: &str,
        display_name: Option<&str>,
        is_personal: bool,
    ) -> Result<Organization, OrganizationError>;

    async fn find_by_id(&self, id: Uuid) -> Result<Option<Organization>, OrganizationError>;

    async fn find_by_slug(&self, slug: &str) -> Result<Option<Organization>, OrganizationError>;

    /// Find the personal org for a user. Returns None if one hasn't been created yet.
    async fn find_personal_org(
        &self,
        user_id: Uuid,
    ) -> Result<Option<Organization>, OrganizationError>;

    async fn update_display_name(
        &self,
        id: Uuid,
        display_name: Option<&str>,
    ) -> Result<Organization, OrganizationError>;

    async fn soft_delete(&self, id: Uuid) -> Result<(), OrganizationError>;

    /// Atomically create an organization and add the user as Owner with full permissions.
    /// Implementations must perform both operations in a single transaction.
    async fn create_with_owner(
        &self,
        slug: &str,
        display_name: Option<&str>,
        is_personal: bool,
        owner_user_id: Uuid,
    ) -> Result<Organization, OrganizationError>;
}
