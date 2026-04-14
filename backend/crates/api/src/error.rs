//! Unified application error type for Axum handlers.
//!
//! All route handlers return `Result<T, AppError>`. The `AppError` enum
//! implements `IntoResponse` so Axum can convert it to an HTTP response.
//! Service-layer errors implement `From` into `AppError`, enabling the
//! `?` operator throughout handlers without explicit `.map_err()` calls.

use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Serialize;

use application::auth::login::LoginError;
use application::feed::service::FeedServiceError;
use application::onboarding::service::OnboardingError;
use application::organization::service::OrgServiceError;
use application::tag::service::TagServiceError;
use application::user::service::UserServiceError;

/// Unified error type for all API route handlers.
pub enum AppError {
    /// 400 Bad Request — invalid input from the client.
    BadRequest(String),
    /// 401 Unauthorized — missing or invalid authentication.
    Unauthorized(String),
    /// 403 Forbidden — authenticated but not permitted.
    Forbidden(String),
    /// 404 Not Found — requested resource doesn't exist.
    NotFound(String),
    /// 409 Conflict — resource already exists or constraint violated.
    Conflict(String),
    /// 500 Internal Server Error — unexpected failure.
    Internal(String),
    /// 502 Bad Gateway — upstream provider failure.
    BadGateway(String),
}

/// JSON envelope for error responses. Every error returns this shape.
#[derive(Serialize)]
struct ErrorBody {
    error: String,
    code: &'static str,
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, code, message) = match self {
            AppError::BadRequest(msg) => (StatusCode::BAD_REQUEST, "bad_request", msg),
            AppError::Unauthorized(msg) => (StatusCode::UNAUTHORIZED, "unauthorized", msg),
            AppError::Forbidden(msg) => (StatusCode::FORBIDDEN, "forbidden", msg),
            AppError::NotFound(msg) => (StatusCode::NOT_FOUND, "not_found", msg),
            AppError::Conflict(msg) => (StatusCode::CONFLICT, "conflict", msg),
            AppError::Internal(msg) => {
                tracing::error!("Internal error: {msg}");
                (StatusCode::INTERNAL_SERVER_ERROR, "internal_error", "Internal server error".into())
            }
            AppError::BadGateway(msg) => (StatusCode::BAD_GATEWAY, "bad_gateway", msg),
        };
        (status, Json(ErrorBody { error: message, code })).into_response()
    }
}

// --- From implementations for service errors ---------------------------------

impl From<OrgServiceError> for AppError {
    fn from(e: OrgServiceError) -> Self {
        match e {
            OrgServiceError::NotFound => AppError::NotFound("Organization not found".into()),
            OrgServiceError::SlugTaken(s) => AppError::Conflict(format!("Slug already taken: {s}")),
            OrgServiceError::InvalidSlug(msg) => AppError::BadRequest(msg),
            OrgServiceError::Forbidden => AppError::Forbidden("Permission denied".into()),
            OrgServiceError::CannotDeletePersonal => {
                AppError::Forbidden("Cannot delete a personal organization".into())
            }
            OrgServiceError::CannotRemoveOwner => {
                AppError::Forbidden("Cannot remove the owner from an organization".into())
            }
            OrgServiceError::Internal(msg) => AppError::Internal(msg),
        }
    }
}

impl From<TagServiceError> for AppError {
    fn from(e: TagServiceError) -> Self {
        match e {
            TagServiceError::NotFound => AppError::NotFound("Tag not found".into()),
            TagServiceError::NotAttached => {
                AppError::NotFound("Tag is not attached to this entity".into())
            }
            TagServiceError::NameTaken(s) => {
                AppError::Conflict(format!("Tag name already taken: {s}"))
            }
            TagServiceError::Immutable => {
                AppError::Forbidden("Entity-backed tags cannot be modified".into())
            }
            TagServiceError::InvalidCategory => AppError::BadRequest(
                "This category cannot be used for user-created tags. Use 'metadata' or 'general'."
                    .into(),
            ),
            TagServiceError::InvalidName(msg) => AppError::BadRequest(msg),
            TagServiceError::AlreadyAttached => {
                AppError::Conflict("Tag is already attached to this entity".into())
            }
            TagServiceError::Internal(msg) => AppError::Internal(msg),
        }
    }
}

impl From<UserServiceError> for AppError {
    fn from(e: UserServiceError) -> Self {
        match e {
            UserServiceError::NotFound => AppError::NotFound("User not found".into()),
            UserServiceError::Internal(msg) => AppError::Internal(msg),
        }
    }
}

impl From<FeedServiceError> for AppError {
    fn from(e: FeedServiceError) -> Self {
        match e {
            FeedServiceError::FeedNotFound => AppError::NotFound("Feed not found".into()),
            FeedServiceError::ItemNotFound => AppError::NotFound("Feed item not found".into()),
            FeedServiceError::SystemFeedUndeletable => {
                AppError::Forbidden("System feeds cannot be deleted".into())
            }
            FeedServiceError::Forbidden => AppError::Forbidden("Permission denied".into()),
            FeedServiceError::SlugTaken(s) => {
                AppError::Conflict(format!("Feed slug already taken: {s}"))
            }
            FeedServiceError::Internal(msg) => AppError::Internal(msg),
        }
    }
}

impl From<OnboardingError> for AppError {
    fn from(e: OnboardingError) -> Self {
        match e {
            OnboardingError::UserNotFound => AppError::NotFound("User not found".into()),
            OnboardingError::PersonalOrgNotFound => {
                AppError::NotFound("Personal organization not found".into())
            }
            OnboardingError::Internal(msg) => AppError::Internal(msg),
        }
    }
}

impl From<LoginError> for AppError {
    fn from(e: LoginError) -> Self {
        match e {
            LoginError::InvalidEmail => AppError::BadRequest("Invalid email".into()),
            LoginError::UserNotFound => AppError::Unauthorized("User not found".into()),
            LoginError::InternalError(msg) => AppError::Internal(msg),
            LoginError::IdentityResolverFailed => {
                AppError::BadGateway("Failed to resolve identity".into())
            }
            LoginError::PdsNotFound => {
                AppError::NotFound("No PDS found for account".into())
            }
            LoginError::OAuth(msg) => {
                tracing::error!("OAuth error: {msg}");
                AppError::BadGateway("OAuth provider failure".into())
            }
            LoginError::InvalidState => {
                AppError::BadRequest("Invalid or expired session state".into())
            }
            LoginError::DidMismatch => {
                AppError::BadGateway("Identity mismatch with OAuth provider".into())
            }
        }
    }
}
