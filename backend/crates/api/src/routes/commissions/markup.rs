//! `POST /commissions/{id}/files/{file_id}/markup` — a Participant attaches a
//! Markup to a file entry. Validated strictly and stored raw, append-only;
//! never moves any status.

use axum::{
    Json,
    extract::{Path, State, rejection::JsonRejection},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use chrono::Utc;
use domain::{
    elements::commission::{ChangelogEntryKind, CommissionId, FileKey, Markup, NewChangelogEntry},
    ports::UnitOfWork,
};
use serde_json::json;
use tower_sessions::Session;
use uuid::Uuid;

use crate::{AppState, problem::Problem};

/// Attaches a Markup to a file entry. Any-Participant-gated; `404
/// file_not_found` for a file not in this commission, `422` for an invalid
/// markup shape. Lands as a `markup_added` changelog entry. Returns
/// `201 Created`.
///
/// ⚠️ contract-decision-needed: `Markup` is an unschematized passthrough
/// (tracks `VERSIONING.md` §8 Q9).
pub(super) async fn add_markup(
    State(state): State<AppState>,
    Path((id, file_id)): Path<(Uuid, Uuid)>,
    session: Session,
    body: Result<Json<Markup>, JsonRejection>,
) -> Result<Response, Problem> {
    let user = super::current_user(&state, &session).await?;
    let commission = CommissionId::new(id);
    super::require_participant(&state, commission, user.id).await?;

    let key = FileKey::new(file_id);
    state
        .commissions
        .find_file(commission, key)
        .await?
        .ok_or_else(Problem::file_not_found)?;

    let Json(markup) = body.map_err(|rejection| Problem::invalid_request(rejection.body_text()))?;
    markup
        .validate()
        .map_err(|e| Problem::invalid_request(format!("Invalid markup: {e}.")))?;

    let entry = NewChangelogEntry::event(
        commission,
        ChangelogEntryKind::MarkupAdded,
        user.id,
        json!({
            "file_id": *key,
            "markup": markup,
        }),
        Utc::now(),
    );
    state
        .transaction(async move |uow: &mut dyn UnitOfWork| uow.changelog().append(&entry).await)
        .await?;

    Ok(StatusCode::CREATED.into_response())
}
