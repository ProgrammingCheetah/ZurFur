use axum::{
    extract::FromRequestParts,
    http::request::Parts,
};

use application::auth::service::ZurfurClaims;

use crate::error::AppError;
use crate::state::SharedState;

/// Axum extractor that validates a JWT from the `Authorization: Bearer <token>` header
/// and provides the decoded claims to the handler.
pub struct AuthUser(pub ZurfurClaims);

impl FromRequestParts<SharedState> for AuthUser {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &SharedState,
    ) -> Result<Self, Self::Rejection> {
        let token = parts
            .headers
            .get("authorization")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| {
                // RFC 6750: auth scheme is case-insensitive
                let (scheme, token) = v.trim().split_once(' ')?;
                if scheme.eq_ignore_ascii_case("bearer") {
                    Some(token.trim_start())
                } else {
                    None
                }
            })
            .ok_or(AppError::Unauthorized("Missing or invalid Authorization header".into()))?;

        let claims = state.auth_service.verify_access_token(token)
            .map_err(|_| AppError::Unauthorized("Invalid or expired token".into()))?;

        Ok(AuthUser(claims))
    }
}
