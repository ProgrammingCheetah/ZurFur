use domain::oauth_state_store::{OAuthStateError, OAuthStateStore};
use std::collections::HashMap;
use tokio::sync::RwLock;

/// In-memory implementation of `OAuthStateStore` for development.
/// For production, swap to a Redis-backed implementation of the same trait.
pub struct InMemoryOAuthStateStore {
    inner: RwLock<HashMap<String, String>>,
}

impl InMemoryOAuthStateStore {
    pub fn new() -> Self {
        Self {
            inner: RwLock::new(HashMap::new()),
        }
    }
}

impl Default for InMemoryOAuthStateStore {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl OAuthStateStore for InMemoryOAuthStateStore {
    async fn store_did(&self, state: &str, did: &str) -> Result<(), OAuthStateError> {
        self.inner
            .write()
            .await
            .insert(state.to_string(), did.to_string());
        Ok(())
    }

    async fn take_did(&self, state: &str) -> Result<Option<String>, OAuthStateError> {
        Ok(self.inner.write().await.remove(state))
    }
}
