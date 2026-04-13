//! Tag — root aggregate for typed identity and metadata.
//!
//! ARCHITECTURE DECISIONS:
//!   Tags are fully decoupled — no `entity_id` field. The connection between
//!   a tag and any entity lives entirely in `entity_tag`. A tag doesn't know
//!   what it's attached to.
//!
//!   `category` is a PostgreSQL ENUM (stable set): organization, character,
//!   metadata, general. It describes what the tag IS, not how it's connected.
//!   Defaults to `general`. If faceted search later needs finer granularity
//!   (species vs art_style), the enum gains a new value.

use chrono::{DateTime, Utc};
use uuid::Uuid;

/// What kind of tag this is. Stored as a PostgreSQL ENUM (`tag_category`).
///
/// This is intrinsic to the tag — it describes what the tag IS, not how it's
/// connected to entities. The enum grows slowly and intentionally; if faceted
/// search later needs finer granularity (species, art_style), a new variant
/// is added via `ALTER TYPE tag_category ADD VALUE`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TagCategory {
    /// Represents an organization's identity. Auto-created, immutable.
    Organization,
    /// Represents a character's identity. Auto-created, immutable.
    Character,
    /// Descriptive attribute. Community-curated.
    Metadata,
    /// Free-form user-created tag.
    General,
}

impl TagCategory {
    /// Returns the string representation matching the PostgreSQL ENUM value.
    pub fn as_str(&self) -> &'static str {
        match self {
            TagCategory::Organization => "organization",
            TagCategory::Character => "character",
            TagCategory::Metadata => "metadata",
            TagCategory::General => "general",
        }
    }

    /// Parses a string into a `TagCategory`. Returns `None` for unknown values.
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "organization" => Some(TagCategory::Organization),
            "character" => Some(TagCategory::Character),
            "metadata" => Some(TagCategory::Metadata),
            "general" => Some(TagCategory::General),
            _ => None,
        }
    }

    /// Whether tags of this category are immutable (cannot be renamed or deleted).
    pub fn is_immutable(&self) -> bool {
        matches!(self, TagCategory::Organization | TagCategory::Character)
    }
}

impl From<TagCategory> for &'static str {
    fn from(tc: TagCategory) -> Self {
        tc.as_str()
    }
}

impl TryFrom<&str> for TagCategory {
    type Error = String;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        TagCategory::from_str(s).ok_or_else(|| format!("Unknown tag category: {s}"))
    }
}

/// A tag — root aggregate for typed identity and metadata.
///
/// Tags are fully decoupled: no `entity_id` field. The connection between
/// a tag and any entity lives entirely in `entity_tag`. A tag doesn't know
/// what it's attached to.
///
/// `usage_count` is denormalized for sorting/display performance. It is
/// incremented/decremented when tags are attached/detached via `entity_tag`.
#[derive(Debug, Clone)]
pub struct Tag {
    pub id: Uuid,
    pub category: TagCategory,
    pub name: String,
    pub usage_count: i32,
    pub is_approved: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, thiserror::Error)]
pub enum TagError {
    #[error("Tag not found")]
    NotFound,
    #[error("Tag name already exists: {0}")]
    NameTaken(String),
    #[error("Entity-backed tags cannot be modified")]
    Immutable,
    #[error("Database error: {0}")]
    Database(String),
}

/// Repository trait for tag persistence. Implementations live in the persistence crate.
#[async_trait::async_trait]
pub trait TagRepository: Send + Sync {
    /// Create a new tag. Returns `NameTaken` if the (name, category) pair already exists.
    async fn create(
        &self,
        category: TagCategory,
        name: &str,
        is_approved: bool,
    ) -> Result<Tag, TagError>;

    /// Find a tag by its UUID. Returns `None` if not found.
    async fn find_by_id(&self, id: Uuid) -> Result<Option<Tag>, TagError>;

    /// Find a tag by its exact name within a category.
    async fn find_by_name_and_category(
        &self,
        name: &str,
        category: TagCategory,
    ) -> Result<Option<Tag>, TagError>;

    /// List tags in a category, ordered by usage count descending. Paginated.
    async fn list_by_category(
        &self,
        category: TagCategory,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<Tag>, TagError>;

    /// Fetch multiple tags by their UUIDs in a single query.
    async fn list_by_ids(&self, ids: &[Uuid]) -> Result<Vec<Tag>, TagError>;

    /// Prefix search on tag name (case-insensitive). Ordered by usage count descending.
    async fn search_by_name(
        &self,
        query: &str,
        limit: i64,
    ) -> Result<Vec<Tag>, TagError>;

    /// Update a tag's name and approval status. Immutability is enforced at the
    /// application layer (TagService), not here.
    async fn update(
        &self,
        id: Uuid,
        name: &str,
        is_approved: bool,
    ) -> Result<Tag, TagError>;

    /// Atomically increment the tag's usage count by 1. Called when a tag is attached.
    async fn increment_usage_count(&self, id: Uuid) -> Result<(), TagError>;

    /// Atomically decrement the tag's usage count by 1 (floored at 0). Called when detached.
    async fn decrement_usage_count(&self, id: Uuid) -> Result<(), TagError>;

    /// Hard-delete a tag. Immutability is enforced at the application layer.
    async fn delete(&self, id: Uuid) -> Result<(), TagError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tag_category_round_trip() {
        let variants = [
            (TagCategory::Organization, "organization"),
            (TagCategory::Character, "character"),
            (TagCategory::Metadata, "metadata"),
            (TagCategory::General, "general"),
        ];
        for (variant, s) in variants {
            assert_eq!(variant.as_str(), s);
            assert_eq!(TagCategory::from_str(s), Some(variant));
        }
    }

    #[test]
    fn tag_category_from_str_returns_none_for_unknown() {
        assert_eq!(TagCategory::from_str("species"), None);
        assert_eq!(TagCategory::from_str(""), None);
    }

    #[test]
    fn immutable_categories() {
        assert!(TagCategory::Organization.is_immutable());
        assert!(TagCategory::Character.is_immutable());
        assert!(!TagCategory::Metadata.is_immutable());
        assert!(!TagCategory::General.is_immutable());
    }
}
