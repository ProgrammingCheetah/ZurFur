//! Use cases about [`Account`]s.

use domain::{
    datetime::DateTimeUtc,
    elements::{
        account::{Account, AccountId, AccountName, ListingScope},
        did::Did,
        handle::{Handle, HandleDomain},
        invitation::{Invitation, InvitationId, InvitationState},
        role::{Role, RoleAlias},
        user::UserId,
        user_account::UserAccount,
    },
    ports::{
        AccountStore, Database, DidBelongsToAnotherActor, DidMinter, HandleTaken, UnitOfWork,
        UserStore,
    },
};
use shared::settings::{HANDLE_CHANGE_LIMIT, HANDLE_CHANGE_WINDOW, HANDLE_QUARANTINE_WINDOW};

use crate::transaction;

/// Why an account use case could not answer. One enum per module: a driver
/// maps each variant to its own surface (problem+json, `{class, code}`).
///
/// `Display` is deliberately terse and never interpolates the cause — a
/// store error can carry SQL, constraint names or custody paths, and a
/// driver printing `{err}` must not leak them. The cause stays on
/// [`source`](std::error::Error::source) for tracing.
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
    NonExistent,
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
            AccountError::NonExistent => write!(f, "This account does not exist"),
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
        }
    }
}

impl std::error::Error for AccountError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            AccountError::HandleTaken
            | AccountError::UnsupportedHandle
            | AccountError::IncorrectRole
            | AccountError::NonExistent
            | AccountError::RenamedTooRecently
            | AccountError::HandleUnchanged
            | AccountError::NoPendingInvitation
            | AccountError::NotAMember
            | AccountError::OwnerCannotLeave
            | AccountError::IncorrectTransferOfAccount
            | AccountError::DidBelongsToAnotherActor
            | AccountError::AlreadyMember
            | AccountError::CannotTransferToSelf => None,
            AccountError::Infrastructure(e) => Some(e.as_ref()),
        }
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListAccountsQuery {
    pub actor: UserId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountListing {
    pub id: AccountId,
    pub did: Did,
    pub handle: Handle,
    pub name: AccountName,
    pub role: Role, // This is fine because it IS domain, but it is ONLY data (enum)
    /// The caller's own alias for [`role`](Self::role) on this account, if
    /// they set one — carried on the membership, not on [`Role`] itself.
    pub alias: Option<RoleAlias>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListAccountsResult {
    pub accounts: Vec<AccountListing>,
}

pub async fn list_accounts(
    query: ListAccountsQuery,
    ports: &AccountPorts<'_>,
) -> Result<ListAccountsResult, AccountError> {
    let memberships = ports
        .accounts
        .list_for_user(query.actor, ListingScope::SelfView)
        .await
        .map_err(AccountError::Infrastructure)?;

    let accounts = memberships
        .into_iter()
        .map(|m| AccountListing {
            id: m.account.id,
            did: m.account.did,
            handle: m.account.handle,
            name: m.account.name,
            role: m.role,
            alias: m.alias,
        })
        .collect();

    Ok(ListAccountsResult { accounts })
}

/// `create_account`'s input: the founder and the account's chosen name and
/// handle, both already validated by their newtypes at the driver's boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateAccountCommand {
    /// The founder. Precondition: a user the store recognizes — the driver
    /// authenticated them (session, identity file). An unrecognized id fails
    /// the Owner membership write as [`AccountError::Infrastructure`], not a
    /// typed variant.
    pub actor: UserId,
    pub name: AccountName,
    pub handle: Handle,
}

/// The founded account, as the drivers render it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateAccountResult {
    pub account_id: AccountId,
    pub did: Did,
    pub handle: Handle,
    pub name: AccountName,
}

/// Found a new Account for `actor` and make them its Owner (one per call, never
/// idempotent). Pre-checks the handle (live claim, then Zurfur-namespace
/// quarantine), mints the `did:plc`, then commits account + Owner membership in
/// one unit of work; the unique index surfaces any race as `HandleTaken`.
///
/// Errors: `HandleTaken`, `Infrastructure`.
pub async fn create_account(
    command: CreateAccountCommand,
    ports: AccountPorts<'_>,
    handle_domain: &HandleDomain,
    now: DateTimeUtc,
) -> Result<CreateAccountResult, AccountError> {
    let live_claim = ports
        .accounts
        .find_did_by_handle(&command.handle)
        .await
        .map_err(AccountError::Infrastructure)?;
    if live_claim.is_some() {
        return Err(AccountError::HandleTaken);
    }

    if command.handle.is_in_namespace(handle_domain) {
        let quarantined = ports
            .accounts
            .handle_reserved_for_other(&command.handle, None, now - HANDLE_QUARANTINE_WINDOW)
            .await
            .map_err(AccountError::Infrastructure)?;
        if quarantined {
            return Err(AccountError::HandleTaken);
        }
    }

    let did = ports
        .did_minter
        .mint(&command.handle)
        .await
        .map_err(AccountError::Infrastructure)?;

    let (account, owner) = Account::open(command.actor, did, command.handle, command.name, now);
    // The `async move` closure owns what it writes and hands the committed
    // account back out for the result.
    let account = transaction(ports.database, async move |uow: &mut dyn UnitOfWork| {
        uow.accounts().create(&account, &owner).await?;
        Ok(account)
    })
    .await
    .map_err(|err| {
        if err.downcast_ref::<HandleTaken>().is_some() {
            AccountError::HandleTaken
        } else {
            AccountError::Infrastructure(err)
        }
    })?;

    Ok(CreateAccountResult {
        account_id: account.id,
        did: account.did,
        handle: account.handle,
        name: account.name,
    })
}

/// Whether an account holds any account-anchored fact (DD `23003138`) — the seam
/// deciding soft-vs-hard deletion. Constant `false` while no fact store exists;
/// the compile guards in `api::routes::accounts` / `adapter_pg::account` break the
/// build when the first fact table is registered, forcing a real query here.
pub async fn account_has_facts(_account_id: AccountId) -> Result<bool, AccountError> {
    Ok(false)
}

/// `delete_account`'s input: who is deleting, and which account.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeleteAccountCommand {
    /// The acting user, already authenticated by the driver. Authority is
    /// checked here: they must hold `Owner` on [`account_id`](Self::account_id).
    pub actor: UserId,
    pub account_id: AccountId,
}

/// Which deletion happened — the account's fact-bearing state decided it, and the
/// drivers render it (the wire says, the interface renders; Engineer ruling
/// 2026-07-25).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeleteOutcome {
    /// The account held facts: the row is kept, the handle stays reserved and the
    /// `did:plc` stays live.
    Soft,
    /// The account was fact-free: the row is gone, its handle freed, and its
    /// `did:plc` tombstoned as a separate retryable step.
    Hard,
}

/// The settled deletion, as the drivers render it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeleteAccountResult {
    pub outcome: DeleteOutcome,
}

/// Delete an account as its Owner: soft (row kept, handle reserved, DID live)
/// when it holds account-anchored facts, hard (handle freed, DID tombstoned as a
/// separate retryable step) when empty — DD `23003138`.
///
/// Errors: `NonExistent`, `IncorrectRole` (not the Owner), `Infrastructure`.
pub async fn delete_account(
    command: DeleteAccountCommand,
    ports: AccountPorts<'_>,
) -> Result<DeleteAccountResult, AccountError> {
    let Some(account) = ports
        .accounts
        .find(command.account_id)
        .await
        .map_err(AccountError::Infrastructure)?
    else {
        return Err(AccountError::NonExistent);
    };
    let Some(role) = ports
        .accounts
        .role_of(command.actor, account.id)
        .await
        .map_err(AccountError::Infrastructure)?
    else {
        return Err(AccountError::IncorrectRole);
    };

    // Owner-only (DD 23003138), the rank rule above the membership floor:
    // deletion is the Owner's alone, unlike grant/revoke which any sufficiently
    // ranked member reaches.
    if !matches!(role, Role::Owner) {
        return Err(AccountError::IncorrectRole);
    }

    // Both facts leave `account` before the branches: each `async move` closure
    // captures what it names, so naming `account` inside one would move the whole
    // struct into that future — taking the `did` the tombstone step still needs
    // with it.
    let account_id = account.id;
    let did = account.did;

    let outcome = if account_has_facts(account_id).await? {
        transaction(ports.database, async move |uow: &mut dyn UnitOfWork| {
            uow.accounts().soft_delete(account_id).await
        })
        .await
        .map_err(AccountError::Infrastructure)?;
        DeleteOutcome::Soft
    } else {
        transaction(ports.database, async move |uow: &mut dyn UnitOfWork| {
            uow.accounts().hard_delete(account_id).await
        })
        .await
        .map_err(AccountError::Infrastructure)?;

        // The private hard-delete above already freed the handle. Tombstoning the
        // account's `did:plc` is a separate, retryable **public** step — never a
        // cross-store transaction with the private delete (the mint path's
        // mirror). A failure here does not undo the delete — the account is gone
        // and its handle freed — so we log and still report success rather than
        // resurrecting a deleted account; the tombstone is re-submittable, and a
        // higher-authority key can reverse it within the native ~72h window.
        if let Err(err) = ports.did_minter.tombstone(&did).await {
            tracing::warn!(
                error = ?err,
                did = %did.as_str(),
                "did:plc tombstone failed after hard delete; the PLC recovery window still applies"
            );
        }
        DeleteOutcome::Hard
    };

    Ok(DeleteAccountResult { outcome })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangeHandleCommand {
    pub account_id: AccountId,
    pub actor: UserId,
    pub handle: Handle,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangeHandleResult {
    pub id: AccountId,
    pub handle: Handle,
    pub did: Did,
    pub name: AccountName,
}
pub async fn change_handle(
    command: ChangeHandleCommand,
    ports: AccountPorts<'_>,
    handle_domain: &HandleDomain,
    now: DateTimeUtc,
) -> Result<ChangeHandleResult, AccountError> {
    let Some(account) = ports
        .accounts
        .find(command.account_id)
        .await
        .map_err(AccountError::Infrastructure)?
    else {
        return Err(AccountError::NonExistent);
    };
    let Some(role) = ports
        .accounts
        .role_of(command.actor, account.id)
        .await
        .map_err(AccountError::Infrastructure)?
    else {
        return Err(AccountError::IncorrectRole);
    };
    // Owner-only (DD 27852802 §2), mirroring `delete_account`'s rank rule.
    if !matches!(role, Role::Owner) {
        return Err(AccountError::IncorrectRole);
    }

    if command.handle == account.handle {
        return Err(AccountError::HandleUnchanged);
    }

    if !command.handle.is_in_namespace(handle_domain) {
        return Err(AccountError::UnsupportedHandle);
    }

    if ports
        .accounts
        .count_handle_changes_since(account.id, now - HANDLE_CHANGE_WINDOW)
        .await
        .map_err(AccountError::Infrastructure)?
        >= HANDLE_CHANGE_LIMIT
    {
        return Err(AccountError::RenamedTooRecently);
    }

    if ports
        .accounts
        .find_did_by_handle(&command.handle)
        .await
        .map_err(AccountError::Infrastructure)?
        .is_some()
    {
        return Err(AccountError::HandleTaken);
    }

    if ports
        .accounts
        .handle_reserved_for_other(
            &command.handle,
            Some(account.id),
            now - HANDLE_QUARANTINE_WINDOW,
        )
        .await
        .map_err(AccountError::Infrastructure)?
    {
        return Err(AccountError::HandleTaken);
    }

    ports
        .did_minter
        .update_handle(&account.did, &command.handle)
        .await
        .map_err(AccountError::Infrastructure)?;

    let old_handle = account.handle.clone();
    let account_id = account.id;
    let new_handle = command.handle.clone();

    if let Err(err) = transaction(ports.database, async move |uow: &mut dyn UnitOfWork| {
        uow.accounts()
            .change_handle(account_id, &old_handle, &new_handle, now)
            .await
    })
    .await
    {
        if err.downcast_ref::<HandleTaken>().is_some() {
            return Err(AccountError::HandleTaken);
        }
        return Err(AccountError::Infrastructure(err));
    }

    Ok(ChangeHandleResult {
        id: account.id,
        did: account.did,
        handle: command.handle,
        name: account.name,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcceptInvitationCommand {
    pub invited_actor: UserId,
    pub into_account: AccountId,
    pub listed_on_profile: bool,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcceptInvitationResult {
    pub account_id: AccountId,
    pub role: Role,
    pub user_did: Did,
}
pub async fn accept_invitation(
    command: AcceptInvitationCommand,
    ports: AccountPorts<'_>,
) -> Result<AcceptInvitationResult, AccountError> {
    let Some(invited_user) = ports
        .users
        .find(command.invited_actor)
        .await
        .map_err(AccountError::Infrastructure)?
    else {
        return Err(AccountError::NonExistent);
    };

    let Some(into_account) = ports
        .accounts
        .find(command.into_account)
        .await
        .map_err(AccountError::Infrastructure)?
    else {
        return Err(AccountError::NonExistent);
    };

    let Some(invitation) = ports
        .accounts
        .find_pending_invitation(into_account.id, invited_user.id)
        .await
        .map_err(AccountError::Infrastructure)?
    else {
        return Err(AccountError::NoPendingInvitation);
    };

    let invitation_result = transaction(ports.database, async move |uow: &mut dyn UnitOfWork| {
        uow.accounts()
            .accept_invitation(invitation, command.listed_on_profile)
            .await
    })
    .await
    .map(|user_account| AcceptInvitationResult {
        account_id: user_account.account_id,
        role: user_account.role,
        user_did: invited_user.did,
    })
    .map_err(AccountError::Infrastructure)?;

    Ok(invitation_result)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LeaveAccountCommand {
    pub leaving_actor: UserId,
    pub account_id: AccountId,
}

pub async fn leave_account(
    command: LeaveAccountCommand,
    ports: AccountPorts<'_>,
) -> Result<(), AccountError> {
    let Some(leaving_actor) = ports
        .users
        .find(command.leaving_actor)
        .await
        .map_err(AccountError::Infrastructure)?
    else {
        return Err(AccountError::NonExistent);
    };

    let Some(account) = ports
        .accounts
        .find(command.account_id)
        .await
        .map_err(AccountError::Infrastructure)?
    else {
        return Err(AccountError::NonExistent);
    };

    match ports
        .accounts
        .role_of(leaving_actor.id, account.id)
        .await
        .map_err(AccountError::Infrastructure)?
    {
        None => return Err(AccountError::NotAMember),
        Some(Role::Owner) => return Err(AccountError::OwnerCannotLeave),
        Some(_) => {}
    }

    transaction(ports.database, async move |uow: &mut dyn UnitOfWork| {
        uow.accounts().leave(leaving_actor.id, account.id).await
    })
    .await
    .map_err(AccountError::Infrastructure)?;

    Ok(())
}

/// `grant_role`'s input. The grantee is named by DID: a never-seen DID is
/// provisioned as a User on the spot (one DID, one User).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GrantRoleCommand {
    pub account_id: AccountId,
    pub receiving_actor: Did,
    pub granting_actor_id: UserId,
    pub role: Role,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GrantRoleResult {
    pub account_id: AccountId,
    pub role: Role,
    pub user_did: Did,
}

pub async fn grant_role(
    command: GrantRoleCommand,
    ports: AccountPorts<'_>,
) -> Result<GrantRoleResult, AccountError> {
    // Owner is never granted here — that is `transfer_ownership`.
    if matches!(command.role, Role::Owner) {
        return Err(AccountError::IncorrectTransferOfAccount);
    }

    let Some(granting_user) = ports
        .users
        .find(command.granting_actor_id)
        .await
        .map_err(AccountError::Infrastructure)?
    else {
        return Err(AccountError::NonExistent);
    };
    let Some(account) = ports
        .accounts
        .find(command.account_id)
        .await
        .map_err(AccountError::Infrastructure)?
    else {
        return Err(AccountError::NonExistent);
    };
    let Some(granting_user_role) = ports
        .accounts
        .role_of(granting_user.id, account.id)
        .await
        .map_err(AccountError::Infrastructure)?
    else {
        return Err(AccountError::IncorrectRole);
    };
    if !granting_user_role.can_grant(&command.role) {
        return Err(AccountError::IncorrectRole);
    }

    // Recognize the grantee (idempotent: an existing User is returned as-is).
    let receiving_user = transaction(ports.database, async move |uow: &mut dyn UnitOfWork| {
        uow.users().provision(&command.receiving_actor).await
    })
    .await
    .map_err(|err| match err.downcast_ref::<DidBelongsToAnotherActor>() {
        Some(_) => AccountError::DidBelongsToAnotherActor,
        None => AccountError::Infrastructure(err),
    })?;

    // Re-roling an existing member requires outranking their CURRENT role too —
    // this is what stops a silent demotion (an Owner is never re-roled here, an
    // Admin never re-roles a peer Admin). Re-granting the same role is an upsert.
    if let Some(current_role) = ports
        .accounts
        .role_of(receiving_user.id, account.id)
        .await
        .map_err(AccountError::Infrastructure)?
        && !granting_user_role.can_grant(&current_role)
    {
        return Err(AccountError::IncorrectRole);
    }

    let member = UserAccount {
        user_id: receiving_user.id,
        account_id: account.id,
        role: command.role,
        alias: None,
    };
    let role = transaction(ports.database, async move |uow: &mut dyn UnitOfWork| {
        uow.accounts().grant_role(&member).await?;
        Ok(member.role)
    })
    .await
    .map_err(AccountError::Infrastructure)?;

    Ok(GrantRoleResult {
        account_id: account.id,
        role,
        user_did: receiving_user.did,
    })
}

/// `revoke_role`'s input: who revokes, which account, and the member (by DID).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RevokeRoleCommand {
    pub account_id: AccountId,
    pub revoking_actor_id: UserId,
    pub target: Did,
}

/// The unseated membership, as the drivers render it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RevokeRoleResult {
    pub account_id: AccountId,
    pub user_did: Did,
}

/// Revoke a member's role — the inverse of [`grant_role`]: the actor must
/// outrank the target's current role ([`Role::can_grant`]), so an Owner is
/// never revoked here. Re-homes the target's children and revokes their
/// pending issued invitations in the same transaction.
///
/// Errors: `NonExistent` (account), `IncorrectRole` (actor isn't a member or
/// can't revoke that rank), `NotAMember` (target holds no membership),
/// `Infrastructure`.
pub async fn revoke_role(
    command: RevokeRoleCommand,
    ports: AccountPorts<'_>,
) -> Result<RevokeRoleResult, AccountError> {
    let Some(account) = ports
        .accounts
        .find(command.account_id)
        .await
        .map_err(AccountError::Infrastructure)?
    else {
        return Err(AccountError::NonExistent);
    };
    let Some(actor_role) = ports
        .accounts
        .role_of(command.revoking_actor_id, account.id)
        .await
        .map_err(AccountError::Infrastructure)?
    else {
        return Err(AccountError::IncorrectRole);
    };

    // An unrecognized DID holds no membership either.
    let Some(target) = ports
        .users
        .find_by_did(&command.target)
        .await
        .map_err(AccountError::Infrastructure)?
    else {
        return Err(AccountError::NotAMember);
    };
    let Some(target_role) = ports
        .accounts
        .role_of(target.id, account.id)
        .await
        .map_err(AccountError::Infrastructure)?
    else {
        return Err(AccountError::NotAMember);
    };
    if !actor_role.can_grant(&target_role) {
        return Err(AccountError::IncorrectRole);
    }

    let account_id = account.id;
    let target_id = target.id;
    transaction(ports.database, async move |uow: &mut dyn UnitOfWork| {
        uow.accounts().revoke_role(target_id, account_id).await
    })
    .await
    .map_err(AccountError::Infrastructure)?;

    Ok(RevokeRoleResult {
        account_id,
        user_did: target.did,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InviteToAccountCommand {
    pub account_id: AccountId,
    pub inviting_actor_id: UserId,
    pub invitee: Did,
    pub role: Role,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InviteOutcome {
    Minted,
    AlreadyPending,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InviteToAccountResult {
    pub outcome: InviteOutcome,
    pub invitation_id: InvitationId,
    pub account_id: AccountId,
    pub role: Role,
    pub state: InvitationState,
    pub invitee_did: Did,
}

pub async fn invite_to_account(
    command: InviteToAccountCommand,
    ports: AccountPorts<'_>,
    now: DateTimeUtc,
) -> Result<InviteToAccountResult, AccountError> {
    let Some(account) = ports
        .accounts
        .find(command.account_id)
        .await
        .map_err(AccountError::Infrastructure)?
    else {
        return Err(AccountError::NonExistent);
    };
    let Some(inviting_actor_role) = ports
        .accounts
        .role_of(command.inviting_actor_id, account.id)
        .await
        .map_err(AccountError::Infrastructure)?
    else {
        return Err(AccountError::IncorrectRole);
    };
    if !inviting_actor_role.can_grant(&command.role) {
        return Err(AccountError::IncorrectRole);
    }

    // Recognize the invitee (idempotent: an existing User is returned as-is).
    let invited_user = transaction(ports.database, async move |uow: &mut dyn UnitOfWork| {
        uow.users().provision(&command.invitee).await
    })
    .await
    .map_err(|err| match err.downcast_ref::<DidBelongsToAnotherActor>() {
        Some(_) => AccountError::DidBelongsToAnotherActor,
        None => AccountError::Infrastructure(err),
    })?;

    if ports
        .accounts
        .role_of(invited_user.id, account.id)
        .await
        .map_err(AccountError::Infrastructure)?
        .is_some()
    {
        return Err(AccountError::AlreadyMember);
    }

    if let Some(existing) = ports
        .accounts
        .find_pending_invitation(account.id, invited_user.id)
        .await
        .map_err(AccountError::Infrastructure)?
    {
        return Ok(InviteToAccountResult {
            outcome: InviteOutcome::AlreadyPending,
            invitation_id: existing.id,
            account_id: account.id,
            role: existing.role,
            state: existing.state,
            invitee_did: invited_user.did,
        });
    }

    let invitation = Invitation::issue(
        account.id,
        invited_user.id,
        command.role,
        command.inviting_actor_id,
        now,
    );
    let minted = invitation.id;
    transaction(ports.database, async move |uow: &mut dyn UnitOfWork| {
        uow.accounts().create_invitation(&invitation).await
    })
    .await
    .map_err(AccountError::Infrastructure)?;

    // A duplicate invite can race the insert (dropped via `ON CONFLICT DO
    // NOTHING`); report whichever row survives, never fabricate one.
    let account_id = account.id;
    let invited_user_id = invited_user.id;
    let stored = match ports
        .accounts
        .find_pending_invitation(account_id, invited_user_id)
        .await
        .map_err(AccountError::Infrastructure)?
    {
        Some(stored) => stored,
        None => ports
            .accounts
            .find_invitation(minted)
            .await
            .map_err(AccountError::Infrastructure)?
            .ok_or_else(|| {
                AccountError::Infrastructure(anyhow::anyhow!(
                    "the invitation could not be confirmed after being created"
                ))
            })?,
    };
    let outcome = if stored.id == minted {
        InviteOutcome::Minted
    } else {
        InviteOutcome::AlreadyPending
    };

    Ok(InviteToAccountResult {
        outcome,
        invitation_id: stored.id,
        account_id,
        role: stored.role,
        state: stored.state,
        invitee_did: invited_user.did,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RevokeInvitationCommand {
    pub account_id: AccountId,
    pub revoking_actor_id: UserId,
    pub invitee: Did,
}

pub async fn revoke_invitation(
    command: RevokeInvitationCommand,
    ports: AccountPorts<'_>,
    now: DateTimeUtc,
) -> Result<(), AccountError> {
    let Some(account) = ports
        .accounts
        .find(command.account_id)
        .await
        .map_err(AccountError::Infrastructure)?
    else {
        return Err(AccountError::NonExistent);
    };
    let Some(revoking_actor_role) = ports
        .accounts
        .role_of(command.revoking_actor_id, account.id)
        .await
        .map_err(AccountError::Infrastructure)?
    else {
        return Err(AccountError::IncorrectRole);
    };

    // An unrecognized invitee DID holds no pending invitation either.
    let Some(invited_user) = ports
        .users
        .find_by_did(&command.invitee)
        .await
        .map_err(AccountError::Infrastructure)?
    else {
        return Ok(());
    };
    let Some(mut invitation) = ports
        .accounts
        .find_pending_invitation(account.id, invited_user.id)
        .await
        .map_err(AccountError::Infrastructure)?
    else {
        return Ok(());
    };

    if !revoking_actor_role.can_grant(&invitation.role) {
        return Err(AccountError::IncorrectRole);
    }

    invitation.revoke(now).map_err(|_| {
        AccountError::Infrastructure(anyhow::anyhow!(
            "the invitation could not be revoked: it was no longer pending"
        ))
    })?;

    let invitation_id = invitation.id;
    transaction(ports.database, async move |uow: &mut dyn UnitOfWork| {
        uow.accounts().revoke_invitation(invitation_id).await
    })
    .await
    .map_err(AccountError::Infrastructure)?;

    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeclineInvitationCommand {
    pub account_id: AccountId,
    pub declining_actor: UserId,
}

pub async fn decline_invitation(
    command: DeclineInvitationCommand,
    ports: AccountPorts<'_>,
    now: DateTimeUtc,
) -> Result<(), AccountError> {
    let Some(account) = ports
        .accounts
        .find(command.account_id)
        .await
        .map_err(AccountError::Infrastructure)?
    else {
        return Err(AccountError::NonExistent);
    };

    let Some(mut invitation) = ports
        .accounts
        .find_pending_invitation(account.id, command.declining_actor)
        .await
        .map_err(AccountError::Infrastructure)?
    else {
        return Err(AccountError::NoPendingInvitation);
    };

    invitation.revoke(now).map_err(|_| {
        AccountError::Infrastructure(anyhow::anyhow!(
            "the invitation could not be declined: it was no longer pending"
        ))
    })?;

    let invitation_id = invitation.id;
    transaction(ports.database, async move |uow: &mut dyn UnitOfWork| {
        uow.accounts().revoke_invitation(invitation_id).await
    })
    .await
    .map_err(AccountError::Infrastructure)?;

    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransferOwnershipCommand {
    pub account_id: AccountId,
    pub old_owner: UserId,
    pub new_owner: Did,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransferOwnershipResult {
    pub account_id: AccountId,
    pub new_owner_did: Did,
    pub previous_owner_did: Did,
}

pub async fn transfer_ownership(
    command: TransferOwnershipCommand,
    ports: AccountPorts<'_>,
) -> Result<TransferOwnershipResult, AccountError> {
    let Some(old_owner) = ports
        .users
        .find(command.old_owner)
        .await
        .map_err(AccountError::Infrastructure)?
    else {
        return Err(AccountError::NonExistent);
    };
    let Some(account) = ports
        .accounts
        .find(command.account_id)
        .await
        .map_err(AccountError::Infrastructure)?
    else {
        return Err(AccountError::NonExistent);
    };
    let Some(old_owner_role) = ports
        .accounts
        .role_of(old_owner.id, account.id)
        .await
        .map_err(AccountError::Infrastructure)?
    else {
        return Err(AccountError::IncorrectRole);
    };
    // Owner-only (DESIGN/Roles rule 8), mirroring `delete_account`'s rank rule.
    if !matches!(old_owner_role, Role::Owner) {
        return Err(AccountError::IncorrectRole);
    }

    let Some(new_owner) = ports
        .users
        .find_by_did(&command.new_owner)
        .await
        .map_err(AccountError::Infrastructure)?
    else {
        return Err(AccountError::NotAMember);
    };
    if ports
        .accounts
        .role_of(new_owner.id, account.id)
        .await
        .map_err(AccountError::Infrastructure)?
        .is_none()
    {
        return Err(AccountError::NotAMember);
    }

    // Ownership moves to *another* member (DESIGN/Roles rule 8).
    if new_owner.id == old_owner.id {
        return Err(AccountError::CannotTransferToSelf);
    }

    let account_id = account.id;
    let old_owner_id = old_owner.id;
    let new_owner_id = new_owner.id;
    transaction(ports.database, async move |uow: &mut dyn UnitOfWork| {
        uow.accounts()
            .transfer_ownership(old_owner_id, new_owner_id, account_id)
            .await
    })
    .await
    .map_err(AccountError::Infrastructure)?;

    Ok(TransferOwnershipResult {
        account_id,
        new_owner_did: new_owner.did,
        previous_owner_did: old_owner.did,
    })
}
