use std::sync::Arc;

use domain::organization::{Organization, OrganizationRepository};
use domain::organization_member::{
    OrganizationMember, OrganizationMemberRepository, Permissions,
};
use domain::organization_profile::{
    CommissionStatus, OrganizationProfile, OrganizationProfileRepository,
};
use uuid::Uuid;

#[derive(Debug, thiserror::Error)]
pub enum OrgServiceError {
    #[error("Organization not found")]
    NotFound,
    #[error("Slug already taken: {0}")]
    SlugTaken(String),
    #[error("Invalid slug: {0}")]
    InvalidSlug(String),
    #[error("Permission denied")]
    Forbidden,
    #[error("Cannot delete a personal organization")]
    CannotDeletePersonal,
    #[error("Cannot remove the owner from an organization")]
    CannotRemoveOwner,
    #[error("Invalid commission status: {0}")]
    InvalidStatus(String),
    #[error("Internal error: {0}")]
    Internal(String),
}

/// Response for get_org — combines org, members, and optional profile.
#[derive(Debug)]
pub struct OrgDetail {
    pub org: Organization,
    pub members: Vec<OrganizationMember>,
    pub profile: Option<OrganizationProfile>,
}

/// Service for organization CRUD, member management, and profile operations.
///
/// ARCHITECTURE DECISIONS:
///   All permission checks and business rules (owner-only, can't remove owner,
///   can't delete personal org) are enforced here at the service layer, not in
///   the database. This keeps the DB schema simple and business rules in Rust
///   where they're testable with mock repos.
///
///   Slug validation is also at the service layer — the DB only enforces
///   uniqueness via the UNIQUE index. Format rules and reserved word checks
///   are in validate_slug().
pub struct OrganizationService {
    org_repo: Arc<dyn OrganizationRepository>,
    member_repo: Arc<dyn OrganizationMemberRepository>,
    profile_repo: Arc<dyn OrganizationProfileRepository>,
}

/// Reserved slugs that cannot be used for organization names.
const RESERVED_SLUGS: &[&str] = &[
    "me", "new", "admin", "settings", "api", "auth", "organizations", "users", "search",
    "explore", "notifications", "help", "about", "terms", "privacy",
];

impl OrganizationService {
    pub fn new(
        org_repo: Arc<dyn OrganizationRepository>,
        member_repo: Arc<dyn OrganizationMemberRepository>,
        profile_repo: Arc<dyn OrganizationProfileRepository>,
    ) -> Self {
        Self {
            org_repo,
            member_repo,
            profile_repo,
        }
    }

    /// Validate a slug against format rules and reserved words.
    ///
    /// Rules: lowercase alphanumeric + hyphens, 2-64 chars, cannot start/end
    /// with hyphen, not in the reserved list.
    pub fn validate_slug(slug: &str) -> Result<(), OrgServiceError> {
        if slug.len() < 2 || slug.len() > 64 {
            return Err(OrgServiceError::InvalidSlug(
                "Slug must be between 2 and 64 characters".into(),
            ));
        }

        if !slug
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        {
            return Err(OrgServiceError::InvalidSlug(
                "Slug must contain only lowercase letters, numbers, and hyphens".into(),
            ));
        }

        if slug.starts_with('-') || slug.ends_with('-') {
            return Err(OrgServiceError::InvalidSlug(
                "Slug cannot start or end with a hyphen".into(),
            ));
        }

        if RESERVED_SLUGS.contains(&slug) {
            return Err(OrgServiceError::InvalidSlug(format!(
                "'{slug}' is a reserved name"
            )));
        }

        Ok(())
    }

    /// Sanitize a handle into a valid slug for personal org creation.
    /// Strips `.bsky.social` suffix, lowercases, replaces invalid chars with hyphens.
    pub fn slug_from_handle(handle: &str) -> String {
        let base = handle
            .strip_suffix(".bsky.social")
            .unwrap_or(handle)
            .to_lowercase();

        let sanitized: String = base
            .chars()
            .map(|c| {
                if c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' {
                    c
                } else {
                    '-'
                }
            })
            .collect();

        // Trim leading/trailing hyphens
        let trimmed = sanitized.trim_matches('-');

        if trimmed.len() < 2 {
            // Fallback for very short handles
            format!("user-{}", &Uuid::new_v4().to_string()[..8])
        } else if trimmed.len() > 64 {
            trimmed[..64].trim_end_matches('-').to_string()
        } else {
            trimmed.to_string()
        }
    }

    /// Create a new organization. The creating user becomes the owner.
    pub async fn create_org(
        &self,
        user_id: Uuid,
        slug: &str,
        display_name: &str,
    ) -> Result<OrgDetail, OrgServiceError> {
        Self::validate_slug(slug)?;

        let org = self
            .org_repo
            .create(slug, Some(display_name), false, user_id)
            .await
            .map_err(|e| match e {
                domain::organization::OrganizationError::SlugTaken(s) => {
                    OrgServiceError::SlugTaken(s)
                }
                other => OrgServiceError::Internal(other.to_string()),
            })?;

        let member = self
            .member_repo
            .add(
                org.id,
                user_id,
                "owner",
                None,
                true,
                Permissions::new(Permissions::ALL),
            )
            .await
            .map_err(|e| OrgServiceError::Internal(e.to_string()))?;

        Ok(OrgDetail {
            org,
            members: vec![member],
            profile: None,
        })
    }

    /// Create a personal organization for a user. Called during signup.
    ///
    /// ARCHITECTURE DECISIONS:
    ///   display_name is NULL for personal orgs — resolved from the owner's
    ///   username/handle at the API layer. This avoids duplicating the user's
    ///   handle (which syncs from Bluesky) and prevents stale-data drift.
    ///
    ///   No transaction wraps the org creation + member add. These are two
    ///   separate repo calls behind Arc<dyn Trait>. If the member add fails
    ///   after org creation, the org exists without an owner — a self-healing
    ///   check can be added later. MVP trade-off; TODO for Feature 4 UoW pattern.
    pub async fn create_personal_org(
        &self,
        user_id: Uuid,
        slug: &str,
    ) -> Result<Organization, OrgServiceError> {
        let org = self
            .org_repo
            .create(slug, None, true, user_id)
            .await
            .map_err(|e| OrgServiceError::Internal(e.to_string()))?;

        self.member_repo
            .add(
                org.id,
                user_id,
                "owner",
                None,
                true,
                Permissions::new(Permissions::ALL),
            )
            .await
            .map_err(|e| OrgServiceError::Internal(e.to_string()))?;

        Ok(org)
    }

    /// Get an organization by ID with its members and profile.
    pub async fn get_org_by_id(&self, org_id: Uuid) -> Result<OrgDetail, OrgServiceError> {
        let org = self
            .org_repo
            .find_by_id(org_id)
            .await
            .map_err(|e| OrgServiceError::Internal(e.to_string()))?
            .ok_or(OrgServiceError::NotFound)?;

        self.load_org_detail(org).await
    }

    /// Get an organization by slug with its members and profile.
    pub async fn get_org(&self, slug: &str) -> Result<OrgDetail, OrgServiceError> {
        let org = self
            .org_repo
            .find_by_slug(slug)
            .await
            .map_err(|e| OrgServiceError::Internal(e.to_string()))?
            .ok_or(OrgServiceError::NotFound)?;

        self.load_org_detail(org).await
    }

    async fn load_org_detail(
        &self,
        org: Organization,
    ) -> Result<OrgDetail, OrgServiceError> {
        let members = self
            .member_repo
            .list_by_org(org.id)
            .await
            .map_err(|e| OrgServiceError::Internal(e.to_string()))?;

        let profile = self
            .profile_repo
            .find_by_org_id(org.id)
            .await
            .map_err(|e| OrgServiceError::Internal(e.to_string()))?;

        Ok(OrgDetail {
            org,
            members,
            profile,
        })
    }

    /// Update an org's display name. Owner only.
    pub async fn update_org(
        &self,
        org_id: Uuid,
        actor_id: Uuid,
        display_name: Option<&str>,
    ) -> Result<Organization, OrgServiceError> {
        self.require_owner(org_id, actor_id).await?;

        self.org_repo
            .update_display_name(org_id, display_name)
            .await
            .map_err(|e| OrgServiceError::Internal(e.to_string()))
    }

    /// Soft-delete an organization. Owner only. Personal orgs cannot be deleted.
    pub async fn delete_org(
        &self,
        org_id: Uuid,
        actor_id: Uuid,
    ) -> Result<(), OrgServiceError> {
        let org = self
            .org_repo
            .find_by_id(org_id)
            .await
            .map_err(|e| OrgServiceError::Internal(e.to_string()))?
            .ok_or(OrgServiceError::NotFound)?;

        if org.is_personal {
            return Err(OrgServiceError::CannotDeletePersonal);
        }

        self.require_owner(org_id, actor_id).await?;

        self.org_repo
            .soft_delete(org_id)
            .await
            .map_err(|e| OrgServiceError::Internal(e.to_string()))
    }

    /// Update org profile (bio, commission status). Requires MANAGE_PROFILE permission.
    pub async fn update_profile(
        &self,
        org_id: Uuid,
        actor_id: Uuid,
        bio: Option<&str>,
        commission_status: CommissionStatus,
    ) -> Result<OrganizationProfile, OrgServiceError> {
        self.require_permission(org_id, actor_id, Permissions::MANAGE_PROFILE)
            .await?;

        self.profile_repo
            .upsert(org_id, bio, commission_status)
            .await
            .map_err(|e| OrgServiceError::Internal(e.to_string()))
    }

    /// Add a member to an org. Requires MANAGE_MEMBERS permission.
    pub async fn add_member(
        &self,
        org_id: Uuid,
        actor_id: Uuid,
        target_user_id: Uuid,
        role: &str,
        title: Option<&str>,
    ) -> Result<OrganizationMember, OrgServiceError> {
        self.require_permission(org_id, actor_id, Permissions::MANAGE_MEMBERS)
            .await?;

        self.member_repo
            .add(
                org_id,
                target_user_id,
                role,
                title,
                false,
                Permissions::default(),
            )
            .await
            .map_err(|e| OrgServiceError::Internal(e.to_string()))
    }

    /// Update a member's role and title. Requires MANAGE_MEMBERS permission.
    /// Updating permissions requires being the owner.
    pub async fn update_member(
        &self,
        org_id: Uuid,
        actor_id: Uuid,
        target_user_id: Uuid,
        role: &str,
        title: Option<&str>,
        permissions: Option<Permissions>,
    ) -> Result<OrganizationMember, OrgServiceError> {
        self.require_permission(org_id, actor_id, Permissions::MANAGE_MEMBERS)
            .await?;

        let mut member = self
            .member_repo
            .update_role_and_title(org_id, target_user_id, role, title)
            .await
            .map_err(|e| OrgServiceError::Internal(e.to_string()))?;

        if let Some(perms) = permissions {
            // Only the owner can change permissions
            self.require_owner(org_id, actor_id).await?;
            member = self
                .member_repo
                .update_permissions(org_id, target_user_id, perms)
                .await
                .map_err(|e| OrgServiceError::Internal(e.to_string()))?;
        }

        Ok(member)
    }

    /// Remove a member from an org. Requires MANAGE_MEMBERS permission.
    /// The owner cannot be removed.
    pub async fn remove_member(
        &self,
        org_id: Uuid,
        actor_id: Uuid,
        target_user_id: Uuid,
    ) -> Result<(), OrgServiceError> {
        self.require_permission(org_id, actor_id, Permissions::MANAGE_MEMBERS)
            .await?;

        // Check if target is the owner
        let target_member = self
            .member_repo
            .find_by_org_and_user(org_id, target_user_id)
            .await
            .map_err(|e| OrgServiceError::Internal(e.to_string()))?
            .ok_or(OrgServiceError::NotFound)?;

        if target_member.is_owner {
            return Err(OrgServiceError::CannotRemoveOwner);
        }

        self.member_repo
            .remove(org_id, target_user_id)
            .await
            .map_err(|e| OrgServiceError::Internal(e.to_string()))
    }

    // --- Permission helpers --------------------------------------------------

    async fn get_actor_member(
        &self,
        org_id: Uuid,
        actor_id: Uuid,
    ) -> Result<OrganizationMember, OrgServiceError> {
        self.member_repo
            .find_by_org_and_user(org_id, actor_id)
            .await
            .map_err(|e| OrgServiceError::Internal(e.to_string()))?
            .ok_or(OrgServiceError::Forbidden)
    }

    async fn require_owner(
        &self,
        org_id: Uuid,
        actor_id: Uuid,
    ) -> Result<OrganizationMember, OrgServiceError> {
        let member = self.get_actor_member(org_id, actor_id).await?;
        if !member.is_owner {
            return Err(OrgServiceError::Forbidden);
        }
        Ok(member)
    }

    async fn require_permission(
        &self,
        org_id: Uuid,
        actor_id: Uuid,
        permission: u64,
    ) -> Result<OrganizationMember, OrgServiceError> {
        let member = self.get_actor_member(org_id, actor_id).await?;
        if !member.permissions.has(permission) {
            return Err(OrgServiceError::Forbidden);
        }
        Ok(member)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::organization::{Organization, OrganizationError, OrganizationRepository};
    use domain::organization_member::{
        OrganizationMember, OrganizationMemberError, OrganizationMemberRepository, Permissions,
    };
    use domain::organization_profile::{
        CommissionStatus, OrganizationProfile, OrganizationProfileError,
        OrganizationProfileRepository,
    };
    use tokio::sync::Mutex;

    // --- Mock Repositories ---------------------------------------------------

    #[derive(Default)]
    struct MockOrgRepo {
        orgs: Mutex<Vec<Organization>>,
    }

    #[async_trait::async_trait]
    impl OrganizationRepository for MockOrgRepo {
        async fn create(
            &self,
            slug: &str,
            display_name: Option<&str>,
            is_personal: bool,
            created_by: Uuid,
        ) -> Result<Organization, OrganizationError> {
            let mut orgs = self.orgs.lock().await;
            if orgs.iter().any(|o| o.slug == slug) {
                return Err(OrganizationError::SlugTaken(slug.into()));
            }
            let org = Organization {
                id: Uuid::new_v4(),
                slug: slug.into(),
                display_name: display_name.map(String::from),
                is_personal,
                created_by,
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
            };
            orgs.push(org.clone());
            Ok(org)
        }

        async fn find_by_id(&self, id: Uuid) -> Result<Option<Organization>, OrganizationError> {
            Ok(self.orgs.lock().await.iter().find(|o| o.id == id).cloned())
        }

        async fn find_by_slug(
            &self,
            slug: &str,
        ) -> Result<Option<Organization>, OrganizationError> {
            Ok(self
                .orgs
                .lock()
                .await
                .iter()
                .find(|o| o.slug == slug)
                .cloned())
        }

        async fn find_personal_org(
            &self,
            user_id: Uuid,
        ) -> Result<Option<Organization>, OrganizationError> {
            Ok(self
                .orgs
                .lock()
                .await
                .iter()
                .find(|o| o.created_by == user_id && o.is_personal)
                .cloned())
        }

        async fn update_display_name(
            &self,
            id: Uuid,
            display_name: Option<&str>,
        ) -> Result<Organization, OrganizationError> {
            let mut orgs = self.orgs.lock().await;
            let org = orgs
                .iter_mut()
                .find(|o| o.id == id)
                .ok_or(OrganizationError::NotFound)?;
            org.display_name = display_name.map(String::from);
            Ok(org.clone())
        }

        async fn soft_delete(&self, id: Uuid) -> Result<(), OrganizationError> {
            let orgs = self.orgs.lock().await;
            if orgs.iter().any(|o| o.id == id) {
                Ok(())
            } else {
                Err(OrganizationError::NotFound)
            }
        }
    }

    #[derive(Default)]
    struct MockMemberRepo {
        members: Mutex<Vec<OrganizationMember>>,
    }

    #[async_trait::async_trait]
    impl OrganizationMemberRepository for MockMemberRepo {
        async fn add(
            &self,
            org_id: Uuid,
            user_id: Uuid,
            role: &str,
            title: Option<&str>,
            is_owner: bool,
            permissions: Permissions,
        ) -> Result<OrganizationMember, OrganizationMemberError> {
            let mut members = self.members.lock().await;
            if members
                .iter()
                .any(|m| m.org_id == org_id && m.user_id == user_id)
            {
                return Err(OrganizationMemberError::AlreadyMember);
            }
            let member = OrganizationMember {
                id: Uuid::new_v4(),
                org_id,
                user_id,
                role: role.into(),
                title: title.map(String::from),
                is_owner,
                permissions,
                joined_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
            };
            members.push(member.clone());
            Ok(member)
        }

        async fn find_by_org_and_user(
            &self,
            org_id: Uuid,
            user_id: Uuid,
        ) -> Result<Option<OrganizationMember>, OrganizationMemberError> {
            Ok(self
                .members
                .lock()
                .await
                .iter()
                .find(|m| m.org_id == org_id && m.user_id == user_id)
                .cloned())
        }

        async fn list_by_org(
            &self,
            org_id: Uuid,
        ) -> Result<Vec<OrganizationMember>, OrganizationMemberError> {
            Ok(self
                .members
                .lock()
                .await
                .iter()
                .filter(|m| m.org_id == org_id)
                .cloned()
                .collect())
        }

        async fn list_by_user(
            &self,
            user_id: Uuid,
        ) -> Result<Vec<OrganizationMember>, OrganizationMemberError> {
            Ok(self
                .members
                .lock()
                .await
                .iter()
                .filter(|m| m.user_id == user_id)
                .cloned()
                .collect())
        }

        async fn update_role_and_title(
            &self,
            org_id: Uuid,
            user_id: Uuid,
            role: &str,
            title: Option<&str>,
        ) -> Result<OrganizationMember, OrganizationMemberError> {
            let mut members = self.members.lock().await;
            let member = members
                .iter_mut()
                .find(|m| m.org_id == org_id && m.user_id == user_id)
                .ok_or(OrganizationMemberError::NotFound)?;
            member.role = role.into();
            member.title = title.map(String::from);
            Ok(member.clone())
        }

        async fn update_permissions(
            &self,
            org_id: Uuid,
            user_id: Uuid,
            permissions: Permissions,
        ) -> Result<OrganizationMember, OrganizationMemberError> {
            let mut members = self.members.lock().await;
            let member = members
                .iter_mut()
                .find(|m| m.org_id == org_id && m.user_id == user_id)
                .ok_or(OrganizationMemberError::NotFound)?;
            member.permissions = permissions;
            Ok(member.clone())
        }

        async fn remove(
            &self,
            org_id: Uuid,
            user_id: Uuid,
        ) -> Result<(), OrganizationMemberError> {
            let mut members = self.members.lock().await;
            let len_before = members.len();
            members.retain(|m| !(m.org_id == org_id && m.user_id == user_id));
            if members.len() == len_before {
                Err(OrganizationMemberError::NotFound)
            } else {
                Ok(())
            }
        }
    }

    #[derive(Default)]
    struct MockProfileRepo {
        profiles: Mutex<Vec<OrganizationProfile>>,
    }

    #[async_trait::async_trait]
    impl OrganizationProfileRepository for MockProfileRepo {
        async fn upsert(
            &self,
            org_id: Uuid,
            bio: Option<&str>,
            commission_status: CommissionStatus,
        ) -> Result<OrganizationProfile, OrganizationProfileError> {
            let mut profiles = self.profiles.lock().await;
            profiles.retain(|p| p.org_id != org_id);
            let profile = OrganizationProfile {
                org_id,
                bio: bio.map(String::from),
                commission_status,
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
            };
            profiles.push(profile.clone());
            Ok(profile)
        }

        async fn find_by_org_id(
            &self,
            org_id: Uuid,
        ) -> Result<Option<OrganizationProfile>, OrganizationProfileError> {
            Ok(self
                .profiles
                .lock()
                .await
                .iter()
                .find(|p| p.org_id == org_id)
                .cloned())
        }
    }

    // --- Helpers --------------------------------------------------------------

    fn build_service(
        org_repo: MockOrgRepo,
        member_repo: MockMemberRepo,
        profile_repo: MockProfileRepo,
    ) -> OrganizationService {
        OrganizationService::new(
            Arc::new(org_repo),
            Arc::new(member_repo),
            Arc::new(profile_repo),
        )
    }

    // --- Slug Validation Tests ------------------------------------------------

    #[test]
    fn valid_slugs() {
        assert!(OrganizationService::validate_slug("my-org").is_ok());
        assert!(OrganizationService::validate_slug("ab").is_ok());
        assert!(OrganizationService::validate_slug("test-org-123").is_ok());
    }

    #[test]
    fn slug_too_short() {
        assert!(OrganizationService::validate_slug("a").is_err());
    }

    #[test]
    fn slug_invalid_chars() {
        assert!(OrganizationService::validate_slug("My-Org").is_err());
        assert!(OrganizationService::validate_slug("my_org").is_err());
        assert!(OrganizationService::validate_slug("my org").is_err());
    }

    #[test]
    fn slug_starts_or_ends_with_hyphen() {
        assert!(OrganizationService::validate_slug("-my-org").is_err());
        assert!(OrganizationService::validate_slug("my-org-").is_err());
    }

    #[test]
    fn slug_reserved_words() {
        assert!(OrganizationService::validate_slug("admin").is_err());
        assert!(OrganizationService::validate_slug("me").is_err());
        assert!(OrganizationService::validate_slug("settings").is_err());
    }

    #[test]
    fn slug_from_handle_strips_bsky_suffix() {
        assert_eq!(
            OrganizationService::slug_from_handle("coolartist.bsky.social"),
            "coolartist"
        );
    }

    #[test]
    fn slug_from_handle_sanitizes_special_chars() {
        assert_eq!(
            OrganizationService::slug_from_handle("cool.artist.com"),
            "cool-artist-com"
        );
    }

    // --- Create Org Tests ----------------------------------------------------

    #[tokio::test]
    async fn create_org_makes_user_owner() {
        let user_id = Uuid::new_v4();
        let svc = build_service(
            MockOrgRepo::default(),
            MockMemberRepo::default(),
            MockProfileRepo::default(),
        );

        let detail = svc.create_org(user_id, "my-studio", "My Studio").await.unwrap();
        assert_eq!(detail.org.slug, "my-studio");
        assert!(!detail.org.is_personal);
        assert_eq!(detail.members.len(), 1);
        assert!(detail.members[0].is_owner);
        assert_eq!(detail.members[0].user_id, user_id);
    }

    #[tokio::test]
    async fn create_org_with_invalid_slug_fails() {
        let svc = build_service(
            MockOrgRepo::default(),
            MockMemberRepo::default(),
            MockProfileRepo::default(),
        );

        let err = svc.create_org(Uuid::new_v4(), "admin", "Admin").await.unwrap_err();
        assert!(matches!(err, OrgServiceError::InvalidSlug(_)));
    }

    #[tokio::test]
    async fn create_personal_org_has_null_display_name() {
        let user_id = Uuid::new_v4();
        let svc = build_service(
            MockOrgRepo::default(),
            MockMemberRepo::default(),
            MockProfileRepo::default(),
        );

        let org = svc.create_personal_org(user_id, "testuser").await.unwrap();
        assert!(org.is_personal);
        assert!(org.display_name.is_none());
    }

    // --- Permission Tests ----------------------------------------------------

    #[tokio::test]
    async fn update_org_by_non_owner_fails() {
        let owner_id = Uuid::new_v4();
        let other_id = Uuid::new_v4();
        let svc = build_service(
            MockOrgRepo::default(),
            MockMemberRepo::default(),
            MockProfileRepo::default(),
        );

        let detail = svc.create_org(owner_id, "my-org", "My Org").await.unwrap();

        // Add non-owner member
        svc.add_member(detail.org.id, owner_id, other_id, "member", None)
            .await
            .unwrap();

        let err = svc
            .update_org(detail.org.id, other_id, Some("New Name"))
            .await
            .unwrap_err();
        assert!(matches!(err, OrgServiceError::Forbidden));
    }

    #[tokio::test]
    async fn delete_personal_org_fails() {
        let user_id = Uuid::new_v4();
        let svc = build_service(
            MockOrgRepo::default(),
            MockMemberRepo::default(),
            MockProfileRepo::default(),
        );

        let org = svc.create_personal_org(user_id, "testuser").await.unwrap();
        let err = svc.delete_org(org.id, user_id).await.unwrap_err();
        assert!(matches!(err, OrgServiceError::CannotDeletePersonal));
    }

    #[tokio::test]
    async fn delete_org_by_owner_succeeds() {
        let user_id = Uuid::new_v4();
        let svc = build_service(
            MockOrgRepo::default(),
            MockMemberRepo::default(),
            MockProfileRepo::default(),
        );

        let detail = svc.create_org(user_id, "my-org", "My Org").await.unwrap();
        svc.delete_org(detail.org.id, user_id).await.unwrap();
    }

    #[tokio::test]
    async fn update_profile_without_permission_fails() {
        let owner_id = Uuid::new_v4();
        let other_id = Uuid::new_v4();
        let svc = build_service(
            MockOrgRepo::default(),
            MockMemberRepo::default(),
            MockProfileRepo::default(),
        );

        let detail = svc.create_org(owner_id, "my-org", "My Org").await.unwrap();

        // Add member with no permissions
        svc.add_member(detail.org.id, owner_id, other_id, "member", None)
            .await
            .unwrap();

        let err = svc
            .update_profile(detail.org.id, other_id, Some("bio"), CommissionStatus::Open)
            .await
            .unwrap_err();
        assert!(matches!(err, OrgServiceError::Forbidden));
    }

    #[tokio::test]
    async fn update_profile_with_permission_succeeds() {
        let owner_id = Uuid::new_v4();
        let svc = build_service(
            MockOrgRepo::default(),
            MockMemberRepo::default(),
            MockProfileRepo::default(),
        );

        let detail = svc.create_org(owner_id, "my-org", "My Org").await.unwrap();

        let profile = svc
            .update_profile(
                detail.org.id,
                owner_id,
                Some("Artist studio"),
                CommissionStatus::Open,
            )
            .await
            .unwrap();

        assert_eq!(profile.bio.as_deref(), Some("Artist studio"));
        assert_eq!(profile.commission_status, CommissionStatus::Open);
    }

    #[tokio::test]
    async fn remove_owner_fails() {
        let user_id = Uuid::new_v4();
        let svc = build_service(
            MockOrgRepo::default(),
            MockMemberRepo::default(),
            MockProfileRepo::default(),
        );

        let detail = svc.create_org(user_id, "my-org", "My Org").await.unwrap();
        let err = svc
            .remove_member(detail.org.id, user_id, user_id)
            .await
            .unwrap_err();
        assert!(matches!(err, OrgServiceError::CannotRemoveOwner));
    }

    #[tokio::test]
    async fn remove_non_owner_member_succeeds() {
        let owner_id = Uuid::new_v4();
        let member_id = Uuid::new_v4();
        let svc = build_service(
            MockOrgRepo::default(),
            MockMemberRepo::default(),
            MockProfileRepo::default(),
        );

        let detail = svc.create_org(owner_id, "my-org", "My Org").await.unwrap();
        svc.add_member(detail.org.id, owner_id, member_id, "member", None)
            .await
            .unwrap();

        svc.remove_member(detail.org.id, owner_id, member_id)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn get_org_returns_org_with_members_and_profile() {
        let user_id = Uuid::new_v4();
        let svc = build_service(
            MockOrgRepo::default(),
            MockMemberRepo::default(),
            MockProfileRepo::default(),
        );

        let detail = svc.create_org(user_id, "my-org", "My Org").await.unwrap();
        svc.update_profile(
            detail.org.id,
            user_id,
            Some("test bio"),
            CommissionStatus::Closed,
        )
        .await
        .unwrap();

        let fetched = svc.get_org("my-org").await.unwrap();
        assert_eq!(fetched.org.slug, "my-org");
        assert_eq!(fetched.members.len(), 1);
        assert!(fetched.profile.is_some());
        assert_eq!(fetched.profile.unwrap().bio.as_deref(), Some("test bio"));
    }
}
