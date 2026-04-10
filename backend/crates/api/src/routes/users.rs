use application::user::service::UserServiceError;
use axum::{
    Json, Router,
    extract::State,
    http::StatusCode,
    routing::get,
};
use domain::content_rating::ContentRating;
use serde::{Deserialize, Serialize};

use crate::middleware::AuthUser;
use crate::state::SharedState;

// --- Response types ----------------------------------------------------------

#[derive(Serialize)]
struct MembershipResponse {
    org_id: String,
    role: String,
    title: Option<String>,
}

#[derive(Serialize)]
struct OrgProfileResponse {
    bio: Option<String>,
    commission_status: String,
}

#[derive(Serialize)]
struct PersonalOrgResponse {
    id: String,
    slug: String,
    display_name: Option<String>,
    profile: Option<OrgProfileResponse>,
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
    max_content_rating: String,
}

#[derive(Deserialize)]
struct UpdatePreferencesRequest {
    max_content_rating: String,
}

// --- Handlers ----------------------------------------------------------------

/// GET /users/me — user identity + personal org profile + all org memberships.
async fn get_me(
    State(state): State<SharedState>,
    AuthUser(claims): AuthUser,
) -> Result<Json<UserProfileResponse>, (StatusCode, String)> {
    let user_id: uuid::Uuid = claims
        .sub
        .parse()
        .map_err(|_| (StatusCode::BAD_REQUEST, "Invalid user ID in token".into()))?;

    let profile = state
        .user_service
        .get_my_profile(user_id)
        .await
        .map_err(map_user_error)?;

    let personal_org = profile.personal_org.map(|org| {
        let org_profile = profile.personal_org_profile.map(|p| OrgProfileResponse {
            bio: p.bio,
            commission_status: p.commission_status.as_str().to_string(),
        });

        PersonalOrgResponse {
            id: org.id.to_string(),
            slug: org.slug,
            // Resolve NULL display_name from owner's handle/username
            display_name: org
                .display_name
                .or_else(|| profile.user.handle.clone())
                .or_else(|| Some(profile.user.username.clone())),
            profile: org_profile,
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

/// GET /users/me/preferences
async fn get_preferences(
    State(state): State<SharedState>,
    AuthUser(claims): AuthUser,
) -> Result<Json<PreferencesResponse>, (StatusCode, String)> {
    let user_id: uuid::Uuid = claims
        .sub
        .parse()
        .map_err(|_| (StatusCode::BAD_REQUEST, "Invalid user ID in token".into()))?;

    let prefs = state
        .user_service
        .get_preferences(user_id)
        .await
        .map_err(map_user_error)?;

    Ok(Json(PreferencesResponse {
        max_content_rating: prefs.max_content_rating.as_str().to_string(),
    }))
}

/// PUT /users/me/preferences
async fn update_preferences(
    State(state): State<SharedState>,
    AuthUser(claims): AuthUser,
    Json(body): Json<UpdatePreferencesRequest>,
) -> Result<Json<PreferencesResponse>, (StatusCode, String)> {
    let user_id: uuid::Uuid = claims
        .sub
        .parse()
        .map_err(|_| (StatusCode::BAD_REQUEST, "Invalid user ID in token".into()))?;

    let rating = ContentRating::from_str(&body.max_content_rating).ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            format!(
                "Invalid content rating: '{}'. Must be 'sfw', 'questionable', or 'nsfw'",
                body.max_content_rating
            ),
        )
    })?;

    let prefs = state
        .user_service
        .set_max_content_rating(user_id, rating)
        .await
        .map_err(map_user_error)?;

    Ok(Json(PreferencesResponse {
        max_content_rating: prefs.max_content_rating.as_str().to_string(),
    }))
}

// --- Router ------------------------------------------------------------------

pub fn router() -> Router<SharedState> {
    Router::new()
        .route("/me", get(get_me))
        .route(
            "/me/preferences",
            get(get_preferences).put(update_preferences),
        )
}

// --- Error mapping -----------------------------------------------------------

fn map_user_error(e: UserServiceError) -> (StatusCode, String) {
    match e {
        UserServiceError::NotFound => (StatusCode::NOT_FOUND, "User not found".into()),
        UserServiceError::InvalidRating(r) => {
            (StatusCode::BAD_REQUEST, format!("Invalid content rating: {r}"))
        }
        UserServiceError::Internal(inner) => {
            eprintln!("Internal user service error: {inner}");
            (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error".into())
        }
    }
}
