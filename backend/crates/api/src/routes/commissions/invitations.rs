//! `POST`/`DELETE /commissions/{id}/invitations` — the owner invites a User to
//! a vacant Seat, or revokes a pending offer (issue + revoke only; accept and
//! decline are separate).

use axum::{
    Json,
    extract::{Path, State, rejection::JsonRejection},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use chrono::Utc;
use domain::{
    elements::{
        commission::{CommissionId, ElementId, SeatInvitation},
        did::Did,
        invitation::InvitationState,
    },
    ports::{DidBelongsToAnotherActor, UnitOfWork},
};
use serde::{Deserialize, Serialize};
use tower_sessions::Session;
use uuid::Uuid;

use super::require_owner;
use crate::{AppState, problem::Problem};

/// `POST /commissions/{id}/invitations`'s body: the seat offer, in full — see
/// [`invite_to_seat`].
#[derive(Serialize)]
struct InviteToSeatResponse {
    commission: String,
    id: String,
    seat: String,
    state: &'static str,
    user: String,
}

/// `DELETE /commissions/{id}/invitations`'s `200` body — see
/// [`revoke_seat_invitation`].
#[derive(Serialize)]
struct RevokeSeatInvitationResponse {
    commission: String,
    seat: String,
    user: String,
}

/// The `POST /commissions/{id}/invitations` request body: the `seat` to offer
/// (its element id) and the `user` to invite, named by their `did`.
#[derive(Deserialize)]
pub(super) struct InviteToSeatBody {
    seat: Uuid,
    user: String,
}

/// Issues a pending invitation offering a vacant seat to a User (by `did`).
/// Owner-only; `404 element_not_found` for a seat not in this commission,
/// `409 seat_filled` for an occupied seat. Idempotent — re-inviting the same
/// pending user returns the existing offer (`200`); otherwise `201 Created`.
pub(super) async fn invite_to_seat(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    session: Session,
    body: Result<Json<InviteToSeatBody>, JsonRejection>,
) -> Result<Response, Problem> {
    let user = super::current_user(&state, &session).await?;
    let commission = CommissionId::new(id);
    require_owner(&state, commission, &user).await?;

    let Json(body) = body.map_err(|_| {
        Problem::invalid_request(
            "Provide a seat and a user to invite, e.g. {\"seat\": \"…\", \"user\": \"did:plc:…\"}.",
        )
    })?;
    let seat_id = body.seat;
    let seat = ElementId::new(seat_id);

    let seats = state.commissions.seats(commission).await?;
    let target = seats
        .iter()
        .find(|s| s.id == seat)
        .ok_or_else(Problem::element_not_found)?;
    if target.occupant.is_some() {
        return Err(Problem::seat_filled());
    }

    let invited = state
        .transaction(async move |uow: &mut dyn UnitOfWork| {
            uow.users().provision(&Did::new(body.user)).await
        })
        .await
        .map_err(|err| match err.downcast_ref::<DidBelongsToAnotherActor>() {
            Some(_) => Problem::did_belongs_to_another_actor(),
            None => Problem::from(err),
        })?;

    if let Some(existing) = state
        .commissions
        .find_pending_seat_invitation(commission, seat, invited.id)
        .await?
    {
        let existing_offer = InviteToSeatResponse {
            commission: id.to_string(),
            id: existing.id.to_string(),
            seat: seat_id.to_string(),
            state: existing.state.as_str(),
            user: invited.did.as_str().to_owned(),
        };
        let response = (StatusCode::OK, Json(existing_offer)).into_response();
        return Ok(response);
    }

    let invitation = SeatInvitation::issue(commission, seat, invited.id, user.id, Utc::now());
    let minted = invitation.id;
    state
        .transaction(async move |uow: &mut dyn UnitOfWork| {
            uow.commissions().create_seat_invitation(&invitation).await
        })
        .await?;

    // A duplicate invite racing past the pending check is dropped by the store
    // (ON CONFLICT DO NOTHING); re-read to answer 200/201 from the row that
    // actually survives, not the one this call minted.
    let (status, offer_id, offer_state) = match state
        .commissions
        .find_pending_seat_invitation(commission, seat, invited.id)
        .await?
    {
        Some(stored) if stored.id == minted => {
            let state = stored.state;
            (StatusCode::CREATED, stored.id, state)
        }
        Some(stored) => {
            let state = stored.state;
            (StatusCode::OK, stored.id, state)
        }
        None => (StatusCode::CREATED, minted, InvitationState::Pending),
    };
    let offer = InviteToSeatResponse {
        commission: id.to_string(),
        id: offer_id.to_string(),
        seat: seat_id.to_string(),
        state: offer_state.as_str(),
        user: invited.did.as_str().to_owned(),
    };
    let response = (status, Json(offer)).into_response();
    Ok(response)
}

/// The `DELETE /commissions/{id}/invitations` request body: the `seat` and
/// the invited User's `did` identify the pending offer.
#[derive(Deserialize)]
pub(super) struct RevokeSeatInvitationBody {
    seat: Uuid,
    user: String,
}

/// Revokes a pending seat invitation so it can no longer be accepted.
/// Owner-only and idempotent — an unknown DID or no pending offer is a `200`
/// no-op. Every path echoes `{ commission, seat, user }`.
pub(super) async fn revoke_seat_invitation(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    session: Session,
    body: Result<Json<RevokeSeatInvitationBody>, JsonRejection>,
) -> Result<Response, Problem> {
    let user = super::current_user(&state, &session).await?;
    let commission = CommissionId::new(id);
    require_owner(&state, commission, &user).await?;

    let Json(body) = body.map_err(|_| {
        Problem::invalid_request(
            "Provide the seat and invited user to revoke, e.g. {\"seat\": \"…\", \"user\": \"did:plc:…\"}.",
        )
    })?;
    let seat = ElementId::new(body.seat);
    let invited_did = body.user;

    let revoked = || {
        let response_body = RevokeSeatInvitationResponse {
            commission: id.to_string(),
            seat: body.seat.to_string(),
            user: invited_did.clone(),
        };
        (StatusCode::OK, Json(response_body)).into_response()
    };

    let Some(invited_user) = state
        .users
        .find_by_did(&Did::new(invited_did.clone()))
        .await?
    else {
        return Ok(revoked());
    };

    let Some(mut invitation) = state
        .commissions
        .find_pending_seat_invitation(commission, seat, invited_user.id)
        .await?
    else {
        return Ok(revoked());
    };

    invitation.revoke(Utc::now()).map_err(|_| {
        Problem::internal_error("Could not revoke the invitation. Please try again.")
    })?;
    state
        .transaction(async move |uow: &mut dyn UnitOfWork| {
            uow.commissions()
                .revoke_seat_invitation(invitation.id)
                .await
        })
        .await?;

    Ok(revoked())
}

#[cfg(test)]
mod tests {
    //! Pins the two response bodies' wire shapes.

    use super::*;

    #[test]
    fn invite_to_seat_response_serializes_every_field_as_a_string() {
        let body = InviteToSeatResponse {
            commission: "commission-id".to_string(),
            id: "offer-id".to_string(),
            seat: "seat-id".to_string(),
            state: "pending",
            user: "did:plc:invitee".to_string(),
        };

        assert_eq!(
            serde_json::to_string(&body).unwrap(),
            r#"{"commission":"commission-id","id":"offer-id","seat":"seat-id","state":"pending","user":"did:plc:invitee"}"#
        );
    }

    #[test]
    fn revoke_seat_invitation_response_serializes_every_field_as_a_string() {
        let body = RevokeSeatInvitationResponse {
            commission: "commission-id".to_string(),
            seat: "seat-id".to_string(),
            user: "did:plc:invitee".to_string(),
        };

        assert_eq!(
            serde_json::to_string(&body).unwrap(),
            r#"{"commission":"commission-id","seat":"seat-id","user":"did:plc:invitee"}"#
        );
    }
}
