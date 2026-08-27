//! `PUT /commissions/{id}/maturity` — the owner rates the commission (Safe /
//! Suggestive / Nudity / Adult plus a Graphic flag, DD `29982722`).
//! Replace-only: no `DELETE` sibling, so a rating can never clear.

use axum::{
    Json,
    extract::{Path, State, rejection::JsonRejection},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use domain::{
    elements::{
        commission::CommissionId,
        maturity::{Maturity, MaturityRating},
    },
    ports::UnitOfWork,
};
use serde::Deserialize;
use tower_sessions::Session;
use uuid::Uuid;

use super::require_owner;
use crate::{AppState, problem::Problem};

/// The `PUT /commissions/{id}/maturity` request body: the rating token, plus
/// the optional Graphic flag (defaults to `false`).
#[derive(Deserialize)]
pub(super) struct SetMaturityBody {
    rating: String,
    #[serde(default)]
    graphic: bool,
}

/// Rates (or re-rates) the commission. Owner-only; `422
/// unknown_maturity_rating` for a token outside the vocabulary. Returns
/// `204 No Content`.
pub(super) async fn set_maturity(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    session: Session,
    body: Result<Json<SetMaturityBody>, JsonRejection>,
) -> Result<Response, Problem> {
    let user = super::current_user(&state, &session).await?;
    let commission = CommissionId::new(id);
    require_owner(&state, commission, &user).await?;

    let Json(body) = body.map_err(|_| Problem::invalid_request("Malformed request body."))?;
    let rating = MaturityRating::try_from(body.rating.as_str()).map_err(|_| {
        Problem::unknown_maturity_rating(format!(
            "{:?} is not a maturity rating; expected one of: safe, suggestive, nudity, adult.",
            body.rating,
        ))
    })?;
    let maturity = Maturity {
        rating,
        graphic: body.graphic,
    };

    state
        .transaction(async move |uow: &mut dyn UnitOfWork| {
            uow.commissions().set_maturity(commission, maturity).await
        })
        .await?;

    Ok(StatusCode::NO_CONTENT.into_response())
}
