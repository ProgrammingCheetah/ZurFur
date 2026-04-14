use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    routing::{get, post, put},
};
use serde::{Deserialize, Serialize};

use crate::error::AppError;
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
    permissions: u64,
}

#[derive(Serialize)]
struct OrgDetailResponse {
    org: OrgResponse,
    members: Vec<MemberResponse>,
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

async fn create_org(
    State(state): State<SharedState>,
    AuthUser(claims): AuthUser,
    Json(body): Json<CreateOrgRequest>,
) -> Result<(StatusCode, Json<OrgDetailResponse>), AppError> {
    let user_id = parse_user_id(&claims.sub)?;

    let detail = state
        .org_service
        .create_org(user_id, &body.slug, &body.display_name)
        .await?;

    // TODO(review): org + member + tag + feed creation spans 4 separate DB operations with no transaction — partial failure leaves inconsistent state (Feature 3.5)
    // Orchestration: auto-create org tag + bio feed (best-effort)
    if let Err(e) = state
        .tag_service
        .create_entity_tag(
            domain::tag::TagCategory::Organization,
            domain::entity_tag::TaggableEntityType::Org,
            detail.org.id,
            &body.slug,
        )
        .await
    {
        tracing::warn!(org_id = %detail.org.id, error = %e, "Failed to create org tag");
    }

    if let Err(e) = state
        .feed_service
        .create_system_feed(detail.org.id, "bio", "Bio")
        .await
    {
        tracing::warn!(org_id = %detail.org.id, error = %e, "Failed to create bio feed");
    }

    Ok((StatusCode::CREATED, Json(to_detail_response(detail))))
}

async fn get_org(
    State(state): State<SharedState>,
    Path(id_or_slug): Path<String>,
    AuthUser(_claims): AuthUser,
) -> Result<Json<OrgDetailResponse>, AppError> {
    let detail = if let Ok(id) = id_or_slug.parse::<uuid::Uuid>() {
        state.org_service.get_org_by_id(id).await
    } else {
        state.org_service.get_org(&id_or_slug).await
    }?;

    Ok(Json(to_detail_response(detail)))
}

async fn update_org(
    State(state): State<SharedState>,
    Path(id_or_slug): Path<String>,
    AuthUser(claims): AuthUser,
    Json(body): Json<UpdateOrgRequest>,
) -> Result<Json<OrgResponse>, AppError> {
    let user_id = parse_user_id(&claims.sub)?;
    let org_id = resolve_org_id(&state, &id_or_slug).await?;

    let org = state
        .org_service
        .update_org(org_id, user_id, body.display_name.as_deref())
        .await?;

    Ok(Json(to_org_response(&org)))
}

async fn delete_org(
    State(state): State<SharedState>,
    Path(id_or_slug): Path<String>,
    AuthUser(claims): AuthUser,
) -> Result<StatusCode, AppError> {
    let user_id = parse_user_id(&claims.sub)?;
    let org_id = resolve_org_id(&state, &id_or_slug).await?;

    state
        .org_service
        .delete_org(org_id, user_id)
        .await?;

    Ok(StatusCode::NO_CONTENT)
}

async fn list_members(
    State(state): State<SharedState>,
    Path(id): Path<String>,
    AuthUser(_claims): AuthUser,
) -> Result<Json<Vec<MemberResponse>>, AppError> {
    let org_id = parse_uuid(&id)?;

    let detail = state
        .org_service
        .get_org_by_id(org_id)
        .await?;

    Ok(Json(
        detail
            .members
            .iter()
            .map(to_member_response)
            .collect(),
    ))
}

async fn add_member(
    State(state): State<SharedState>,
    Path(id): Path<String>,
    AuthUser(claims): AuthUser,
    Json(body): Json<AddMemberRequest>,
) -> Result<(StatusCode, Json<MemberResponse>), AppError> {
    let user_id = parse_user_id(&claims.sub)?;
    let org_id = parse_uuid(&id)?;
    let target_user_id = parse_uuid(&body.user_id)?;

    let role = domain::organization_member::Role::from_str(&body.role).ok_or_else(|| {
        AppError::BadRequest(format!(
            "Invalid role: '{}'. Must be 'owner', 'admin', 'mod', or 'member'",
            body.role,
        ))
    })?;

    let member = state
        .org_service
        .add_member(org_id, user_id, target_user_id, role, body.title.as_deref())
        .await?;

    Ok((StatusCode::CREATED, Json(to_member_response(&member))))
}

async fn update_member(
    State(state): State<SharedState>,
    Path((id, target_user_id_str)): Path<(String, String)>,
    AuthUser(claims): AuthUser,
    Json(body): Json<UpdateMemberRequest>,
) -> Result<Json<MemberResponse>, AppError> {
    let user_id = parse_user_id(&claims.sub)?;
    let org_id = parse_uuid(&id)?;
    let target_user_id = parse_uuid(&target_user_id_str)?;

    let role = domain::organization_member::Role::from_str(&body.role).ok_or_else(|| {
        AppError::BadRequest(format!(
            "Invalid role: '{}'. Must be 'owner', 'admin', 'mod', or 'member'",
            body.role,
        ))
    })?;

    let permissions = body
        .permissions
        .map(domain::organization_member::Permissions::new);

    let member = state
        .org_service
        .update_member(
            org_id,
            user_id,
            target_user_id,
            role,
            body.title.as_deref(),
            permissions,
        )
        .await?;

    Ok(Json(to_member_response(&member)))
}

async fn remove_member(
    State(state): State<SharedState>,
    Path((id, target_user_id_str)): Path<(String, String)>,
    AuthUser(claims): AuthUser,
) -> Result<StatusCode, AppError> {
    let user_id = parse_user_id(&claims.sub)?;
    let org_id = parse_uuid(&id)?;
    let target_user_id = parse_uuid(&target_user_id_str)?;

    state
        .org_service
        .remove_member(org_id, user_id, target_user_id)
        .await?;

    Ok(StatusCode::NO_CONTENT)
}

// --- Router ------------------------------------------------------------------

/// Build the organization route group (CRUD, members, feeds).
pub fn router() -> Router<SharedState> {
    Router::new()
        .route("/", post(create_org))
        .route("/{id_or_slug}", get(get_org).put(update_org).delete(delete_org))
        .route("/{id}/members", get(list_members).post(add_member))
        .route(
            "/{id}/members/{user_id}",
            put(update_member).delete(remove_member),
        )
        .route(
            "/{id}/feeds",
            get(super::feeds::list_org_feeds).post(super::feeds::create_org_feed),
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
        role: m.role.as_str().to_string(),
        title: m.title.clone(),
        permissions: m.permissions.0,
    }
}

fn to_detail_response(
    detail: application::organization::service::OrgDetail,
) -> OrgDetailResponse {
    OrgDetailResponse {
        org: to_org_response(&detail.org),
        members: detail.members.iter().map(to_member_response).collect(),
    }
}

// --- Helpers (re-exported from shared module) --------------------------------

pub(super) use super::helpers::{parse_user_id, parse_uuid};

async fn resolve_org_id(
    state: &SharedState,
    id_or_slug: &str,
) -> Result<uuid::Uuid, AppError> {
    if let Ok(id) = id_or_slug.parse::<uuid::Uuid>() {
        Ok(id)
    } else {
        let detail = state
            .org_service
            .get_org(id_or_slug)
            .await?;
        Ok(detail.org.id)
    }
}
