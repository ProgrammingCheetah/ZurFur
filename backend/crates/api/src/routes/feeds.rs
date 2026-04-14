use application::feed::service::{FeedServiceError, NewFeedElement};
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{get, post},
};
use domain::feed_element::FeedElementType;
use serde::{Deserialize, Serialize};

use crate::middleware::AuthUser;
use crate::state::SharedState;

// --- Request / Response types ------------------------------------------------

#[derive(Serialize)]
pub(crate) struct FeedResponse {
    id: String,
    slug: String,
    display_name: String,
    description: Option<String>,
    feed_type: String,
}

#[derive(Serialize)]
struct FeedItemResponse {
    id: String,
    feed_id: String,
    author_type: String,
    author_id: String,
    created_at: String,
    elements: Vec<FeedElementResponse>,
}

#[derive(Serialize)]
struct FeedElementResponse {
    id: String,
    element_type: String,
    content_json: String,
    position: i32,
}

#[derive(Deserialize)]
pub(crate) struct CreateFeedRequest {
    slug: String,
    display_name: String,
    description: Option<String>,
}

#[derive(Deserialize)]
struct UpdateFeedRequest {
    display_name: String,
    description: Option<String>,
}

#[derive(Deserialize)]
struct PostToFeedRequest {
    elements: Vec<NewElementRequest>,
}

#[derive(Deserialize)]
struct NewElementRequest {
    element_type: String,
    content_json: String,
    position: i32,
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

// --- Feed CRUD routes (mounted at /feeds) ------------------------------------

async fn get_feed(
    State(state): State<SharedState>,
    Path(id): Path<String>,
    AuthUser(_claims): AuthUser,
) -> Result<Json<FeedResponse>, (StatusCode, String)> {
    let feed_id = parse_uuid(&id)?;

    let feed = state
        .feed_service
        .get_feed(feed_id)
        .await
        .map_err(map_feed_error)?;

    Ok(Json(to_feed_response(&feed)))
}

async fn update_feed(
    State(state): State<SharedState>,
    Path(id): Path<String>,
    AuthUser(claims): AuthUser,
    Json(body): Json<UpdateFeedRequest>,
) -> Result<Json<FeedResponse>, (StatusCode, String)> {
    let user_id = parse_user_id(&claims.sub)?;
    let feed_id = parse_uuid(&id)?;

    let feed = state
        .feed_service
        .update_feed(feed_id, user_id, &body.display_name, body.description.as_deref())
        .await
        .map_err(map_feed_error)?;

    Ok(Json(to_feed_response(&feed)))
}

async fn delete_feed(
    State(state): State<SharedState>,
    Path(id): Path<String>,
    AuthUser(claims): AuthUser,
) -> Result<StatusCode, (StatusCode, String)> {
    let user_id = parse_user_id(&claims.sub)?;
    let feed_id = parse_uuid(&id)?;

    state
        .feed_service
        .delete_feed(feed_id, user_id)
        .await
        .map_err(map_feed_error)?;

    Ok(StatusCode::NO_CONTENT)
}

async fn post_to_feed(
    State(state): State<SharedState>,
    Path(id): Path<String>,
    AuthUser(claims): AuthUser,
    Json(body): Json<PostToFeedRequest>,
) -> Result<(StatusCode, Json<FeedItemResponse>), (StatusCode, String)> {
    let user_id = parse_user_id(&claims.sub)?;
    let feed_id = parse_uuid(&id)?;

    let elements: Vec<NewFeedElement> = body
        .elements
        .into_iter()
        .map(|e| {
            let element_type = FeedElementType::from_str(&e.element_type).ok_or_else(|| {
                (
                    StatusCode::BAD_REQUEST,
                    format!("Invalid element type: '{}'", e.element_type),
                )
            })?;
            Ok(NewFeedElement {
                element_type,
                content_json: e.content_json,
                position: e.position,
            })
        })
        .collect::<Result<Vec<_>, (StatusCode, String)>>()?;

    let result = state
        .feed_service
        .post_to_feed(feed_id, user_id, elements)
        .await
        .map_err(map_feed_error)?;

    Ok((StatusCode::CREATED, Json(to_item_response(&result))))
}

async fn list_feed_items(
    State(state): State<SharedState>,
    Path(id): Path<String>,
    AuthUser(_claims): AuthUser,
    Query(pagination): Query<PaginationQuery>,
) -> Result<Json<Vec<FeedItemResponse>>, (StatusCode, String)> {
    let feed_id = parse_uuid(&id)?;

    let limit = pagination.limit.clamp(1, 100);
    let items = state
        .feed_service
        .list_feed_items(feed_id, limit, pagination.offset)
        .await
        .map_err(map_feed_error)?;

    let response: Vec<FeedItemResponse> = items.iter().map(to_item_response).collect();
    Ok(Json(response))
}

async fn delete_feed_item(
    State(state): State<SharedState>,
    Path((_, item_id_str)): Path<(String, String)>,
    AuthUser(claims): AuthUser,
) -> Result<StatusCode, (StatusCode, String)> {
    let user_id = parse_user_id(&claims.sub)?;
    let item_id = parse_uuid(&item_id_str)?;

    state
        .feed_service
        .delete_feed_item(item_id, user_id)
        .await
        .map_err(map_feed_error)?;

    Ok(StatusCode::NO_CONTENT)
}

// --- Org-scoped feed routes (mounted on organizations router) ----------------

pub(crate) async fn list_org_feeds(
    State(state): State<SharedState>,
    Path(id): Path<String>,
    AuthUser(_claims): AuthUser,
) -> Result<Json<Vec<FeedResponse>>, (StatusCode, String)> {
    let org_id = parse_uuid(&id)?;

    let feeds = state
        .feed_service
        .list_feeds_for_entity(domain::entity_feed::EntityType::Org, org_id)
        .await
        .map_err(map_feed_error)?;

    let response: Vec<FeedResponse> = feeds.iter().map(to_feed_response).collect();
    Ok(Json(response))
}

pub(crate) async fn create_org_feed(
    State(state): State<SharedState>,
    Path(id): Path<String>,
    AuthUser(claims): AuthUser,
    Json(body): Json<CreateFeedRequest>,
) -> Result<(StatusCode, Json<FeedResponse>), (StatusCode, String)> {
    let user_id = parse_user_id(&claims.sub)?;
    let org_id = parse_uuid(&id)?;

    let feed = state
        .feed_service
        .create_custom_feed(
            org_id,
            user_id,
            &body.slug,
            &body.display_name,
            body.description.as_deref(),
        )
        .await
        .map_err(map_feed_error)?;

    Ok((StatusCode::CREATED, Json(to_feed_response(&feed))))
}

// --- Router ------------------------------------------------------------------

/// Build the feed route group (CRUD, items, elements).
pub fn feed_router() -> Router<SharedState> {
    Router::new()
        .route("/{id}", get(get_feed).put(update_feed).delete(delete_feed))
        .route("/{id}/items", post(post_to_feed).get(list_feed_items))
        .route("/{id}/items/{item_id}", axum::routing::delete(delete_feed_item))
}

// --- Response mapping --------------------------------------------------------

pub(super) fn to_feed_response(f: &domain::feed::Feed) -> FeedResponse {
    FeedResponse {
        id: f.id.to_string(),
        slug: f.slug.clone(),
        display_name: f.display_name.clone(),
        description: f.description.clone(),
        feed_type: f.feed_type.as_str().to_string(),
    }
}

fn to_item_response(
    item_with_elements: &application::feed::service::FeedItemWithElements,
) -> FeedItemResponse {
    FeedItemResponse {
        id: item_with_elements.item.id.to_string(),
        feed_id: item_with_elements.item.feed_id.to_string(),
        author_type: item_with_elements.item.author_type.as_str().to_string(),
        author_id: item_with_elements.item.author_id.to_string(),
        created_at: item_with_elements.item.created_at.to_rfc3339(),
        elements: item_with_elements
            .elements
            .iter()
            .map(|e| FeedElementResponse {
                id: e.id.to_string(),
                element_type: e.element_type.as_str().to_string(),
                content_json: e.content_json.clone(),
                position: e.position,
            })
            .collect(),
    }
}

// --- Error mapping -----------------------------------------------------------

fn map_feed_error(e: FeedServiceError) -> (StatusCode, String) {
    match e {
        FeedServiceError::FeedNotFound => (StatusCode::NOT_FOUND, "Feed not found".into()),
        FeedServiceError::ItemNotFound => (StatusCode::NOT_FOUND, "Feed item not found".into()),
        FeedServiceError::SystemFeedUndeletable => (
            StatusCode::FORBIDDEN,
            "System feeds cannot be deleted".into(),
        ),
        FeedServiceError::Forbidden => (StatusCode::FORBIDDEN, "Permission denied".into()),
        FeedServiceError::SlugTaken(s) => {
            (StatusCode::CONFLICT, format!("Feed slug already taken: {s}"))
        }
        FeedServiceError::Internal(inner) => {
            eprintln!("Internal feed service error: {inner}");
            (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error".into())
        }
    }
}

// Shared helpers.
use super::helpers::{parse_user_id, parse_uuid};
