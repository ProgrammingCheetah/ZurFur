use chrono::{DateTime, Utc};
use uuid::Uuid;

/// Commission availability status for an organization.
///
/// ARCHITECTURE DECISIONS:
///   Stored as TEXT + CHECK constraint in PostgreSQL (not a DB enum) because this
///   set is likely to grow as the platform evolves (e.g., 'paused', 'by_request').
///   Adding a TEXT value only requires updating the CHECK constraint, whereas
///   adding a PG enum value requires ALTER TYPE which has limitations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommissionStatus {
    Open,
    Closed,
    Waitlist,
}

impl CommissionStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            CommissionStatus::Open => "open",
            CommissionStatus::Closed => "closed",
            CommissionStatus::Waitlist => "waitlist",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "open" => Some(CommissionStatus::Open),
            "closed" => Some(CommissionStatus::Closed),
            "waitlist" => Some(CommissionStatus::Waitlist),
            _ => None,
        }
    }
}

/// An organization's public profile — bio and commission status.
///
/// ARCHITECTURE DECISIONS:
///   This is a separate table (not columns on `organizations`) because not every
///   org needs a profile. The row is created on first update, not on org creation.
///   This keeps the organizations table lean for orgs that are purely structural
///   (e.g., friend groups with no commission activity).
#[derive(Debug, Clone)]
pub struct OrganizationProfile {
    pub org_id: Uuid,
    pub bio: Option<String>,
    pub commission_status: CommissionStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, thiserror::Error)]
pub enum OrganizationProfileError {
    #[error("Organization profile not found")]
    NotFound,
    #[error("Invalid commission status: {0}")]
    InvalidStatus(String),
    #[error("Database error: {0}")]
    Database(String),
}

#[async_trait::async_trait]
pub trait OrganizationProfileRepository: Send + Sync {
    async fn upsert(
        &self,
        org_id: Uuid,
        bio: Option<&str>,
        commission_status: CommissionStatus,
    ) -> Result<OrganizationProfile, OrganizationProfileError>;

    async fn find_by_org_id(
        &self,
        org_id: Uuid,
    ) -> Result<Option<OrganizationProfile>, OrganizationProfileError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_all_variants() {
        for (variant, expected) in [
            (CommissionStatus::Open, "open"),
            (CommissionStatus::Closed, "closed"),
            (CommissionStatus::Waitlist, "waitlist"),
        ] {
            assert_eq!(variant.as_str(), expected);
            assert_eq!(CommissionStatus::from_str(expected), Some(variant));
        }
    }

    #[test]
    fn from_str_returns_none_for_unknown() {
        assert_eq!(CommissionStatus::from_str("paused"), None);
        assert_eq!(CommissionStatus::from_str(""), None);
        assert_eq!(CommissionStatus::from_str("OPEN"), None);
    }
}
