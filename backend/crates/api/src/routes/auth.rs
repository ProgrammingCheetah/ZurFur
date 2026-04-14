use axum::{
    Json, Router,
    extract::State,
    http::StatusCode,
    routing::{get, post},
};
use serde::{Deserialize, Serialize};

use crate::error::AppError;
use crate::middleware::AuthUser;
use crate::state::SharedState;

use super::helpers::parse_user_id;

// --- Request / Response types ------------------------------------------------

/// Request body for `POST /auth/start`.
#[derive(Deserialize)]
pub struct StartLoginRequest {
    pub handle: String,
}

/// Response body for `POST /auth/start`.
#[derive(Serialize)]
pub struct StartLoginResponse {
    pub redirect_url: String,
    pub state: String,
}

/// Request body for `POST /auth/callback`.
#[derive(Deserialize)]
pub struct CallbackQuery {
    pub code: String,
    pub state: String,
    /// Issuer identifier from the authorization server.
    /// TODO: validate against expected issuer to prevent mix-up attacks.
    #[allow(dead_code)]
    pub iss: Option<String>,
}

/// Response body for `POST /auth/callback`.
#[derive(Serialize)]
pub struct CallbackResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub user_id: String,
    pub did: String,
    pub handle: Option<String>,
    pub is_new_user: bool,
}

/// Request body for `POST /auth/refresh`.
#[derive(Deserialize)]
pub struct RefreshRequest {
    pub refresh_token: String,
}

/// Response body for `POST /auth/refresh`.
#[derive(Serialize)]
pub struct RefreshResponse {
    pub access_token: String,
    pub refresh_token: String,
}

/// Response body for `GET /auth/me`.
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
) -> Result<Json<StartLoginResponse>, AppError> {
    let handle = body.handle.trim();
    if handle.is_empty() {
        return Err(AppError::BadRequest("Handle is required".into()));
    }
    // Basic format validation: handles are domain-like or DIDs
    if !handle.starts_with("did:") && (!handle.contains('.') || handle.len() > 253) {
        return Err(AppError::BadRequest("Invalid handle format".into()));
    }

    eprintln!("[api] POST /auth/start handle={handle}");
    let result = state
        .auth_service
        .start_login(handle)
        .await
        .map_err(|e| {
            eprintln!("[api] POST /auth/start FAILED: {e}");
            AppError::from(e)
        })?;

    Ok(Json(StartLoginResponse {
        redirect_url: result.redirect_url,
        state: result.state,
    }))
}

async fn callback(
    State(state): State<SharedState>,
    Json(params): Json<CallbackQuery>,
) -> Result<Json<CallbackResponse>, AppError> {
    eprintln!("[api] POST /auth/callback state={}", params.state);
    let result = state
        .auth_service
        .complete_login(&params.code, &params.state)
        .await
        .map_err(|e| {
            eprintln!("[api] POST /auth/callback FAILED: {e}");
            AppError::from(e)
        })?;
    eprintln!("[api] POST /auth/callback succeeded, user_id={}, is_new={}", result.user_id, result.is_new_user);

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
) -> Result<Json<RefreshResponse>, AppError> {
    let result = state
        .auth_service
        .refresh_session(&body.refresh_token)
        .await?;

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
) -> Result<StatusCode, AppError> {
    let user_id = parse_user_id(&claims.sub)?;

    state
        .auth_service
        .logout(user_id)
        .await?;

    Ok(StatusCode::NO_CONTENT)
}

// --- Router ------------------------------------------------------------------

/// Build the auth route group (start, callback, refresh, me, logout).
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

