//! `PUT`/`DELETE /commissions/{id}/channel` — declare or clear the commission's
//! external linked-channel pointer.

use axum::{
    Json,
    extract::{Path, State, rejection::JsonRejection},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use chrono::Utc;
use domain::{
    elements::commission::{ChangelogEntryKind, ChannelPointer, CommissionId, NewChangelogEntry},
    ports::UnitOfWork,
};
use serde::Deserialize;
use serde_json::json;
use tower_sessions::Session;
use uuid::Uuid;

use super::require_owner;
use crate::{AppState, problem::Problem};

/// The `PUT /commissions/{id}/channel` request body: the raw pointer text.
#[derive(Deserialize)]
pub(super) struct LinkChannelBody {
    channel: String,
}

/// Declares (or replaces) the commission's linked channel. Owner-only;
/// `422` on an invalid pointer. Idempotent — re-declaring the same pointer is
/// a no-op. Returns `204 No Content`.
pub(super) async fn link_channel(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    session: Session,
    body: Result<Json<LinkChannelBody>, JsonRejection>,
) -> Result<Response, Problem> {
    let user = super::current_user(&state, &session).await?;
    let commission = CommissionId::new(id);
    require_owner(&state, commission, &user).await?;

    let Json(body) = body.map_err(|_| Problem::invalid_request("Malformed request body."))?;
    let pointer = ChannelPointer::try_from(body.channel)
        .map_err(|e| Problem::invalid_request(e.to_string()))?;

    let entry = NewChangelogEntry::event(
        commission,
        ChangelogEntryKind::ChannelLinked,
        user.id,
        json!({ "channel": pointer.as_str() }),
        Utc::now(),
    );
    state
        .transaction(async move |uow: &mut dyn UnitOfWork| {
            let changed = uow
                .commissions()
                .set_linked_channel(commission, Some(&pointer))
                .await?;
            if changed {
                uow.changelog().append(&entry).await?;
            }
            Ok(())
        })
        .await?;

    Ok(StatusCode::NO_CONTENT.into_response())
}

/// Clears the commission's linked channel. Owner-only and idempotent — no
/// entry is appended if there was nothing to clear. Returns `204 No Content`.
pub(super) async fn clear_channel(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    session: Session,
) -> Result<Response, Problem> {
    let user = super::current_user(&state, &session).await?;
    let commission = CommissionId::new(id);
    let found = require_owner(&state, commission, &user).await?;

    let Some(previous) = found.linked_channel else {
        return Ok(StatusCode::NO_CONTENT.into_response());
    };

    let entry = NewChangelogEntry::event(
        commission,
        ChangelogEntryKind::ChannelUnlinked,
        user.id,
        json!({ "channel": previous.as_str() }),
        Utc::now(),
    );
    state
        .transaction(async move |uow: &mut dyn UnitOfWork| {
            let changed = uow
                .commissions()
                .set_linked_channel(commission, None)
                .await?;
            if changed {
                uow.changelog().append(&entry).await?;
            }
            Ok(())
        })
        .await?;

    Ok(StatusCode::NO_CONTENT.into_response())
}
