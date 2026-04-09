use application::auth::service::AuthService;
use application::organization::service::OrganizationService;
use application::user::service::UserService;
use atproto_oauth::storage_lru::LruOAuthRequestStorage;
use std::sync::Arc;

/// ARCHITECTURE DECISIONS:
///   UserService and OrganizationService are stored directly (not behind
///   Arc<dyn Trait>). They are not swappable at the AppState level — if
///   testing needs different behavior, mock the repos, not the services.
pub struct AppState {
    pub auth: AuthService<LruOAuthRequestStorage>,
    pub user: UserService,
    pub org: OrganizationService,
}

/// State type for Axum: shared across handlers via Arc.
pub type SharedState = Arc<AppState>;
