use axum::{
    extract::FromRequestParts,
    http::{StatusCode, request::Parts},
};

use application::auth::service::ZurfurClaims;

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
            .and_then(|v| {
                let v = v.trim();
                // RFC 6750: auth scheme is case-insensitive
                if v.len() > 7 && v[..7].eq_ignore_ascii_case("bearer ") {
                    Some(v[7..].trim_start())
                } else {
                    None
                }
            })
            .ok_or((StatusCode::UNAUTHORIZED, "Missing or invalid Authorization header".into()))?;

        let claims = state.auth.verify_access_token(token).map_err(|_| {
            (
                StatusCode::UNAUTHORIZED,
                "Invalid or expired token".into(),
            )
        })?;

        Ok(AuthUser(claims))
    }
}
