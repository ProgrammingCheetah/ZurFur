//! The orchestrator's factory: one [`Ports`] bag assembled by the composition
//! root, one [`App`] over it, and per-entity namespaces vended from it. Drivers
//! hold an [`App`] and pass `now`; they never see a port.

use std::sync::Arc;

use domain::ports::{
    AccountStore, ChangelogStore, CommissionStore, Database, DidMinter, FileStore, ProfileCache,
    ProfileSource, UserStore,
};

use crate::{account::Accounts, commission::Commissions, user::Users};

/// Every port the orchestrator may use, built once by the composition root.
/// Reads are pool-side (outside a unit); writes and in-unit reads come from
/// [`Database::begin`]. Optional entries are adapters a driver profile may lack
/// (a CLI without a blob store); a namespace that needs one fails to build
/// with [`MissingPort`] rather than at first use.
pub struct Ports {
    pub database: Arc<dyn Database>,
    pub users: Arc<dyn UserStore>,
    pub accounts: Arc<dyn AccountStore>,
    pub commissions: Arc<dyn CommissionStore>,
    pub changelog: Arc<dyn ChangelogStore>,
    pub profile_source: Arc<dyn ProfileSource>,
    pub profile_cache: Arc<dyn ProfileCache>,
    pub did_minter: Arc<dyn DidMinter>,
    pub files: Arc<dyn FileStore>,
}

/// The orchestrator, built once at composition. A namespace over [`Ports`];
/// each accessor vends the entity's use cases with the ports already bound.
pub struct App {
    ports: Ports,
}

impl App {
    /// Wrap the assembled ports.
    pub fn new(ports: Ports) -> Self {
        Self { ports }
    }

    /// The bag itself, for a not-yet-migrated call site that still builds a
    /// per-module `*Ports` view. Goes when the last one migrates.
    pub fn ports(&self) -> &Ports {
        &self.ports
    }

    /// Commission use cases. Panics if the composition root omitted the blob
    /// store — a boot-time misconfiguration, not a runtime path.
    pub fn commissions(&self) -> Commissions<'_> {
        Commissions::from(self)
    }

    /// Account use cases. Panics if the composition root omitted the DID minter.
    pub fn accounts(&self) -> Accounts<'_> {
        Accounts::from(self)
    }

    /// User use cases.
    pub fn users(&self) -> Users<'_> {
        Users::from(self)
    }
}

/// A namespace needs an optional port the composition root did not supply.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MissingPort {
    Files,
    DidMinter,
}

impl std::fmt::Display for MissingPort {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = match self {
            MissingPort::Files => "files",
            MissingPort::DidMinter => "did_minter",
        };
        write!(f, "missing port: {name}")
    }
}

impl std::error::Error for MissingPort {}
