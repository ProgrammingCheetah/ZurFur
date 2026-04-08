use chrono::{DateTime, Duration, Utc};
use domain::oauth_state_store::{OAuthStateError, OAuthStateStore};
use std::collections::HashMap;
use tokio::sync::RwLock;

struct Entry {
    value: String,
    expires_at: DateTime<Utc>,
}

/// In-memory implementation of `OAuthStateStore` for development.
/// Entries expire after 10 minutes (matching the OAuth request TTL).
/// For production, swap to a Redis-backed implementation of the same trait.
pub struct InMemoryOAuthStateStore {
    inner: RwLock<HashMap<String, Entry>>,
    ttl: Duration,
}

impl InMemoryOAuthStateStore {
    pub fn new() -> Self {
        Self {
            inner: RwLock::new(HashMap::new()),
            ttl: Duration::minutes(10),
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
        let mut map = self.inner.write().await;

        // Evict expired entries on write to bound memory growth
        let now = Utc::now();
        map.retain(|_, e| e.expires_at > now);

        map.insert(
            state.to_string(),
            Entry {
                value: did.to_string(),
                expires_at: now + self.ttl,
            },
        );
        Ok(())
    }

    async fn take_did(&self, state: &str) -> Result<Option<String>, OAuthStateError> {
        let mut map = self.inner.write().await;
        match map.remove(state) {
            Some(entry) if entry.expires_at > Utc::now() => Ok(Some(entry.value)),
            _ => Ok(None),
        }
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

    #[tokio::test]
    async fn expired_entry_returns_none() {
        let store = InMemoryOAuthStateStore {
            inner: RwLock::new(HashMap::new()),
            ttl: Duration::zero(),
        };
        store.store_did("state-1", "did:plc:abc123").await.unwrap();

        // Entry has already expired (zero TTL)
        let result = store.take_did("state-1").await.unwrap();
        assert!(result.is_none());
    }
}
