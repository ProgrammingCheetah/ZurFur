//! The commissions route group: the commission JSON API, split per area
//! (create, list, changelog, notes, channel, delete, archive, maturity,
//! elements, slots, seats, invitations, status, deadline, files, markup,
//! positioning). Mounted under the first-party-`Origin` (CSRF) layer.

use axum::{
    Router,
    extract::DefaultBodyLimit,
    routing::{delete, get, post, put},
};
use domain::elements::{
    commission::{Commission, CommissionId},
    user::{User, UserId},
};
use tower_sessions::Session;
use uuid::Uuid;

use application::commission::CommissionError;

use crate::{AppState, SESSION_USER_KEY, problem::Problem};

/// Maps a commission use-case error onto the wire problem: `404` for a missing
/// commission, `403` for insufficient standing, `401` when the session's user
/// no longer exists, `500` for the store.
impl From<CommissionError> for Problem {
    fn from(err: CommissionError) -> Self {
        match err {
            CommissionError::Infrastructure(err) => Problem::from(err),
            CommissionError::UserNotFound => Problem::not_authenticated(),
            CommissionError::CommissionNotFound => Problem::commission_not_found(),
            CommissionError::CommissionAlreadyArchived => {
                Problem::invalid_request("This commission is already archived.")
            }
            CommissionError::InsufficientPermissions => Problem::forbidden(),
        }
    }
}

/// Domain time → the contract's wire timestamp type.
pub(super) fn wire_timestamp(at: domain::datetime::DateTimeUtc) -> crate::wire_time::WireTimestamp {
    crate::wire_time::WireTimestamp::from(at)
}

/// Wire timestamp → domain time, or `None` if outside the protobuf
/// `Timestamp` range (years 0001–9999).
pub(super) fn from_wire_timestamp(
    at: crate::wire_time::WireTimestamp,
) -> Option<domain::datetime::DateTimeUtc> {
    at.as_datetime()
}

mod archive;
mod changelog;
mod channel;
mod create;
mod deadline;
mod delete;
mod elements;
mod files;
mod invitations;
mod list;
mod markup;
mod maturity;
mod notes;
mod ports;
mod positioning;
mod seats;
mod slots;
mod status;

/// Slack, in bytes, added above [`Config::max_upload_bytes`](crate::Config::max_upload_bytes)
/// for the upload route's request body-size limit, to cover the
/// `multipart/form-data` envelope overhead.
const UPLOAD_BODY_SLACK_BYTES: usize = 1024 * 1024;

/// The commissions route group; mounted under the CSRF
/// [`require_first_party_origin`](super::require_first_party_origin) layer.
/// `max_upload_bytes` sizes the upload route's body-size limit.
pub(crate) fn commissions_router(max_upload_bytes: usize) -> Router<AppState> {
    let upload_body_limit = max_upload_bytes.saturating_add(UPLOAD_BODY_SLACK_BYTES);
    Router::new()
        .route(
            "/commissions",
            get(list::list_commissions).post(create::create_commission),
        )
        .route(
            "/commissions/{id}",
            axum::routing::delete(delete::delete_commission),
        )
        .route(
            "/commissions/{id}/changelog",
            get(changelog::read_changelog),
        )
        .route("/commissions/{id}/notes", post(notes::write_note))
        .route(
            "/commissions/{id}/channel",
            put(channel::link_channel).delete(channel::clear_channel),
        )
        .route(
            "/commissions/{id}/archive",
            post(archive::archive_commission),
        )
        .route(
            "/commissions/{id}/unarchive",
            post(archive::unarchive_commission),
        )
        .route(
            "/commissions/{id}/placements",
            post(positioning::place_commission),
        )
        .route("/commissions/{id}/grants", post(positioning::grant_view))
        .route(
            "/commissions/{id}/grants/{account_id}",
            delete(positioning::revoke_view),
        )
        .route("/commissions/{id}/maturity", put(maturity::set_maturity))
        .route("/commissions/{id}/elements", post(elements::add_element))
        .route(
            "/commissions/{id}/elements/{element}",
            delete(elements::remove_element),
        )
        .route("/commissions/{id}/slots", post(slots::declare_slots))
        .route("/commissions/{id}/seats", post(seats::declare_seat))
        .route(
            "/commissions/{id}/invitations",
            post(invitations::invite_to_seat).delete(invitations::revoke_seat_invitation),
        )
        .route(
            "/commissions/{id}/status/direction",
            put(status::set_direction_status).delete(status::clear_direction_status),
        )
        .route(
            "/commissions/{id}/deadline",
            put(deadline::set_deadline).delete(deadline::clear_deadline),
        )
        .route(
            "/commissions/{id}/status/deadline",
            put(deadline::set_deadline_status).delete(deadline::clear_deadline_status),
        )
        .route(
            "/commissions/{id}/files",
            post(files::upload_file).layer(DefaultBodyLimit::max(upload_body_limit)),
        )
        .route(
            "/commissions/{id}/files/{file_id}",
            get(files::download_file),
        )
        .route(
            "/commissions/{id}/files/{file_id}/markup",
            post(markup::add_markup),
        )
}

/// Resolves the session to the acting [`User`], or `401` if unauthenticated
/// or the User has vanished.
async fn current_user(state: &AppState, session: &Session) -> Result<User, Problem> {
    let id = session
        .get::<Uuid>(SESSION_USER_KEY)
        .await
        .ok()
        .flatten()
        .ok_or_else(Problem::not_authenticated)?;
    state
        .users
        .find(UserId::new(id))
        .await
        .ok()
        .flatten()
        .ok_or_else(Problem::not_authenticated)
}

/// Admits `user` only if they are a participant of `commission`; otherwise
/// `404 commission_not_found` — never `403`, which would leak existence.
async fn require_participant(
    state: &AppState,
    commission: CommissionId,
    user: UserId,
) -> Result<(), Problem> {
    if state.commissions.is_participant(commission, user).await? {
        Ok(())
    } else {
        Err(Problem::commission_not_found())
    }
}

/// Admits only the commission's owner. A non-participant gets `404
/// commission_not_found`; a non-owner participant gets `403`. Returns the
/// resolved [`Commission`] so callers needn't re-read it.
async fn require_owner(
    state: &AppState,
    commission: CommissionId,
    user: &User,
) -> Result<Commission, Problem> {
    let found = state
        .commissions
        .find(commission)
        .await?
        .ok_or_else(Problem::commission_not_found)?;
    if found.owner_id == user.id {
        return Ok(found);
    }
    Err(
        if state
            .commissions
            .is_participant(commission, user.id)
            .await?
        {
            Problem::forbidden()
        } else {
            Problem::commission_not_found()
        },
    )
}
