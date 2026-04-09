//! User domain concept and module.
//!
//! The **User** is atomic — it holds only authentication identity data (DID,
//! handle, email, username). All public-facing identity (roles, bios, titles,
//! capabilities) lives on Organizations, not on User.
//!
//! **Responsibilities:**
//! - User profile assembly (user + personal org + memberships)
//! - Content rating preferences (SFW/Questionable/NSFW viewer control)
//!
//! **Key invariants:**
//! - User never gains feature flags or role fields — meaning comes from org membership
//! - Preferences are per-user, not per-org

pub mod service;
