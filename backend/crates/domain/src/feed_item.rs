//! FeedItem — a single post/event within a feed.
//!
//! A feed item is the container for one logical entry. It can have multiple
//! elements (text + image + file attachment, etc.) via FeedElement.
//!
//! ARCHITECTURE DECISIONS:
//!   Feed items are immutable once created — there is no `updated_at` field
//!   and no `update` method on the repository. The item structure (author,
//!   feed, timestamp) cannot change. Individual elements within an item CAN
//!   be edited via FeedElementRepository::update_content, but the post itself
//!   is append-only. This models a chronological event log where entries are
//!   facts, not drafts.

use chrono::{DateTime, Utc};
use uuid::Uuid;

/// Who authored the feed item.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthorType {
    User,
    Org,
    System,
}

impl AuthorType {
    pub fn as_str(&self) -> &'static str {
        match self {
            AuthorType::User => "user",
            AuthorType::Org => "org",
            AuthorType::System => "system",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "user" => Some(AuthorType::User),
            "org" => Some(AuthorType::Org),
            "system" => Some(AuthorType::System),
            _ => None,
        }
    }
}

impl From<AuthorType> for &'static str {
    fn from(at: AuthorType) -> Self {
        at.as_str()
    }
}

impl TryFrom<&str> for AuthorType {
    type Error = String;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        AuthorType::from_str(s).ok_or_else(|| format!("Unknown author type: {s}"))
    }
}

#[derive(Debug, Clone)]
pub struct FeedItem {
    pub id: Uuid,
    pub feed_id: Uuid,
    pub author_type: AuthorType,
    pub author_id: Uuid,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, thiserror::Error)]
pub enum FeedItemError {
    #[error("Feed item not found")]
    NotFound,
    #[error("Database error: {0}")]
    Database(String),
}

#[async_trait::async_trait]
pub trait FeedItemRepository: Send + Sync {
    async fn create(
        &self,
        feed_id: Uuid,
        author_type: AuthorType,
        author_id: Uuid,
    ) -> Result<FeedItem, FeedItemError>;

    async fn find_by_id(&self, id: Uuid) -> Result<Option<FeedItem>, FeedItemError>;

    async fn list_by_feed(
        &self,
        feed_id: Uuid,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<FeedItem>, FeedItemError>;

    async fn delete(&self, id: Uuid) -> Result<(), FeedItemError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn author_type_round_trip() {
        let variants = [
            (AuthorType::User, "user"),
            (AuthorType::Org, "org"),
            (AuthorType::System, "system"),
        ];
        for (variant, s) in variants {
            assert_eq!(variant.as_str(), s);
            assert_eq!(AuthorType::from_str(s), Some(variant));
        }
    }

    #[test]
    fn author_type_from_str_returns_none_for_unknown() {
        assert_eq!(AuthorType::from_str("bot"), None);
        assert_eq!(AuthorType::from_str(""), None);
    }
}
