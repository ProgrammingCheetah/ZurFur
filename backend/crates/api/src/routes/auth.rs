use application::auth::login::LoginError;
use axum::{
    Json, Router,
    extract::State,
    http::StatusCode,
    routing::{get, post},
};
use serde::{Deserialize, Serialize};

use crate::middleware::AuthUser;
use crate::state::SharedState;

// --- Request / Response types ------------------------------------------------

#[derive(Deserialize)]
pub struct StartLoginRequest {
    pub handle: String,
}

#[derive(Serialize)]
pub struct StartLoginResponse {
    pub redirect_url: String,
    pub state: String,
}

#[derive(Deserialize)]
pub struct CallbackQuery {
    pub code: String,
    pub state: String,
    /// Issuer identifier from the authorization server.
    /// TODO: validate against expected issuer to prevent mix-up attacks.
    pub iss: Option<String>,
}

#[derive(Serialize)]
pub struct CallbackResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub user_id: String,
    pub did: String,
    pub handle: Option<String>,
    pub is_new_user: bool,
}

#[derive(Deserialize)]
pub struct RefreshRequest {
    pub refresh_token: String,
}

#[derive(Serialize)]
pub struct RefreshResponse {
    pub access_token: String,
    pub refresh_token: String,
}

#[derive(Serialize)]
pub struct MeResponse {
    pub user_id: String,
    pub did: String,
    pub handle: Option<String>,
}

// --- Handlers ----------------------------------------------------------------

async fn start_login(
    State(state): State<SharedState>,
    Json(body): Json<StartLoginRequest>,
) -> Result<Json<StartLoginResponse>, (StatusCode, String)> {
    let handle = body.handle.trim();
    if handle.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "Handle is required".into()));
    }
    // Basic format validation: handles are domain-like or DIDs
    if !handle.starts_with("did:") && (!handle.contains('.') || handle.len() > 253) {
        return Err((StatusCode::BAD_REQUEST, "Invalid handle format".into()));
    }

    let result = state
        .auth
        .start_login(handle)
        .await
        .map_err(map_login_error)?;

    Ok(Json(StartLoginResponse {
        redirect_url: result.redirect_url,
        state: result.state,
    }))
}

async fn callback(
    State(state): State<SharedState>,
    Json(params): Json<CallbackQuery>,
) -> Result<Json<CallbackResponse>, (StatusCode, String)> {
    let result = state
        .auth
        .complete_login(&params.code, &params.state)
        .await
        .map_err(map_login_error)?;

    Ok(Json(CallbackResponse {
        access_token: result.access_token,
        refresh_token: result.refresh_token,
        user_id: result.user_id.to_string(),
        did: result.did,
        handle: result.handle,
        is_new_user: result.is_new_user,
    }))
}

async fn refresh(
    State(state): State<SharedState>,
    Json(body): Json<RefreshRequest>,
) -> Result<Json<RefreshResponse>, (StatusCode, String)> {
    let result = state
        .auth
        .refresh_session(&body.refresh_token)
        .await
        .map_err(map_login_error)?;

    Ok(Json(RefreshResponse {
        access_token: result.access_token,
        refresh_token: result.refresh_token,
    }))
}

async fn me(AuthUser(claims): AuthUser) -> Json<MeResponse> {
    Json(MeResponse {
        user_id: claims.sub,
        did: claims.did,
        handle: claims.handle,
    })
}

async fn logout(
    State(state): State<SharedState>,
    AuthUser(claims): AuthUser,
) -> Result<StatusCode, (StatusCode, String)> {
    let user_id: uuid::Uuid = claims
        .sub
        .parse()
        .map_err(|_| (StatusCode::BAD_REQUEST, "Invalid user ID in token".into()))?;

    state
        .auth
        .logout(user_id)
        .await
        .map_err(map_login_error)?;

    Ok(StatusCode::NO_CONTENT)
}

// --- Router ------------------------------------------------------------------

pub fn router() -> Router<SharedState> {
    Router::new()
        .route("/start", post(start_login))
        // POST, not GET: the browser redirects to the *frontend* callback page,
        // which extracts code/state from the URL and POSTs them here as JSON.
        // This avoids exposing tokens in browser history, referrer headers, and caches.
        .route("/callback", post(callback))
        .route("/refresh", post(refresh))
        .route("/me", get(me))
        .route("/logout", post(logout))
}

// --- Error mapping -----------------------------------------------------------
// Internal details are logged server-side and never exposed to clients.
// Uses eprintln! for now; will migrate to `tracing` when structured logging is added.

fn map_login_error(e: LoginError) -> (StatusCode, String) {
    match e {
        LoginError::InvalidEmail => (StatusCode::BAD_REQUEST, "Invalid email".into()),
        LoginError::UserNotFound => (StatusCode::UNAUTHORIZED, "User not found".into()),
        LoginError::InternalError(inner) => {
            eprintln!("Internal login error: {inner}");
            (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error".into())
        }
        LoginError::IdentityResolverFailed => {
            (StatusCode::BAD_GATEWAY, "Failed to resolve identity".into())
        }
        LoginError::PdsNotFound => (StatusCode::NOT_FOUND, "No PDS found for account".into()),
        LoginError::OAuth(inner) => {
            eprintln!("OAuth error: {inner}");
            (StatusCode::BAD_GATEWAY, "OAuth provider error".into())
        }
        LoginError::InvalidState => {
            (StatusCode::BAD_REQUEST, "Invalid or expired session state".into())
        }
        LoginError::DidMismatch => {
            (StatusCode::BAD_GATEWAY, "Identity mismatch with OAuth provider".into())
        }
    }
}
