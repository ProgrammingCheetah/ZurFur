//! Account positioning endpoints (Ownership Separation DD `29130754`): the
//! owner places a commission in an account's position, and manages that
//! account's view grants (`/placements`, `/grants`). Owner-only in v1.

use axum::{
    Json,
    extract::{Path, State, rejection::JsonRejection},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use chrono::Utc;
use domain::{
    elements::{
        account::{Account, AccountId},
        commission::{ChangelogEntryKind, CommissionId, GrantLevel, NewChangelogEntry},
    },
    ports::UnitOfWork,
};
use serde::Deserialize;
use serde_json::json;
use tower_sessions::Session;
use uuid::Uuid;

use super::require_owner;
use crate::{AppState, problem::Problem};

/// The `POST /commissions/{id}/placements` body: the target account.
#[derive(Deserialize)]
pub(super) struct PlaceBody {
    account_id: String,
}

/// The `POST /commissions/{id}/grants` body: the target account and the key's
/// level (`presentation` / `description` / `total`).
#[derive(Deserialize)]
pub(super) struct GrantBody {
    account_id: String,
    level: String,
}

/// Parses a body-supplied account id and resolves it to a live [`Account`].
/// `422` for a malformed id; `404 account_not_found` for one that resolves to
/// nothing (absent or soft-deleted).
async fn resolve_live_account(state: &AppState, raw: &str) -> Result<Account, Problem> {
    let account = AccountId::new(
        Uuid::parse_str(raw).map_err(|_| Problem::invalid_request("Malformed account id."))?,
    );
    state
        .accounts
        .find(account)
        .await?
        .ok_or_else(Problem::account_not_found)
}

/// Places the commission in an account's position: appends a placement-log
/// row and repoints the current-placement pointer, atomically. Owner-only.
/// No changelog entry. Returns `204 No Content`.
pub(super) async fn place_commission(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    session: Session,
    body: Result<Json<PlaceBody>, JsonRejection>,
) -> Result<Response, Problem> {
    let user = super::current_user(&state, &session).await?;
    let commission = CommissionId::new(id);
    require_owner(&state, commission, &user).await?;

    let Json(body) = body.map_err(|_| Problem::invalid_request("Malformed request body."))?;
    let account = resolve_live_account(&state, &body.account_id).await?.id;

    let now = Utc::now();
    state
        .transaction(async move |uow: &mut dyn UnitOfWork| {
            uow.commissions()
                .place(commission, account, user.id, now)
                .await
        })
        .await?;

    Ok(StatusCode::NO_CONTENT.into_response())
}

/// Issues an account a view grant at an explicit level (`presentation` /
/// `description` / `total`). Owner-only; `422` for an unrecognized level.
/// Re-granting replaces the level. Returns `204 No Content`.
pub(super) async fn grant_view(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    session: Session,
    body: Result<Json<GrantBody>, JsonRejection>,
) -> Result<Response, Problem> {
    let user = super::current_user(&state, &session).await?;
    let commission = CommissionId::new(id);
    require_owner(&state, commission, &user).await?;

    let Json(body) = body.map_err(|_| Problem::invalid_request("Malformed request body."))?;
    let level = GrantLevel::parse(&body.level).ok_or_else(|| {
        Problem::invalid_request(
            "level must be one of: presentation, description, total.".to_string(),
        )
    })?;
    let account = resolve_live_account(&state, &body.account_id).await?;
    let account_id = account.id;

    let entry = NewChangelogEntry::event(
        commission,
        ChangelogEntryKind::ViewGrantIssued,
        user.id,
        json!({
            "account_id": *account_id,
            "account_handle": account.handle.as_str(),
            "level": level.as_str(),
        }),
        Utc::now(),
    );
    state
        .transaction(async move |uow: &mut dyn UnitOfWork| {
            uow.commissions()
                .grant_view(commission, account_id, level)
                .await?;
            uow.changelog().append(&entry).await
        })
        .await?;

    Ok(StatusCode::NO_CONTENT.into_response())
}

/// Revokes an account's view grant, hard-deleting the key. Owner-only and
/// idempotent — revoking an account with no key is a no-op. Returns `204 No
/// Content`.
pub(super) async fn revoke_view(
    State(state): State<AppState>,
    Path((id, account_id)): Path<(Uuid, Uuid)>,
    session: Session,
) -> Result<Response, Problem> {
    let user = super::current_user(&state, &session).await?;
    let commission = CommissionId::new(id);
    require_owner(&state, commission, &user).await?;
    let account = AccountId::new(account_id);

    let handle = state
        .accounts
        .find(account)
        .await?
        .map(|a| a.handle.as_str().to_owned());
    let entry = NewChangelogEntry::event(
        commission,
        ChangelogEntryKind::ViewGrantRevoked,
        user.id,
        json!({ "account_id": *account, "account_handle": handle }),
        Utc::now(),
    );
    state
        .transaction(async move |uow: &mut dyn UnitOfWork| {
            let mut commissions = uow.commissions();
            if commissions.revoke_view(commission, account).await? {
                drop(commissions);
                uow.changelog().append(&entry).await?;
            }
            Ok(())
        })
        .await?;

    Ok(StatusCode::NO_CONTENT.into_response())
}
