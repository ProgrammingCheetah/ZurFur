mod routes;
pub mod state;

pub use state::{AppState, AuthService, SharedState};

pub fn router(state: AppState) -> axum::Router {
    routes::router().with_state(std::sync::Arc::new(state))
}
