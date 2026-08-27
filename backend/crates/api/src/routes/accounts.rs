//! HTTP driver for the account routes over `application::account` —
//! founding, membership, and the invitation lifecycle. Mounted under the
//! first-party-`Origin` (CSRF) layer.

use application::account::{
    AcceptInvitationCommand, AccountError, AccountPorts, ChangeHandleCommand, ChangeHandleResult,
    CreateAccountCommand, CreateAccountResult, DeclineInvitationCommand, DeleteAccountCommand,
    DeleteAccountResult, DeleteOutcome, GrantRoleCommand, InviteOutcome, InviteToAccountCommand,
    LeaveAccountCommand, ListAccountsQuery, RevokeInvitationCommand, RevokeRoleCommand,
    TransferOwnershipCommand,
};
use axum::{
    Json, Router,
    extract::{
        Path, State,
        rejection::{JsonRejection, PathRejection},
    },
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{delete, get, patch, post},
};
use chrono::Utc;
use domain::elements::{
    account::{AccountId, AccountName},
    did::Did,
    handle::Handle,
    role::Role,
    user::{User, UserId},
};
use serde::{Deserialize, Serialize};
use tower_sessions::Session;
use uuid::Uuid;

use crate::generated::{
    AccountMembership, ChangeHandleRequest, ChangeHandleResponse, CreateAccountRequest,
    CreateAccountResponse, DeleteAccountResponse, ListAccountsResponse,
};
use crate::problem::Problem;
use crate::{AppState, SESSION_USER_KEY};

/// The accounts route group: founding, membership (grant/revoke/leave), and
/// the invitation lifecycle (invite/revoke/decline/accept).
pub(crate) fn accounts_router() -> Router<AppState> {
    Router::new()
        .route("/accounts", get(list_accounts).post(create_account))
        .route("/accounts/{id}", delete(delete_account))
        .route("/accounts/{id}/handle", patch(change_handle))
        .route(
            "/accounts/{id}/members",
            post(grant_role).delete(revoke_role),
        )
        .route("/accounts/{id}/members/me", delete(leave_account))
        .route("/accounts/{id}/transfer", post(transfer_ownership))
        .route(
            "/accounts/{id}/invitations",
            post(invite_user_to_account).delete(revoke_invitation_to_account),
        )
        .route(
            "/accounts/{id}/invitations/decline",
            post(decline_invitation),
        )
        .route("/accounts/{id}/invitations/accept", post(accept_invitation))
}

/// Resolves the session to the acting [`User`], or `401` if unauthenticated
/// or the User has vanished.
async fn require_user(state: &AppState, session: &Session) -> Result<User, Problem> {
    let id = session
        .get::<Uuid>(SESSION_USER_KEY)
        .await
        .ok()
        .flatten()
        .ok_or_else(Problem::not_authenticated)?;
    state
        .users
        .find(UserId::new(id))
        .await
        .ok()
        .flatten()
        .ok_or_else(Problem::not_authenticated)
}

/// `GET /accounts` — every live account the signed-in visitor holds a role in
/// (not owned-only).
///
/// - `200 { "accounts": [ { "id", "did", "handle", "name", "role", "alias" }, … ] }`
/// - `401` — not signed in
async fn list_accounts(
    State(state): State<AppState>,
    session: Session,
) -> Result<Response, Problem> {
    let user = require_user(&state, &session).await?;

    let query = ListAccountsQuery { actor: user.id };
    let ports = account_ports(&state);
    let accounts = application::account::list_accounts(query, &ports)
        .await
        .map(|result| result.accounts)
        .map_err(|_| Problem::service_unavailable("There seems to be a problem on our side"))?
        .into_iter()
        .map(|account| AccountMembership {
            id: account.id.to_string(),
            did: account.did.to_string(),
            role: account.role.to_string(),
            handle: account.handle.as_str().into(),
            name: account.name.as_str().into(),
            alias: account.alias.map(|alias| alias.to_string()),
        })
        .collect();

    let body = ListAccountsResponse { accounts };
    let response = (StatusCode::OK, Json(body)).into_response();
    Ok(response)
}

/// `POST /accounts` — founds a new Account for the signed-in visitor as its
/// Owner.
///
/// - `201 { "id", "did", "handle", "name" }`
/// - `401` — not signed in
/// - `422 invalid_request` — missing/malformed body, blank name, or bad handle
/// - `409 handle_taken` — handle already claimed (live or tombstoned)
async fn create_account(
    State(state): State<AppState>,
    session: Session,
    body: Result<Json<CreateAccountRequest>, JsonRejection>,
) -> Result<Response, Problem> {
    let user = require_user(&state, &session).await?;

    let Json(body) =
        body.map_err(|_| Problem::invalid_request("A name and handle are required."))?;
    let name = body
        .name
        .parse::<AccountName>()
        .map_err(|err| Problem::invalid_request(err.to_string()))?;
    let handle = body
        .handle
        .parse::<Handle>()
        .map_err(|err| Problem::invalid_request(err.to_string()))?;

    let command = CreateAccountCommand {
        actor: user.id,
        name,
        handle,
    };
    let founded = application::account::create_account(
        command,
        account_ports(&state),
        &state.config.handle_domain,
        Utc::now(),
    )
    .await
    .map_err(|err| match err {
        AccountError::HandleTaken => Problem::handle_taken(),
        AccountError::Infrastructure(err) => Problem::from(err),
        AccountError::IncorrectRole => Problem::forbidden(),
        AccountError::NonExistent => Problem::account_not_found(),
        AccountError::UnsupportedHandle => Problem::unsupported_handle(
            "Changing to a handle outside the Zurfur namespace isn't supported yet.",
        ),
        AccountError::HandleUnchanged => {
            Problem::invalid_request("That is already the account's handle.")
        }
        AccountError::RenamedTooRecently => Problem::rate_limited(
            "Too many handle changes recently. Please wait before changing it again.",
        ),
        // Unreachable here: membership-only outcomes of accept/leave.
        AccountError::NoPendingInvitation => Problem::no_pending_invitation(),
        AccountError::NotAMember => Problem::member_not_found(),
        AccountError::OwnerCannotLeave => Problem::owner_cannot_leave(),
        AccountError::IncorrectTransferOfAccount => Problem::forbidden(),
        AccountError::DidBelongsToAnotherActor => Problem::did_belongs_to_another_actor(),
        AccountError::AlreadyMember => {
            Problem::already_member("That user is already a member of this account.")
        }
        AccountError::CannotTransferToSelf => Problem::invalid_request(
            "You already own this account; transfer ownership to another member.",
        ),
    })?;

    let response = (
        StatusCode::CREATED,
        Json(CreateAccountResponse::from(founded)),
    )
        .into_response();
    Ok(response)
}

/// Projects the founded account onto the wire response.
impl From<CreateAccountResult> for CreateAccountResponse {
    fn from(founded: CreateAccountResult) -> Self {
        CreateAccountResponse {
            id: founded.account_id.to_string(),
            did: founded.did.as_str().to_owned(),
            handle: founded.handle.as_str().to_owned(),
            name: founded.name.as_str().to_owned(),
        }
    }
}

/// Borrows this crate's ports for the `application::account` use cases.
fn account_ports(state: &AppState) -> AccountPorts<'_> {
    AccountPorts {
        accounts: &*state.accounts,
        users: &*state.users,
        did_minter: &*state.did_minter,
        database: &*state.database,
    }
}

// Must fail to compile once the first account-fact table is registered in
// `adapter_pg::ACCOUNT_FACT_TABLES`, forcing `account_has_facts` to become a
// real query instead of its constant-`false` body.
const _: () = assert!(
    adapter_pg::ACCOUNT_FACT_TABLES.is_empty(),
    "an account-anchored fact store was registered: replace the constant-`false` body \
     of application::account::account_has_facts with a real query over it (an account \
     bearing such a fact must be soft-deleted, never hard-deleted), then remove this \
     guard and its sibling in adapter_pg::account"
);

/// `DELETE /accounts/{id}` — the Owner deletes their account.
///
/// - `200 { "outcome": "soft" | "hard" }`
/// - `401` — not signed in
/// - `403` — not this account's Owner
/// - `404` — no such live account
async fn delete_account(
    State(state): State<AppState>,
    account_id: Result<Path<Uuid>, PathRejection>,
    session: Session,
) -> Result<Response, Problem> {
    let user = require_user(&state, &session).await?;

    let Path(account_id) = account_id.map_err(|_| Problem::account_not_found())?;
    let command = DeleteAccountCommand {
        account_id: AccountId::new(account_id),
        actor: user.id,
    };

    let deleted = application::account::delete_account(command, account_ports(&state))
        .await
        .map_err(|err| match err {
            AccountError::Infrastructure(err) => {
                tracing::error!(error = ?err, "deleting the account failed in the store");
                Problem::from(err)
            }
            AccountError::IncorrectRole => Problem::forbidden(),
            AccountError::NonExistent => Problem::account_not_found(),
            AccountError::HandleTaken => Problem::handle_taken(),
            AccountError::UnsupportedHandle => Problem::unsupported_handle(
                "Changing to a handle outside the Zurfur namespace isn't supported yet.",
            ),
            AccountError::HandleUnchanged => {
                Problem::invalid_request("That is already the account's handle.")
            }
            AccountError::RenamedTooRecently => Problem::rate_limited(
                "Too many handle changes recently. Please wait before changing it again.",
            ),
            // Unreachable here: membership-only outcomes of accept/leave.
            AccountError::NoPendingInvitation => Problem::no_pending_invitation(),
            AccountError::NotAMember => Problem::member_not_found(),
            AccountError::OwnerCannotLeave => Problem::owner_cannot_leave(),
            AccountError::IncorrectTransferOfAccount => Problem::forbidden(),
            AccountError::DidBelongsToAnotherActor => Problem::did_belongs_to_another_actor(),
            AccountError::AlreadyMember => {
                Problem::already_member("That user is already a member of this account.")
            }
            AccountError::CannotTransferToSelf => Problem::invalid_request(
                "You already own this account; transfer ownership to another member.",
            ),
        })?;

    let response = (StatusCode::OK, Json(DeleteAccountResponse::from(deleted))).into_response();
    Ok(response)
}

/// Projects the deletion outcome onto the wire response.
impl From<DeleteAccountResult> for DeleteAccountResponse {
    fn from(deleted: DeleteAccountResult) -> Self {
        let outcome = match deleted.outcome {
            DeleteOutcome::Soft => "soft",
            DeleteOutcome::Hard => "hard",
        };
        DeleteAccountResponse {
            outcome: outcome.to_owned(),
        }
    }
}

/// `PATCH /accounts/{id}/handle` — the Owner changes the account's handle
/// post-onboarding. Order matters: the DID document updates first, then the
/// private store (no cross-store transaction; DD 27852802).
///
/// - `200 { "id", "did", "handle", "name" }`
/// - `401` — not signed in · `403` — not this account's Owner
/// - `404` — no such live account
/// - `409 handle_taken` — held by another account (live, tombstoned, or quarantined)
/// - `422` — malformed body, invalid handle, unchanged handle, or `unsupported_handle` (BYO target)
/// - `429 rate_limited` — too many recent changes · `503` — DID minter unavailable
async fn change_handle(
    State(state): State<AppState>,
    account_id: Result<Path<Uuid>, PathRejection>,
    session: Session,
    body: Result<Json<ChangeHandleRequest>, JsonRejection>,
) -> Result<Response, Problem> {
    let user = require_user(&state, &session).await?;

    let Path(account_id) = account_id.map_err(|_| Problem::account_not_found())?;
    let Json(body) = body.map_err(|_| Problem::invalid_request("A new handle is required."))?;
    let handle = body
        .handle
        .parse::<Handle>()
        .map_err(|err| Problem::invalid_request(err.to_string()))?;

    let command = ChangeHandleCommand {
        account_id: AccountId::new(account_id),
        actor: user.id,
        handle,
    };
    let renamed = application::account::change_handle(
        command,
        account_ports(&state),
        &state.config.handle_domain,
        Utc::now(),
    )
    .await
    .map_err(|err| match err {
        AccountError::NonExistent => Problem::account_not_found(),
        AccountError::IncorrectRole => Problem::forbidden(),
        AccountError::HandleUnchanged => {
            Problem::invalid_request("That is already the account's handle.")
        }
        AccountError::RenamedTooRecently => Problem::rate_limited(
            "Too many handle changes recently. Please wait before changing it again.",
        ),
        // Unreachable here: membership-only outcomes of accept/leave.
        AccountError::NoPendingInvitation => Problem::no_pending_invitation(),
        AccountError::NotAMember => Problem::member_not_found(),
        AccountError::OwnerCannotLeave => Problem::owner_cannot_leave(),
        AccountError::IncorrectTransferOfAccount => Problem::forbidden(),
        AccountError::DidBelongsToAnotherActor => Problem::did_belongs_to_another_actor(),
        AccountError::AlreadyMember => {
            Problem::already_member("That user is already a member of this account.")
        }
        AccountError::CannotTransferToSelf => Problem::invalid_request(
            "You already own this account; transfer ownership to another member.",
        ),
        AccountError::HandleTaken => Problem::handle_taken(),
        AccountError::UnsupportedHandle => Problem::unsupported_handle(
            "Changing to a handle outside the Zurfur namespace isn't supported yet.",
        ),
        AccountError::Infrastructure(err) => Problem::from(err),
    })?;

    let response = (StatusCode::OK, Json(ChangeHandleResponse::from(renamed))).into_response();
    Ok(response)
}

/// Projects the renamed account onto the wire response.
impl From<ChangeHandleResult> for ChangeHandleResponse {
    fn from(renamed: ChangeHandleResult) -> Self {
        ChangeHandleResponse {
            id: renamed.id.to_string(),
            did: renamed.did.as_str().to_owned(),
            handle: renamed.handle.as_str().to_owned(),
            name: renamed.name.as_str().to_owned(),
        }
    }
}

/// The accept-invitation request body: whether the new membership is shown on
/// the invitee's public profile.
#[derive(Deserialize)]
struct AcceptInvitationBody {
    pub listed_on_profile: bool,
}

/// `POST /accounts/{id}/invitations/accept`'s `200` body.
#[derive(Serialize)]
struct AcceptInvitationResponse {
    account: String,
    role: String,
    user: String,
}

/// `POST /accounts/{id}/invitations/accept` — the invited User accepts their
/// own pending invitation and becomes a member.
///
/// - `200 { "account", "role", "user" }`
/// - `404 no_pending_invitation` — no pending offer for this user
/// - `400` — malformed body
async fn accept_invitation(
    State(state): State<AppState>,
    Path(account_id): Path<Uuid>,
    session: Session,
    body: Result<Json<AcceptInvitationBody>, JsonRejection>,
) -> Result<Response, Problem> {
    let Json(body) = body.map_err(|_| Problem::invalid_request("Malformed JSON"))?;
    let invited_user = require_user(&state, &session).await?;
    let command = AcceptInvitationCommand {
        invited_actor: invited_user.id,
        into_account: AccountId::new(account_id),
        listed_on_profile: body.listed_on_profile,
    };
    let ports = account_ports(&state);

    let accepted = application::account::accept_invitation(command, ports)
        .await
        .map_err(|err| match err {
            AccountError::NoPendingInvitation => Problem::no_pending_invitation(),
            AccountError::NonExistent => Problem::account_not_found(),
            AccountError::Infrastructure(err) => Problem::from(err),
            // Unreachable from accepting: no rank check, no handle claimed, no rename.
            AccountError::IncorrectRole => Problem::forbidden(),
            AccountError::HandleTaken => Problem::handle_taken(),
            AccountError::UnsupportedHandle => Problem::unsupported_handle(
                "Changing to a handle outside the Zurfur namespace isn't supported yet.",
            ),
            AccountError::HandleUnchanged => {
                Problem::invalid_request("That is already the account's handle.")
            }
            AccountError::RenamedTooRecently => Problem::rate_limited(
                "Too many handle changes recently. Please wait before changing it again.",
            ),
            AccountError::NotAMember => Problem::member_not_found(),
            AccountError::OwnerCannotLeave => Problem::owner_cannot_leave(),
            AccountError::IncorrectTransferOfAccount => Problem::forbidden(),
            AccountError::DidBelongsToAnotherActor => Problem::did_belongs_to_another_actor(),
            AccountError::AlreadyMember => {
                Problem::already_member("That user is already a member of this account.")
            }
            AccountError::CannotTransferToSelf => Problem::invalid_request(
                "You already own this account; transfer ownership to another member.",
            ),
        })?;

    let body = AcceptInvitationResponse {
        account: accepted.account_id.to_string(),
        role: accepted.role.to_string(),
        user: invited_user.did.as_str().to_owned(),
    };
    let response = (StatusCode::OK, Json(body)).into_response();
    Ok(response)
}

/// `DELETE /accounts/{id}/members/me` — the signed-in member leaves the account.
///
/// - `204 No Content`
/// - `404` — not a member
/// - `409` — the sole Owner can't leave while still Owner (transfer or delete first)
async fn leave_account(
    State(state): State<AppState>,
    Path(account_id): Path<Uuid>,
    session: Session,
) -> Result<Response, Problem> {
    let leaving_user = require_user(&state, &session).await?;
    let account = AccountId::new(account_id);

    let command = LeaveAccountCommand {
        account_id: account,
        leaving_actor: leaving_user.id,
    };

    let ports = account_ports(&state);

    application::account::leave_account(command, ports)
        .await
        .map_err(|err| match err {
            AccountError::NotAMember => Problem::member_not_found(),
            AccountError::OwnerCannotLeave => Problem::owner_cannot_leave(),
            AccountError::IncorrectTransferOfAccount => Problem::forbidden(),
            AccountError::DidBelongsToAnotherActor => Problem::did_belongs_to_another_actor(),
            AccountError::AlreadyMember => {
                Problem::already_member("That user is already a member of this account.")
            }
            AccountError::CannotTransferToSelf => Problem::invalid_request(
                "You already own this account; transfer ownership to another member.",
            ),
            AccountError::NonExistent => Problem::account_not_found(),
            AccountError::Infrastructure(err) => Problem::from(err),
            // Unreachable from leaving: no rank check, no invitation, no handle, no rename.
            AccountError::IncorrectRole => Problem::forbidden(),
            AccountError::NoPendingInvitation => Problem::no_pending_invitation(),
            AccountError::HandleTaken => Problem::handle_taken(),
            AccountError::UnsupportedHandle => Problem::unsupported_handle(
                "Changing to a handle outside the Zurfur namespace isn't supported yet.",
            ),
            AccountError::HandleUnchanged => {
                Problem::invalid_request("That is already the account's handle.")
            }
            AccountError::RenamedTooRecently => Problem::rate_limited(
                "Too many handle changes recently. Please wait before changing it again.",
            ),
        })?;

    Ok(StatusCode::NO_CONTENT.into_response())
}

/// The body of `POST /accounts/{id}/members`: grantee by `did`, and `role`
/// (`"admin" | "manager" | "member"`; `"owner"` never grantable here).
///
/// Example: `{ "user": "did:plc:abc123", "role": "admin" }`.
#[derive(Deserialize)]
struct GrantRoleBody {
    user: String,
    role: String,
}

/// `POST /accounts/{id}/members`'s `200` body — see [`grant_role`].
#[derive(Serialize)]
struct GrantRoleResponse {
    account: String,
    role: String,
    user: String,
}

/// `POST /accounts/{id}/members` — grants a role, seating the grantee as a
/// member if they aren't one yet.
///
/// - `200 { "account", "user", "role" }`
/// - `401` — not signed in · `403` — not allowed to grant that role
/// - `404` — no such account
/// - `422` — malformed body or an unknown role discriminant
async fn grant_role(
    State(state): State<AppState>,
    account_id: Result<Path<Uuid>, PathRejection>,
    session: Session,
    body: Result<Json<GrantRoleBody>, JsonRejection>,
) -> Result<Response, Problem> {
    let user = require_user(&state, &session).await?;
    let Path(account_id) = account_id.map_err(|_| Problem::account_not_found())?;
    let Json(body) = body.map_err(|_| {
        Problem::invalid_request(
            "Provide a member to grant, e.g. {\"user\": \"did:plc:…\", \"role\": \"admin\"}.",
        )
    })?;
    let role = body
        .role
        .parse::<Role>()
        .map_err(|err| Problem::unknown_role(err.to_string()))?;

    let command = GrantRoleCommand {
        account_id: AccountId::new(account_id),
        receiving_actor: Did::new(body.user),
        granting_actor_id: user.id,
        role,
    };
    let granted = application::account::grant_role(command, account_ports(&state))
        .await
        .map_err(|err| match err {
            AccountError::NonExistent => Problem::account_not_found(),
            // The granter's own standing: a non-member has no authority here (403,
            // the membership floor), as does a member who can't grant that role.
            AccountError::NotAMember | AccountError::IncorrectRole => Problem::forbidden(),
            AccountError::IncorrectTransferOfAccount => Problem::forbidden(),
            AccountError::DidBelongsToAnotherActor => Problem::did_belongs_to_another_actor(),
            AccountError::AlreadyMember => {
                Problem::already_member("That user is already a member of this account.")
            }
            AccountError::CannotTransferToSelf => Problem::invalid_request(
                "You already own this account; transfer ownership to another member.",
            ),
            AccountError::Infrastructure(err) => Problem::from(err),
            // Unreachable from granting: no handle, no rename, no invitation, no leave.
            AccountError::HandleTaken => Problem::handle_taken(),
            AccountError::UnsupportedHandle => Problem::unsupported_handle(
                "Changing to a handle outside the Zurfur namespace isn't supported yet.",
            ),
            AccountError::HandleUnchanged => {
                Problem::invalid_request("That is already the account's handle.")
            }
            AccountError::RenamedTooRecently => Problem::rate_limited(
                "Too many handle changes recently. Please wait before changing it again.",
            ),
            AccountError::NoPendingInvitation => Problem::no_pending_invitation(),
            AccountError::OwnerCannotLeave => Problem::owner_cannot_leave(),
        })?;

    let body = GrantRoleResponse {
        account: granted.account_id.to_string(),
        role: granted.role.to_string(),
        user: granted.user_did.as_str().to_owned(),
    };
    let response = (StatusCode::OK, Json(body)).into_response();
    Ok(response)
}

/// The body of `DELETE /accounts/{id}/members`: the member to revoke, named by
/// `did`. No role — a revoke removes the membership whatever role it holds.
///
/// Example: `{ "user": "did:plc:abc123" }`.
#[derive(Deserialize)]
struct RevokeRoleBody {
    user: String,
}

/// `DELETE /accounts/{id}/members`'s `200` body — see [`revoke_role`].
#[derive(Serialize)]
struct RevokeRoleResponse {
    account: String,
    user: String,
}

/// `DELETE /accounts/{id}/members` — revokes a member's role, the inverse of
/// `grant_role`.
///
/// - `200 { "account", "user" }`
/// - `401` — not signed in · `403` — not allowed to revoke that member
/// - `404` — no such account, or not a member
/// - `422` — malformed body
async fn revoke_role(
    State(state): State<AppState>,
    account_id: Result<Path<Uuid>, PathRejection>,
    session: Session,
    body: Result<Json<RevokeRoleBody>, JsonRejection>,
) -> Result<Response, Problem> {
    let user = require_user(&state, &session).await?;
    let Path(account_id) = account_id.map_err(|_| Problem::account_not_found())?;
    let Json(body) = body.map_err(|_| {
        Problem::invalid_request("Provide a member to revoke, e.g. {\"user\": \"did:plc:…\"}.")
    })?;

    let command = RevokeRoleCommand {
        account_id: AccountId::new(account_id),
        revoking_actor_id: user.id,
        target: Did::new(body.user),
    };
    let revoked = application::account::revoke_role(command, account_ports(&state))
        .await
        .map_err(|err| match err {
            AccountError::NonExistent => Problem::account_not_found(),
            AccountError::IncorrectRole => Problem::forbidden(),
            AccountError::NotAMember => Problem::member_not_found(),
            AccountError::Infrastructure(err) => Problem::from(err),
            // Unreachable from revoking: no grant target, no DID provisioning, no
            // handle, no rename, no invitation, no leave.
            AccountError::IncorrectTransferOfAccount => Problem::forbidden(),
            AccountError::DidBelongsToAnotherActor => Problem::did_belongs_to_another_actor(),
            AccountError::AlreadyMember => {
                Problem::already_member("That user is already a member of this account.")
            }
            AccountError::CannotTransferToSelf => Problem::invalid_request(
                "You already own this account; transfer ownership to another member.",
            ),
            AccountError::HandleTaken => Problem::handle_taken(),
            AccountError::UnsupportedHandle => Problem::unsupported_handle(
                "Changing to a handle outside the Zurfur namespace isn't supported yet.",
            ),
            AccountError::HandleUnchanged => {
                Problem::invalid_request("That is already the account's handle.")
            }
            AccountError::RenamedTooRecently => Problem::rate_limited(
                "Too many handle changes recently. Please wait before changing it again.",
            ),
            AccountError::NoPendingInvitation => Problem::no_pending_invitation(),
            AccountError::OwnerCannotLeave => Problem::owner_cannot_leave(),
        })?;

    let body = RevokeRoleResponse {
        account: revoked.account_id.to_string(),
        user: revoked.user_did.as_str().to_owned(),
    };
    let response = (StatusCode::OK, Json(body)).into_response();
    Ok(response)
}

/// The body of `POST /accounts/{id}/invitations`: invitee by `did`, and `role`
/// (`"admin" | "manager" | "member"`; `"owner"` never offerable by invitation).
///
/// Example: `{ "user": "did:plc:abc123", "role": "member" }`.
#[derive(Deserialize)]
struct InviteUserToAccountBody {
    user: String,
    role: String,
}

/// `POST /accounts/{id}/invitations`'s response body (both the idempotent
/// `200` re-invite and the minted `201`) — see [`invite_user_to_account`].
#[derive(Serialize)]
struct InviteUserToAccountResponse {
    account: String,
    id: String,
    role: String,
    state: String,
    user: String,
}

/// `POST /accounts/{id}/invitations` — issues a pending invitation. A
/// duplicate invite is idempotent (existing offer returned, `200`); a fresh
/// offer is `201`.
///
/// - `200`/`201 { "account", "id", "role", "state", "user" }`
/// - `401` — not signed in · `403` — not allowed to invite that role
/// - `409` — invitee is already a member
/// - `422` — malformed body or an unknown role discriminant
async fn invite_user_to_account(
    State(state): State<AppState>,
    account_id: Result<Path<Uuid>, PathRejection>,
    session: Session,
    body: Result<Json<InviteUserToAccountBody>, JsonRejection>,
) -> Result<Response, Problem> {
    let user = require_user(&state, &session).await?;
    let Path(account_id) = account_id.map_err(|_| Problem::account_not_found())?;
    let Json(body) = body.map_err(|_| {
        Problem::invalid_request(
            "Provide a user to invite and a role, e.g. {\"user\": \"did:plc:…\", \"role\": \"member\"}.",
        )
    })?;
    let role = body
        .role
        .parse::<Role>()
        .map_err(|err| Problem::unknown_role(err.to_string()))?;

    let command = InviteToAccountCommand {
        account_id: AccountId::new(account_id),
        inviting_actor_id: user.id,
        invitee: Did::new(body.user),
        role,
    };
    let invited =
        application::account::invite_to_account(command, account_ports(&state), Utc::now())
            .await
            .map_err(|err| match err {
                AccountError::NonExistent => Problem::account_not_found(),
                AccountError::IncorrectRole => Problem::forbidden(),
                AccountError::DidBelongsToAnotherActor => Problem::did_belongs_to_another_actor(),
                AccountError::AlreadyMember => {
                    Problem::already_member("That user is already a member of this account.")
                }
                AccountError::Infrastructure(err) => Problem::from(err),
                // Unreachable from inviting: no handle, no rename, no leave, no
                // transfer, no accept, no self-transfer.
                AccountError::HandleTaken => Problem::handle_taken(),
                AccountError::UnsupportedHandle => Problem::unsupported_handle(
                    "Changing to a handle outside the Zurfur namespace isn't supported yet.",
                ),
                AccountError::HandleUnchanged => {
                    Problem::invalid_request("That is already the account's handle.")
                }
                AccountError::RenamedTooRecently => Problem::rate_limited(
                    "Too many handle changes recently. Please wait before changing it again.",
                ),
                AccountError::NoPendingInvitation => Problem::no_pending_invitation(),
                AccountError::NotAMember => Problem::member_not_found(),
                AccountError::OwnerCannotLeave => Problem::owner_cannot_leave(),
                AccountError::IncorrectTransferOfAccount => Problem::forbidden(),
                AccountError::CannotTransferToSelf => Problem::invalid_request(
                    "You already own this account; transfer ownership to another member.",
                ),
            })?;

    let status = match invited.outcome {
        InviteOutcome::Minted => StatusCode::CREATED,
        InviteOutcome::AlreadyPending => StatusCode::OK,
    };
    let offer = InviteUserToAccountResponse {
        id: invited.invitation_id.to_string(),
        account: invited.account_id.to_string(),
        user: invited.invitee_did.as_str().to_owned(),
        role: invited.role.to_string(),
        state: invited.state.to_string(),
    };
    let response = (status, Json(offer)).into_response();
    Ok(response)
}

/// The body of `DELETE /accounts/{id}/invitations`: the invitation, addressed
/// by the invited User's `did` (at most one pending offer per account/user).
///
/// Example: `{ "user": "did:plc:abc123" }`.
#[derive(Deserialize)]
struct RevokeInvitationBody {
    user: String,
}

/// `DELETE /accounts/{id}/invitations`'s `200` body — see
/// [`revoke_invitation_to_account`].
#[derive(Serialize)]
struct RevokeInvitationResponse {
    account: String,
    user: String,
}

/// `DELETE /accounts/{id}/invitations` — revokes a pending invitation.
/// Idempotent: unknown user or no pending offer is a `200` no-op.
///
/// - `200 { "account", "user" }`
/// - `401` — not signed in · `403` — not allowed to revoke that invitation
async fn revoke_invitation_to_account(
    State(state): State<AppState>,
    account_id: Result<Path<Uuid>, PathRejection>,
    session: Session,
    body: Result<Json<RevokeInvitationBody>, JsonRejection>,
) -> Result<Response, Problem> {
    let user = require_user(&state, &session).await?;
    let Path(account_id) = account_id.map_err(|_| Problem::account_not_found())?;
    let Json(body) = body.map_err(|_| {
        Problem::invalid_request(
            "Provide the invited user to revoke, e.g. {\"user\": \"did:plc:…\"}.",
        )
    })?;
    let invited_did = body.user.clone();

    let command = RevokeInvitationCommand {
        account_id: AccountId::new(account_id),
        revoking_actor_id: user.id,
        invitee: Did::new(body.user),
    };
    application::account::revoke_invitation(command, account_ports(&state), Utc::now())
        .await
        .map_err(|err| match err {
            AccountError::NonExistent => Problem::account_not_found(),
            AccountError::IncorrectRole => Problem::forbidden(),
            AccountError::Infrastructure(err) => Problem::from(err),
            // Unreachable from revoking an invitation: no grant target, no DID
            // provisioning, no handle, no rename, no membership check, no leave,
            // no transfer, no accept.
            AccountError::HandleTaken => Problem::handle_taken(),
            AccountError::UnsupportedHandle => Problem::unsupported_handle(
                "Changing to a handle outside the Zurfur namespace isn't supported yet.",
            ),
            AccountError::HandleUnchanged => {
                Problem::invalid_request("That is already the account's handle.")
            }
            AccountError::RenamedTooRecently => Problem::rate_limited(
                "Too many handle changes recently. Please wait before changing it again.",
            ),
            AccountError::NoPendingInvitation => Problem::no_pending_invitation(),
            AccountError::NotAMember => Problem::member_not_found(),
            AccountError::OwnerCannotLeave => Problem::owner_cannot_leave(),
            AccountError::IncorrectTransferOfAccount => Problem::forbidden(),
            AccountError::DidBelongsToAnotherActor => Problem::did_belongs_to_another_actor(),
            AccountError::AlreadyMember => {
                Problem::already_member("That user is already a member of this account.")
            }
            AccountError::CannotTransferToSelf => Problem::invalid_request(
                "You already own this account; transfer ownership to another member.",
            ),
        })?;

    let response_body = RevokeInvitationResponse {
        account: account_id.to_string(),
        user: invited_did,
    };
    let response = (StatusCode::OK, Json(response_body)).into_response();
    Ok(response)
}

/// `POST /accounts/{id}/invitations/decline`'s `200` body: the declined
/// offer's account and the declining user — see [`decline_invitation`].
#[derive(Serialize)]
struct DeclineInvitationResponse {
    account: String,
    user: String,
}

/// `POST /accounts/{id}/invitations/decline` — the invitee declines their own
/// pending invitation.
///
/// - `200 { "account", "user" }`
/// - `404 no_pending_invitation` — nothing pending for this user
async fn decline_invitation(
    State(state): State<AppState>,
    session: Session,
    Path(account_id): Path<Uuid>,
) -> Result<Response, Problem> {
    let actor = require_user(&state, &session).await?;

    let command = DeclineInvitationCommand {
        account_id: AccountId::new(account_id),
        declining_actor: actor.id,
    };
    application::account::decline_invitation(command, account_ports(&state), Utc::now())
        .await
        .map_err(|err| match err {
            AccountError::NonExistent => Problem::account_not_found(),
            AccountError::NoPendingInvitation => Problem::no_pending_invitation(),
            AccountError::Infrastructure(err) => Problem::from(err),
            // Unreachable from declining: no rank check, no grant, no handle, no
            // rename, no transfer, no accept.
            AccountError::IncorrectRole => Problem::forbidden(),
            AccountError::NotAMember => Problem::member_not_found(),
            AccountError::OwnerCannotLeave => Problem::owner_cannot_leave(),
            AccountError::IncorrectTransferOfAccount => Problem::forbidden(),
            AccountError::DidBelongsToAnotherActor => Problem::did_belongs_to_another_actor(),
            AccountError::HandleTaken => Problem::handle_taken(),
            AccountError::UnsupportedHandle => Problem::unsupported_handle(
                "Changing to a handle outside the Zurfur namespace isn't supported yet.",
            ),
            AccountError::HandleUnchanged => {
                Problem::invalid_request("That is already the account's handle.")
            }
            AccountError::RenamedTooRecently => Problem::rate_limited(
                "Too many handle changes recently. Please wait before changing it again.",
            ),
            AccountError::AlreadyMember => {
                Problem::already_member("That user is already a member of this account.")
            }
            AccountError::CannotTransferToSelf => Problem::invalid_request(
                "You already own this account; transfer ownership to another member.",
            ),
        })?;

    let body = DeclineInvitationResponse {
        account: account_id.to_string(),
        user: actor.did.as_str().to_owned(),
    };
    let response = (StatusCode::OK, Json(body)).into_response();
    Ok(response)
}

/// The body of `POST /accounts/{id}/transfer`: the incoming Owner, named by
/// `new_owner` DID.
///
/// Example: `{ "new_owner": "did:plc:abc123" }`.
#[derive(Deserialize)]
struct TransferOwnershipBody {
    new_owner: String,
}

/// `POST /accounts/{id}/transfer`'s `200` body — see [`transfer_ownership`].
#[derive(Serialize)]
struct TransferOwnershipResponse {
    account: String,
    owner: String,
    previous_owner: String,
}

/// `POST /accounts/{id}/transfer` — transfers ownership to another existing
/// member, immediately and unilaterally (no recipient acceptance, no PLC
/// write — the account's `did:plc` is stable).
///
/// - `200 { "account", "owner", "previous_owner" }`
/// - `401` — not signed in · `403` — not the account's current Owner
/// - `404` — no such account, or `new_owner` is not a member
/// - `422` — malformed body, or transferring to oneself
async fn transfer_ownership(
    State(state): State<AppState>,
    account_id: Result<Path<Uuid>, PathRejection>,
    session: Session,
    body: Result<Json<TransferOwnershipBody>, JsonRejection>,
) -> Result<Response, Problem> {
    let user = require_user(&state, &session).await?;
    let Path(account_id) = account_id.map_err(|_| Problem::account_not_found())?;
    let Json(body) = body.map_err(|_| {
        Problem::invalid_request("Provide the new owner, e.g. {\"new_owner\": \"did:plc:…\"}.")
    })?;

    let command = TransferOwnershipCommand {
        account_id: AccountId::new(account_id),
        old_owner: user.id,
        new_owner: Did::new(body.new_owner),
    };
    let transferred = application::account::transfer_ownership(command, account_ports(&state))
        .await
        .map_err(|err| match err {
            AccountError::NonExistent => Problem::account_not_found(),
            AccountError::IncorrectRole => Problem::forbidden(),
            AccountError::NotAMember => Problem::member_not_found(),
            AccountError::CannotTransferToSelf => Problem::invalid_request(
                "You already own this account; transfer ownership to another member.",
            ),
            AccountError::Infrastructure(err) => Problem::from(err),
            // Unreachable from transferring: no handle, no rename, no invitation,
            // no leave, no grant, no accept.
            AccountError::HandleTaken => Problem::handle_taken(),
            AccountError::UnsupportedHandle => Problem::unsupported_handle(
                "Changing to a handle outside the Zurfur namespace isn't supported yet.",
            ),
            AccountError::HandleUnchanged => {
                Problem::invalid_request("That is already the account's handle.")
            }
            AccountError::RenamedTooRecently => Problem::rate_limited(
                "Too many handle changes recently. Please wait before changing it again.",
            ),
            AccountError::NoPendingInvitation => Problem::no_pending_invitation(),
            AccountError::OwnerCannotLeave => Problem::owner_cannot_leave(),
            AccountError::IncorrectTransferOfAccount => Problem::forbidden(),
            AccountError::DidBelongsToAnotherActor => Problem::did_belongs_to_another_actor(),
            AccountError::AlreadyMember => {
                Problem::already_member("That user is already a member of this account.")
            }
        })?;

    let body = TransferOwnershipResponse {
        account: transferred.account_id.to_string(),
        owner: transferred.new_owner_did.as_str().to_owned(),
        previous_owner: transferred.previous_owner_did.as_str().to_owned(),
    };
    let response = (StatusCode::OK, Json(body)).into_response();
    Ok(response)
}

#[cfg(test)]
mod tests {
    //! Pins each response body's wire shape: every field a string.

    use super::*;

    #[test]
    fn accept_invitation_response_serializes_every_field_as_a_string() {
        let body = AcceptInvitationResponse {
            account: "account-id".to_string(),
            role: "member".to_string(),
            user: "did:plc:invitee".to_string(),
        };
        assert_eq!(
            serde_json::to_string(&body).unwrap(),
            r#"{"account":"account-id","role":"member","user":"did:plc:invitee"}"#
        );
    }

    #[test]
    fn grant_role_response_serializes_every_field_as_a_string() {
        let body = GrantRoleResponse {
            account: "account-id".to_string(),
            role: "admin".to_string(),
            user: "did:plc:grantee".to_string(),
        };
        assert_eq!(
            serde_json::to_string(&body).unwrap(),
            r#"{"account":"account-id","role":"admin","user":"did:plc:grantee"}"#
        );
    }

    #[test]
    fn revoke_role_response_serializes_every_field_as_a_string() {
        let body = RevokeRoleResponse {
            account: "account-id".to_string(),
            user: "did:plc:target".to_string(),
        };
        assert_eq!(
            serde_json::to_string(&body).unwrap(),
            r#"{"account":"account-id","user":"did:plc:target"}"#
        );
    }

    #[test]
    fn invite_user_to_account_response_serializes_every_field_as_a_string() {
        let body = InviteUserToAccountResponse {
            account: "account-id".to_string(),
            id: "offer-id".to_string(),
            role: "member".to_string(),
            state: "pending".to_string(),
            user: "did:plc:invitee".to_string(),
        };
        assert_eq!(
            serde_json::to_string(&body).unwrap(),
            r#"{"account":"account-id","id":"offer-id","role":"member","state":"pending","user":"did:plc:invitee"}"#
        );
    }

    #[test]
    fn revoke_invitation_response_serializes_every_field_as_a_string() {
        let body = RevokeInvitationResponse {
            account: "account-id".to_string(),
            user: "did:plc:invitee".to_string(),
        };
        assert_eq!(
            serde_json::to_string(&body).unwrap(),
            r#"{"account":"account-id","user":"did:plc:invitee"}"#
        );
    }

    #[test]
    fn decline_invitation_response_serializes_every_field_as_a_string() {
        let body = DeclineInvitationResponse {
            account: "account-id".to_string(),
            user: "did:plc:invitee".to_string(),
        };
        assert_eq!(
            serde_json::to_string(&body).unwrap(),
            r#"{"account":"account-id","user":"did:plc:invitee"}"#
        );
    }

    #[test]
    fn transfer_ownership_response_serializes_every_field_as_a_string() {
        let body = TransferOwnershipResponse {
            account: "account-id".to_string(),
            owner: "did:plc:new-owner".to_string(),
            previous_owner: "did:plc:old-owner".to_string(),
        };
        assert_eq!(
            serde_json::to_string(&body).unwrap(),
            r#"{"account":"account-id","owner":"did:plc:new-owner","previous_owner":"did:plc:old-owner"}"#
        );
    }
}
