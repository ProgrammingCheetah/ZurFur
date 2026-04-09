//! Feed entity — the universal content container.
//!
//! ARCHITECTURE DECISIONS:
//!   Feeds are a root aggregate. Every major view (gallery, activity stream,
//!   commission history, notifications) is a feed renderer with filters.
//!   Ownership is polymorphic via `entity_feeds` — an org, character,
//!   commission, or user can own feeds. System feeds (type='system') are
//!   auto-created and cannot be deleted or renamed.

use chrono::{DateTime, Utc};
use uuid::Uuid;

/// Whether a feed is system-managed or user-created.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FeedType {
    /// Auto-created, undeletable feeds (updates, gallery, activity, commissions).
    System,
    /// User-created feeds that can be renamed and deleted.
    Custom,
}

impl FeedType {
    pub fn as_str(&self) -> &'static str {
        match self {
            FeedType::System => "system",
            FeedType::Custom => "custom",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "system" => Some(FeedType::System),
            "custom" => Some(FeedType::Custom),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Feed {
    pub id: Uuid,
    pub slug: String,
    pub display_name: String,
    pub description: Option<String>,
    pub feed_type: FeedType,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

#[derive(Debug, thiserror::Error)]
pub enum FeedError {
    #[error("Feed not found")]
    NotFound,
    #[error("System feeds cannot be deleted")]
    SystemFeedUndeletable,
    #[error("Slug already taken for this entity: {0}")]
    SlugTaken(String),
    #[error("Database error: {0}")]
    Database(String),
}

#[async_trait::async_trait]
pub trait FeedRepository: Send + Sync {
    async fn create(
        &self,
        slug: &str,
        display_name: &str,
        description: Option<&str>,
        feed_type: FeedType,
    ) -> Result<Feed, FeedError>;

    async fn find_by_id(&self, id: Uuid) -> Result<Option<Feed>, FeedError>;

    async fn update(
        &self,
        id: Uuid,
        display_name: &str,
        description: Option<&str>,
    ) -> Result<Feed, FeedError>;

    async fn soft_delete(&self, id: Uuid) -> Result<(), FeedError>;

    async fn list_by_ids(&self, ids: &[Uuid]) -> Result<Vec<Feed>, FeedError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn feed_type_round_trip() {
        for (variant, s) in [(FeedType::System, "system"), (FeedType::Custom, "custom")] {
            assert_eq!(variant.as_str(), s);
            assert_eq!(FeedType::from_str(s), Some(variant));
        }
    }

    #[test]
    fn feed_type_from_str_returns_none_for_unknown() {
        assert_eq!(FeedType::from_str("other"), None);
        assert_eq!(FeedType::from_str(""), None);
    }
}
