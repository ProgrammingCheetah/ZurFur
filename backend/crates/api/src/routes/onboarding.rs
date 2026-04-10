use application::onboarding::service::OnboardingError;
use axum::{Json, Router, extract::State, http::StatusCode, routing::post};
use domain::onboarding_role::OnboardingRole;
use serde::{Deserialize, Serialize};

use crate::middleware::AuthUser;
use crate::state::SharedState;

#[derive(Deserialize)]
struct CompleteOnboardingRequest {
    role: String,
}

#[derive(Serialize)]
struct FeedResponse {
    id: String,
    slug: String,
    display_name: String,
    feed_type: String,
}

#[derive(Serialize)]
struct OnboardingResponse {
    role: String,
    feeds_created: Vec<FeedResponse>,
}

async fn complete_onboarding(
    State(state): State<SharedState>,
    AuthUser(claims): AuthUser,
    Json(body): Json<CompleteOnboardingRequest>,
) -> Result<Json<OnboardingResponse>, (StatusCode, String)> {
    let user_id: uuid::Uuid = claims
        .sub
        .parse()
        .map_err(|_| (StatusCode::BAD_REQUEST, "Invalid user ID in token".into()))?;

    let role = OnboardingRole::from_str(&body.role).ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            format!(
                "Invalid onboarding role: '{}'. Must be 'artist', 'crafter_maker', 'commissioner_client', or 'coder_developer'",
                body.role
            ),
        )
    })?;

    let result = state
        .onboarding_service
        .complete_onboarding(user_id, role)
        .await
        .map_err(map_onboarding_error)?;

    let feeds_created = result
        .feeds_created
        .iter()
        .map(|f| FeedResponse {
            id: f.id.to_string(),
            slug: f.slug.clone(),
            display_name: f.display_name.clone(),
            feed_type: f.feed_type.as_str().to_string(),
        })
        .collect();

    let response = OnboardingResponse {
        role: result.onboarding_role.as_str().to_string(),
        feeds_created,
    };

    Ok(Json(response))
}

pub fn router() -> Router<SharedState> {
    Router::new().route("/complete", post(complete_onboarding))
}

fn map_onboarding_error(e: OnboardingError) -> (StatusCode, String) {
    match e {
        OnboardingError::UserNotFound => (StatusCode::NOT_FOUND, "User not found".into()),
        OnboardingError::PersonalOrgNotFound => (
            StatusCode::NOT_FOUND,
            "Personal organization not found".into(),
        ),
        OnboardingError::InvalidRole(r) => {
            (StatusCode::BAD_REQUEST, format!("Invalid role: {r}"))
        }
        OnboardingError::Internal(inner) => {
            eprintln!("Internal onboarding error: {inner}");
            (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error".into())
        }
    }
}
