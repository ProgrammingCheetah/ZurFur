//! The deadline axis: `PUT`/`DELETE /commissions/{id}/deadline` (the deadline
//! envelope field) and `PUT`/`DELETE /commissions/{id}/status/deadline` (the
//! manual Delayed flag). `Late` is system-derived and never accepted here.

use application::commission::deadline::{
    ClearDeadlineCommand, SetDeadlineCommand, SetDeadlineStatusCommand,
};
use axum::{
    Json,
    extract::{Path, State, rejection::JsonRejection},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use chrono::Utc;
use domain::{
    datetime::DateTimeUtc,
    elements::{
        commission::{ChangelogEntryKind, CommissionId, DeadlineStatus, NewChangelogEntry},
        user::UserId,
    },
    ports::UnitOfWork,
};
use serde::Deserialize;
use serde_json::json;
use tower_sessions::Session;
use uuid::Uuid;

use crate::{AppState, problem::Problem, routes::commissions::ports::commission_ports};

/// The `PUT /commissions/{id}/deadline` request body: the new deadline as an
/// RFC 3339 timestamp. Clearing is `DELETE`, not a null body.
#[derive(Deserialize)]
pub(super) struct SetDeadlineBody {
    deadline: crate::wire_time::WireTimestamp,
}

/// The `PUT /commissions/{id}/status/deadline` request body: the deadline-axis
/// token to set. Only `delayed` — the manual slipping flag — is a Participant's
/// to set; `late` is refused (the system's word). Clearing is `DELETE`.
#[derive(Deserialize)]
pub(super) struct SetDeadlineStatusBody {
    status: String,
}

/// Sets (or moves) the commission's deadline. Any-Participant-gated;
/// idempotent on an unchanged value. Records `deadline_set` or
/// `deadline_extended`. Returns `204 No Content`.
pub(super) async fn set_deadline(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    session: Session,
    body: Result<Json<SetDeadlineBody>, JsonRejection>,
) -> Result<Response, Problem> {
    let user = super::current_user(&state, &session).await?;

    let Json(body) = body.map_err(|_| Problem::invalid_request("Malformed request body."))?;
    let deadline = super::from_wire_timestamp(body.deadline)
        .ok_or_else(|| Problem::invalid_request("Deadline outside the representable range."))?;
    let command = SetDeadlineCommand {
        actor_id: user.id,
        commission_id: CommissionId::new(id),
        deadline,
    };

    let ports = commission_ports(&state);
    let now = Utc::now();
    application::commission::deadline::set(command, ports, now)
        .await
        // FIXME: Claude -- Fix errors
        .map_err(Problem::service_unavailable)?;
    Ok(StatusCode::NO_CONTENT.into_response())
}

/// Clears the commission's deadline, wiping the deadline axis with it.
/// Any-Participant-gated and idempotent. Returns `204 No Content`.
pub(super) async fn clear_deadline(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    session: Session,
) -> Result<Response, Problem> {
    let user = super::current_user(&state, &session).await?;

    let command = ClearDeadlineCommand {
        actor_id: user.id,
        commission_id: CommissionId::new(id),
    };

    let ports = commission_ports(&state);

    let now = Utc::now();
    application::commission::deadline::clear(command, ports, now)
        .await
        // FIXME: Claude -- Fix errors
        .map_err(Problem::service_unavailable)?;
    Ok(StatusCode::NO_CONTENT.into_response())
}

/// Sets the manual Delayed status. Any-Participant-gated; only `delayed` is
/// accepted (`late` is system-only, `422`). `409` with no deadline or if
/// already Late. Idempotent. Returns `204 No Content`.
pub(super) async fn set_deadline_status(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    session: Session,
    body: Result<Json<SetDeadlineStatusBody>, JsonRejection>,
) -> Result<Response, Problem> {
    let user = super::current_user(&state, &session).await?;
    let commission = CommissionId::new(id);
    let Json(body) = body.map_err(|_| Problem::invalid_request("Malformed request body."))?;

    let command = SetDeadlineStatusCommand {
        actor_id: user.id,
        commission_id: CommissionId::new(id),
        status: body.status.parse::<DeadlineStatus>()?,
    };
    let ports = commission_ports(&state);

    let now = Utc::now();
    application::commission::deadline::set_status(command, ports, now)
        .await
        // FIXME: Claude -- Add errors
        .map_err(Problem::service_unavailable)?;

    Ok(StatusCode::NO_CONTENT.into_response())
}

/// Clears the manual Delayed flag. Any-Participant-gated and idempotent; a
/// standing Late is a `409` (`commission_late`) — extend or clear the
/// deadline instead. Returns `204 No Content`.
pub(super) async fn clear_deadline_status(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    session: Session,
) -> Result<Response, Problem> {
    let user = super::current_user(&state, &session).await?;
    let command = ClearDeadlineCommand {
        actor_id: user.id,
        commission_id: CommissionId::new(id),
    };

    let ports = commission_ports(&state);
    let now = Utc::now();
    application::commission::deadline::clear(command, ports, now)
        .await
        // FIXME: Claude -- Fix error
        .map_err(Problem::service_unavailable)?;

    Ok(StatusCode::NO_CONTENT.into_response())
}
