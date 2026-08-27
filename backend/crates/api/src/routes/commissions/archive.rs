//! `POST /commissions/{id}/archive` / `POST /commissions/{id}/unarchive` — the
//! owner archives, or un-archives, a commission (the soft-delete path, DD
//! `3014657`).

use application::commission::{
    CommissionError, archive::ArchiveCommissionCommand, unarchive::UnarchiveCommissionCommand,
};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use chrono::Utc;
use domain::{
    elements::commission::{ChangelogEntryKind, CommissionId, NewChangelogEntry},
    ports::UnitOfWork,
};
use serde_json::json;
use tower_sessions::Session;
use uuid::Uuid;

use super::require_owner;
use crate::{AppState, problem::Problem, routes::commissions::ports::commission_ports};

/// Archives the commission: owner-only, leaves the active views while the
/// record survives. Idempotent — archiving an already-archived commission is
/// a no-op. Returns `204 No Content`.
pub(super) async fn archive_commission(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    session: Session,
) -> Result<Response, Problem> {
    let user = super::current_user(&state, &session).await?;
    let commission_id = CommissionId::new(id);
    let now = Utc::now();

    let command = ArchiveCommissionCommand {
        actor_id: user.id,
        commission_id,
    };

    let ports = commission_ports(&state);
    let outcome = application::commission::archive(command, ports, now).await;
    match outcome {
        // Idempotent: archiving an archived commission is a no-op.
        Ok(_) | Err(CommissionError::CommissionAlreadyAtState) => {}
        Err(err) => return Err(Problem::from(err)),
    }

    Ok(StatusCode::NO_CONTENT.into_response())
}

/// Un-archives the commission, returning it to active views. Owner-only and
/// idempotent, mirroring [`archive_commission`]. Returns `204 No Content`.
pub(super) async fn unarchive_commission(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    session: Session,
) -> Result<Response, Problem> {
    let user = super::current_user(&state, &session).await?;

    let command = UnarchiveCommissionCommand {
        commission_id: CommissionId::new(id),
        actor_id: user.id,
    };
    let ports = commission_ports(&state);

    let now = Utc::now();
    application::commission::unarchive(command, ports, now)
        .await
        // FIXME: CLAUDE -- Add the correct errors in here, please
        .map_err(Problem::service_unavailable)?;
    Ok(StatusCode::NO_CONTENT.into_response())
}
