/// Pluggable storage for OAuth state-to-DID mapping.
///
/// During the OAuth flow, we need to store the resolved DID when `/auth/start` is called,
/// then retrieve it when `/auth/callback` fires (Bluesky only sends code + state, not DID).
/// This trait abstracts the storage so it can be backed by in-memory HashMap (dev)
/// or Redis (production) without changing any other code.
#[async_trait::async_trait]
pub trait OAuthStateStore: Send + Sync {
    async fn store_did(&self, state: &str, did: &str) -> Result<(), OAuthStateError>;

    /// Retrieve and remove the DID for a given state (single-use).
    async fn take_did(&self, state: &str) -> Result<Option<String>, OAuthStateError>;
}

#[derive(Debug, thiserror::Error)]
pub enum OAuthStateError {
    #[error("Storage error: {0}")]
    Storage(String),
}
