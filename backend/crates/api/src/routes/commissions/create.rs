//! `POST /commissions` — any signed-in User creates a commission they own
//! (ZMVP-65; no Account required, a user-scoped write — ZMVP-47, DD 26247170),
//! and the act itself is the changelog's genesis entry (ZMVP-87; the Changelog
//! DD's taxonomy includes "creation itself").

use application::commission::create::CreateCommissionCommand;
use axum::{
    Json,
    extract::{State, rejection::JsonRejection},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use chrono::Utc;
use domain::{
    elements::{
        commission::{ChangelogEntryKind, Commission, CommissionTitle, NewChangelogEntry},
        maturity::{Maturity, MaturityRating},
    },
    ports::UnitOfWork,
};
use serde_json::json;
use tower_sessions::Session;

use super::{from_wire_timestamp, list::wire_commission};
use crate::{AppState, problem::Problem};
use crate::{
    generated::{CreateCommissionRequest, CreateCommissionResponse},
    routes::commissions::ports::commission_ports,
};

/// Creates a commission owned by the signed-in caller (Draft lifecycle), and
/// records its `created` changelog entry atomically with the row.
///
/// `201 Created` on success; `422 invalid_request` for a missing/malformed
/// body or blank title; `422 unknown_maturity_rating` for an out-of-vocabulary
/// `maturity.rating`.
pub(super) async fn create_commission(
    State(state): State<AppState>,
    session: Session,
    body: Result<Json<CreateCommissionRequest>, JsonRejection>,
) -> Result<Response, Problem> {
    let user = super::current_user(&state, &session).await?;

    let Json(body) = body.map_err(|_| Problem::invalid_request("Malformed request body."))?;
    let title = CommissionTitle::try_from(body.title)
        .map_err(|e| Problem::invalid_request(e.to_string()))?;
    let maturity = body
        .maturity
        .map(|input| {
            let rating = MaturityRating::try_from(input.rating.as_str()).map_err(|_| {
                Problem::unknown_maturity_rating(format!(
                    "{:?} is not a maturity rating; expected one of: safe, suggestive, nudity, adult.",
                    input.rating,
                ))
            })?;
            Ok::<_, Problem>(Maturity {
                rating,
                graphic: input.graphic,
            })
        })
        .transpose()?;

    let deadline = body
        .deadline
        .map(|at| {
            from_wire_timestamp(at)
                .ok_or_else(|| Problem::invalid_request("deadline is out of range"))
        })
        .transpose()?;

    let now = Utc::now();

    let command = CreateCommissionCommand {
        maturity,
        deadline,
        actor_id: user.id,
        title,
    };

    let ports = commission_ports(&state);

    let commission = application::commission::create(command, ports, now).await?;

    let body = CreateCommissionResponse {
        id: commission.into(),
    };
    let response = (StatusCode::CREATED, Json(body)).into_response();
    Ok(response)
}
