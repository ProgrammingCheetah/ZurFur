//! Authentication domain concept and module.
//!
//! Handles AT Protocol OAuth authentication: identity resolution, PAR + PKCE +
//! DPoP flow, token exchange, JWT issuance, refresh token rotation, and logout.
//!
//! **Responsibilities:**
//! - OAuth login flow orchestration (start → callback → tokens)
//! - Zurfur JWT issuance and verification (HS256, 15-min default TTL)
//! - Refresh token creation and single-use rotation (SHA-256 hashed before storage)
//! - AT Protocol session storage (access/refresh tokens for Bluesky API calls)
//! - Personal org auto-creation on first login
//! - Public JWK derivation for OAuth client metadata

pub mod login;
pub mod service;
