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
///
/// This is a SEPARATE enum from `EntityType` (used by `entity_feed`) because
/// the set of taggable entities differs from the set of feed-owning entities.
/// Keeping them separate avoids coupling between the tag and feed systems.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaggableEntityType {
    Org,
    Commission,
    FeedItem,
    Character,
    FeedElement,
}

impl TaggableEntityType {
    /// Returns the string representation matching the database CHECK constraint value.
    pub fn as_str(&self) -> &'static str {
        match self {
            TaggableEntityType::Org => "org",
            TaggableEntityType::Commission => "commission",
            TaggableEntityType::FeedItem => "feed_item",
            TaggableEntityType::Character => "character",
            TaggableEntityType::FeedElement => "feed_element",
        }
    }

    /// Parses a string into a `TaggableEntityType`. Returns `None` for unknown values.
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

/// A polymorphic junction row connecting a tag to any entity.
///
/// No timestamps — this is a pure relationship, same as `EntityFeed`.
/// The composite primary key is (entity_type, entity_id, tag_id).
#[derive(Debug, Clone)]
pub struct EntityTag {
    /// What kind of entity this tag is attached to.
    pub entity_type: TaggableEntityType,
    /// The UUID of the entity (org, commission, feed_item, etc.).
    pub entity_id: Uuid,
    /// The UUID of the tag.
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

/// Repository trait for entity-tag junction persistence.
#[async_trait::async_trait]
pub trait EntityTagRepository: Send + Sync {
    /// Attach a tag to an entity. Returns `AlreadyAttached` if the triple already exists.
    async fn attach(
        &self,
        entity_type: TaggableEntityType,
        entity_id: Uuid,
        tag_id: Uuid,
    ) -> Result<EntityTag, EntityTagError>;

    /// Remove a tag from an entity. Returns `NotFound` if the triple doesn't exist.
    async fn detach(
        &self,
        entity_type: TaggableEntityType,
        entity_id: Uuid,
        tag_id: Uuid,
    ) -> Result<(), EntityTagError>;

    /// List all tags attached to a specific entity.
    async fn list_by_entity(
        &self,
        entity_type: TaggableEntityType,
        entity_id: Uuid,
    ) -> Result<Vec<EntityTag>, EntityTagError>;

    /// Reverse lookup: list all entities that have a specific tag attached.
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
