use application::auth::service::AuthService;
use application::feed::service::FeedService;
use application::tag::service::TagService;
use application::onboarding::service::OnboardingService;
use application::organization::service::OrganizationService;
use application::user::service::UserService;
use atproto_oauth::storage_lru::LruOAuthRequestStorage;
use std::sync::Arc;

/// ARCHITECTURE DECISIONS:
///   Services are stored directly (not behind Arc<dyn Trait>). They are not
///   swappable at the AppState level — if testing needs different behavior,
///   mock the repos, not the services.
pub struct AppState {
    pub auth_service: AuthService<LruOAuthRequestStorage>,
    pub user_service: UserService,
    pub org_service: OrganizationService,
    pub onboarding_service: OnboardingService,
    pub feed_service: FeedService,
    pub tag_service: TagService,
}

/// State type for Axum: shared across handlers via Arc.
pub type SharedState = Arc<AppState>;
