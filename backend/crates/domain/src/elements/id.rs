//! [`IdError`] — the shared parse failure for every UUID-backed domain id, and
//! the private helper their `FromStr` impls delegate to.

use std::str::FromStr;

/// Why a string failed to parse as a UUID-backed domain id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IdError {
    /// The input isn't a valid UUID.
    NotAUuid,
    ParsingError,
}

impl std::fmt::Display for IdError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotAUuid => write!(f, "Value not a UUID"),
            Self::ParsingError => write!(f, "Parsing Error"),
        }
    }
}

impl std::error::Error for IdError {}

/// Parses `s` as a UUID, mapping any failure to [`IdError::NotAUuid`] — the one
/// helper every UUID-backed id newtype's `FromStr` delegates to.
pub(crate) fn parse_uuid(s: &str) -> Result<uuid::Uuid, IdError> {
    uuid::Uuid::from_str(s).map_err(|_| IdError::NotAUuid)
}
