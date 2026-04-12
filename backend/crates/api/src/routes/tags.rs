use application::tag::service::{TagService, TagServiceError};
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{get, post},
};
use domain::entity_tag::TaggableEntityType;
use domain::tag::TagCategory;
use serde::{Deserialize, Serialize};

use crate::state::SharedState;
use super::organizations::{parse_user_id, parse_uuid};
use crate::middleware::AuthUser;

// --- Request / Response types ------------------------------------------------

#[derive(Serialize)]
pub(crate) struct TagResponse {
    id: String,
    category: String,
    name: String,
    usage_count: i32,
    is_approved: bool,
}

pub(crate) fn to_tag_response(tag: &domain::tag::Tag) -> TagResponse {
    TagResponse {
        id: tag.id.to_string(),
        category: tag.category.as_str().to_string(),
        name: tag.name.clone(),
        usage_count: tag.usage_count,
        is_approved: tag.is_approved,
    }
}

#[derive(Deserialize)]
struct CreateTagRequest {
    category: String,
    name: String,
}

#[derive(Deserialize)]
struct UpdateTagRequest {
    name: String,
    is_approved: Option<bool>,
}

#[derive(Deserialize)]
struct AttachTagRequest {
    entity_type: String,
    entity_id: String,
    tag_id: String,
}

#[derive(Deserialize)]
struct DetachTagRequest {
    entity_type: String,
    entity_id: String,
    tag_id: String,
}

#[derive(Deserialize)]
struct SearchQuery {
    q: String,
    #[serde(default = "default_limit")]
    limit: i64,
}

#[derive(Deserialize)]
struct PaginationQuery {
    #[serde(default = "default_limit")]
    limit: i64,
    #[serde(default)]
    offset: i64,
}

fn default_limit() -> i64 {
    20
}

// --- Handlers ----------------------------------------------------------------

async fn create_tag(
    State(state): State<SharedState>,
    AuthUser(_claims): AuthUser,
    Json(body): Json<CreateTagRequest>,
) -> Result<(StatusCode, Json<TagResponse>), (StatusCode, String)> {
    let category = TagCategory::from_str(&body.category).ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            format!(
                "Invalid category: '{}'. Must be 'organization', 'character', 'metadata', or 'general'",
                body.category
            ),
        )
    })?;

    let tag = state
        .tag_service
        .create_tag(category, &body.name)
        .await
        .map_err(map_tag_error)?;

    Ok((StatusCode::CREATED, Json(to_tag_response(&tag))))
}

async fn get_tag(
    State(state): State<SharedState>,
    Path(id): Path<String>,
    AuthUser(_claims): AuthUser,
) -> Result<Json<TagResponse>, (StatusCode, String)> {
    let tag_id = parse_uuid(&id)?;
    let tag = state.tag_service.get_tag(tag_id).await.map_err(map_tag_error)?;
    Ok(Json(to_tag_response(&tag)))
}

async fn search_tags(
    State(state): State<SharedState>,
    Query(params): Query<SearchQuery>,
    AuthUser(_claims): AuthUser,
) -> Result<Json<Vec<TagResponse>>, (StatusCode, String)> {
    let tags = state
        .tag_service
        .search_tags(&params.q, params.limit)
        .await
        .map_err(map_tag_error)?;

    Ok(Json(tags.iter().map(to_tag_response).collect()))
}

async fn list_by_category(
    State(state): State<SharedState>,
    Path(category_str): Path<String>,
    Query(params): Query<PaginationQuery>,
    AuthUser(_claims): AuthUser,
) -> Result<Json<Vec<TagResponse>>, (StatusCode, String)> {
    let category = TagCategory::from_str(&category_str).ok_or_else(|| {
        (StatusCode::BAD_REQUEST, format!("Invalid category: '{category_str}'"))
    })?;

    let tags = state
        .tag_service
        .list_tags_by_category(category, params.limit, params.offset)
        .await
        .map_err(map_tag_error)?;

    Ok(Json(tags.iter().map(to_tag_response).collect()))
}

async fn update_tag(
    State(state): State<SharedState>,
    Path(id): Path<String>,
    AuthUser(_claims): AuthUser,
    Json(body): Json<UpdateTagRequest>,
) -> Result<Json<TagResponse>, (StatusCode, String)> {
    let tag_id = parse_uuid(&id)?;
    let tag = state
        .tag_service
        .update_tag(tag_id, &body.name, body.is_approved.unwrap_or(false))
        .await
        .map_err(map_tag_error)?;

    Ok(Json(to_tag_response(&tag)))
}

async fn delete_tag(
    State(state): State<SharedState>,
    Path(id): Path<String>,
    AuthUser(_claims): AuthUser,
) -> Result<StatusCode, (StatusCode, String)> {
    let tag_id = parse_uuid(&id)?;
    state.tag_service.delete_tag(tag_id).await.map_err(map_tag_error)?;
    Ok(StatusCode::NO_CONTENT)
}

async fn approve_tag(
    State(state): State<SharedState>,
    Path(id): Path<String>,
    AuthUser(_claims): AuthUser,
) -> Result<Json<TagResponse>, (StatusCode, String)> {
    let tag_id = parse_uuid(&id)?;
    let tag = state.tag_service.approve_tag(tag_id).await.map_err(map_tag_error)?;
    Ok(Json(to_tag_response(&tag)))
}

async fn attach_tag(
    State(state): State<SharedState>,
    AuthUser(_claims): AuthUser,
    Json(body): Json<AttachTagRequest>,
) -> Result<(StatusCode, Json<serde_json::Value>), (StatusCode, String)> {
    let entity_type = parse_entity_type(&body.entity_type)?;
    let entity_id = parse_uuid(&body.entity_id)?;
    let tag_id = parse_uuid(&body.tag_id)?;

    state
        .tag_service
        .attach_tag(entity_type, entity_id, tag_id)
        .await
        .map_err(map_tag_error)?;

    Ok((
        StatusCode::CREATED,
        Json(serde_json::json!({
            "entity_type": body.entity_type,
            "entity_id": body.entity_id,
            "tag_id": body.tag_id,
        })),
    ))
}

async fn detach_tag(
    State(state): State<SharedState>,
    AuthUser(_claims): AuthUser,
    Json(body): Json<DetachTagRequest>,
) -> Result<StatusCode, (StatusCode, String)> {
    let entity_type = parse_entity_type(&body.entity_type)?;
    let entity_id = parse_uuid(&body.entity_id)?;
    let tag_id = parse_uuid(&body.tag_id)?;

    state
        .tag_service
        .detach_tag(entity_type, entity_id, tag_id)
        .await
        .map_err(map_tag_error)?;

    Ok(StatusCode::NO_CONTENT)
}

async fn list_entity_tags(
    State(state): State<SharedState>,
    Path((entity_type_str, entity_id_str)): Path<(String, String)>,
    AuthUser(_claims): AuthUser,
) -> Result<Json<Vec<TagResponse>>, (StatusCode, String)> {
    let entity_type = parse_entity_type(&entity_type_str)?;
    let entity_id = parse_uuid(&entity_id_str)?;

    let tags = state
        .tag_service
        .list_tags_for_entity(entity_type, entity_id)
        .await
        .map_err(map_tag_error)?;

    Ok(Json(tags.iter().map(to_tag_response).collect()))
}

// --- Router ------------------------------------------------------------------

pub fn router() -> Router<SharedState> {
    Router::new()
        .route("/", post(create_tag))
        .route("/search", get(search_tags))
        .route("/category/{category}", get(list_by_category))
        .route("/attach", post(attach_tag))
        .route("/detach", post(detach_tag))
        .route("/entity/{entity_type}/{entity_id}", get(list_entity_tags))
        .route("/{id}", get(get_tag).put(update_tag).delete(delete_tag))
        .route("/{id}/approve", post(approve_tag))
}

// --- Error mapping -----------------------------------------------------------

fn map_tag_error(e: TagServiceError) -> (StatusCode, String) {
    match e {
        TagServiceError::NotFound => (StatusCode::NOT_FOUND, "Tag not found".into()),
        TagServiceError::NameTaken(s) => {
            (StatusCode::CONFLICT, format!("Tag name already taken: {s}"))
        }
        TagServiceError::Immutable => (
            StatusCode::FORBIDDEN,
            "Entity-backed tags cannot be modified".into(),
        ),
        TagServiceError::InvalidName(msg) => (StatusCode::BAD_REQUEST, msg),
        TagServiceError::AlreadyAttached => (
            StatusCode::CONFLICT,
            "Tag is already attached to this entity".into(),
        ),
        TagServiceError::Internal(inner) => {
            eprintln!("Internal tag service error: {inner}");
            (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error".into())
        }
    }
}

// --- Helpers -----------------------------------------------------------------

fn parse_entity_type(s: &str) -> Result<TaggableEntityType, (StatusCode, String)> {
    TaggableEntityType::from_str(s).ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            format!(
                "Invalid entity type: '{s}'. Must be 'org', 'commission', 'feed_item', 'character', or 'feed_element'"
            ),
        )
    })
}
