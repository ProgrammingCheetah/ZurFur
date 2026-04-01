pub mod middleware;
mod routes;
pub mod state;

pub use state::{AppState, SharedState};

use tower_http::cors::{Any, CorsLayer};

pub fn router(state: AppState) -> axum::Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    routes::router()
        .with_state(std::sync::Arc::new(state))
        .layer(cors)
}
