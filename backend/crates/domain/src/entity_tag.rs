//! EntityTag — polymorphic junction attaching tags to entities.
//!
//! ARCHITECTURE DECISIONS:
//!   Same pattern as entity_feed: entity_type + entity_id, no FK to entity
//!   tables, validated at the application layer. No timestamps on the junction.
//!
//!   The discriminator uses `EntityKind` from `entity.rs` — a unified enum
//!   replacing the former `TaggableEntityType` (which only covered 5 variants).

use uuid::Uuid;

use crate::entity::EntityKind;

/// A polymorphic junction row connecting a tag to any entity.
///
/// No timestamps — this is a pure relationship, same as `EntityFeed`.
/// The composite primary key is (entity_type, entity_id, tag_id).
#[derive(Debug, Clone)]
pub struct EntityTag {
    /// What kind of entity this tag is attached to.
    pub entity_type: EntityKind,
    /// The UUID of the entity (org, commission, feed_item, etc.).
    pub entity_id: Uuid,
    /// The UUID of the tag.
    pub tag_id: Uuid,
}

/// Errors from entity tag operations.
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
        entity_type: EntityKind,
        entity_id: Uuid,
        tag_id: Uuid,
    ) -> Result<EntityTag, EntityTagError>;

    /// Remove a tag from an entity. Returns `NotFound` if the triple doesn't exist.
    async fn detach(
        &self,
        entity_type: EntityKind,
        entity_id: Uuid,
        tag_id: Uuid,
    ) -> Result<(), EntityTagError>;

    /// List all tags attached to a specific entity.
    async fn list_by_entity(
        &self,
        entity_type: EntityKind,
        entity_id: Uuid,
    ) -> Result<Vec<EntityTag>, EntityTagError>;

    /// Reverse lookup: list all entities that have a specific tag attached.
    async fn list_by_tag(
        &self,
        tag_id: Uuid,
    ) -> Result<Vec<EntityTag>, EntityTagError>;
}
