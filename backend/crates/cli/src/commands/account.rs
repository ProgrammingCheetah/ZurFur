//! The `account` namespace: what the acting identity does with Accounts.
//! `create` (ZMVP-205 slice 4) is the CLI face of
//! [`application::account::create_account`] — the same use case behind
//! `POST /accounts`, so both drivers found accounts through one path.
//! `delete` (slice 5) is the same arrangement over
//! [`application::account::delete_account`], the use case behind
//! `DELETE /accounts/{id}` — with one thing the HTTP driver has no place for:
//! an irreversible operation asks first ([`crate::confirm`]).

use std::path::Path;

use application::account::{
    AccountError, AccountPorts, CreateAccountCommand, CreateAccountResult, DeleteAccountCommand,
    DeleteAccountResult, DeleteOutcome, create_account, delete_account,
};
use chrono::Utc;
use clap::Subcommand;
use composition::Runtime;
use domain::elements::{
    account::{AccountId, AccountName},
    handle::Handle,
};
use serde::Serialize;
use uuid::Uuid;

use crate::{CliError, confirm::confirm_destructive, principal::Principal};

/// The account operations.
#[derive(Debug, Subcommand)]
pub enum AccountOp {
    /// Create (found) a new Account, with the acting identity as its Owner.
    Create {
        /// The account's display name.
        #[arg(long)]
        name: String,
        /// The account's handle — `<label>.zurfur.app` or a brought domain.
        #[arg(long)]
        handle: String,
    },
    /// Delete an Account the acting identity owns — soft if it holds facts,
    /// hard if empty. Asks to confirm first (Engineer ruling 2026-08-28).
    Delete {
        /// The account's id (UUIDv7).
        account_id: Uuid,
        /// Skip the confirmation prompt (for scripts).
        #[arg(long, short = 'y')]
        yes: bool,
    },
}

/// `create`'s projection — the same keys and spelling as the HTTP
/// `CreateAccountResponse` (`{id, did, handle, name}`).
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Founded {
    id: String,
    did: String,
    handle: String,
    name: String,
}

impl From<CreateAccountResult> for Founded {
    fn from(founded: CreateAccountResult) -> Self {
        Founded {
            id: founded.account_id.to_string(),
            did: founded.did.as_str().to_owned(),
            handle: founded.handle.as_str().to_owned(),
            name: founded.name.as_str().to_owned(),
        }
    }
}

/// `delete`'s projection — the same key and spelling as the HTTP
/// `DeleteAccountResponse` (`{outcome}`), which the CLI cannot name (it lives
/// inside `api`, behind axum). A hand copy, pinned to the wire by the parity
/// test `api/tests/delete_account_parity.rs`.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Deleted {
    outcome: String,
}

impl From<DeleteAccountResult> for Deleted {
    fn from(deleted: DeleteAccountResult) -> Self {
        let outcome = match deleted.outcome {
            DeleteOutcome::Soft => "soft",
            DeleteOutcome::Hard => "hard",
        };
        Deleted {
            outcome: outcome.to_owned(),
        }
    }
}

/// Run one account op over the runtime as the identity at `identity_path`.
pub async fn run(
    runtime: &Runtime,
    identity_path: &Path,
    op: AccountOp,
) -> Result<serde_json::Value, CliError> {
    match op {
        AccountOp::Create { name, handle } => {
            // Founding is a write: resolve the principal first (the HTTP
            // driver's `require_user`), then parse — the same 422-class
            // refusals as the API's `invalid_request`, before anything is minted.
            let principal = Principal::resolve(runtime, identity_path).await?;
            let name = name
                .parse::<AccountName>()
                .map_err(|err| CliError::domain("invalid_request", err))?;
            let handle = handle
                .parse::<Handle>()
                .map_err(|err| CliError::domain("invalid_request", err))?;

            let command = CreateAccountCommand {
                actor: principal.user.id,
                name,
                handle,
            };
            let founded = create_account(
                command,
                account_ports(runtime),
                &runtime.config.handle_domain,
                Utc::now(),
            )
            .await
            .map_err(|err| match err {
                AccountError::HandleTaken => CliError::domain("handle_taken", err),
                // The terse `Display` is what the user sees; the cause goes to
                // the diagnostics channel (stderr, before the problem line).
                AccountError::Infrastructure(_) => {
                    tracing::error!(error = ?err, "founding the account failed");
                    CliError::infra("internal_error", err)
                }
                // Unreachable from founding — nothing here acts on an existing
                // account, so there is no role to fail and no account to be
                // missing. Mapped rather than swept under a wildcard so the match
                // stays exhaustive and a future variant lands as a compile error.
                AccountError::IncorrectRole => CliError::domain("forbidden", err),
                AccountError::NonExistent => CliError::domain("account_not_found", err),
                // Unreachable from founding — the namespace check is change_handle's own.
                AccountError::UnsupportedHandle => CliError::domain("unsupported_handle", err),
                // Unreachable from founding — both are change_handle-only outcomes.
                AccountError::HandleUnchanged => CliError::domain("invalid_request", err),
                AccountError::RenamedTooRecently => CliError::domain("rate_limited", err),
                // Unreachable here: membership-only outcomes of accept/leave.
                AccountError::NoPendingInvitation => CliError::domain("no_pending_invitation", err),
                AccountError::NotAMember => CliError::domain("member_not_found", err),
                AccountError::OwnerCannotLeave => CliError::domain("owner_cannot_leave", err),
                AccountError::IncorrectTransferOfAccount => CliError::domain("forbidden", err),
                AccountError::DidBelongsToAnotherActor => {
                    CliError::domain("did_belongs_to_another_actor", err)
                }
                // Unreachable here: membership-only outcomes of invite/transfer.
                AccountError::AlreadyMember => CliError::domain("already_member", err),
                AccountError::CannotTransferToSelf => CliError::domain("invalid_request", err),
            })?;
            let body = Founded::from(founded);
            Ok(serde_json::to_value(body).expect("Founded serializes"))
        }
        AccountOp::Delete { account_id, yes } => {
            // Deleting is a write: resolve the principal first (the HTTP
            // driver's `require_user`), so an unrecognized caller is turned
            // away before any account is loaded. `account_id` is already a
            // `Uuid` — clap parsed it, so a malformed one never reaches here
            // and is clap's usage error (exit 2). That is this driver's
            // analogue of the API's 404-on-non-uuid: the terminal has a usage
            // channel the HTTP surface doesn't.
            let principal = Principal::resolve(runtime, identity_path).await?;
            // Then confirm, and only then act. The order is deliberate: an
            // anonymous caller is `not_authenticated` before any question is
            // asked, and nothing is looked up before the answer — the account
            // is the use case's to load, so a declined prompt leaks nothing
            // about whether the id names anything.
            if !yes {
                let prompt = format!(
                    "Delete account {account_id}? This frees its handle and tombstones its \
                     did:plc; it cannot be undone after the PLC recovery window. [y/N] "
                );
                confirm_destructive(&prompt)?;
            }
            let command = DeleteAccountCommand {
                actor: principal.user.id,
                account_id: AccountId::new(account_id),
            };
            let deleted = delete_account(command, account_ports(runtime))
                .await
                .map_err(|err| match err {
                    AccountError::NonExistent => CliError::domain("account_not_found", err),
                    AccountError::IncorrectRole => CliError::domain("forbidden", err),
                    // The terse `Display` is what the user sees; the cause goes
                    // to the diagnostics channel (stderr, before the problem
                    // line).
                    AccountError::Infrastructure(_) => {
                        tracing::error!(error = ?err, "deleting the account failed");
                        CliError::infra("internal_error", err)
                    }
                    // Unreachable from delete (no handle is claimed here);
                    // mapped rather than caught by a wildcard so the
                    // exhaustiveness keeps its value.
                    AccountError::HandleTaken => CliError::domain("handle_taken", err),
                    // Unreachable from delete — no handle-namespace check runs here.
                    AccountError::UnsupportedHandle => CliError::domain("unsupported_handle", err),
                    // Unreachable from delete — both are change_handle-only outcomes.
                    AccountError::HandleUnchanged => CliError::domain("invalid_request", err),
                    AccountError::RenamedTooRecently => CliError::domain("rate_limited", err),
                    // Unreachable here: membership-only outcomes of accept/leave.
                    AccountError::NoPendingInvitation => {
                        CliError::domain("no_pending_invitation", err)
                    }
                    AccountError::NotAMember => CliError::domain("member_not_found", err),
                    AccountError::OwnerCannotLeave => CliError::domain("owner_cannot_leave", err),
                    AccountError::IncorrectTransferOfAccount => CliError::domain("forbidden", err),
                    AccountError::DidBelongsToAnotherActor => {
                        CliError::domain("did_belongs_to_another_actor", err)
                    }
                    // Unreachable here: membership-only outcomes of invite/transfer.
                    AccountError::AlreadyMember => CliError::domain("already_member", err),
                    AccountError::CannotTransferToSelf => CliError::domain("invalid_request", err),
                })?;
            let body = Deleted::from(deleted);
            Ok(serde_json::to_value(body).expect("Deleted serializes"))
        }
    }
}

/// The account use cases' ports, borrowed off the runtime bag — this
/// driver's copy of the API's `account_ports`.
fn account_ports(runtime: &Runtime) -> AccountPorts<'_> {
    AccountPorts {
        accounts: &*runtime.accounts,
        users: &*runtime.users,
        did_minter: &*runtime.did_minter,
        database: &*runtime.database,
    }
}
