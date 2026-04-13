use chrono::{DateTime, Duration, Utc};
use domain::oauth_state_store::{OAuthStateData, OAuthStateError, OAuthStateStore};
use std::collections::HashMap;
use tokio::sync::RwLock;

struct Entry {
    data: OAuthStateData,
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
    /// Create a new in-memory store with a 10-minute entry TTL.
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
    async fn store(&self, state: &str, data: OAuthStateData) -> Result<(), OAuthStateError> {
        let mut map = self.inner.write().await;

        // Evict expired entries on write to bound memory growth
        let now = Utc::now();
        map.retain(|_, e| e.expires_at > now);

        map.insert(
            state.to_string(),
            Entry {
                data,
                expires_at: now + self.ttl,
            },
        );
        Ok(())
    }

    async fn take(&self, state: &str) -> Result<Option<OAuthStateData>, OAuthStateError> {
        let mut map = self.inner.write().await;
        match map.remove(state) {
            Some(entry) if entry.expires_at > Utc::now() => Ok(Some(entry.data)),
            _ => Ok(None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_data(did: &str) -> OAuthStateData {
        OAuthStateData {
            did: did.to_string(),
            handle: None,
        }
    }

    #[tokio::test]
    async fn store_and_take_returns_data() {
        let store = InMemoryOAuthStateStore::new();
        store.store("state-1", test_data("did:plc:abc123")).await.unwrap();

        let data = store.take("state-1").await.unwrap().unwrap();
        assert_eq!(data.did, "did:plc:abc123");
    }

    #[tokio::test]
    async fn take_is_single_use() {
        let store = InMemoryOAuthStateStore::new();
        store.store("state-1", test_data("did:plc:abc123")).await.unwrap();

        assert!(store.take("state-1").await.unwrap().is_some());
        assert!(store.take("state-1").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn take_unknown_state_returns_none() {
        let store = InMemoryOAuthStateStore::new();
        assert!(store.take("nonexistent").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn store_overwrites_existing_state() {
        let store = InMemoryOAuthStateStore::new();
        store.store("state-1", test_data("did:plc:first")).await.unwrap();
        store.store("state-1", test_data("did:plc:second")).await.unwrap();

        let data = store.take("state-1").await.unwrap().unwrap();
        assert_eq!(data.did, "did:plc:second");
    }

    #[tokio::test]
    async fn multiple_states_are_independent() {
        let store = InMemoryOAuthStateStore::new();
        store.store("state-a", test_data("did:plc:aaa")).await.unwrap();
        store.store("state-b", test_data("did:plc:bbb")).await.unwrap();

        assert_eq!(store.take("state-a").await.unwrap().unwrap().did, "did:plc:aaa");
        assert_eq!(store.take("state-b").await.unwrap().unwrap().did, "did:plc:bbb");
    }

    #[tokio::test]
    async fn expired_entry_returns_none() {
        let store = InMemoryOAuthStateStore {
            inner: RwLock::new(HashMap::new()),
            ttl: Duration::zero(),
        };
        store.store("state-1", test_data("did:plc:abc123")).await.unwrap();
        assert!(store.take("state-1").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn stores_handle_when_provided() {
        let store = InMemoryOAuthStateStore::new();
        let data = OAuthStateData {
            did: "did:plc:abc123".to_string(),
            handle: Some("test.bsky.social".to_string()),
        };
        store.store("state-1", data).await.unwrap();

        let result = store.take("state-1").await.unwrap().unwrap();
        assert_eq!(result.handle, Some("test.bsky.social".to_string()));
    }
}
