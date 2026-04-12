//! Organization domain concept and module.
//!
//! An **Organization** is the universal identity container in Zurfur. Every user
//! gets a personal org on first login — this personal org IS the user's public
//! profile. All roles, titles, bios, and capabilities are expressed through org
//! membership, not through fields on the User entity.
//!
//! **Responsibilities:**
//! - Org CRUD (create, read, update, soft-delete)
//! - Member management (add, update role/title/permissions, remove)
//! - Profile management (bio, commission status)
//! - Slug validation and reserved word enforcement
//! - Permission checks (owner-only, MANAGE_PROFILE, MANAGE_MEMBERS, etc.)
//!
//! **Key invariants:**
//! - Every user has exactly one personal org (`is_personal = true`)
//! - Personal orgs cannot be deleted
//! - Owners cannot be removed from their own org
//! - Slug must be unique among non-deleted orgs (enforced at DB level)
//! - `display_name` is NULL for personal orgs (resolved from owner's handle at API layer)
//!
//! **Relationships:**
//! - User → Organization (via OrganizationMember)
//! - Organization → OrganizationProfile (1:1 optional)
//! - Organization → Feeds (future, via entity_feeds)
//! - Organization → Characters (future, scoped to org)

pub mod service;
