//! EntityFeed — polymorphic join attaching feeds to entities.
//!
//! ARCHITECTURE DECISIONS:
//!   Each feed belongs to exactly one entity (PK is feed_id). An entity
//!   (org, character, commission, user) can own many feeds. entity_type
//!   is not a foreign key — validation happens at the application layer.
//!   This avoids multi-table FK complexity while keeping the schema flexible.

use uuid::Uuid;

/// The type of entity that owns a feed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntityType {
    Org,
    Character,
    Commission,
    User,
}

impl EntityType {
    pub fn as_str(&self) -> &'static str {
        match self {
            EntityType::Org => "org",
            EntityType::Character => "character",
            EntityType::Commission => "commission",
            EntityType::User => "user",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "org" => Some(EntityType::Org),
            "character" => Some(EntityType::Character),
            "commission" => Some(EntityType::Commission),
            "user" => Some(EntityType::User),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct EntityFeed {
    pub feed_id: Uuid,
    pub entity_type: EntityType,
    pub entity_id: Uuid,
}

#[derive(Debug, thiserror::Error)]
pub enum EntityFeedError {
    #[error("Entity feed not found")]
    NotFound,
    #[error("Feed already attached to this entity")]
    AlreadyAttached,
    #[error("Database error: {0}")]
    Database(String),
}

#[async_trait::async_trait]
pub trait EntityFeedRepository: Send + Sync {
    async fn attach(
        &self,
        feed_id: Uuid,
        entity_type: EntityType,
        entity_id: Uuid,
    ) -> Result<EntityFeed, EntityFeedError>;

    async fn find_by_feed_id(&self, feed_id: Uuid) -> Result<Option<EntityFeed>, EntityFeedError>;

    async fn list_by_entity(
        &self,
        entity_type: EntityType,
        entity_id: Uuid,
    ) -> Result<Vec<EntityFeed>, EntityFeedError>;

    async fn detach(&self, feed_id: Uuid) -> Result<(), EntityFeedError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entity_type_round_trip() {
        let variants = [
            (EntityType::Org, "org"),
            (EntityType::Character, "character"),
            (EntityType::Commission, "commission"),
            (EntityType::User, "user"),
        ];
        for (variant, s) in variants {
            assert_eq!(variant.as_str(), s);
            assert_eq!(EntityType::from_str(s), Some(variant));
        }
    }

    #[test]
    fn entity_type_from_str_returns_none_for_unknown() {
        assert_eq!(EntityType::from_str("team"), None);
        assert_eq!(EntityType::from_str(""), None);
    }
}
