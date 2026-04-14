//! Tag API routes: CRUD, search, attach/detach, approval.
//!
//! All routes require authentication. Authorization (role-gating approve/delete
//! to admins/mods) is tracked for a future iteration.

use application::tag::service::TagServiceError;
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
use super::helpers::parse_uuid;
use crate::middleware::AuthUser;

/// Default page size for paginated tag queries.
const DEFAULT_PAGE_SIZE: i64 = 20;

// --- Request / Response types ------------------------------------------------

/// JSON response for a single tag.
#[derive(Serialize)]
pub(crate) struct TagResponse {
    id: String,
    category: String,
    name: String,
    usage_count: i32,
    is_approved: bool,
}

impl From<&domain::tag::Tag> for TagResponse {
    fn from(tag: &domain::tag::Tag) -> Self {
        Self {
            id: tag.id.to_string(),
            category: tag.category.as_str().to_string(),
            name: tag.name.clone(),
            usage_count: tag.usage_count,
            is_approved: tag.is_approved,
        }
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
    DEFAULT_PAGE_SIZE
}

// --- Handlers ----------------------------------------------------------------

/// POST /tags — create a user-submitted tag (metadata or general only).
// TODO(review): no authorization check — any authenticated user can create tags. Gate behind admin/mod role when roles are wired.
async fn create_tag(
    State(state): State<SharedState>,
    AuthUser(_claims): AuthUser,
    Json(body): Json<CreateTagRequest>,
) -> Result<(StatusCode, Json<TagResponse>), (StatusCode, String)> {
    let category = TagCategory::try_from(body.category.as_str()).map_err(|_| {
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

    Ok((StatusCode::CREATED, Json(TagResponse::from(&tag))))
}

/// GET /tags/:id — get a tag by UUID.
async fn get_tag(
    State(state): State<SharedState>,
    Path(id): Path<String>,
    AuthUser(_claims): AuthUser,
) -> Result<Json<TagResponse>, (StatusCode, String)> {
    let tag_id = parse_uuid(&id)?;
    let tag = state.tag_service.get_tag(tag_id).await.map_err(map_tag_error)?;
    Ok(Json(TagResponse::from(&tag)))
}

/// GET /tags/search?q=&limit= — prefix search on tag name.
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

    Ok(Json(tags.iter().map(TagResponse::from).collect()))
}

/// GET /tags/category/:category?limit=&offset= — list tags by category, paginated.
async fn list_by_category(
    State(state): State<SharedState>,
    Path(category_str): Path<String>,
    Query(params): Query<PaginationQuery>,
    AuthUser(_claims): AuthUser,
) -> Result<Json<Vec<TagResponse>>, (StatusCode, String)> {
    let category = TagCategory::try_from(category_str.as_str()).map_err(|_| {
        (StatusCode::BAD_REQUEST, format!("Invalid category: '{category_str}'"))
    })?;

    let offset = params.offset.max(0);

    let tags = state
        .tag_service
        .list_tags_by_category(category, params.limit, offset)
        .await
        .map_err(map_tag_error)?;

    Ok(Json(tags.iter().map(TagResponse::from).collect()))
}

/// PUT /tags/:id — update a tag's name (and optionally approval). Metadata/general only.
/// Preserves existing `is_approved` when the field is omitted from the request.
async fn update_tag(
    State(state): State<SharedState>,
    Path(id): Path<String>,
    AuthUser(_claims): AuthUser,
    Json(body): Json<UpdateTagRequest>,
) -> Result<Json<TagResponse>, (StatusCode, String)> {
    let tag_id = parse_uuid(&id)?;

    let is_approved = match body.is_approved {
        Some(v) => v,
        None => {
            let existing = state.tag_service.get_tag(tag_id).await.map_err(map_tag_error)?;
            existing.is_approved
        }
    };

    let tag = state
        .tag_service
        .update_tag(tag_id, &body.name, is_approved)
        .await
        .map_err(map_tag_error)?;

    Ok(Json(TagResponse::from(&tag)))
}

/// DELETE /tags/:id — hard-delete a tag. Metadata/general only.
// TODO(review): no authorization check — any authenticated user can delete tags. Gate behind admin/mod role when roles are wired.
async fn delete_tag(
    State(state): State<SharedState>,
    Path(id): Path<String>,
    AuthUser(_claims): AuthUser,
) -> Result<StatusCode, (StatusCode, String)> {
    let tag_id = parse_uuid(&id)?;
    state.tag_service.delete_tag(tag_id).await.map_err(map_tag_error)?;
    Ok(StatusCode::NO_CONTENT)
}

/// POST /tags/:id/approve — mark a tag as approved. Metadata/general only.
// TODO(review): no authorization check — any authenticated user can approve tags. Must be admin/mod-only.
async fn approve_tag(
    State(state): State<SharedState>,
    Path(id): Path<String>,
    AuthUser(_claims): AuthUser,
) -> Result<Json<TagResponse>, (StatusCode, String)> {
    let tag_id = parse_uuid(&id)?;
    let tag = state.tag_service.approve_tag(tag_id).await.map_err(map_tag_error)?;
    Ok(Json(TagResponse::from(&tag)))
}

/// POST /tags/attach — attach an existing tag to an entity. Increments usage count.
// TODO(review): no authorization check — any authenticated user can attach tags to any entity. Should verify entity ownership.
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

/// POST /tags/detach — remove a tag from an entity. Decrements usage count.
// TODO(review): no authorization check — any authenticated user can detach tags from any entity. Should verify entity ownership.
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

/// GET /tags/entity/:type/:id — list all tags attached to an entity.
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

    Ok(Json(tags.iter().map(TagResponse::from).collect()))
}

// --- Router ------------------------------------------------------------------

/// Build the tag router. Mounted at `/tags` in the main router.
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

/// Map `TagServiceError` to an HTTP status code and message for the client.
fn map_tag_error(e: TagServiceError) -> (StatusCode, String) {
    match e {
        TagServiceError::NotFound => (StatusCode::NOT_FOUND, "Tag not found".into()),
        TagServiceError::NotAttached => (
            StatusCode::NOT_FOUND,
            "Tag is not attached to this entity".into(),
        ),
        TagServiceError::NameTaken(s) => {
            (StatusCode::CONFLICT, format!("Tag name already taken: {s}"))
        }
        TagServiceError::Immutable => (
            StatusCode::FORBIDDEN,
            "Entity-backed tags cannot be modified".into(),
        ),
        TagServiceError::InvalidCategory => (
            StatusCode::BAD_REQUEST,
            "This category cannot be used for user-created tags. Use 'metadata' or 'general'.".into(),
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

/// Parse a string into a `TaggableEntityType`, returning 400 on invalid input.
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
