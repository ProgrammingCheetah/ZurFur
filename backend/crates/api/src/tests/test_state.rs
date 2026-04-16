//! Test AppState construction and JWT helpers for integration tests.

use std::num::NonZeroUsize;
use std::sync::Arc;

use application::auth::login::OAuthConfig;
use application::auth::service::{AuthService, create_default_oauth_storage};
use application::feed::service::FeedService;
use application::onboarding::service::OnboardingService;
use application::organization::service::OrganizationService;
use application::user::service::UserService;
use domain::entity_feed::EntityFeedRepository;
use domain::feed::FeedRepository;
use domain::feed_element::FeedElementRepository;
use domain::feed_item::FeedItemRepository;
use domain::organization::OrganizationRepository;
use domain::organization_member::OrganizationMemberRepository;
use domain::user::UserRepository;
use domain::user_preferences::UserPreferencesRepository;
use shared::JwtConfig;
use uuid::Uuid;

use super::mock_auth::{MockRefreshRepo, MockSessionRepo, MockStateStore};
use application::tag::service::TagService;
use domain::entity_tag::EntityTagRepository;
use domain::tag::TagRepository;

use super::mock_feeds::{MockEntityFeedRepo, MockFeedElementRepo, MockFeedItemRepo, MockFeedRepo};
use super::mock_organizations::{MockMemberRepo, MockOrgRepo};
use super::mock_tags::{MockEntityTagRepo, MockTagRepo};
use super::mock_users::{MockPreferencesRepo, MockUserRepo};
use crate::AppState;

pub fn test_jwt_config() -> JwtConfig {
    JwtConfig {
        secret: b"test-secret-for-api-tests".to_vec(),
        access_expiry_secs: 900,
        refresh_expiry_secs: 2592000,
    }
}

fn build_app_state(
    user_repo: Arc<dyn UserRepository>,
    org_repo: Arc<dyn OrganizationRepository>,
    member_repo: Arc<dyn OrganizationMemberRepository>,
    preferences_repo: Arc<dyn UserPreferencesRepository>,
    feed_repo: Arc<dyn FeedRepository>,
    entity_feed_repo: Arc<dyn EntityFeedRepository>,
    feed_item_repo: Arc<dyn FeedItemRepository>,
    feed_element_repo: Arc<dyn FeedElementRepository>,
    tag_repo: Arc<dyn TagRepository>,
    entity_tag_repo: Arc<dyn EntityTagRepository>,
) -> AppState {
    let oauth_config = OAuthConfig {
        redirect_uri: "http://localhost:5173/callback".into(),
        client_id: "https://zurfur.app".into(),
        private_signing_key_data: atproto_identity::key::KeyData::new(
            atproto_identity::key::KeyType::P256Private,
            vec![0u8; 32],
        ),
        plc_hostname: "plc.directory".into(),
    };

    let state_store = Arc::new(MockStateStore::default());
    let session_repo = Arc::new(MockSessionRepo::default());
    let refresh_repo = Arc::new(MockRefreshRepo::default());
    let oauth_storage = create_default_oauth_storage(NonZeroUsize::new(10).unwrap());

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
        user_repo.clone(),
        org_repo.clone(),
        member_repo.clone(),
        preferences_repo,
    );

    let org_service = OrganizationService::new(
        org_repo.clone(),
        member_repo.clone(),
    );

    let onboarding_service = OnboardingService::new(
        user_repo,
        org_repo,
        feed_repo.clone(),
        entity_feed_repo.clone(),
    );

    let feed_service = FeedService::new(
        feed_repo,
        entity_feed_repo,
        feed_item_repo,
        feed_element_repo,
        member_repo,
    );

    let tag_service = TagService::new(tag_repo, entity_tag_repo);

    AppState {
        auth_service,
        user_service,
        org_service,
        onboarding_service,
        feed_service,
        tag_service,
    }
}

pub fn test_app_state() -> AppState {
    let shared_members = Arc::new(tokio::sync::Mutex::new(Vec::new()));
    let shared_entity_feeds = Arc::new(tokio::sync::Mutex::new(Vec::new()));

    let user_repo: Arc<dyn UserRepository> = Arc::new(MockUserRepo::default());
    let org_repo: Arc<dyn OrganizationRepository> = Arc::new(MockOrgRepo {
        shared_members: shared_members.clone(),
        ..MockOrgRepo::default()
    });
    let member_repo: Arc<dyn OrganizationMemberRepository> = Arc::new(MockMemberRepo {
        members: shared_members,
    });
    let preferences_repo: Arc<dyn UserPreferencesRepository> =
        Arc::new(MockPreferencesRepo::default());

    let feed_repo: Arc<dyn FeedRepository> =
        Arc::new(MockFeedRepo::with_shared_entity_feeds(shared_entity_feeds.clone()));
    let entity_feed_repo: Arc<dyn EntityFeedRepository> =
        Arc::new(MockEntityFeedRepo::with_shared(shared_entity_feeds));
    let feed_item_repo: Arc<dyn FeedItemRepository> = Arc::new(MockFeedItemRepo::default());
    let feed_element_repo: Arc<dyn FeedElementRepository> =
        Arc::new(MockFeedElementRepo::default());

    let shared_entity_tags = Arc::new(tokio::sync::Mutex::new(Vec::new()));
    let tag_repo: Arc<dyn TagRepository> = Arc::new(MockTagRepo::new(shared_entity_tags.clone()));
    let entity_tag_repo: Arc<dyn EntityTagRepository> =
        Arc::new(MockEntityTagRepo::new(shared_entity_tags));

    build_app_state(
        user_repo,
        org_repo,
        member_repo,
        preferences_repo,
        feed_repo,
        entity_feed_repo,
        feed_item_repo,
        feed_element_repo,
        tag_repo,
        entity_tag_repo,
    )
}

/// Create an AppState pre-populated with a user and their personal org.
/// Returns (AppState, user_id) so tests can issue JWTs for the pre-existing user.
pub fn test_app_state_with_user() -> (AppState, Uuid) {
    use chrono::Utc;
    use domain::organization::Organization;
    use domain::organization_member::{OrganizationMember, Permissions, Role};
    use domain::user::User;

    let user_id = Uuid::new_v4();
    let org_id = Uuid::new_v4();

    let user = User {
        id: user_id,
        did: Some("did:plc:testuser".into()),
        handle: Some("testuser.bsky.social".into()),
        email: Some("test@example.com".into()),
        username: "testuser".into(),
        onboarding_completed_at: None,
    };

    let personal_org = Organization {
        id: org_id,
        slug: "testuser".into(),
        display_name: None,
        is_personal: true,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };

    let owner_member = OrganizationMember {
        id: Uuid::new_v4(),
        org_id,
        user_id,
        role: Role::Owner,
        title: None,
        permissions: Permissions::new(Permissions::ALL),
        joined_at: Utc::now(),
        updated_at: Utc::now(),
    };

    let shared_members = Arc::new(tokio::sync::Mutex::new(vec![owner_member]));
    let shared_entity_feeds = Arc::new(tokio::sync::Mutex::new(Vec::new()));

    let user_repo: Arc<dyn UserRepository> = Arc::new(MockUserRepo {
        users: tokio::sync::Mutex::new(vec![user]),
    });
    let org_repo: Arc<dyn OrganizationRepository> = Arc::new(MockOrgRepo {
        orgs: tokio::sync::Mutex::new(vec![personal_org]),
        personal_org_owners: tokio::sync::Mutex::new(vec![(user_id, org_id)]),
        shared_members: shared_members.clone(),
    });
    let member_repo: Arc<dyn OrganizationMemberRepository> = Arc::new(MockMemberRepo {
        members: shared_members,
    });
    let preferences_repo: Arc<dyn UserPreferencesRepository> =
        Arc::new(MockPreferencesRepo::default());

    let feed_repo: Arc<dyn FeedRepository> =
        Arc::new(MockFeedRepo::with_shared_entity_feeds(shared_entity_feeds.clone()));
    let entity_feed_repo: Arc<dyn EntityFeedRepository> =
        Arc::new(MockEntityFeedRepo::with_shared(shared_entity_feeds));
    let feed_item_repo: Arc<dyn FeedItemRepository> = Arc::new(MockFeedItemRepo::default());
    let feed_element_repo: Arc<dyn FeedElementRepository> =
        Arc::new(MockFeedElementRepo::default());

    let shared_entity_tags = Arc::new(tokio::sync::Mutex::new(Vec::new()));
    let tag_repo: Arc<dyn TagRepository> = Arc::new(MockTagRepo::new(shared_entity_tags.clone()));
    let entity_tag_repo: Arc<dyn EntityTagRepository> =
        Arc::new(MockEntityTagRepo::new(shared_entity_tags));

    let state = build_app_state(
        user_repo,
        org_repo,
        member_repo,
        preferences_repo,
        feed_repo,
        entity_feed_repo,
        feed_item_repo,
        feed_element_repo,
        tag_repo,
        entity_tag_repo,
    );

    (state, user_id)
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
