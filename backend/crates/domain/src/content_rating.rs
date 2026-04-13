/// Content rating for user-generated content across the platform.
///
/// ARCHITECTURE DECISIONS:
///   Defined in domain (not shared) because it is a business concept tied to
///   content entities, not a utility. The shared crate is reserved for
///   infrastructure (JWT, config).
///
///   Ordered lowest-to-highest so that `viewer_max >= content_rating` comparisons
///   work naturally via the derived PartialOrd/Ord.
///
///   Backed by a PostgreSQL ENUM type (`content_rating`) rather than TEXT + CHECK
///   because the set is stable and unlikely to change. Adding a variant is still
///   possible via `ALTER TYPE content_rating ADD VALUE`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ContentRating {
    Sfw,
    Questionable,
    Nsfw,
}

impl ContentRating {
    /// Returns the string representation matching the database value.
    pub fn as_str(&self) -> &'static str {
        match self {
            ContentRating::Sfw => "sfw",
            ContentRating::Questionable => "questionable",
            ContentRating::Nsfw => "nsfw",
        }
    }

    /// Parse from a database string value. Returns `None` for unknown values.
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "sfw" => Some(ContentRating::Sfw),
            "questionable" => Some(ContentRating::Questionable),
            "nsfw" => Some(ContentRating::Nsfw),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_all_variants() {
        for (variant, expected) in [
            (ContentRating::Sfw, "sfw"),
            (ContentRating::Questionable, "questionable"),
            (ContentRating::Nsfw, "nsfw"),
        ] {
            assert_eq!(variant.as_str(), expected);
            assert_eq!(ContentRating::from_str(expected), Some(variant));
        }
    }

    #[test]
    fn from_str_returns_none_for_unknown() {
        assert_eq!(ContentRating::from_str("extreme"), None);
        assert_eq!(ContentRating::from_str(""), None);
        assert_eq!(ContentRating::from_str("SFW"), None);
    }

    #[test]
    fn ordering_sfw_less_than_questionable_less_than_nsfw() {
        assert!(ContentRating::Sfw < ContentRating::Questionable);
        assert!(ContentRating::Questionable < ContentRating::Nsfw);
        assert!(ContentRating::Sfw < ContentRating::Nsfw);
    }
}
