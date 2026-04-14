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

use crate::feed_element::{FeedElement, FeedElementType};

/// Who authored the feed item.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthorType {
    User,
    Org,
    System,
}

impl AuthorType {
    /// Returns the string representation matching the database value.
    pub fn as_str(&self) -> &'static str {
        match self {
            AuthorType::User => "user",
            AuthorType::Org => "org",
            AuthorType::System => "system",
        }
    }

    /// Parse from a database string value. Returns `None` for unknown values.
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

/// A single post or event within a feed, containing one or more elements.
#[derive(Debug, Clone)]
pub struct FeedItem {
    pub id: Uuid,
    pub feed_id: Uuid,
    pub author_type: AuthorType,
    pub author_id: Uuid,
    pub created_at: DateTime<Utc>,
}

/// Errors from feed item operations.
#[derive(Debug, thiserror::Error)]
pub enum FeedItemError {
    #[error("Feed item not found")]
    NotFound,
    #[error("Database error: {0}")]
    Database(String),
}

/// Repository trait for feed item persistence.
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

    /// Atomically create a feed item and all its elements in a single transaction.
    async fn create_with_elements(
        &self,
        feed_id: Uuid,
        author_type: AuthorType,
        author_id: Uuid,
        elements: &[NewFeedElementInput],
    ) -> Result<(FeedItem, Vec<FeedElement>), FeedItemError>;
}

/// Input for creating a feed element as part of `create_with_elements`.
pub struct NewFeedElementInput {
    pub element_type: FeedElementType,
    pub content_json: String,
    pub position: i32,
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
