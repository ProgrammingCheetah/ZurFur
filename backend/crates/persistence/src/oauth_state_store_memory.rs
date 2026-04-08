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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn store_and_take_returns_did() {
        let store = InMemoryOAuthStateStore::new();
        store.store_did("state-1", "did:plc:abc123").await.unwrap();

        let did = store.take_did("state-1").await.unwrap();
        assert_eq!(did, Some("did:plc:abc123".to_string()));
    }

    #[tokio::test]
    async fn take_is_single_use() {
        let store = InMemoryOAuthStateStore::new();
        store.store_did("state-1", "did:plc:abc123").await.unwrap();

        let first = store.take_did("state-1").await.unwrap();
        assert!(first.is_some());

        let second = store.take_did("state-1").await.unwrap();
        assert!(second.is_none());
    }

    #[tokio::test]
    async fn take_unknown_state_returns_none() {
        let store = InMemoryOAuthStateStore::new();
        let result = store.take_did("nonexistent").await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn store_overwrites_existing_state() {
        let store = InMemoryOAuthStateStore::new();
        store.store_did("state-1", "did:plc:first").await.unwrap();
        store.store_did("state-1", "did:plc:second").await.unwrap();

        let did = store.take_did("state-1").await.unwrap();
        assert_eq!(did, Some("did:plc:second".to_string()));
    }

    #[tokio::test]
    async fn multiple_states_are_independent() {
        let store = InMemoryOAuthStateStore::new();
        store.store_did("state-a", "did:plc:aaa").await.unwrap();
        store.store_did("state-b", "did:plc:bbb").await.unwrap();

        let a = store.take_did("state-a").await.unwrap();
        let b = store.take_did("state-b").await.unwrap();
        assert_eq!(a, Some("did:plc:aaa".to_string()));
        assert_eq!(b, Some("did:plc:bbb".to_string()));
    }
}
