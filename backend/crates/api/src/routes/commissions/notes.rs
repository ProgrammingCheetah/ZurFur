//! `POST /commissions/{id}/notes` — a Participant writes a free-text note into
//! the changelog stream. Standalone; no reply affordances.

use axum::{
    Json,
    extract::{Path, State, rejection::JsonRejection},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use chrono::Utc;
use domain::{
    elements::commission::{CommissionId, NewChangelogEntry},
    ports::UnitOfWork,
    string_builder::StringBuilder,
};
use serde::Deserialize;
use tower_sessions::Session;
use uuid::Uuid;

use crate::{AppState, problem::Problem};

/// The `POST /commissions/{id}/notes` request body: just the free text.
#[derive(Deserialize)]
pub(super) struct WriteNoteBody {
    note: String,
}

/// Appends a standalone note entry to the commission's stream.
/// Participant-only; `422` for an empty note. Returns `201 Created`.
pub(super) async fn write_note(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    session: Session,
    body: Result<Json<WriteNoteBody>, JsonRejection>,
) -> Result<Response, Problem> {
    let user = super::current_user(&state, &session).await?;
    let commission = CommissionId::new(id);
    super::require_participant(&state, commission, user.id).await?;

    let Json(body) = body.map_err(|_| Problem::invalid_request("Malformed request body."))?;
    let text = StringBuilder::new(body.note)
        .trimmed()
        .non_empty()
        .build()
        .map_err(|_| Problem::invalid_request("A note must not be empty."))?;

    let entry = NewChangelogEntry::note(commission, user.id, text, Utc::now());
    state
        .transaction(async move |uow: &mut dyn UnitOfWork| uow.changelog().append(&entry).await)
        .await?;

    Ok(StatusCode::CREATED.into_response())
}
