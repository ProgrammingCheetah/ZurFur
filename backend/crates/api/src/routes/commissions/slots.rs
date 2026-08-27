//! `POST /commissions/{id}/slots` — the owner declares a batch of Character
//! Slots: positions with a required title and optional notes (DESIGN/Slots
//! `5931025`). The body is an array; the batch lands all-or-nothing. No fill
//! surface exists here — an empty Slot is a valid, permanent state.

use axum::{
    Json,
    extract::{Path, State, rejection::JsonRejection},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use chrono::Utc;
use domain::{
    elements::commission::{CommissionId, NewSlot, SlotTitle},
    ports::UnitOfWork,
};
use serde::{Deserialize, Serialize};
use tower_sessions::Session;
use uuid::Uuid;

use super::require_owner;
use crate::{AppState, problem::Problem};

/// `POST /commissions/{id}/slots`'s `201` body: the id of each newly declared
/// slot's carrying element, in request order — see [`declare_slots`].
#[derive(Serialize)]
struct DeclareSlotsResponse {
    ids: Vec<Uuid>,
}

/// One Slot of the `POST /commissions/{id}/slots` request body (a JSON array
/// of these): the address (`tab` + `surface`), the required title, and
/// optional notes. No occupant/character field.
#[derive(Deserialize)]
pub(super) struct DeclareSlotBody {
    tab: Uuid,
    surface: String,
    title: String,
    #[serde(default)]
    notes: Option<String>,
}

/// Declares a batch of Slots, as the commission's owner. All-or-nothing.
/// Owner-only; `422` for an empty array, an invalid title, or a bad address
/// (`404 tab_not_found` / `422 unknown_surface`). Returns `201 Created` with
/// `{"ids": ["…", …]}` in request order.
pub(super) async fn declare_slots(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    session: Session,
    body: Result<Json<Vec<DeclareSlotBody>>, JsonRejection>,
) -> Result<Response, Problem> {
    let user = super::current_user(&state, &session).await?;
    let commission = CommissionId::new(id);
    require_owner(&state, commission, &user).await?;

    let Json(body) = body.map_err(|_| Problem::invalid_request("Malformed request body."))?;
    if body.is_empty() {
        return Err(Problem::invalid_request(
            "Declare at least one slot: the body is an array of slot objects.",
        ));
    }

    let now = Utc::now();
    let mut slots = Vec::with_capacity(body.len());
    for entry in body {
        let title = SlotTitle::try_from(entry.title)
            .map_err(|err| Problem::invalid_request(format!("Invalid slot title: {err}.")))?;
        let notes = entry
            .notes
            .as_deref()
            .map(str::trim)
            .filter(|notes| !notes.is_empty())
            .map(str::to_owned);
        let address = super::elements::address(entry.tab, entry.surface)?;
        slots.push(NewSlot::contributed_at(
            commission, address, title, notes, user.id, now,
        ));
    }
    let element_ids: Vec<Uuid> = slots.iter().map(|slot| *slot.id).collect();

    state
        .transaction(async move |uow: &mut dyn UnitOfWork| {
            uow.commissions().declare_slots(&slots).await
        })
        .await
        .map_err(super::elements::to_problem)?;

    let body = DeclareSlotsResponse { ids: element_ids };
    Ok((StatusCode::CREATED, Json(body)).into_response())
}

#[cfg(test)]
mod tests {
    //! Pins the `201` body's wire shape: `{"ids": ["<uuid>", …]}`.

    use super::*;

    #[test]
    fn declare_slots_response_serializes_to_a_bare_ids_array() {
        let first = Uuid::parse_str("0192f6f0-0000-7000-8000-000000000004").unwrap();
        let second = Uuid::parse_str("0192f6f0-0000-7000-8000-000000000005").unwrap();
        let body = DeclareSlotsResponse {
            ids: vec![first, second],
        };

        assert_eq!(
            serde_json::to_string(&body).unwrap(),
            format!("{{\"ids\":[\"{first}\",\"{second}\"]}}")
        );
    }
}
