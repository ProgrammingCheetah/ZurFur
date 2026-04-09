//! Test AppState construction and JWT helpers for integration tests.

use std::num::NonZeroUsize;
use std::sync::Arc;

use application::auth::login::OAuthConfig;
use application::auth::service::{AuthService, create_default_oauth_storage};
use application::organization::service::OrganizationService;
use application::user::service::UserService;
use domain::organization::OrganizationRepository;
use domain::organization_member::OrganizationMemberRepository;
use domain::organization_profile::OrganizationProfileRepository;
use domain::user::UserRepository;
use domain::user_preferences::UserPreferencesRepository;
use shared::JwtConfig;
use uuid::Uuid;

use super::mock_auth::{MockRefreshRepo, MockSessionRepo, MockStateStore};
use super::mock_organizations::{MockMemberRepo, MockOrgProfileRepo, MockOrgRepo};
use super::mock_users::{MockPreferencesRepo, MockUserRepo};
use crate::AppState;

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
        auth_service,
        user_service,
        org_service,
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
