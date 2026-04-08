//! AuthService orchestrator: wires AT Protocol OAuth with user persistence and JWT sessions.

use std::num::NonZeroUsize;
use std::sync::Arc;

use chrono::{Duration, Utc};
use domain::atproto_session::{AtprotoSessionEntity, AtprotoSessionRepository};
use domain::oauth_state_store::OAuthStateStore;
use domain::refresh_token::RefreshTokenRepository;
use domain::user::UserRepository;
use shared::JwtConfig;
use uuid::Uuid;

use super::login::{
    LoginError, OAuthConfig, complete_oauth_login, default_oauth_storage, resolve_did,
    start_oauth_login,
};

use atproto_oauth::storage::OAuthRequestStorage;

/// Claims embedded in Zurfur platform JWTs (short-lived access tokens).
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct ZurfurClaims {
    /// User UUID
    pub sub: String,
    /// AT Protocol DID
    pub did: String,
    pub handle: Option<String>,
    /// Unix timestamp expiration
    pub exp: i64,
}

/// Returned by `POST /auth/start`.
pub struct StartLoginResponse {
    pub redirect_url: String,
    pub state: String,
}

/// Returned by `GET /auth/callback`.
pub struct CompleteLoginResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub user_id: Uuid,
    pub did: String,
    pub handle: Option<String>,
    pub is_new_user: bool,
}

/// Returned by `POST /auth/refresh`.
pub struct RefreshResponse {
    pub access_token: String,
    pub refresh_token: String,
}

/// The central authentication service. Holds all dependencies and orchestrates the OAuth flow,
/// user creation, session storage, and JWT issuance.
pub struct AuthService<S: OAuthRequestStorage> {
    pub oauth_config: OAuthConfig,
    pub jwt_config: JwtConfig,
    oauth_storage: Arc<S>,
    state_store: Arc<dyn OAuthStateStore>,
    user_repo: Arc<dyn UserRepository>,
    session_repo: Arc<dyn AtprotoSessionRepository>,
    refresh_repo: Arc<dyn RefreshTokenRepository>,
}

impl<S: OAuthRequestStorage> AuthService<S> {
    pub fn new(
        oauth_config: OAuthConfig,
        jwt_config: JwtConfig,
        oauth_storage: Arc<S>,
        state_store: Arc<dyn OAuthStateStore>,
        user_repo: Arc<dyn UserRepository>,
        session_repo: Arc<dyn AtprotoSessionRepository>,
        refresh_repo: Arc<dyn RefreshTokenRepository>,
    ) -> Self {
        Self {
            oauth_config,
            jwt_config,
            oauth_storage,
            state_store,
            user_repo,
            session_repo,
            refresh_repo,
        }
    }

    /// Step 1: Start the OAuth login flow. Returns a redirect URL for the user's browser.
    pub async fn start_login(
        &self,
        handle_or_did: &str,
    ) -> Result<StartLoginResponse, LoginError> {
        let did = resolve_did(handle_or_did).await?;

        let result =
            start_oauth_login(handle_or_did, &self.oauth_config, self.oauth_storage.as_ref())
                .await?;

        // Store DID keyed by state so the callback can look it up securely
        self.state_store
            .store_did(&result.state, &did)
            .await
            .map_err(|e| LoginError::InternalError(e.to_string()))?;

        Ok(StartLoginResponse {
            redirect_url: result.redirect_url,
            state: result.state,
        })
    }

    /// Step 2: Complete the OAuth flow after Bluesky redirects back with code + state.
    /// Finds or creates the user, stores AT Protocol tokens, issues JWT + refresh token.
    pub async fn complete_login(
        &self,
        code: &str,
        state: &str,
    ) -> Result<CompleteLoginResponse, LoginError> {
        // Retrieve DID from server-side storage (prevents client-side tampering)
        let did = self
            .state_store
            .take_did(state)
            .await
            .map_err(|e| LoginError::InternalError(e.to_string()))?
            .ok_or(LoginError::InvalidState)?;

        // Exchange code for AT Protocol tokens
        let session =
            complete_oauth_login(code, state, &did, &self.oauth_config, self.oauth_storage.as_ref())
                .await?;

        // Find or create user
        let (user, is_new) = match self
            .user_repo
            .find_by_did(&session.did)
            .await
            .map_err(|e| LoginError::InternalError(e.to_string()))?
        {
            Some(user) => (user, false),
            None => {
                let user = self
                    .user_repo
                    .create_from_atproto(
                        &session.did,
                        session.handle.as_deref(),
                        session.email.as_deref(),
                    )
                    .await
                    .map_err(|e| LoginError::InternalError(e.to_string()))?;
                (user, true)
            }
        };

        // Store AT Protocol tokens for later Bluesky API calls (sub-features 1.2-1.4)
        let atproto_session = AtprotoSessionEntity {
            id: Uuid::new_v4(),
            user_id: user.id,
            did: session.did.clone(),
            access_token: session.access_token,
            refresh_token: session.refresh_token,
            expires_at: Utc::now() + Duration::seconds(session.expires_in_secs as i64),
            pds_url: None,
        };
        self.session_repo
            .upsert(&atproto_session)
            .await
            .map_err(|e| LoginError::InternalError(e.to_string()))?;

        // Issue Zurfur access JWT
        let access_token = self.issue_access_jwt(&user.id, &session.did, &user.handle)?;

        // Generate and store refresh token
        let refresh_token = self.create_refresh_token(user.id).await?;

        Ok(CompleteLoginResponse {
            access_token,
            refresh_token,
            user_id: user.id,
            did: session.did,
            handle: user.handle,
            is_new_user: is_new,
        })
    }

    /// Rotate a refresh token: validate the old one, delete it, issue a new pair.
    pub async fn refresh_session(
        &self,
        raw_refresh_token: &str,
    ) -> Result<RefreshResponse, LoginError> {
        let token_hash = hash_token(raw_refresh_token);

        let stored = self
            .refresh_repo
            .find_by_hash(&token_hash)
            .await
            .map_err(|e| LoginError::InternalError(e.to_string()))?
            .ok_or(LoginError::InvalidState)?;

        // Check expiration
        if stored.expires_at < Utc::now() {
            // Clean up expired token
            let _ = self.refresh_repo.delete_by_hash(&token_hash).await;
            return Err(LoginError::InvalidState);
        }

        // Single-use rotation: delete the old token
        self.refresh_repo
            .delete_by_hash(&token_hash)
            .await
            .map_err(|e| LoginError::InternalError(e.to_string()))?;

        // Look up user to populate JWT claims
        let user = self
            .user_repo
            .find_by_id(stored.user_id)
            .await
            .map_err(|e| LoginError::InternalError(e.to_string()))?
            .ok_or(LoginError::UserNotFound)?;

        let did = user
            .did
            .as_deref()
            .ok_or_else(|| LoginError::InternalError("User has no DID".into()))?;

        let access_token = self.issue_access_jwt(&user.id, did, &user.handle)?;
        let refresh_token = self.create_refresh_token(user.id).await?;

        Ok(RefreshResponse {
            access_token,
            refresh_token,
        })
    }

    /// Logout: delete all refresh tokens and AT Protocol session for a user.
    pub async fn logout(&self, user_id: Uuid) -> Result<(), LoginError> {
        self.refresh_repo
            .delete_all_for_user(user_id)
            .await
            .map_err(|e| LoginError::InternalError(e.to_string()))?;

        self.session_repo
            .delete_by_user_id(user_id)
            .await
            .map_err(|e| LoginError::InternalError(e.to_string()))?;

        Ok(())
    }

    // --- Private helpers --------------------------------------------------------

    fn issue_access_jwt(
        &self,
        user_id: &Uuid,
        did: &str,
        handle: &Option<String>,
    ) -> Result<String, LoginError> {
        let claims = ZurfurClaims {
            sub: user_id.to_string(),
            did: did.to_string(),
            handle: handle.clone(),
            exp: (Utc::now() + Duration::seconds(self.jwt_config.access_expiry_secs as i64))
                .timestamp(),
        };
        shared::jwt::create(&claims, &self.jwt_config.secret)
            .map_err(|e| LoginError::InternalError(e.to_string()))
    }

    async fn create_refresh_token(&self, user_id: Uuid) -> Result<String, LoginError> {
        let raw_token = Uuid::new_v4().to_string();
        let token_hash = hash_token(&raw_token);
        let expires_at =
            Utc::now() + Duration::seconds(self.jwt_config.refresh_expiry_secs as i64);

        self.refresh_repo
            .create(user_id, &token_hash, expires_at)
            .await
            .map_err(|e| LoginError::InternalError(e.to_string()))?;

        Ok(raw_token)
    }
}

/// Default constructor for in-memory OAuth request storage (LRU).
pub fn create_default_oauth_storage(
    capacity: NonZeroUsize,
) -> Arc<atproto_oauth::storage_lru::LruOAuthRequestStorage> {
    Arc::new(default_oauth_storage(capacity))
}

/// SHA-256 hash a raw refresh token for storage.
fn hash_token(raw: &str) -> String {
    use sha2::Digest;
    let hash = sha2::Sha256::digest(raw.as_bytes());
    format!("{:x}", hash)
}
