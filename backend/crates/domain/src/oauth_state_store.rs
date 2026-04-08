/// Data stored alongside OAuth state during the login flow.
#[derive(Debug, Clone)]
pub struct OAuthStateData {
    pub did: String,
    pub handle: Option<String>,
}

/// Pluggable storage for OAuth state data.
///
/// During the OAuth flow, we store the resolved identity when `/auth/start` is called,
/// then retrieve it when `/auth/callback` fires (Bluesky only sends code + state).
/// This trait abstracts the storage so it can be backed by in-memory HashMap (dev)
/// or Redis (production) without changing any other code.
#[async_trait::async_trait]
pub trait OAuthStateStore: Send + Sync {
    async fn store(&self, state: &str, data: OAuthStateData) -> Result<(), OAuthStateError>;

    /// Retrieve and remove the data for a given state (single-use).
    async fn take(&self, state: &str) -> Result<Option<OAuthStateData>, OAuthStateError>;
}

#[derive(Debug, thiserror::Error)]
pub enum OAuthStateError {
    #[error("Storage error: {0}")]
    Storage(String),
}
