use application::auth::service::ZurfurClaims;
use axum::{
    extract::FromRequestParts,
    http::{StatusCode, request::Parts},
};

use crate::state::SharedState;

/// Axum extractor that validates a JWT from the `Authorization: Bearer <token>` header
/// and provides the decoded claims to the handler.
pub struct AuthUser(pub ZurfurClaims);

impl FromRequestParts<SharedState> for AuthUser {
    type Rejection = (StatusCode, String);

    async fn from_request_parts(
        parts: &mut Parts,
        state: &SharedState,
    ) -> Result<Self, Self::Rejection> {
        let token = parts
            .headers
            .get("authorization")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "))
            .ok_or((StatusCode::UNAUTHORIZED, "Missing or invalid Authorization header".into()))?;

        let claims: ZurfurClaims =
            shared::jwt::verify(token, &state.auth.jwt_config.secret).map_err(|_| {
                (
                    StatusCode::UNAUTHORIZED,
                    "Invalid or expired token".into(),
                )
            })?;

        Ok(AuthUser(claims))
    }
}
