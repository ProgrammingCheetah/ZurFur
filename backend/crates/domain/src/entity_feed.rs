//! EntityFeed — polymorphic join attaching feeds to entities.
//!
//! ARCHITECTURE DECISIONS:
//!   Each feed belongs to exactly one entity (PK is feed_id). An entity
//!   can own many feeds. entity_type is not a foreign key — validation
//!   happens at the application layer. This avoids multi-table FK complexity
//!   while keeping the schema flexible.
//!
//!   The discriminator uses `EntityKind` from `entity.rs` — a unified enum
//!   replacing the former `EntityType` (which only covered 4 variants).

use uuid::Uuid;

use crate::entity::EntityKind;

/// A polymorphic join attaching a feed to an entity.
#[derive(Debug, Clone)]
pub struct EntityFeed {
    pub feed_id: Uuid,
    pub entity_type: EntityKind,
    pub entity_id: Uuid,
}

/// Errors from entity feed operations.
#[derive(Debug, thiserror::Error)]
pub enum EntityFeedError {
    #[error("Entity feed not found")]
    NotFound,
    #[error("Feed is already attached to an entity")]
    AlreadyAttached,
    #[error("Database error: {0}")]
    Database(String),
}

/// Repository trait for entity feed persistence.
#[async_trait::async_trait]
pub trait EntityFeedRepository: Send + Sync {
    async fn attach(
        &self,
        feed_id: Uuid,
        entity_type: EntityKind,
        entity_id: Uuid,
    ) -> Result<EntityFeed, EntityFeedError>;

    async fn find_by_feed_id(&self, feed_id: Uuid) -> Result<Option<EntityFeed>, EntityFeedError>;

    async fn list_by_entity(
        &self,
        entity_type: EntityKind,
        entity_id: Uuid,
    ) -> Result<Vec<EntityFeed>, EntityFeedError>;

    async fn detach(&self, feed_id: Uuid) -> Result<(), EntityFeedError>;
}
