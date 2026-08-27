//! Use cases about [`Commission`](domain::elements::commission::Commission)s.
//!
//! The **deadline sweep** (ZMVP-86, conductor ruling E12) is the first of them —
//! and the one place the system acts on a commission rather than a Participant.
//! The deadline axis is otherwise entirely Participant-moved (the manual Delayed
//! flag, the deadline itself); the sweep's whole authority is: when a
//! commission's deadline has passed, say so in the changelog as a **system
//! entry** (no actor). It is provably scoped to exactly that — it calls
//! [`lapsed_deadlines`](domain::ports::CommissionWrites::lapsed_deadlines)
//! (which already excludes terminal lifecycles, already-Late commissions, and
//! anything without a deadline — AC4) and appends to the changelog; it holds no
//! handle that could move a Lifecycle or a direction status.
//!
//! [`sweep_deadlines`] is the whole policy. The wall-clock timer and the
//! advisory-lock leader election that drive it live in `api` — a driver
//! concern — so the policy stays deterministic and testable at an injected
//! instant.

use domain::{
    datetime::DateTimeUtc,
    elements::commission::{ChangelogEntryKind, NewChangelogEntry},
    ports::{ChangelogStore, CommissionStore, Database, DidMinter, UnitOfWork, UserStore},
};
use serde_json::json;

use crate::transaction;
pub mod archive;
pub mod changelog;
pub mod create;
pub mod deadline;
pub mod delete;
pub mod files;
pub mod unarchive;

pub(crate) mod getters;

pub use archive::archive;
pub use create::create;
pub use delete::delete;
pub use unarchive::unarchive;

pub struct CommissionPorts<'a> {
    pub commissions: &'a dyn CommissionStore,
    pub changelog: &'a dyn ChangelogStore,
    pub users: &'a dyn UserStore,
    pub did_minter: &'a dyn DidMinter,
    pub database: &'a dyn Database,
}

pub type CommissionResult<T> = Result<T, CommissionError>;

/// Why a commission use case could not answer. One enum per module: a driver
/// maps each variant to its own surface (problem+json, `{class, code}`).
///
/// `Display` is deliberately terse and never interpolates the cause — a store
/// error can carry SQL or constraint names, and a driver printing `{err}` must
/// not leak them. The cause stays on [`source`](std::error::Error::source) for
/// tracing.
#[derive(Debug)]
pub enum CommissionError {
    /// The commission store failed. The unit of work rolled back whole, so
    /// nothing was marked halfway; the caller may retry.
    Infrastructure(anyhow::Error),
    UserNotFound,
    CommissionNotFound,
    CommissionAlreadyAtState,
    InsufficientPermissions,
    NotAMember,
    InvalidStateRequested,
}

impl std::fmt::Display for CommissionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Infrastructure(_) => write!(f, "the commission store failed"),
            Self::UserNotFound => write!(f, "The user could not be found"),
            Self::CommissionNotFound => write!(f, "The commission could not be found"),
            Self::CommissionAlreadyAtState => write!(f, "This commission is already in this state"),
            Self::InsufficientPermissions => write!(f, "Insufficient permissions to do this"),
            Self::NotAMember => write!(f, "Not a member of this commission"),
            Self::InvalidStateRequested => write!(f, "The state couldn't get set"),
        }
    }
}

impl std::error::Error for CommissionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Infrastructure(e) => Some(e.as_ref()),
            _ => None,
        }
    }
}

/// What one [`sweep_deadlines`] pass did, as the drivers render it: how many
/// commissions this pass newly recorded as Late.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SweepResult {
    pub marked_late: usize,
}

/// Run **one** deadline sweep as of `now`.
///
/// A use case with no actor: `now` is injected (never read from a wall clock
/// here — the `datetime` doctrine, so the policy is deterministic by
/// construction) and the whole pass is **one unit of work** (ruling E12). The
/// candidate scan
/// ([`lapsed_deadlines`](domain::ports::CommissionWrites::lapsed_deadlines) —
/// deadline passed, not already Late, lifecycle not terminal) and each matching
/// **system** changelog entry (actor `NULL`, payload naming the missed
/// `deadline` and the standing flag — `delayed` or null — it replaced) commit
/// atomically or roll back together, so a swept commission without its Late
/// entry is unrepresentable (Changelog DD D4). A standing manual Delayed
/// upgrades to Late here (Engineer ruling 2026-07-05); a commission already
/// Late is never re-marked or re-logged — the *next* entry for the same
/// commission takes a fresh deadline miss (extend, then miss again).
///
/// A commission with no deadline never receives a deadline-axis value (AC4):
/// the scan cannot return one.
pub async fn sweep_deadlines(
    database: &dyn Database,
    now: DateTimeUtc,
) -> Result<SweepResult, CommissionError> {
    let marked_late = transaction(database, async move |uow: &mut dyn UnitOfWork| {
        let lapsed = uow.commissions().lapsed_deadlines(now).await?;
        for lapse in &lapsed {
            // Log-only: `Late` is derived on lookup and never persisted
            // (Engineer ruling 2026-07-08). This pass just records the
            // transition once, so hooks/plugins have an event to consume.
            let entry = NewChangelogEntry::system(
                lapse.id,
                ChangelogEntryKind::Late,
                json!({
                    "deadline": lapse.deadline,
                    "from": lapse.status.map(|s| s.as_str()),
                }),
                now,
            );
            uow.changelog().append(&entry).await?;
        }
        Ok(lapsed.len())
    })
    .await
    .map_err(CommissionError::Infrastructure)?;

    Ok(SweepResult { marked_late })
}
