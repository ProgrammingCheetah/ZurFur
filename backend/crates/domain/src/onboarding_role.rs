//! OnboardingRole — first-login role selection enum.
//!
//! ARCHITECTURE DECISIONS:
//!   This is a domain enum only (no table). The onboarding wizard uses it to
//!   determine which system feeds to auto-create on the user's personal org.
//!   Artist/CrafterMaker trigger commissions feed creation, while
//!   CommissionerClient/CoderDeveloper do not.

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

    /// Whether this role triggers creation of a commissions feed on the
    /// user's personal org.
    pub fn creates_commissions_feed(&self) -> bool {
        matches!(self, OnboardingRole::Artist | OnboardingRole::CrafterMaker)
    }
}

impl From<OnboardingRole> for &'static str {
    fn from(role: OnboardingRole) -> Self {
        role.as_str()
    }
}

impl TryFrom<&str> for OnboardingRole {
    type Error = String;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        OnboardingRole::from_str(s).ok_or_else(|| format!("Unknown onboarding role: {s}"))
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
    fn artist_roles_create_commissions_feed() {
        assert!(OnboardingRole::Artist.creates_commissions_feed());
        assert!(OnboardingRole::CrafterMaker.creates_commissions_feed());
        assert!(!OnboardingRole::CommissionerClient.creates_commissions_feed());
        assert!(!OnboardingRole::CoderDeveloper.creates_commissions_feed());
    }
}
