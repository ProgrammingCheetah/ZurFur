use std::time::Duration;

use application::auth::login::{LoginEmailCommand, LoginError};
use axum::{Json, Router, extract::State, http::StatusCode, routing::get, routing::post};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use shared::jwt;

use crate::state::SharedState;

#[derive(Serialize)]
struct LoginResponse {
    jwt: String,
}

impl From<String> for LoginResponse {
    fn from(jwt: String) -> Self {
        Self { jwt }
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct Claims {
    id: String,
    email: String,
    exp: DateTime<Utc>,
}

async fn login(
    State(state): State<SharedState>,
) -> Result<Json<LoginResponse>, (StatusCode, String)> {
    let cmd = LoginEmailCommand {
        email: String::new(),
    };

    let result = state.auth.login.execute(cmd).await.map_err(|e| {
        let (status, msg) = match &e {
            LoginError::InvalidEmail => (StatusCode::BAD_REQUEST, "Invalid email".into()),
            LoginError::UserNotFound => (StatusCode::UNAUTHORIZED, "User not found".into()),
            LoginError::InternalError(_) => {
                (StatusCode::INTERNAL_SERVER_ERROR, "Internal error".into())
            }
        };
        (status, msg)
    })?;

    let claims = Claims {
        id: result.id,
        email: result.email,
        exp: Utc::now() + Duration::from_secs(30 * 24 * 60 * 60),
    };

    let jwt = jwt::create(&claims, b"secret").map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to create token".into(),
        )
    })?;

    Ok(Json(LoginResponse::from(jwt)))
}

pub fn router() -> Router<SharedState> {
    Router::new()
        .route("/", get(|| async { () }))
        .route("/login", post(login))
}
