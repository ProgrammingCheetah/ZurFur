//! `POST /commissions/{id}/seats` — the owner declares a Seat on the
//! commission (Referenceable/Slot/Seat DD `28311564`): a structural
//! participant position, born vacant, typed by an open kind.

use axum::{
    Json,
    extract::{Path, State, rejection::JsonRejection},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use chrono::Utc;
use domain::{
    elements::commission::{
        ChangelogEntryKind, CommissionId, NewChangelogEntry, NewSeat, SeatKind, SeatLink,
        SeatPrompt,
    },
    ports::UnitOfWork,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tower_sessions::Session;
use uuid::Uuid;

use super::require_owner;
use crate::{AppState, problem::Problem};

/// `POST /commissions/{id}/seats`'s `201` body: the new seat's element id — see
/// [`declare_seat`].
#[derive(Serialize)]
struct DeclareSeatResponse {
    id: Uuid,
}

/// The `POST /commissions/{id}/seats` request body: the address (`tab` +
/// `surface`), the seat's typed `kind`, and optional `prompt`/`link`
/// requirements. No occupant field — seats are born vacant.
#[derive(Deserialize)]
pub(super) struct DeclareSeatBody {
    tab: Uuid,
    surface: String,
    kind: String,
    prompt: Option<String>,
    link: Option<String>,
}

/// Declares a Seat into one of the commission's declared surfaces, as its
/// owner. Owner-only; `422` for a malformed body or invalid prompt/link,
/// `404 tab_not_found` / `422 unknown_surface` for a bad address. Returns
/// `201 Created` with `{"id": "…"}`.
pub(super) async fn declare_seat(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    session: Session,
    body: Result<Json<DeclareSeatBody>, JsonRejection>,
) -> Result<Response, Problem> {
    let user = super::current_user(&state, &session).await?;
    let commission = CommissionId::new(id);
    require_owner(&state, commission, &user).await?;

    let Json(body) = body.map_err(|_| Problem::invalid_request("Malformed request body."))?;
    let kind =
        SeatKind::try_from(body.kind).map_err(|e| Problem::invalid_request(e.to_string()))?;
    let prompt = body
        .prompt
        .map(SeatPrompt::try_from)
        .transpose()
        .map_err(|e| Problem::invalid_request(e.to_string()))?;
    let link = body
        .link
        .map(SeatLink::try_from)
        .transpose()
        .map_err(|e| Problem::invalid_request(e.to_string()))?;

    let address = super::elements::address(body.tab, body.surface)?;

    let now = Utc::now();
    let seat = NewSeat::contributed_at(commission, address, kind, prompt, link, user.id, now);
    let seat_id = *seat.id;
    let entry = NewChangelogEntry::event(
        commission,
        ChangelogEntryKind::SeatDeclared,
        user.id,
        json!({ "kind": seat.kind.as_str(), "seat": seat_id }),
        now,
    );

    state
        .transaction(async move |uow: &mut dyn UnitOfWork| {
            uow.commissions().declare_seat(&seat).await?;
            uow.changelog().append(&entry).await
        })
        .await
        .map_err(super::elements::to_problem)?;

    let body = DeclareSeatResponse { id: seat_id };
    Ok((StatusCode::CREATED, Json(body)).into_response())
}

#[cfg(test)]
mod tests {
    //! Pins the `201` body's wire shape: `{"id": "<uuid>"}`.

    use super::*;

    #[test]
    fn declare_seat_response_serializes_to_a_bare_id_object() {
        let id = Uuid::parse_str("0192f6f0-0000-7000-8000-000000000003").unwrap();
        let body = DeclareSeatResponse { id };

        assert_eq!(
            serde_json::to_string(&body).unwrap(),
            format!("{{\"id\":\"{id}\"}}")
        );
    }
}
