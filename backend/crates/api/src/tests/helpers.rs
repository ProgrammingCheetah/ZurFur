//! Test helpers: mock repositories and test AppState construction.

use std::collections::HashMap;
use std::num::NonZeroUsize;
use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use domain::atproto_session::{AtprotoSessionEntity, AtprotoSessionRepository};
use domain::oauth_state_store::{OAuthStateData, OAuthStateError, OAuthStateStore};
use domain::refresh_token::{RefreshTokenEntity, RefreshTokenRepository};
use domain::user::{User, UserError, UserRepository};
use tokio::sync::Mutex;
use uuid::Uuid;

use application::auth::login::OAuthConfig;
use application::auth::service::{AuthService, create_default_oauth_storage};
use shared::JwtConfig;

use crate::AppState;

// --- Mock UserRepository -----------------------------------------------------

#[derive(Default)]
pub struct MockUserRepo {
    pub users: Mutex<Vec<User>>,
}

#[async_trait]
impl UserRepository for MockUserRepo {
    async fn find_by_email(&self, _email: &str) -> Result<Option<User>, UserError> {
        Ok(None)
    }
    async fn find_by_id(&self, id: Uuid) -> Result<Option<User>, UserError> {
        Ok(self.users.lock().await.iter().find(|u| u.id == id).cloned())
    }
    async fn find_by_did(&self, did: &str) -> Result<Option<User>, UserError> {
        Ok(self
            .users
            .lock()
            .await
            .iter()
            .find(|u| u.did.as_deref() == Some(did))
            .cloned())
    }
    async fn create_from_atproto(
        &self,
        did: &str,
        handle: Option<&str>,
        email: Option<&str>,
    ) -> Result<User, UserError> {
        let user = User {
            id: Uuid::new_v4(),
            did: Some(did.to_string()),
            handle: handle.map(String::from),
            email: email.map(String::from),
            username: handle.unwrap_or(did).to_string(),
        };
        self.users.lock().await.push(user.clone());
        Ok(user)
    }
    async fn update_handle(&self, _user_id: Uuid, _handle: &str) -> Result<(), UserError> {
        Ok(())
    }
}

// --- Mock AtprotoSessionRepository -------------------------------------------

#[derive(Default)]
pub struct MockSessionRepo {
    sessions: Mutex<Vec<AtprotoSessionEntity>>,
}

#[async_trait]
impl AtprotoSessionRepository for MockSessionRepo {
    async fn upsert(&self, session: &AtprotoSessionEntity) -> Result<(), UserError> {
        let mut sessions = self.sessions.lock().await;
        sessions.retain(|s| s.user_id != session.user_id);
        sessions.push(session.clone());
        Ok(())
    }
    async fn find_by_user_id(
        &self,
        user_id: Uuid,
    ) -> Result<Option<AtprotoSessionEntity>, UserError> {
        Ok(self
            .sessions
            .lock()
            .await
            .iter()
            .find(|s| s.user_id == user_id)
            .cloned())
    }
    async fn find_by_did(&self, did: &str) -> Result<Option<AtprotoSessionEntity>, UserError> {
        Ok(self
            .sessions
            .lock()
            .await
            .iter()
            .find(|s| s.did == did)
            .cloned())
    }
    async fn delete_by_user_id(&self, user_id: Uuid) -> Result<(), UserError> {
        self.sessions.lock().await.retain(|s| s.user_id != user_id);
        Ok(())
    }
}

// --- Mock RefreshTokenRepository ---------------------------------------------

#[derive(Default)]
pub struct MockRefreshRepo {
    pub tokens: Mutex<Vec<RefreshTokenEntity>>,
}

#[async_trait]
impl RefreshTokenRepository for MockRefreshRepo {
    async fn create(
        &self,
        user_id: Uuid,
        token_hash: &str,
        expires_at: DateTime<Utc>,
    ) -> Result<(), UserError> {
        self.tokens.lock().await.push(RefreshTokenEntity {
            id: Uuid::new_v4(),
            user_id,
            token_hash: token_hash.to_string(),
            expires_at,
            created_at: Utc::now(),
        });
        Ok(())
    }
    async fn find_by_hash(
        &self,
        token_hash: &str,
    ) -> Result<Option<RefreshTokenEntity>, UserError> {
        Ok(self
            .tokens
            .lock()
            .await
            .iter()
            .find(|t| t.token_hash == token_hash)
            .cloned())
    }
    async fn take_by_hash(
        &self,
        token_hash: &str,
    ) -> Result<Option<RefreshTokenEntity>, UserError> {
        let mut tokens = self.tokens.lock().await;
        if let Some(pos) = tokens.iter().position(|t| t.token_hash == token_hash) {
            Ok(Some(tokens.remove(pos)))
        } else {
            Ok(None)
        }
    }
    async fn delete_all_for_user(&self, user_id: Uuid) -> Result<(), UserError> {
        self.tokens.lock().await.retain(|t| t.user_id != user_id);
        Ok(())
    }
}

// --- Mock OAuthStateStore ----------------------------------------------------

#[derive(Default)]
pub struct MockStateStore {
    inner: Mutex<HashMap<String, OAuthStateData>>,
}

#[async_trait]
impl OAuthStateStore for MockStateStore {
    async fn store(&self, state: &str, data: OAuthStateData) -> Result<(), OAuthStateError> {
        self.inner.lock().await.insert(state.to_string(), data);
        Ok(())
    }
    async fn take(&self, state: &str) -> Result<Option<OAuthStateData>, OAuthStateError> {
        Ok(self.inner.lock().await.remove(state))
    }
}

// --- Test AppState builder ---------------------------------------------------

pub fn test_jwt_config() -> JwtConfig {
    JwtConfig {
        secret: b"test-secret-for-api-tests".to_vec(),
        access_expiry_secs: 900,
        refresh_expiry_secs: 2592000,
    }
}

pub fn test_app_state() -> AppState {
    let oauth_config = OAuthConfig {
        redirect_uri: "http://localhost:5173/callback".into(),
        client_id: "https://zurfur.app".into(),
        private_signing_key_data: atproto_identity::key::KeyData::new(
            atproto_identity::key::KeyType::P256Private,
            vec![0u8; 32],
        ),
    };

    let user_repo = Arc::new(MockUserRepo::default());
    let session_repo = Arc::new(MockSessionRepo::default());
    let refresh_repo = Arc::new(MockRefreshRepo::default());
    let state_store = Arc::new(MockStateStore::default());
    let oauth_storage = create_default_oauth_storage(NonZeroUsize::new(10).unwrap());

    let auth_service = AuthService::new(
        oauth_config,
        test_jwt_config(),
        oauth_storage,
        state_store,
        user_repo,
        session_repo,
        refresh_repo,
    );

    AppState {
        auth: auth_service,
    }
}

/// Issue a valid JWT for testing protected routes.
pub fn issue_test_jwt(user_id: &Uuid, did: &str, handle: Option<&str>) -> String {
    use application::auth::service::ZurfurClaims;
    let config = test_jwt_config();
    let claims = ZurfurClaims {
        sub: user_id.to_string(),
        did: did.to_string(),
        handle: handle.map(String::from),
        exp: (chrono::Utc::now() + chrono::Duration::hours(1)).timestamp(),
    };
    shared::jwt::create(&claims, &config.secret).unwrap()
}
