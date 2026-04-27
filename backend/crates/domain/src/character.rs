use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::content_rating::ContentRating;

/// Visibility level for a character.
///
/// ARCHITECTURE DECISIONS:
///   Backed by a PostgreSQL ENUM (`character_visibility`) because the set is
///   stable and maps directly to access-control logic. `Controlled` is defined
///   but deferred — behaves as `Private` until the `character_access` table is
///   built.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CharacterVisibility {
    Public,
    Private,
    Controlled,
    Unlisted,
}

impl CharacterVisibility {
    /// Returns the string representation matching the database value.
    pub fn as_str(&self) -> &'static str {
        match self {
            CharacterVisibility::Public => "public",
            CharacterVisibility::Private => "private",
            CharacterVisibility::Controlled => "controlled",
            CharacterVisibility::Unlisted => "unlisted",
        }
    }

    /// Parse from a database string value. Returns `None` for unknown values.
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "public" => Some(CharacterVisibility::Public),
            "private" => Some(CharacterVisibility::Private),
            "controlled" => Some(CharacterVisibility::Controlled),
            "unlisted" => Some(CharacterVisibility::Unlisted),
            _ => None,
        }
    }
}

impl From<CharacterVisibility> for &'static str {
    fn from(cv: CharacterVisibility) -> Self {
        cv.as_str()
    }
}

impl TryFrom<&str> for CharacterVisibility {
    type Error = String;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        CharacterVisibility::from_str(s).ok_or_else(|| format!("Unknown character visibility: {s}"))
    }
}

/// An original character (OC) owned by an organization.
///
/// ARCHITECTURE DECISIONS:
///   Characters are owned children of organizations, not independent aggregates.
///   `org_id` is a direct FK — the only cross-aggregate reference allowed
///   because characters cannot exist without an owning org.
///
///   Description is a TEXT column (not a feed) for simplicity in the core phase.
///   Migration to a feed (version history, template support) is straightforward.
///
///   The character's "gallery" is a filtered view of the org's feed — content
///   posted to the org's feed tagged with this character. No separate
///   `entity_feed` relationship needed.
///
///   Characters implement `Entity`, `Taggable`, and `FeedOwnable` (all default
///   validations). `FeedOwnable` is unused initially but costs nothing.
#[derive(Debug, Clone)]
pub struct Character {
    pub id: Uuid,
    pub org_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub content_rating: ContentRating,
    pub visibility: CharacterVisibility,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

/// Errors from character operations.
#[derive(Debug, thiserror::Error)]
pub enum CharacterError {
    #[error("Character not found")]
    NotFound,
    #[error("Database error: {0}")]
    Database(String),
}

/// Repository trait for character persistence.
#[async_trait::async_trait]
pub trait CharacterRepository: Send + Sync {
    /// Create a new character owned by an organization.
    async fn create(
        &self,
        org_id: Uuid,
        name: &str,
        description: Option<&str>,
        content_rating: ContentRating,
        visibility: CharacterVisibility,
    ) -> Result<Character, CharacterError>;

    /// Find a character by ID, excluding soft-deleted.
    async fn find_by_id(&self, id: Uuid) -> Result<Option<Character>, CharacterError>;

    /// List characters for an organization with optional filters.
    /// Excludes soft-deleted characters.
    async fn list_by_org(
        &self,
        org_id: Uuid,
        limit: i64,
        offset: i64,
        content_rating: Option<ContentRating>,
    ) -> Result<Vec<Character>, CharacterError>;

    /// Update a character's mutable fields.
    async fn update(
        &self,
        id: Uuid,
        name: &str,
        description: Option<&str>,
        content_rating: ContentRating,
        visibility: CharacterVisibility,
    ) -> Result<Character, CharacterError>;

    /// Soft-delete a character by setting `deleted_at`.
    async fn soft_delete(&self, id: Uuid) -> Result<(), CharacterError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn character_visibility_round_trip() {
        for (variant, expected) in [
            (CharacterVisibility::Public, "public"),
            (CharacterVisibility::Private, "private"),
            (CharacterVisibility::Controlled, "controlled"),
            (CharacterVisibility::Unlisted, "unlisted"),
        ] {
            assert_eq!(variant.as_str(), expected);
            assert_eq!(CharacterVisibility::from_str(expected), Some(variant));
        }
    }

    #[test]
    fn character_visibility_from_str_unknown() {
        assert_eq!(CharacterVisibility::from_str("hidden"), None);
        assert_eq!(CharacterVisibility::from_str(""), None);
        assert_eq!(CharacterVisibility::from_str("Public"), None);
    }
}
