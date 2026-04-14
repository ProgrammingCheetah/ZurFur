//! Shared helpers for route handlers: UUID parsing, user ID extraction, pagination.
//!
//! These are common utilities used across multiple route modules. Extracting
//! them here avoids cross-module imports and type duplication.

use serde::Deserialize;

use crate::error::AppError;

/// Default page size for paginated queries.
pub(super) const DEFAULT_PAGE_SIZE: i64 = 20;

/// Shared pagination parameters for list endpoints.
#[derive(Deserialize)]
pub(super) struct PaginationQuery {
    #[serde(default = "default_limit")]
    pub limit: i64,
    #[serde(default)]
    pub offset: i64,
}

fn default_limit() -> i64 {
    DEFAULT_PAGE_SIZE
}

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
