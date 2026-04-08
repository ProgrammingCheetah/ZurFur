use application::auth::service::AuthService;
use atproto_oauth::storage_lru::LruOAuthRequestStorage;
use std::sync::Arc;

pub struct AppState {
    pub auth: AuthService<LruOAuthRequestStorage>,
}

/// State type for Axum: shared across handlers via Arc.
pub type SharedState = Arc<AppState>;
