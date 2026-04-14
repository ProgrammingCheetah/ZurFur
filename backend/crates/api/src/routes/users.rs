use axum::{
    Json, Router,
    extract::State,
    routing::get,
};
use serde::{Deserialize, Serialize};

use crate::error::AppError;
use crate::middleware::AuthUser;
use crate::state::SharedState;

use super::helpers::parse_user_id;

// --- Response types ----------------------------------------------------------

#[derive(Serialize)]
struct MembershipResponse {
    org_id: String,
    role: String,
    title: Option<String>,
}

#[derive(Serialize)]
struct PersonalOrgResponse {
    id: String,
    slug: String,
    display_name: Option<String>,
}

#[derive(Serialize)]
struct UserProfileResponse {
    user_id: String,
    did: Option<String>,
    handle: Option<String>,
    username: String,
    personal_org: Option<PersonalOrgResponse>,
    memberships: Vec<MembershipResponse>,
}

#[derive(Serialize)]
struct PreferencesResponse {
    settings: serde_json::Value,
}

#[derive(Deserialize)]
struct UpdatePreferencesRequest {
    settings: serde_json::Value,
}

// --- Handlers ----------------------------------------------------------------

async fn get_me(
    State(state): State<SharedState>,
    AuthUser(claims): AuthUser,
) -> Result<Json<UserProfileResponse>, AppError> {
    let user_id = parse_user_id(&claims.sub)?;

    let profile = state
        .user_service
        .get_my_profile(user_id)
        .await?;

    let personal_org = profile.personal_org.map(|org| {
        PersonalOrgResponse {
            id: org.id.to_string(),
            slug: org.slug,
            display_name: org
                .display_name
                .or_else(|| profile.user.handle.clone())
                .or_else(|| Some(profile.user.username.clone())),
        }
    });

    let memberships = profile
        .memberships
        .into_iter()
        .map(|m| MembershipResponse {
            org_id: m.org_id.to_string(),
            role: m.role.as_str().to_string(),
            title: m.title,
        })
        .collect();

    Ok(Json(UserProfileResponse {
        user_id: profile.user.id.to_string(),
        did: profile.user.did,
        handle: profile.user.handle,
        username: profile.user.username,
        personal_org,
        memberships,
    }))
}

async fn get_preferences(
    State(state): State<SharedState>,
    AuthUser(claims): AuthUser,
) -> Result<Json<PreferencesResponse>, AppError> {
    let user_id = parse_user_id(&claims.sub)?;

    let prefs = state
        .user_service
        .get_preferences(user_id)
        .await?;

    let settings: serde_json::Value = serde_json::from_str(&prefs.settings)
        .map_err(|e| {
            AppError::Internal(format!("Corrupt preferences JSON for user {user_id}: {e}"))
        })?;

    Ok(Json(PreferencesResponse { settings }))
}

async fn update_preferences(
    State(state): State<SharedState>,
    AuthUser(claims): AuthUser,
    Json(body): Json<UpdatePreferencesRequest>,
) -> Result<Json<PreferencesResponse>, AppError> {
    let user_id = parse_user_id(&claims.sub)?;

    let settings_str = body.settings.to_string();

    let prefs = state
        .user_service
        .set_preferences(user_id, &settings_str)
        .await?;

    let settings: serde_json::Value = serde_json::from_str(&prefs.settings)
        .map_err(|e| {
            AppError::Internal(format!("Corrupt preferences JSON for user {user_id}: {e}"))
        })?;

    Ok(Json(PreferencesResponse { settings }))
}

// --- Router ------------------------------------------------------------------

/// Build the user route group (profile, preferences).
pub fn router() -> Router<SharedState> {
    Router::new()
        .route("/me", get(get_me))
        .route(
            "/me/preferences",
            get(get_preferences).put(update_preferences),
        )
}

