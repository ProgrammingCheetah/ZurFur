use std::num::NonZeroUsize;
use std::sync::Arc;

use api::{AppState, router};
use application::auth::login::OAuthConfig;
use application::auth::service::{AuthService, create_default_oauth_storage};
use persistence::oauth_state_store_memory::InMemoryOAuthStateStore;
use shared::JwtConfig;
use tokio::net::TcpListener;

#[tokio::main]
async fn main() {
    // Database
    let db_config = persistence::Config::from_env().expect("DATABASE_URL must be set");
    let pool = persistence::connect(&db_config)
        .await
        .expect("Failed to connect to database");

    // JWT
    let jwt_config = JwtConfig::from_env().expect("JWT_SECRET must be set");

    // OAuth
    let oauth_config = OAuthConfig {
        client_id: std::env::var("OAUTH_CLIENT_ID").expect("OAUTH_CLIENT_ID must be set"),
        redirect_uri: std::env::var("OAUTH_REDIRECT_URI").expect("OAUTH_REDIRECT_URI must be set"),
        private_signing_key_data: load_signing_key(),
    };

    // Repositories
    let user_repo = persistence::SqlxUserRepository::from_pool(pool.clone());
    let session_repo = persistence::SqlxAtprotoSessionRepository::from_pool(pool.clone());
    let refresh_repo = persistence::SqlxRefreshTokenRepository::from_pool(pool);

    // Pluggable storage (swap these to Redis-backed implementations for production)
    let oauth_storage = create_default_oauth_storage(NonZeroUsize::new(1000).unwrap());
    let state_store = Arc::new(InMemoryOAuthStateStore::new());

    // Wire up
    let auth_service = AuthService::new(
        oauth_config,
        jwt_config,
        oauth_storage,
        state_store,
        user_repo,
        session_repo,
        refresh_repo,
    );

    let state = AppState {
        auth: auth_service,
    };

    let app = router(state);
    let listener = TcpListener::bind("0.0.0.0:3000").await.unwrap();
    println!("Zurfur API listening on 0.0.0.0:3000");
    axum::serve(listener, app).await.unwrap();
}

/// Load the P256 private key from the OAUTH_PRIVATE_KEY environment variable.
/// The key should be base64-encoded. Generate one with:
/// `atproto_identity::key::generate_key(KeyType::P256Private)`
fn load_signing_key() -> atproto_identity::key::KeyData {
    use base64::Engine;

    let key_b64 =
        std::env::var("OAUTH_PRIVATE_KEY").expect("OAUTH_PRIVATE_KEY must be set (base64-encoded P256 private key)");

    let bytes = base64::engine::general_purpose::STANDARD
        .decode(&key_b64)
        .expect("OAUTH_PRIVATE_KEY is not valid base64");

    atproto_identity::key::KeyData::new(atproto_identity::key::KeyType::P256Private, bytes)
}
