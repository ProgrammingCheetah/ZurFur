//! `DELETE /commissions/{id}` — the owner hard-deletes a fact-free commission
//! (Deletion DD `3014657`). A fact-bearing commission is refused toward
//! Archive instead.

use application::commission::delete::{
    DeleteCommissionCommand, DeleteCommissionResult, DeleteOutcome,
};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use domain::{elements::commission::CommissionId, ports::UnitOfWork};
use tower_sessions::Session;
use uuid::Uuid;

use crate::{AppState, problem::Problem, routes::commissions::ports::commission_ports};

/// Hard-deletes a fact-free commission. Owner-only; `404` for a caller who
/// may not see it. Fact-free → `204 No Content`; fact-bearing → `409
/// commission_has_facts` pointing the caller at Archive.
pub(super) async fn delete_commission(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    session: Session,
) -> Result<Response, Problem> {
    let user = super::current_user(&state, &session).await?;
    let command = DeleteCommissionCommand {
        actor_id: user.id,
        commission_id: CommissionId::new(id),
    };
    let ports = commission_ports(&state);

    let DeleteCommissionResult { outcome } = application::commission::delete(command, ports)
        .await
        // FIXME: Add the correct err
        .map_err(Problem::service_unavailable)?;
    match outcome {
        DeleteOutcome::Deleted => Ok(StatusCode::NO_CONTENT.into_response()),
        DeleteOutcome::HasFacts => Err(Problem::commission_has_facts()),
    }
}
