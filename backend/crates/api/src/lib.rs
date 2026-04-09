pub mod middleware;
mod routes;
pub mod state;
#[cfg(test)]
mod tests;

pub use state::{AppState, SharedState};

use axum::http::{HeaderValue, Method, header};
use tower_http::cors::CorsLayer;

pub fn router(state: AppState) -> axum::Router {
    // Derive tunnel origin from OAUTH_CLIENT_ID so CORS works through cloudflared.
    // The client_id is a URL like "https://tunnel.trycloudflare.com/client-metadata.json"
    // — we extract the origin (scheme + host).
    let mut origins: Vec<HeaderValue> = vec![
        "http://localhost:5173".parse().unwrap(),
        "https://auth.zurfur.app".parse().unwrap(),
        "https://zurfur.app".parse().unwrap(),
    ];

    // OAUTH_CLIENT_ID is validated on startup in main.rs (panics if missing).
    // This is a best-effort CORS origin addition — in tests it's simply absent.
    if let Ok(client_id) = std::env::var("OAUTH_CLIENT_ID") {
        // Extract origin (scheme + host) from URL like "https://host.com/path"
        if let Some(rest) = client_id.strip_prefix("https://") {
            let host = rest.split('/').next().unwrap_or_default();
            if let Ok(hv) = format!("https://{host}").parse::<HeaderValue>() {
                origins.push(hv);
            }
        }
    }

    let cors = CorsLayer::new()
        .allow_origin(origins)
        .allow_methods([Method::GET, Method::POST, Method::PUT, Method::DELETE])
        .allow_headers([header::CONTENT_TYPE, header::AUTHORIZATION]);

    routes::router()
        .with_state(std::sync::Arc::new(state))
        .layer(cors)
}
