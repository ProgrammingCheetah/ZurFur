//! Entity interfaces — shared traits and the unified `EntityKind` enum.
//!
//! ARCHITECTURE DECISIONS:
//!   "Everything is an entity." Every first-class domain object is identified
//!   by a UUID and connects to others through polymorphic junctions. The
//!   composability comes from uniform interfaces.
//!
//!   `EntityKind` replaces the former `EntityType` (entity_feed) and
//!   `TaggableEntityType` (entity_tag) — now that all entities implement all
//!   capability traits, the variant sets are identical and one enum suffices.
//!
//!   Traits have validation hooks with default `Ok(())` implementations.
//!   Services do persistence; traits enforce domain rules. Unused capabilities
//!   cost nothing — new features emerge from new combinations of existing
//!   primitives.

use uuid::Uuid;

use crate::feed_item::AuthorType;

/// The kind of entity. Used as the discriminator in all polymorphic junction
/// tables (entity_feed, entity_tag). Every domain struct maps to exactly one
/// EntityKind variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntityKind {
    User,
    Org,
    Character,
    Commission,
    Feed,
    Tag,
    FeedItem,
    FeedElement,
}

impl EntityKind {
    /// Returns the string representation matching the database value.
    pub fn as_str(&self) -> &'static str {
        match self {
            EntityKind::User => "user",
            EntityKind::Org => "org",
            EntityKind::Character => "character",
            EntityKind::Commission => "commission",
            EntityKind::Feed => "feed",
            EntityKind::Tag => "tag",
            EntityKind::FeedItem => "feed_item",
            EntityKind::FeedElement => "feed_element",
        }
    }

    /// Parse from a database string value. Returns `None` for unknown values.
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "user" => Some(EntityKind::User),
            "org" => Some(EntityKind::Org),
            "character" => Some(EntityKind::Character),
            "commission" => Some(EntityKind::Commission),
            "feed" => Some(EntityKind::Feed),
            "tag" => Some(EntityKind::Tag),
            "feed_item" => Some(EntityKind::FeedItem),
            "feed_element" => Some(EntityKind::FeedElement),
            _ => None,
        }
    }
}

impl From<EntityKind> for &'static str {
    fn from(ek: EntityKind) -> Self {
        ek.as_str()
    }
}

impl TryFrom<&str> for EntityKind {
    type Error = String;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        EntityKind::from_str(s).ok_or_else(|| format!("Unknown entity kind: {s}"))
    }
}

// ---------------------------------------------------------------------------
// Traits
// ---------------------------------------------------------------------------

/// Base trait for all domain entities. Provides identity and kind discrimination.
pub trait Entity: Send + Sync {
    /// The entity's UUID primary key.
    fn id(&self) -> Uuid;

    /// The kind of entity. Used as the discriminator in polymorphic
    /// junction tables (entity_feed, entity_tag).
    fn entity_kind(&self) -> EntityKind;
}

/// Can have tags attached via `entity_tag`. All entities implement this.
///
/// Provides validation hooks that the domain uses to enforce business rules
/// before persistence. Entities that don't care about tag validation use the
/// default `Ok(())`.
pub trait Taggable: Entity {
    /// Domain validation before a tag is attached.
    /// Override to enforce entity-specific rules (e.g., cycle detection for tags tagging tags).
    /// Default: allow all.
    fn validate_tag(&self, _tag_id: Uuid) -> Result<(), String> {
        Ok(())
    }

    /// Domain validation before a tag is detached.
    /// Override to prevent removal of required tags (e.g., identity tags).
    /// Default: allow all.
    fn validate_untag(&self, _tag_id: Uuid) -> Result<(), String> {
        Ok(())
    }
}

/// Can own feeds via `entity_feed`. All entities implement this.
///
/// Same validation hook pattern as Taggable. For most entities this means
/// concrete feeds in the `feeds` table. For Tags, this will be a virtualized
/// query (deferred, post-MVP).
pub trait FeedOwnable: Entity {
    /// Domain validation before a feed is created for this entity.
    /// Override to enforce entity-specific rules.
    /// Default: allow all.
    fn validate_feed_creation(&self) -> Result<(), String> {
        Ok(())
    }

    /// Domain validation before a feed is detached/deleted from this entity.
    /// Override to prevent deletion of required feeds (e.g., system feeds).
    /// Default: allow all.
    fn validate_feed_deletion(&self, _feed_id: Uuid) -> Result<(), String> {
        Ok(())
    }
}

/// Can author feed items. Selective — only actors (User, Organization).
///
/// Characters are posted ON, not BY — authorship traces back to the parent org.
/// `System` is a virtual author (not a struct), so it doesn't implement this.
pub trait Authorable: Entity {
    fn author_type(&self) -> AuthorType;
}

// ---------------------------------------------------------------------------
// Implementations: User
// ---------------------------------------------------------------------------

use crate::user::User;

impl Entity for User {
    fn id(&self) -> Uuid {
        self.id
    }
    fn entity_kind(&self) -> EntityKind {
        EntityKind::User
    }
}

impl Taggable for User {}
impl FeedOwnable for User {}

impl Authorable for User {
    fn author_type(&self) -> AuthorType {
        AuthorType::User
    }
}

// ---------------------------------------------------------------------------
// Implementations: Organization
// ---------------------------------------------------------------------------

use crate::organization::Organization;

impl Entity for Organization {
    fn id(&self) -> Uuid {
        self.id
    }
    fn entity_kind(&self) -> EntityKind {
        EntityKind::Org
    }
}

impl Taggable for Organization {}
impl FeedOwnable for Organization {}

impl Authorable for Organization {
    fn author_type(&self) -> AuthorType {
        AuthorType::Org
    }
}

// ---------------------------------------------------------------------------
// Implementations: Feed
// ---------------------------------------------------------------------------

use crate::feed::Feed;

impl Entity for Feed {
    fn id(&self) -> Uuid {
        self.id
    }
    fn entity_kind(&self) -> EntityKind {
        EntityKind::Feed
    }
}

impl Taggable for Feed {}
impl FeedOwnable for Feed {}

// ---------------------------------------------------------------------------
// Implementations: Tag
// ---------------------------------------------------------------------------

use crate::tag::Tag;

impl Entity for Tag {
    fn id(&self) -> Uuid {
        self.id
    }
    fn entity_kind(&self) -> EntityKind {
        EntityKind::Tag
    }
}

impl Taggable for Tag {}
impl FeedOwnable for Tag {}

// ---------------------------------------------------------------------------
// Implementations: FeedItem
// ---------------------------------------------------------------------------

use crate::feed_item::FeedItem;

impl Entity for FeedItem {
    fn id(&self) -> Uuid {
        self.id
    }
    fn entity_kind(&self) -> EntityKind {
        EntityKind::FeedItem
    }
}

impl Taggable for FeedItem {}
impl FeedOwnable for FeedItem {}

// ---------------------------------------------------------------------------
// Implementations: FeedElement
// ---------------------------------------------------------------------------

use crate::feed_element::FeedElement;

impl Entity for FeedElement {
    fn id(&self) -> Uuid {
        self.id
    }
    fn entity_kind(&self) -> EntityKind {
        EntityKind::FeedElement
    }
}

impl Taggable for FeedElement {}
impl FeedOwnable for FeedElement {}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[test]
    fn entity_kind_round_trip() {
        let variants = [
            (EntityKind::User, "user"),
            (EntityKind::Org, "org"),
            (EntityKind::Character, "character"),
            (EntityKind::Commission, "commission"),
            (EntityKind::Feed, "feed"),
            (EntityKind::Tag, "tag"),
            (EntityKind::FeedItem, "feed_item"),
            (EntityKind::FeedElement, "feed_element"),
        ];
        for (variant, s) in variants {
            assert_eq!(variant.as_str(), s);
            assert_eq!(EntityKind::from_str(s), Some(variant));
        }
    }

    #[test]
    fn entity_kind_from_str_unknown() {
        assert_eq!(EntityKind::from_str("team"), None);
        assert_eq!(EntityKind::from_str(""), None);
    }

    fn test_user() -> User {
        User {
            id: Uuid::new_v4(),
            did: None,
            handle: None,
            email: None,
            username: "test".into(),
            onboarding_completed_at: None,
        }
    }

    fn test_org() -> Organization {
        Organization {
            id: Uuid::new_v4(),
            slug: "test-org".into(),
            display_name: None,
            is_personal: false,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    fn test_feed() -> Feed {
        Feed {
            id: Uuid::new_v4(),
            slug: "updates".into(),
            display_name: "Updates".into(),
            description: None,
            feed_type: crate::feed::FeedType::System,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            deleted_at: None,
        }
    }

    fn test_tag() -> Tag {
        use crate::tag::TagCategory;
        Tag {
            id: Uuid::new_v4(),
            category: TagCategory::General,
            name: "test-tag".into(),
            usage_count: 0,
            is_approved: false,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    fn test_feed_item() -> FeedItem {
        FeedItem {
            id: Uuid::new_v4(),
            feed_id: Uuid::new_v4(),
            author_type: AuthorType::User,
            author_id: Uuid::new_v4(),
            created_at: Utc::now(),
        }
    }

    fn test_feed_element() -> FeedElement {
        use crate::feed_element::FeedElementType;
        FeedElement {
            id: Uuid::new_v4(),
            feed_item_id: Uuid::new_v4(),
            element_type: FeedElementType::Text,
            content_json: "{}".into(),
            position: 0,
        }
    }

    #[test]
    fn user_implements_entity() {
        let u = test_user();
        assert_eq!(u.entity_kind(), EntityKind::User);
    }

    #[test]
    fn org_implements_entity() {
        let o = test_org();
        assert_eq!(o.entity_kind(), EntityKind::Org);
    }

    #[test]
    fn feed_implements_entity() {
        let f = test_feed();
        assert_eq!(f.entity_kind(), EntityKind::Feed);
    }

    #[test]
    fn tag_implements_entity() {
        let t = test_tag();
        assert_eq!(t.entity_kind(), EntityKind::Tag);
    }

    #[test]
    fn feed_item_implements_entity() {
        let fi = test_feed_item();
        assert_eq!(fi.entity_kind(), EntityKind::FeedItem);
    }

    #[test]
    fn feed_element_implements_entity() {
        let fe = test_feed_element();
        assert_eq!(fe.entity_kind(), EntityKind::FeedElement);
    }

    #[test]
    fn user_is_authorable() {
        let u = test_user();
        assert_eq!(u.author_type(), AuthorType::User);
    }

    #[test]
    fn org_is_authorable() {
        let o = test_org();
        assert_eq!(o.author_type(), AuthorType::Org);
    }

    #[test]
    fn taggable_validate_default_allows() {
        let u = test_user();
        assert!(u.validate_tag(Uuid::new_v4()).is_ok());
    }

    #[test]
    fn taggable_validate_untag_default_allows() {
        let u = test_user();
        assert!(u.validate_untag(Uuid::new_v4()).is_ok());
    }

    #[test]
    fn feed_ownable_validate_default_allows() {
        let o = test_org();
        assert!(o.validate_feed_creation().is_ok());
        assert!(o.validate_feed_deletion(Uuid::new_v4()).is_ok());
    }

    /// Compile-time check: functions accepting trait bounds work with correct entity types.
    #[test]
    fn trait_bounds_compile() {
        fn accepts_taggable(_e: &impl Taggable) {}
        fn accepts_feed_ownable(_e: &impl FeedOwnable) {}
        fn accepts_authorable(_e: &impl Authorable) {}

        let u = test_user();
        let o = test_org();
        let f = test_feed();
        let t = test_tag();
        let fi = test_feed_item();
        let fe = test_feed_element();

        // All entities are Taggable
        accepts_taggable(&u);
        accepts_taggable(&o);
        accepts_taggable(&f);
        accepts_taggable(&t);
        accepts_taggable(&fi);
        accepts_taggable(&fe);

        // All entities are FeedOwnable
        accepts_feed_ownable(&u);
        accepts_feed_ownable(&o);
        accepts_feed_ownable(&f);
        accepts_feed_ownable(&t);
        accepts_feed_ownable(&fi);
        accepts_feed_ownable(&fe);

        // Only User and Organization are Authorable
        accepts_authorable(&u);
        accepts_authorable(&o);
    }
}
