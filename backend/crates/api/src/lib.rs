pub mod middleware;
mod routes;
pub mod state;
#[cfg(test)]
mod tests;

pub use state::{AppState, SharedState};

use axum::http::{HeaderValue, Method, header};
use tower_http::cors::CorsLayer;

pub fn router(state: AppState) -> axum::Router {
    // Hardcoded for now; move to env-based config when multi-environment deployment is needed.
    let origins = [
        "http://localhost:5173".parse::<HeaderValue>().unwrap(),
        "https://auth.zurfur.app".parse::<HeaderValue>().unwrap(),
        "https://zurfur.app".parse::<HeaderValue>().unwrap(),
    ];

    let cors = CorsLayer::new()
        .allow_origin(origins)
        .allow_methods([Method::GET, Method::POST, Method::PUT, Method::DELETE])
        .allow_headers([header::CONTENT_TYPE, header::AUTHORIZATION]);

    routes::router()
        .with_state(std::sync::Arc::new(state))
        .layer(cors)
}
