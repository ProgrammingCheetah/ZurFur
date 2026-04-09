use application::organization::service::OrgServiceError;
use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    routing::{get, post, put},
};
use domain::organization_profile::CommissionStatus;
use serde::{Deserialize, Serialize};

use crate::middleware::AuthUser;
use crate::state::SharedState;

// --- Request / Response types ------------------------------------------------

#[derive(Serialize)]
struct OrgResponse {
    id: String,
    slug: String,
    display_name: Option<String>,
    is_personal: bool,
}

#[derive(Serialize)]
struct MemberResponse {
    id: String,
    user_id: String,
    role: String,
    title: Option<String>,
    is_owner: bool,
    permissions: u64,
}

#[derive(Serialize)]
struct OrgProfileResponse {
    bio: Option<String>,
    commission_status: String,
}

#[derive(Serialize)]
struct OrgDetailResponse {
    org: OrgResponse,
    members: Vec<MemberResponse>,
    profile: Option<OrgProfileResponse>,
}

#[derive(Deserialize)]
struct CreateOrgRequest {
    slug: String,
    display_name: String,
}

#[derive(Deserialize)]
struct UpdateOrgRequest {
    display_name: Option<String>,
}

#[derive(Deserialize)]
struct UpdateProfileRequest {
    bio: Option<String>,
    commission_status: String,
}

#[derive(Deserialize)]
struct AddMemberRequest {
    user_id: String,
    role: String,
    title: Option<String>,
}

#[derive(Deserialize)]
struct UpdateMemberRequest {
    role: String,
    title: Option<String>,
    permissions: Option<u64>,
}

// --- Handlers ----------------------------------------------------------------

/// POST /organizations — create a new org (creator = owner).
async fn create_org(
    State(state): State<SharedState>,
    AuthUser(claims): AuthUser,
    Json(body): Json<CreateOrgRequest>,
) -> Result<(StatusCode, Json<OrgDetailResponse>), (StatusCode, String)> {
    let user_id = parse_user_id(&claims.sub)?;

    let detail = state
        .org_service
        .create_org(user_id, &body.slug, &body.display_name)
        .await
        .map_err(map_org_error)?;

    Ok((StatusCode::CREATED, Json(to_detail_response(detail))))
}

/// GET /organizations/:id_or_slug — org details + members + profile.
/// Accepts either a UUID (org id) or a slug string.
async fn get_org(
    State(state): State<SharedState>,
    Path(id_or_slug): Path<String>,
    AuthUser(_claims): AuthUser,
) -> Result<Json<OrgDetailResponse>, (StatusCode, String)> {
    let detail = if let Ok(id) = id_or_slug.parse::<uuid::Uuid>() {
        state.org_service.get_org_by_id(id).await
    } else {
        state.org_service.get_org(&id_or_slug).await
    }
    .map_err(map_org_error)?;

    Ok(Json(to_detail_response(detail)))
}

/// PUT /organizations/:id_or_slug — update display_name (owner only).
async fn update_org(
    State(state): State<SharedState>,
    Path(id_or_slug): Path<String>,
    AuthUser(claims): AuthUser,
    Json(body): Json<UpdateOrgRequest>,
) -> Result<Json<OrgResponse>, (StatusCode, String)> {
    let user_id = parse_user_id(&claims.sub)?;
    let org_id = resolve_org_id(&state, &id_or_slug).await?;

    let org = state
        .org_service
        .update_org(org_id, user_id, body.display_name.as_deref())
        .await
        .map_err(map_org_error)?;

    Ok(Json(to_org_response(&org)))
}

/// DELETE /organizations/:id_or_slug — soft-delete org (owner only, personal orgs rejected).
async fn delete_org(
    State(state): State<SharedState>,
    Path(id_or_slug): Path<String>,
    AuthUser(claims): AuthUser,
) -> Result<StatusCode, (StatusCode, String)> {
    let user_id = parse_user_id(&claims.sub)?;
    let org_id = resolve_org_id(&state, &id_or_slug).await?;

    state
        .org_service
        .delete_org(org_id, user_id)
        .await
        .map_err(map_org_error)?;

    Ok(StatusCode::NO_CONTENT)
}

/// PUT /organizations/:id/profile — update bio/commission_status (MANAGE_PROFILE perm).
async fn update_profile(
    State(state): State<SharedState>,
    Path(id): Path<String>,
    AuthUser(claims): AuthUser,
    Json(body): Json<UpdateProfileRequest>,
) -> Result<Json<OrgProfileResponse>, (StatusCode, String)> {
    let user_id = parse_user_id(&claims.sub)?;
    let org_id = parse_uuid(&id)?;

    let status = CommissionStatus::from_str(&body.commission_status).ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            format!(
                "Invalid commission status: '{}'. Must be 'open', 'closed', or 'waitlist'",
                body.commission_status
            ),
        )
    })?;

    let profile = state
        .org_service
        .update_profile(org_id, user_id, body.bio.as_deref(), status)
        .await
        .map_err(map_org_error)?;

    Ok(Json(OrgProfileResponse {
        bio: profile.bio,
        commission_status: profile.commission_status.as_str().to_string(),
    }))
}

/// GET /organizations/:id/members — list members.
async fn list_members(
    State(state): State<SharedState>,
    Path(id): Path<String>,
    AuthUser(_claims): AuthUser,
) -> Result<Json<Vec<MemberResponse>>, (StatusCode, String)> {
    let org_id = parse_uuid(&id)?;

    // Verify org exists by fetching it
    let detail = state
        .org_service
        .get_org_by_id(org_id)
        .await
        .map_err(map_org_error)?;

    Ok(Json(
        detail
            .members
            .iter()
            .map(to_member_response)
            .collect(),
    ))
}

/// POST /organizations/:id/members — add member (MANAGE_MEMBERS perm).
async fn add_member(
    State(state): State<SharedState>,
    Path(id): Path<String>,
    AuthUser(claims): AuthUser,
    Json(body): Json<AddMemberRequest>,
) -> Result<(StatusCode, Json<MemberResponse>), (StatusCode, String)> {
    let user_id = parse_user_id(&claims.sub)?;
    let org_id = parse_uuid(&id)?;
    let target_user_id = parse_uuid(&body.user_id)?;

    let member = state
        .org_service
        .add_member(org_id, user_id, target_user_id, &body.role, body.title.as_deref())
        .await
        .map_err(map_org_error)?;

    Ok((StatusCode::CREATED, Json(to_member_response(&member))))
}

/// PUT /organizations/:id/members/:user_id — update role/title/permissions.
async fn update_member(
    State(state): State<SharedState>,
    Path((id, target_user_id_str)): Path<(String, String)>,
    AuthUser(claims): AuthUser,
    Json(body): Json<UpdateMemberRequest>,
) -> Result<Json<MemberResponse>, (StatusCode, String)> {
    let user_id = parse_user_id(&claims.sub)?;
    let org_id = parse_uuid(&id)?;
    let target_user_id = parse_uuid(&target_user_id_str)?;

    let permissions = body
        .permissions
        .map(domain::organization_member::Permissions::new);

    let member = state
        .org_service
        .update_member(
            org_id,
            user_id,
            target_user_id,
            &body.role,
            body.title.as_deref(),
            permissions,
        )
        .await
        .map_err(map_org_error)?;

    Ok(Json(to_member_response(&member)))
}

/// DELETE /organizations/:id/members/:user_id — remove member.
async fn remove_member(
    State(state): State<SharedState>,
    Path((id, target_user_id_str)): Path<(String, String)>,
    AuthUser(claims): AuthUser,
) -> Result<StatusCode, (StatusCode, String)> {
    let user_id = parse_user_id(&claims.sub)?;
    let org_id = parse_uuid(&id)?;
    let target_user_id = parse_uuid(&target_user_id_str)?;

    state
        .org_service
        .remove_member(org_id, user_id, target_user_id)
        .await
        .map_err(map_org_error)?;

    Ok(StatusCode::NO_CONTENT)
}

// --- Router ------------------------------------------------------------------

pub fn router() -> Router<SharedState> {
    // All sub-resource routes use /{id}/... where id is a UUID.
    // The GET by slug route shares the same /{param} pattern —
    // the handler disambiguates by trying UUID parse first, then slug lookup.
    Router::new()
        .route("/", post(create_org))
        .route("/{id_or_slug}", get(get_org).put(update_org).delete(delete_org))
        .route("/{id}/profile", put(update_profile))
        .route("/{id}/members", get(list_members).post(add_member))
        .route(
            "/{id}/members/{user_id}",
            put(update_member).delete(remove_member),
        )
}

// --- Response mapping --------------------------------------------------------

fn to_org_response(org: &domain::organization::Organization) -> OrgResponse {
    OrgResponse {
        id: org.id.to_string(),
        slug: org.slug.clone(),
        display_name: org.display_name.clone(),
        is_personal: org.is_personal,
    }
}

fn to_member_response(m: &domain::organization_member::OrganizationMember) -> MemberResponse {
    MemberResponse {
        id: m.id.to_string(),
        user_id: m.user_id.to_string(),
        role: m.role.clone(),
        title: m.title.clone(),
        is_owner: m.is_owner,
        permissions: m.permissions.0,
    }
}

fn to_detail_response(
    detail: application::organization::service::OrgDetail,
) -> OrgDetailResponse {
    let profile = detail.profile.map(|p| OrgProfileResponse {
        bio: p.bio,
        commission_status: p.commission_status.as_str().to_string(),
    });

    OrgDetailResponse {
        org: to_org_response(&detail.org),
        members: detail.members.iter().map(to_member_response).collect(),
        profile,
    }
}

// --- Error mapping -----------------------------------------------------------

fn map_org_error(e: OrgServiceError) -> (StatusCode, String) {
    match e {
        OrgServiceError::NotFound => (StatusCode::NOT_FOUND, "Organization not found".into()),
        OrgServiceError::SlugTaken(s) => {
            (StatusCode::CONFLICT, format!("Slug already taken: {s}"))
        }
        OrgServiceError::InvalidSlug(msg) => (StatusCode::BAD_REQUEST, msg),
        OrgServiceError::Forbidden => (StatusCode::FORBIDDEN, "Permission denied".into()),
        OrgServiceError::CannotDeletePersonal => (
            StatusCode::FORBIDDEN,
            "Cannot delete a personal organization".into(),
        ),
        OrgServiceError::CannotRemoveOwner => (
            StatusCode::FORBIDDEN,
            "Cannot remove the owner from an organization".into(),
        ),
        OrgServiceError::InvalidStatus(s) => {
            (StatusCode::BAD_REQUEST, format!("Invalid commission status: {s}"))
        }
        OrgServiceError::Internal(inner) => {
            eprintln!("Internal org service error: {inner}");
            (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error".into())
        }
    }
}

// --- Helpers -----------------------------------------------------------------

fn parse_user_id(sub: &str) -> Result<uuid::Uuid, (StatusCode, String)> {
    sub.parse()
        .map_err(|_| (StatusCode::BAD_REQUEST, "Invalid user ID in token".into()))
}

fn parse_uuid(s: &str) -> Result<uuid::Uuid, (StatusCode, String)> {
    s.parse()
        .map_err(|_| (StatusCode::BAD_REQUEST, format!("Invalid UUID: {s}")))
}

/// Resolve an org ID from either a UUID string or a slug.
/// Tries UUID parse first; falls back to slug lookup.
async fn resolve_org_id(
    state: &SharedState,
    id_or_slug: &str,
) -> Result<uuid::Uuid, (StatusCode, String)> {
    if let Ok(id) = id_or_slug.parse::<uuid::Uuid>() {
        Ok(id)
    } else {
        let detail = state
            .org_service
            .get_org(id_or_slug)
            .await
            .map_err(map_org_error)?;
        Ok(detail.org.id)
    }
}
