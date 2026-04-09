//! OnboardingRole — first-login role selection enum.
//!
//! ARCHITECTURE DECISIONS:
//!   This is a domain enum only (no table). The onboarding endpoint maps
//!   these to org_members.role values using the default_roles lookup.
//!   artist/crafter_maker map to the "artist" default role, while
//!   commissioner_client/coder_developer map to the "member" default role.
//!   The selection also determines whether a commissions feed is auto-created.

/// Role selected during the onboarding wizard on first login.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OnboardingRole {
    Artist,
    CrafterMaker,
    CommissionerClient,
    CoderDeveloper,
}

impl OnboardingRole {
    pub fn as_str(&self) -> &'static str {
        match self {
            OnboardingRole::Artist => "artist",
            OnboardingRole::CrafterMaker => "crafter_maker",
            OnboardingRole::CommissionerClient => "commissioner_client",
            OnboardingRole::CoderDeveloper => "coder_developer",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "artist" => Some(OnboardingRole::Artist),
            "crafter_maker" => Some(OnboardingRole::CrafterMaker),
            "commissioner_client" => Some(OnboardingRole::CommissionerClient),
            "coder_developer" => Some(OnboardingRole::CoderDeveloper),
            _ => None,
        }
    }

    /// Maps the onboarding selection to the default_roles.name used for
    /// initial org_members.role assignment.
    pub fn default_role_name(&self) -> &'static str {
        match self {
            OnboardingRole::Artist | OnboardingRole::CrafterMaker => "artist",
            OnboardingRole::CommissionerClient | OnboardingRole::CoderDeveloper => "member",
        }
    }

    /// Whether this role triggers creation of a commissions feed on the
    /// user's personal org.
    pub fn creates_commissions_feed(&self) -> bool {
        matches!(self, OnboardingRole::Artist | OnboardingRole::CrafterMaker)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn onboarding_role_round_trip() {
        let variants = [
            (OnboardingRole::Artist, "artist"),
            (OnboardingRole::CrafterMaker, "crafter_maker"),
            (OnboardingRole::CommissionerClient, "commissioner_client"),
            (OnboardingRole::CoderDeveloper, "coder_developer"),
        ];
        for (variant, s) in variants {
            assert_eq!(variant.as_str(), s);
            assert_eq!(OnboardingRole::from_str(s), Some(variant));
        }
    }

    #[test]
    fn onboarding_role_from_str_returns_none_for_unknown() {
        assert_eq!(OnboardingRole::from_str("viewer"), None);
        assert_eq!(OnboardingRole::from_str(""), None);
    }

    #[test]
    fn artist_and_crafter_map_to_artist_role() {
        assert_eq!(OnboardingRole::Artist.default_role_name(), "artist");
        assert_eq!(OnboardingRole::CrafterMaker.default_role_name(), "artist");
    }

    #[test]
    fn commissioner_and_coder_map_to_member_role() {
        assert_eq!(OnboardingRole::CommissionerClient.default_role_name(), "member");
        assert_eq!(OnboardingRole::CoderDeveloper.default_role_name(), "member");
    }

    #[test]
    fn artist_roles_create_commissions_feed() {
        assert!(OnboardingRole::Artist.creates_commissions_feed());
        assert!(OnboardingRole::CrafterMaker.creates_commissions_feed());
        assert!(!OnboardingRole::CommissionerClient.creates_commissions_feed());
        assert!(!OnboardingRole::CoderDeveloper.creates_commissions_feed());
    }
}
