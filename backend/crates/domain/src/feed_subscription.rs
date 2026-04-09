//! FeedSubscription — an org's subscription to a feed.
//!
//! ARCHITECTURE DECISIONS:
//!   Following an org = creating read subscriptions to its feeds. There is no
//!   separate "follows" table. Plugin installation = creating a read_write
//!   subscription. This unifies social graph, content delivery, and plugin
//!   permissions into a single mechanism.

use chrono::{DateTime, Utc};
use uuid::Uuid;

/// Permission level for a feed subscription.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubscriptionPermission {
    /// Can read feed items.
    Read,
    /// Can read and write feed items (used by plugins).
    ReadWrite,
    /// Full control over the feed.
    Admin,
}

impl SubscriptionPermission {
    pub fn as_str(&self) -> &'static str {
        match self {
            SubscriptionPermission::Read => "read",
            SubscriptionPermission::ReadWrite => "read_write",
            SubscriptionPermission::Admin => "admin",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "read" => Some(SubscriptionPermission::Read),
            "read_write" => Some(SubscriptionPermission::ReadWrite),
            "admin" => Some(SubscriptionPermission::Admin),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct FeedSubscription {
    pub id: Uuid,
    pub feed_id: Uuid,
    pub subscriber_org_id: Uuid,
    pub permissions: SubscriptionPermission,
    pub granted_at: DateTime<Utc>,
    pub granted_by_user_id: Uuid,
}

#[derive(Debug, thiserror::Error)]
pub enum FeedSubscriptionError {
    #[error("Subscription not found")]
    NotFound,
    #[error("Already subscribed")]
    AlreadySubscribed,
    #[error("Database error: {0}")]
    Database(String),
}

#[async_trait::async_trait]
pub trait FeedSubscriptionRepository: Send + Sync {
    async fn create(
        &self,
        feed_id: Uuid,
        subscriber_org_id: Uuid,
        permissions: SubscriptionPermission,
        granted_by_user_id: Uuid,
    ) -> Result<FeedSubscription, FeedSubscriptionError>;

    async fn find_by_id(
        &self,
        id: Uuid,
    ) -> Result<Option<FeedSubscription>, FeedSubscriptionError>;

    async fn list_by_feed(
        &self,
        feed_id: Uuid,
    ) -> Result<Vec<FeedSubscription>, FeedSubscriptionError>;

    async fn list_by_subscriber(
        &self,
        subscriber_org_id: Uuid,
    ) -> Result<Vec<FeedSubscription>, FeedSubscriptionError>;

    async fn update_permissions(
        &self,
        id: Uuid,
        permissions: SubscriptionPermission,
    ) -> Result<FeedSubscription, FeedSubscriptionError>;

    async fn delete(&self, id: Uuid) -> Result<(), FeedSubscriptionError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subscription_permission_round_trip() {
        let variants = [
            (SubscriptionPermission::Read, "read"),
            (SubscriptionPermission::ReadWrite, "read_write"),
            (SubscriptionPermission::Admin, "admin"),
        ];
        for (variant, s) in variants {
            assert_eq!(variant.as_str(), s);
            assert_eq!(SubscriptionPermission::from_str(s), Some(variant));
        }
    }

    #[test]
    fn subscription_permission_from_str_returns_none_for_unknown() {
        assert_eq!(SubscriptionPermission::from_str("write"), None);
        assert_eq!(SubscriptionPermission::from_str(""), None);
    }
}
