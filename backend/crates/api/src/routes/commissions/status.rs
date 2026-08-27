//! `PUT`/`DELETE /commissions/{id}/status/direction` — a Participant sets or
//! clears the commission's direction-axis Status. Always an explicit
//! Participant act; a set replaces the current value.

use axum::{
    Json,
    extract::{Path, State, rejection::JsonRejection},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use chrono::Utc;
use domain::{
    elements::{
        commission::{ChangelogEntryKind, CommissionId, DirectionStatus, NewChangelogEntry},
        user::UserId,
    },
    ports::UnitOfWork,
};
use serde::Deserialize;
use serde_json::json;
use tower_sessions::Session;
use uuid::Uuid;

use crate::{AppState, problem::Problem};

/// The `PUT /commissions/{id}/status/direction` request body: the direction
/// value's stable wire token (`waiting_for_input` / `waiting_for_approval` /
/// `changes_requested`). Clearing is `DELETE`, not a null body.
#[derive(Deserialize)]
pub(super) struct SetDirectionStatusBody {
    status: String,
}

/// Sets (or replaces) the commission's direction status. Any-Participant-gated;
/// `422` for a token outside the vocabulary; idempotent on an unchanged
/// value. Returns `204 No Content`.
pub(super) async fn set_direction_status(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    session: Session,
    body: Result<Json<SetDirectionStatusBody>, JsonRejection>,
) -> Result<Response, Problem> {
    let user = super::current_user(&state, &session).await?;
    let commission = CommissionId::new(id);
    super::require_participant(&state, commission, user.id).await?;

    let Json(body) = body.map_err(|_| Problem::invalid_request("Malformed request body."))?;
    let status = DirectionStatus::try_from(body.status.as_str()).map_err(|_| {
        Problem::invalid_request(format!(
            "Unknown direction status {:?}; expected one of: {}.",
            body.status,
            DirectionStatus::ALL
                .iter()
                .map(|s| s.as_str())
                .collect::<Vec<_>>()
                .join(", "),
        ))
    })?;

    apply_direction_status(&state, commission, user.id, Some(status)).await
}

/// Clears the commission's direction status. Any-Participant-gated and
/// idempotent. Returns `204 No Content`.
pub(super) async fn clear_direction_status(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    session: Session,
) -> Result<Response, Problem> {
    let user = super::current_user(&state, &session).await?;
    let commission = CommissionId::new(id);
    super::require_participant(&state, commission, user.id).await?;

    apply_direction_status(&state, commission, user.id, None).await
}

/// Shared set/clear tail: writes the direction status and its changelog
/// entry in one unit of work, appending only on a real change.
async fn apply_direction_status(
    state: &AppState,
    commission: CommissionId,
    actor: UserId,
    to: Option<DirectionStatus>,
) -> Result<Response, Problem> {
    let found = state
        .commissions
        .find(commission)
        .await?
        .ok_or_else(Problem::commission_not_found)?;
    let from = found.direction_status;
    if from == to {
        return Ok(StatusCode::NO_CONTENT.into_response());
    }

    let entry = NewChangelogEntry::event(
        commission,
        ChangelogEntryKind::StatusChanged,
        actor,
        json!({
            "from": from.map(|s| s.as_str()),
            "to": to.map(|s| s.as_str()),
        }),
        Utc::now(),
    );
    state
        .transaction(async move |uow: &mut dyn UnitOfWork| {
            if uow
                .commissions()
                .set_direction_status(commission, to)
                .await?
            {
                uow.changelog().append(&entry).await?;
            }
            Ok(())
        })
        .await?;

    Ok(StatusCode::NO_CONTENT.into_response())
}
