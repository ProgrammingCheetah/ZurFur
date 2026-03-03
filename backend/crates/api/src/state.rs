use application::auth::login::LoginEmailHandler;
use std::sync::Arc;

pub struct AuthService {
    pub login: LoginEmailHandler,
}

pub struct AppState {
    pub auth: AuthService,
}

/// State type for Axum: shared across handlers via Arc (no Clone on AppState required).
pub type SharedState = Arc<AppState>;
