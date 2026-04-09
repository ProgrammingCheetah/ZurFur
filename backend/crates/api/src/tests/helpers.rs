//! Test helpers: mock repositories and test AppState construction.

use std::collections::HashMap;
use std::num::NonZeroUsize;
use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use domain::atproto_session::{AtprotoSessionEntity, AtprotoSessionRepository};
use domain::content_rating::ContentRating;
use domain::oauth_state_store::{OAuthStateData, OAuthStateError, OAuthStateStore};
use domain::organization::{Organization, OrganizationError, OrganizationRepository};
use domain::organization_member::{
    OrganizationMember, OrganizationMemberError, OrganizationMemberRepository, Permissions,
};
use domain::organization_profile::{
    CommissionStatus, OrganizationProfile, OrganizationProfileError, OrganizationProfileRepository,
};
use domain::refresh_token::{RefreshTokenEntity, RefreshTokenRepository};
use domain::user::{User, UserError, UserRepository};
use domain::user_preferences::{UserPreferences, UserPreferencesError, UserPreferencesRepository};
use tokio::sync::Mutex;
use uuid::Uuid;

use application::auth::login::OAuthConfig;
use application::auth::service::{AuthService, create_default_oauth_storage};
use application::organization::service::OrganizationService;
use application::user::service::UserService;
use shared::JwtConfig;

use crate::AppState;

// --- Mock UserRepository -----------------------------------------------------

#[derive(Default)]
pub struct MockUserRepo {
    pub users: Mutex<Vec<User>>,
}

#[async_trait]
impl UserRepository for MockUserRepo {
    async fn find_by_email(&self, _email: &str) -> Result<Option<User>, UserError> {
        Ok(None)
    }
    async fn find_by_id(&self, id: Uuid) -> Result<Option<User>, UserError> {
        Ok(self.users.lock().await.iter().find(|u| u.id == id).cloned())
    }
    async fn find_by_did(&self, did: &str) -> Result<Option<User>, UserError> {
        Ok(self
            .users
            .lock()
            .await
            .iter()
            .find(|u| u.did.as_deref() == Some(did))
            .cloned())
    }
    async fn create_from_atproto(
        &self,
        did: &str,
        handle: Option<&str>,
        email: Option<&str>,
    ) -> Result<User, UserError> {
        let user = User {
            id: Uuid::new_v4(),
            did: Some(did.to_string()),
            handle: handle.map(String::from),
            email: email.map(String::from),
            username: handle.unwrap_or(did).to_string(),
        };
        self.users.lock().await.push(user.clone());
        Ok(user)
    }
    async fn update_handle(&self, _user_id: Uuid, _handle: &str) -> Result<(), UserError> {
        Ok(())
    }
}

// --- Mock AtprotoSessionRepository -------------------------------------------

#[derive(Default)]
pub struct MockSessionRepo {
    sessions: Mutex<Vec<AtprotoSessionEntity>>,
}

#[async_trait]
impl AtprotoSessionRepository for MockSessionRepo {
    async fn upsert(&self, session: &AtprotoSessionEntity) -> Result<(), UserError> {
        let mut sessions = self.sessions.lock().await;
        sessions.retain(|s| s.user_id != session.user_id);
        sessions.push(session.clone());
        Ok(())
    }
    async fn find_by_user_id(
        &self,
        user_id: Uuid,
    ) -> Result<Option<AtprotoSessionEntity>, UserError> {
        Ok(self
            .sessions
            .lock()
            .await
            .iter()
            .find(|s| s.user_id == user_id)
            .cloned())
    }
    async fn find_by_did(&self, did: &str) -> Result<Option<AtprotoSessionEntity>, UserError> {
        Ok(self
            .sessions
            .lock()
            .await
            .iter()
            .find(|s| s.did == did)
            .cloned())
    }
    async fn delete_by_user_id(&self, user_id: Uuid) -> Result<(), UserError> {
        self.sessions.lock().await.retain(|s| s.user_id != user_id);
        Ok(())
    }
}

// --- Mock RefreshTokenRepository ---------------------------------------------

#[derive(Default)]
pub struct MockRefreshRepo {
    pub tokens: Mutex<Vec<RefreshTokenEntity>>,
}

#[async_trait]
impl RefreshTokenRepository for MockRefreshRepo {
    async fn create(
        &self,
        user_id: Uuid,
        token_hash: &str,
        expires_at: DateTime<Utc>,
    ) -> Result<(), UserError> {
        self.tokens.lock().await.push(RefreshTokenEntity {
            id: Uuid::new_v4(),
            user_id,
            token_hash: token_hash.to_string(),
            expires_at,
            created_at: Utc::now(),
        });
        Ok(())
    }
    async fn find_by_hash(
        &self,
        token_hash: &str,
    ) -> Result<Option<RefreshTokenEntity>, UserError> {
        Ok(self
            .tokens
            .lock()
            .await
            .iter()
            .find(|t| t.token_hash == token_hash)
            .cloned())
    }
    async fn take_by_hash(
        &self,
        token_hash: &str,
    ) -> Result<Option<RefreshTokenEntity>, UserError> {
        let mut tokens = self.tokens.lock().await;
        if let Some(pos) = tokens.iter().position(|t| t.token_hash == token_hash) {
            Ok(Some(tokens.remove(pos)))
        } else {
            Ok(None)
        }
    }
    async fn delete_all_for_user(&self, user_id: Uuid) -> Result<(), UserError> {
        self.tokens.lock().await.retain(|t| t.user_id != user_id);
        Ok(())
    }
}

// --- Mock OAuthStateStore ----------------------------------------------------

#[derive(Default)]
pub struct MockStateStore {
    inner: Mutex<HashMap<String, OAuthStateData>>,
}

#[async_trait]
impl OAuthStateStore for MockStateStore {
    async fn store(&self, state: &str, data: OAuthStateData) -> Result<(), OAuthStateError> {
        self.inner.lock().await.insert(state.to_string(), data);
        Ok(())
    }
    async fn take(&self, state: &str) -> Result<Option<OAuthStateData>, OAuthStateError> {
        Ok(self.inner.lock().await.remove(state))
    }
}

// --- Mock OrganizationRepository ---------------------------------------------

#[derive(Default)]
pub struct MockOrgRepo {
    pub orgs: Mutex<Vec<Organization>>,
}

#[async_trait]
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
            created_at: Utc::now(),
            updated_at: Utc::now(),
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

// --- Mock OrganizationMemberRepository ---------------------------------------

#[derive(Default)]
pub struct MockMemberRepo {
    pub members: Mutex<Vec<OrganizationMember>>,
}

#[async_trait]
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
            joined_at: Utc::now(),
            updated_at: Utc::now(),
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
    async fn remove(&self, org_id: Uuid, user_id: Uuid) -> Result<(), OrganizationMemberError> {
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

// --- Mock OrganizationProfileRepository --------------------------------------

#[derive(Default)]
pub struct MockOrgProfileRepo {
    pub profiles: Mutex<Vec<OrganizationProfile>>,
}

#[async_trait]
impl OrganizationProfileRepository for MockOrgProfileRepo {
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
            created_at: Utc::now(),
            updated_at: Utc::now(),
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

// --- Mock UserPreferencesRepository ------------------------------------------

#[derive(Default)]
pub struct MockPreferencesRepo {
    pub prefs: Mutex<Vec<UserPreferences>>,
}

#[async_trait]
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

// --- Test AppState builder ---------------------------------------------------

pub fn test_jwt_config() -> JwtConfig {
    JwtConfig {
        secret: b"test-secret-for-api-tests".to_vec(),
        access_expiry_secs: 900,
        refresh_expiry_secs: 2592000,
    }
}

pub fn test_app_state() -> AppState {
    let oauth_config = OAuthConfig {
        redirect_uri: "http://localhost:5173/callback".into(),
        client_id: "https://zurfur.app".into(),
        private_signing_key_data: atproto_identity::key::KeyData::new(
            atproto_identity::key::KeyType::P256Private,
            vec![0u8; 32],
        ),
    };

    let user_repo: Arc<dyn UserRepository> = Arc::new(MockUserRepo::default());
    let session_repo = Arc::new(MockSessionRepo::default());
    let refresh_repo = Arc::new(MockRefreshRepo::default());
    let state_store = Arc::new(MockStateStore::default());
    let oauth_storage = create_default_oauth_storage(NonZeroUsize::new(10).unwrap());

    let org_repo: Arc<dyn OrganizationRepository> = Arc::new(MockOrgRepo::default());
    let member_repo: Arc<dyn OrganizationMemberRepository> = Arc::new(MockMemberRepo::default());
    let org_profile_repo: Arc<dyn OrganizationProfileRepository> =
        Arc::new(MockOrgProfileRepo::default());
    let preferences_repo: Arc<dyn UserPreferencesRepository> =
        Arc::new(MockPreferencesRepo::default());

    let auth_service = AuthService::new(
        oauth_config,
        test_jwt_config(),
        oauth_storage,
        state_store,
        user_repo.clone(),
        session_repo,
        refresh_repo,
        org_repo.clone(),
        member_repo.clone(),
    );

    let user_service = UserService::new(
        user_repo,
        org_repo.clone(),
        org_profile_repo.clone(),
        member_repo.clone(),
        preferences_repo,
    );

    let org_service = OrganizationService::new(org_repo, member_repo, org_profile_repo);

    AppState {
        auth: auth_service,
        user: user_service,
        org: org_service,
    }
}

/// Issue a valid JWT for testing protected routes.
pub fn issue_test_jwt(user_id: &Uuid, did: &str, handle: Option<&str>) -> String {
    use application::auth::service::ZurfurClaims;
    let config = test_jwt_config();
    let claims = ZurfurClaims {
        sub: user_id.to_string(),
        did: did.to_string(),
        handle: handle.map(String::from),
        exp: (chrono::Utc::now() + chrono::Duration::hours(1)).timestamp(),
    };
    shared::jwt::create(&claims, &config.secret).unwrap()
}
