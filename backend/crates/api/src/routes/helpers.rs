//! Shared helpers for route handlers: UUID parsing, user ID extraction.
//!
//! These are common utilities used across multiple route modules. Extracting
//! them here avoids cross-module imports like `super::organizations::parse_uuid`.

use crate::error::AppError;

/// Parse a string as a UUID, returning a 400 Bad Request on failure.
pub(super) fn parse_uuid(s: &str) -> Result<uuid::Uuid, AppError> {
    s.parse()
        .map_err(|_| AppError::BadRequest(format!("Invalid UUID: {s}")))
}

/// Parse the `sub` claim from a JWT as a UUID.
pub(super) fn parse_user_id(sub: &str) -> Result<uuid::Uuid, AppError> {
    sub.parse()
        .map_err(|_| AppError::BadRequest("Invalid user ID in token".into()))
}
