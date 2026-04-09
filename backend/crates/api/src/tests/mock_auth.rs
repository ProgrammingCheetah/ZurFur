//! Mock implementations for authentication-related repository traits.

use std::collections::HashMap;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use domain::atproto_session::{AtprotoSessionEntity, AtprotoSessionRepository};
use domain::oauth_state_store::{OAuthStateData, OAuthStateError, OAuthStateStore};
use domain::refresh_token::{RefreshTokenEntity, RefreshTokenRepository};
use domain::user::UserError;
use tokio::sync::Mutex;
use uuid::Uuid;

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
        let sessions = self.sessions.lock().await;
        let session = sessions.iter().find(|s| s.user_id == user_id).cloned();
        Ok(session)
    }
    async fn find_by_did(&self, did: &str) -> Result<Option<AtprotoSessionEntity>, UserError> {
        let sessions = self.sessions.lock().await;
        let session = sessions.iter().find(|s| s.did == did).cloned();
        Ok(session)
    }
    async fn delete_by_user_id(&self, user_id: Uuid) -> Result<(), UserError> {
        self.sessions.lock().await.retain(|s| s.user_id != user_id);
        Ok(())
    }
}

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
        let tokens = self.tokens.lock().await;
        let token = tokens.iter().find(|t| t.token_hash == token_hash).cloned();
        Ok(token)
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
        let result = self.inner.lock().await.remove(state);
        Ok(result)
    }
}
