use std::sync::Arc;

use domain::content_rating::ContentRating;
use domain::organization::{Organization, OrganizationRepository};
use domain::organization_member::{OrganizationMember, OrganizationMemberRepository};
use domain::organization_profile::{OrganizationProfile, OrganizationProfileRepository};
use domain::user::{User, UserRepository};
use domain::user_preferences::{UserPreferences, UserPreferencesRepository};
use uuid::Uuid;

#[derive(Debug, thiserror::Error)]
pub enum UserServiceError {
    #[error("User not found")]
    NotFound,
    #[error("Invalid content rating: {0}")]
    InvalidRating(String),
    #[error("Internal error: {0}")]
    Internal(String),
}

/// The full profile response for GET /users/me.
#[derive(Debug)]
pub struct UserProfile {
    pub user: User,
    pub personal_org: Option<Organization>,
    pub personal_org_profile: Option<OrganizationProfile>,
    pub memberships: Vec<OrganizationMember>,
}

/// Service for user profile and preferences operations.
///
/// ARCHITECTURE DECISIONS:
///   UserService does NOT own org/member repos for mutation — that's
///   OrganizationService's job. UserService only reads org data to
///   assemble the user's profile view. This keeps write responsibility
///   in a single service per aggregate.
pub struct UserService {
    user_repo: Arc<dyn UserRepository>,
    org_repo: Arc<dyn OrganizationRepository>,
    org_profile_repo: Arc<dyn OrganizationProfileRepository>,
    member_repo: Arc<dyn OrganizationMemberRepository>,
    preferences_repo: Arc<dyn UserPreferencesRepository>,
}

impl UserService {
    pub fn new(
        user_repo: Arc<dyn UserRepository>,
        org_repo: Arc<dyn OrganizationRepository>,
        org_profile_repo: Arc<dyn OrganizationProfileRepository>,
        member_repo: Arc<dyn OrganizationMemberRepository>,
        preferences_repo: Arc<dyn UserPreferencesRepository>,
    ) -> Self {
        Self {
            user_repo,
            org_repo,
            org_profile_repo,
            member_repo,
            preferences_repo,
        }
    }

    /// Get the authenticated user's full profile: user identity, personal org
    /// (with profile if it exists), and all org memberships.
    pub async fn get_my_profile(&self, user_id: Uuid) -> Result<UserProfile, UserServiceError> {
        let user = self
            .user_repo
            .find_by_id(user_id)
            .await
            .map_err(|e| UserServiceError::Internal(e.to_string()))?
            .ok_or(UserServiceError::NotFound)?;

        let personal_org = self
            .org_repo
            .find_personal_org(user_id)
            .await
            .map_err(|e| UserServiceError::Internal(e.to_string()))?;

        let personal_org_profile = match &personal_org {
            Some(org) => self
                .org_profile_repo
                .find_by_org_id(org.id)
                .await
                .map_err(|e| UserServiceError::Internal(e.to_string()))?,
            None => None,
        };

        let memberships = self
            .member_repo
            .list_by_user(user_id)
            .await
            .map_err(|e| UserServiceError::Internal(e.to_string()))?;

        Ok(UserProfile {
            user,
            personal_org,
            personal_org_profile,
            memberships,
        })
    }

    pub async fn get_preferences(
        &self,
        user_id: Uuid,
    ) -> Result<UserPreferences, UserServiceError> {
        self.preferences_repo
            .get(user_id)
            .await
            .map_err(|e| UserServiceError::Internal(e.to_string()))
    }

    pub async fn set_max_content_rating(
        &self,
        user_id: Uuid,
        rating: ContentRating,
    ) -> Result<UserPreferences, UserServiceError> {
        self.preferences_repo
            .set_max_content_rating(user_id, rating)
            .await
            .map_err(|e| UserServiceError::Internal(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::organization::{Organization, OrganizationError, OrganizationRepository};
    use domain::organization_member::{
        OrganizationMember, OrganizationMemberError, OrganizationMemberRepository, Permissions,
        Role,
    };
    use domain::organization_profile::{
        CommissionStatus, OrganizationProfile, OrganizationProfileError,
        OrganizationProfileRepository,
    };
    use domain::user::{User, UserError, UserRepository};
    use domain::user_preferences::{
        UserPreferences, UserPreferencesError, UserPreferencesRepository,
    };
    use tokio::sync::Mutex;

    // --- Mock Repositories ---------------------------------------------------

    #[derive(Default)]
    struct MockUserRepo {
        users: Mutex<Vec<User>>,
    }

    #[async_trait::async_trait]
    impl UserRepository for MockUserRepo {
        async fn find_by_email(&self, _email: &str) -> Result<Option<User>, UserError> {
            Ok(None)
        }
        async fn find_by_id(&self, id: Uuid) -> Result<Option<User>, UserError> {
            Ok(self.users.lock().await.iter().find(|u| u.id == id).cloned())
        }
        async fn find_by_did(&self, _did: &str) -> Result<Option<User>, UserError> {
            Ok(None)
        }
        async fn create_from_atproto(
            &self,
            _did: &str,
            _handle: Option<&str>,
            _email: Option<&str>,
        ) -> Result<User, UserError> {
            unimplemented!()
        }
        async fn update_handle(&self, _user_id: Uuid, _handle: &str) -> Result<(), UserError> {
            Ok(())
        }
        // TODO: Implement when OnboardingService is built
        async fn mark_onboarding_completed(&self, _user_id: Uuid) -> Result<(), UserError> {
            todo!("mark_onboarding_completed not yet implemented in mock")
        }
    }

    #[derive(Default)]
    struct MockOrgRepo {
        orgs: Mutex<Vec<Organization>>,
    }

    #[async_trait::async_trait]
    impl OrganizationRepository for MockOrgRepo {
        async fn create(
            &self,
            _slug: &str,
            _display_name: Option<&str>,
            _is_personal: bool,
            _created_by: Uuid,
        ) -> Result<Organization, OrganizationError> {
            unimplemented!()
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
            _id: Uuid,
            _display_name: Option<&str>,
        ) -> Result<Organization, OrganizationError> {
            unimplemented!()
        }
        async fn soft_delete(&self, _id: Uuid) -> Result<(), OrganizationError> {
            unimplemented!()
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
            _org_id: Uuid,
            _user_id: Uuid,
            _role: Role,
            _title: Option<&str>,
            _permissions: Permissions,
        ) -> Result<OrganizationMember, OrganizationMemberError> {
            unimplemented!()
        }
        async fn find_by_org_and_user(
            &self,
            _org_id: Uuid,
            _user_id: Uuid,
        ) -> Result<Option<OrganizationMember>, OrganizationMemberError> {
            unimplemented!()
        }
        async fn list_by_org(
            &self,
            _org_id: Uuid,
        ) -> Result<Vec<OrganizationMember>, OrganizationMemberError> {
            unimplemented!()
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
            _org_id: Uuid,
            _user_id: Uuid,
            _role: Role,
            _title: Option<&str>,
        ) -> Result<OrganizationMember, OrganizationMemberError> {
            unimplemented!()
        }
        async fn update_permissions(
            &self,
            _org_id: Uuid,
            _user_id: Uuid,
            _permissions: Permissions,
        ) -> Result<OrganizationMember, OrganizationMemberError> {
            unimplemented!()
        }
        async fn remove(
            &self,
            _org_id: Uuid,
            _user_id: Uuid,
        ) -> Result<(), OrganizationMemberError> {
            unimplemented!()
        }
    }

    #[derive(Default)]
    struct MockOrgProfileRepo {
        profiles: Mutex<Vec<OrganizationProfile>>,
    }

    #[async_trait::async_trait]
    impl OrganizationProfileRepository for MockOrgProfileRepo {
        async fn upsert(
            &self,
            _org_id: Uuid,
            _bio: Option<&str>,
            _commission_status: CommissionStatus,
        ) -> Result<OrganizationProfile, OrganizationProfileError> {
            unimplemented!()
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

    #[derive(Default)]
    struct MockPreferencesRepo {
        prefs: Mutex<Vec<UserPreferences>>,
    }

    #[async_trait::async_trait]
    impl UserPreferencesRepository for MockPreferencesRepo {
        async fn get(&self, user_id: Uuid) -> Result<UserPreferences, UserPreferencesError> {
            Ok(self
                .prefs
                .lock()
                .await
                .iter()
                .find(|p| p.user_id == user_id)
                .cloned()
                .unwrap_or(UserPreferences {
                    user_id,
                    max_content_rating: ContentRating::Sfw,
                }))
        }
        async fn set_max_content_rating(
            &self,
            user_id: Uuid,
            rating: ContentRating,
        ) -> Result<UserPreferences, UserPreferencesError> {
            let mut prefs = self.prefs.lock().await;
            prefs.retain(|p| p.user_id != user_id);
            let updated = UserPreferences {
                user_id,
                max_content_rating: rating,
            };
            prefs.push(updated.clone());
            Ok(updated)
        }
    }

    // --- Helpers --------------------------------------------------------------

    fn test_user(id: Uuid) -> User {
        User {
            id,
            did: Some("did:plc:test".into()),
            handle: Some("test.bsky.social".into()),
            email: None,
            username: "test".into(),
            onboarding_completed_at: None,
        }
    }

    fn test_org(id: Uuid, user_id: Uuid) -> Organization {
        Organization {
            id,
            slug: "test".into(),
            display_name: None,
            is_personal: true,
            created_by: user_id,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        }
    }

    fn build_service(
        user_repo: MockUserRepo,
        org_repo: MockOrgRepo,
        org_profile_repo: MockOrgProfileRepo,
        member_repo: MockMemberRepo,
        prefs_repo: MockPreferencesRepo,
    ) -> UserService {
        UserService::new(
            Arc::new(user_repo),
            Arc::new(org_repo),
            Arc::new(org_profile_repo),
            Arc::new(member_repo),
            Arc::new(prefs_repo),
        )
    }

    // --- Tests ----------------------------------------------------------------

    #[tokio::test]
    async fn get_my_profile_returns_user_and_personal_org() {
        let user_id = Uuid::new_v4();
        let org_id = Uuid::new_v4();

        let user_repo = MockUserRepo {
            users: Mutex::new(vec![test_user(user_id)]),
        };
        let org_repo = MockOrgRepo {
            orgs: Mutex::new(vec![test_org(org_id, user_id)]),
        };

        let svc = build_service(
            user_repo,
            org_repo,
            MockOrgProfileRepo::default(),
            MockMemberRepo::default(),
            MockPreferencesRepo::default(),
        );

        let profile = svc.get_my_profile(user_id).await.unwrap();
        assert_eq!(profile.user.id, user_id);
        assert!(profile.personal_org.is_some());
        assert_eq!(profile.personal_org.unwrap().id, org_id);
    }

    #[tokio::test]
    async fn get_my_profile_for_missing_user_returns_not_found() {
        let svc = build_service(
            MockUserRepo::default(),
            MockOrgRepo::default(),
            MockOrgProfileRepo::default(),
            MockMemberRepo::default(),
            MockPreferencesRepo::default(),
        );

        let err = svc.get_my_profile(Uuid::new_v4()).await.unwrap_err();
        assert!(matches!(err, UserServiceError::NotFound));
    }

    #[tokio::test]
    async fn get_preferences_returns_sfw_default() {
        let svc = build_service(
            MockUserRepo::default(),
            MockOrgRepo::default(),
            MockOrgProfileRepo::default(),
            MockMemberRepo::default(),
            MockPreferencesRepo::default(),
        );

        let prefs = svc.get_preferences(Uuid::new_v4()).await.unwrap();
        assert_eq!(prefs.max_content_rating, ContentRating::Sfw);
    }

    #[tokio::test]
    async fn set_and_get_content_rating_round_trip() {
        let user_id = Uuid::new_v4();
        let svc = build_service(
            MockUserRepo::default(),
            MockOrgRepo::default(),
            MockOrgProfileRepo::default(),
            MockMemberRepo::default(),
            MockPreferencesRepo::default(),
        );

        svc.set_max_content_rating(user_id, ContentRating::Nsfw)
            .await
            .unwrap();

        let prefs = svc.get_preferences(user_id).await.unwrap();
        assert_eq!(prefs.max_content_rating, ContentRating::Nsfw);
    }
}
