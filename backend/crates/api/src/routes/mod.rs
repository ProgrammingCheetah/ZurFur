mod auth;
mod organizations;
mod users;

use axum::{Router, routing::get};

use crate::state::SharedState;

pub fn router() -> Router<SharedState> {
    Router::new()
        .route("/", get(|| async { "Hello, World!" }))
        .route("/client-metadata.json", get(client_metadata))
        .nest("/users", users::router())
        .nest("/auth", auth::router())
        .nest("/organizations", organizations::router())
}

/// Serve OAuth client metadata so Bluesky's auth server can verify our identity.
/// This is fetched by Bluesky when we send a PAR request — the client_id URL
/// must resolve to this JSON document.
async fn client_metadata(
    axum::extract::State(state): axum::extract::State<SharedState>,
) -> axum::Json<serde_json::Value> {
    axum::Json(serde_json::json!({
        "client_id": state.auth.client_id(),
        "client_name": "Zurfur",
        "application_type": "web",
        "dpop_bound_access_tokens": true,
        "grant_types": ["authorization_code", "refresh_token"],
        "redirect_uris": [state.auth.redirect_uri()],
        "response_types": ["code"],
        "scope": "atproto transition:generic",
        "token_endpoint_auth_method": "private_key_jwt",
        "token_endpoint_auth_signing_alg": "ES256",
        "jwks": {
            "keys": [state.auth.public_jwk()]
        }
    }))
}
