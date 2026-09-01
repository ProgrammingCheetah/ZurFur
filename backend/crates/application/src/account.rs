//! Use cases about [`Account`]s.

use domain::{
    datetime::DateTimeUtc,
    elements::{
        account::{Account, AccountId, AccountName, ListingScope},
        did::Did,
        handle::{Handle, HandleDomain},
        invitation::{Invitation, InvitationId, InvitationState},
        role::{Role, RoleAlias},
        user::{User, UserId},
        user_account::UserAccount,
    },
    ports::{
        AccountStore, Database, DidBelongsToAnotherActor, DidMinter, HandleTaken, UnitOfWork,
        UserStore,
    },
};
use shared::settings::{HANDLE_CHANGE_LIMIT, HANDLE_CHANGE_WINDOW, HANDLE_QUARANTINE_WINDOW};

use crate::{ports::WithPorts, transaction};

pub mod change_handle;
pub mod create;
pub mod delete;
pub mod facts;
pub mod invitation;
pub mod leave;
pub mod list;
pub mod role;
pub mod transfer_ownership;
/// Why an account use case could not answer. One enum per module: a driver
/// maps each variant to its own surface (problem+json, `{class, code}`).
///
/// `Display` is deliberately terse and never interpolates the cause — a
/// store error can carry SQL, constraint names or custody paths, and a
/// driver printing `{err}` must not leak them. The cause stays on
/// [`source`](std::error::Error::source) for tracing.

/// Account use cases, with the ports already bound. A namespace, not a
/// mediator: one `impl Accounts<'_>` block per use-case file, one use case each.
#[derive(Clone, Copy)]
pub struct Accounts<'a> {
    ports: &'a crate::Ports,
    did_minter: &'a dyn DidMinter,
}

impl<'a> WithPorts<'a> for Accounts<'a> {
    fn ports(&self) -> &'a crate::Ports {
        self.ports
    }
}

impl<'a> Accounts<'a> {
    /// Bind the namespace to resolved dependencies.
    pub fn new(ports: &'a crate::Ports, did_minter: &'a dyn DidMinter) -> Self {
        Self { ports, did_minter }
    }

    /// The bag this namespace was built over.
    pub fn ports(&self) -> &'a crate::Ports {
        self.ports
    }

    /// The did:plc minter.
    pub fn did_minter(&self) -> &'a dyn DidMinter {
        self.did_minter
    }
}

impl<'a> TryFrom<&'a crate::Ports> for Accounts<'a> {
    type Error = crate::MissingPort;

    /// Fails when the bag carries no DID minter.
    fn try_from(ports: &'a crate::Ports) -> Result<Self, Self::Error> {
        let did_minter = ports.did_minter.as_ref();
        Ok(Self::new(ports, did_minter))
    }
}

impl<'a> From<&'a crate::App> for Accounts<'a> {
    /// Panics on a bag without a DID minter — the composition root's contract.
    fn from(app: &'a crate::App) -> Self {
        Self::try_from(app.ports()).expect("composition root supplies the DID minter")
    }
}

#[derive(Debug)]
pub enum AccountError {
    /// The handle is claimed — by a live account, a tombstoned one (the
    /// global unique index, DD `23003138`), or quarantined to the account
    /// that vacated it (DD `27852802` §4).
    HandleTaken,
    /// The handle's namespace isn't supported for this operation — e.g.
    /// changing *to* a brought (BYO) domain, deferred until bidirectional
    /// verify-before-commit ships (DD `27852802` §6).
    UnsupportedHandle,
    /// A port (the did:plc minter, the account store) failed; nothing was
    /// persisted, the caller may retry.
    Infrastructure(anyhow::Error),
    /// The actor's role on the account doesn't carry the authority this use
    /// case needs — including holding no role at all (a non-member).
    IncorrectRole,
    /// The account named by the command is not a live account: unknown, or
    /// already soft-deleted (DD `23003138`). This may also refer to a non-existent user.
    AccountNotFound,
    /// The target handle is already the account's current one.
    HandleUnchanged,
    /// The account has changed its handle too often recently (DD `27852802` §3).
    RenamedTooRecently,
    /// The actor holds no pending invitation into the account.
    NoPendingInvitation,
    /// The actor holds no membership in the account.
    NotAMember,
    /// The Owner can't leave while still Owner — transfer or delete first.
    OwnerCannotLeave,
    IncorrectTransferOfAccount,
    /// The grantee's DID is interned as a non-User actor (an Account, a Character).
    DidBelongsToAnotherActor,
    AlreadyMember,
    CannotTransferToSelf,
    UserNotFound,
    InvitationAlreadyPending,
}

impl std::fmt::Display for AccountError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AccountError::HandleTaken => write!(f, "{HandleTaken}"),
            AccountError::UnsupportedHandle => write!(
                f,
                "This handle's namespace isn't supported for this operation"
            ),
            AccountError::Infrastructure(_) => write!(f, "an infrastructure port failed"),
            AccountError::IncorrectRole => write!(f, "This role can't perform this action"),
            AccountError::AccountNotFound => write!(f, "This account does not exist"),
            AccountError::HandleUnchanged => write!(f, "Nothing to do"),
            AccountError::RenamedTooRecently => write!(f, "Rate limited"),
            AccountError::NoPendingInvitation => write!(f, "No pending invitation"),
            AccountError::NotAMember => write!(f, "Not a member of this account"),
            AccountError::OwnerCannotLeave => {
                write!(
                    f,
                    "The Owner can't leave; transfer or delete the account first"
                )
            }
            AccountError::IncorrectTransferOfAccount => {
                write!(f, "Owner can't be granted; transfer ownership instead")
            }
            AccountError::DidBelongsToAnotherActor => {
                write!(f, "That DID belongs to another kind of actor")
            }
            AccountError::AlreadyMember => write!(f, "Already a member"),
            AccountError::CannotTransferToSelf => write!(f, "Already the owner"),
            Self::UserNotFound => write!(f, "User not found!"),
            Self::InvitationAlreadyPending => write!(f, "An invitation was already pending"),
        }
    }
}

impl std::error::Error for AccountError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            AccountError::HandleTaken
            | AccountError::UnsupportedHandle
            | AccountError::IncorrectRole
            | AccountError::AccountNotFound
            | AccountError::RenamedTooRecently
            | AccountError::HandleUnchanged
            | AccountError::NoPendingInvitation
            | AccountError::NotAMember
            | AccountError::OwnerCannotLeave
            | AccountError::IncorrectTransferOfAccount
            | AccountError::DidBelongsToAnotherActor
            | AccountError::AlreadyMember
            | AccountError::CannotTransferToSelf
            | Self::InvitationAlreadyPending
            | AccountError::UserNotFound => None,
            AccountError::Infrastructure(e) => Some(e.as_ref()),
        }
    }
}

impl From<anyhow::Error> for AccountError {
    fn from(err: anyhow::Error) -> Self {
        Self::Infrastructure(err)
    }
}

/// The ports the account use cases reach: reads off [`AccountStore`], the
/// account's sovereign identity off [`DidMinter`], writes through a unit of
/// work vended by [`Database`]. Built by each driver off its runtime.
pub struct AccountPorts<'a> {
    pub accounts: &'a dyn AccountStore,
    pub users: &'a dyn UserStore,
    pub did_minter: &'a dyn DidMinter,
    pub database: &'a dyn Database,
}

pub type AccountResult<T> = Result<T, AccountError>;
