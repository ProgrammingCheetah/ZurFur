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
    // Structured logging: configure via RUST_LOG env var (e.g. RUST_LOG=info,api=debug)
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .init();

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
        plc_hostname: std::env::var("PLC_HOSTNAME").unwrap_or_else(|_| "plc.directory".into()),
    };

    // Repositories
    let user_repo = persistence::SqlxUserRepository::from_pool(pool.clone());
    let session_repo = persistence::SqlxAtprotoSessionRepository::from_pool(pool.clone());
    let refresh_repo = persistence::SqlxRefreshTokenRepository::from_pool(pool.clone());
    let org_repo = persistence::SqlxOrganizationRepository::from_pool(pool.clone());
    let member_repo = persistence::SqlxOrganizationMemberRepository::from_pool(pool.clone());
    let preferences_repo = persistence::SqlxUserPreferencesRepository::from_pool(pool.clone());
    let feed_repo = persistence::SqlxFeedRepository::from_pool(pool.clone());
    let entity_feed_repo = persistence::SqlxEntityFeedRepository::from_pool(pool.clone());
    let feed_item_repo = persistence::SqlxFeedItemRepository::from_pool(pool.clone());
    let feed_element_repo = persistence::SqlxFeedElementRepository::from_pool(pool.clone());
    let tag_repo = persistence::SqlxTagRepository::from_pool(pool.clone());
    let entity_tag_repo = persistence::SqlxEntityTagRepository::from_pool(pool);

    // Pluggable storage (swap these to Redis-backed implementations for production)
    let oauth_storage = create_default_oauth_storage(NonZeroUsize::new(1000).unwrap());
    let state_store = Arc::new(InMemoryOAuthStateStore::new());

    // Wire up services
    let auth_service = AuthService::new(
        oauth_config,
        jwt_config,
        oauth_storage,
        state_store,
        user_repo.clone(),
        session_repo,
        refresh_repo,
        org_repo.clone(),
        member_repo.clone(),
    );

    let user_service = application::user::service::UserService::new(
        user_repo.clone(),
        org_repo.clone(),
        member_repo.clone(),
        preferences_repo,
    );

    let org_service = application::organization::service::OrganizationService::new(
        org_repo.clone(),
        member_repo.clone(),
    );

    let onboarding_service = application::onboarding::service::OnboardingService::new(
        user_repo,
        org_repo,
        feed_repo.clone(),
        entity_feed_repo.clone(),
    );

    let feed_service = application::feed::service::FeedService::new(
        feed_repo,
        entity_feed_repo,
        feed_item_repo,
        feed_element_repo,
        member_repo,
    );

    let tag_service = application::tag::service::TagService::new(
        tag_repo,
        entity_tag_repo,
    );

    let state = AppState {
        auth_service,
        user_service,
        org_service,
        onboarding_service,
        feed_service,
        tag_service,
    };

    let app = router(state);
    let bind_addr = std::env::var("BIND_ADDR").unwrap_or_else(|_| "0.0.0.0:3000".into());
    let listener = TcpListener::bind(&bind_addr).await
        .expect("Failed to bind to address");
    tracing::info!("Zurfur API listening on {bind_addr}");
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

    if bytes.len() != 32 {
        panic!(
            "OAUTH_PRIVATE_KEY must decode to exactly 32 bytes (P-256 private scalar), got {}",
            bytes.len()
        );
    }

    atproto_identity::key::KeyData::new(atproto_identity::key::KeyType::P256Private, bytes)
}
