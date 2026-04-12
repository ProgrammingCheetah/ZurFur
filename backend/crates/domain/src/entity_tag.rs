//! EntityTag — polymorphic junction attaching tags to entities.
//!
//! ARCHITECTURE DECISIONS:
//!   Same pattern as entity_feed: entity_type + entity_id, no FK to entity
//!   tables, validated at the application layer. No timestamps on the junction.
//!
//!   Uses a SEPARATE enum from EntityType (entity_feed) because the set of
//!   taggable entities differs from the set of feed-owning entities.

use uuid::Uuid;

/// The type of entity that a tag can be attached to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaggableEntityType {
    Org,
    Commission,
    FeedItem,
    Character,
    FeedElement,
}

impl TaggableEntityType {
    pub fn as_str(&self) -> &'static str {
        match self {
            TaggableEntityType::Org => "org",
            TaggableEntityType::Commission => "commission",
            TaggableEntityType::FeedItem => "feed_item",
            TaggableEntityType::Character => "character",
            TaggableEntityType::FeedElement => "feed_element",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "org" => Some(TaggableEntityType::Org),
            "commission" => Some(TaggableEntityType::Commission),
            "feed_item" => Some(TaggableEntityType::FeedItem),
            "character" => Some(TaggableEntityType::Character),
            "feed_element" => Some(TaggableEntityType::FeedElement),
            _ => None,
        }
    }
}

impl From<TaggableEntityType> for &'static str {
    fn from(tet: TaggableEntityType) -> Self {
        tet.as_str()
    }
}

impl TryFrom<&str> for TaggableEntityType {
    type Error = String;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        TaggableEntityType::from_str(s)
            .ok_or_else(|| format!("Unknown taggable entity type: {s}"))
    }
}

#[derive(Debug, Clone)]
pub struct EntityTag {
    pub entity_type: TaggableEntityType,
    pub entity_id: Uuid,
    pub tag_id: Uuid,
}

#[derive(Debug, thiserror::Error)]
pub enum EntityTagError {
    #[error("Entity tag not found")]
    NotFound,
    #[error("Tag is already attached to this entity")]
    AlreadyAttached,
    #[error("Database error: {0}")]
    Database(String),
}

#[async_trait::async_trait]
pub trait EntityTagRepository: Send + Sync {
    async fn attach(
        &self,
        entity_type: TaggableEntityType,
        entity_id: Uuid,
        tag_id: Uuid,
    ) -> Result<EntityTag, EntityTagError>;

    async fn detach(
        &self,
        entity_type: TaggableEntityType,
        entity_id: Uuid,
        tag_id: Uuid,
    ) -> Result<(), EntityTagError>;

    async fn list_by_entity(
        &self,
        entity_type: TaggableEntityType,
        entity_id: Uuid,
    ) -> Result<Vec<EntityTag>, EntityTagError>;

    async fn list_by_tag(
        &self,
        tag_id: Uuid,
    ) -> Result<Vec<EntityTag>, EntityTagError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn taggable_entity_type_round_trip() {
        let variants = [
            (TaggableEntityType::Org, "org"),
            (TaggableEntityType::Commission, "commission"),
            (TaggableEntityType::FeedItem, "feed_item"),
            (TaggableEntityType::Character, "character"),
            (TaggableEntityType::FeedElement, "feed_element"),
        ];
        for (variant, s) in variants {
            assert_eq!(variant.as_str(), s);
            assert_eq!(TaggableEntityType::from_str(s), Some(variant));
        }
    }

    #[test]
    fn taggable_entity_type_from_str_returns_none_for_unknown() {
        assert_eq!(TaggableEntityType::from_str("user"), None);
        assert_eq!(TaggableEntityType::from_str(""), None);
    }
}
