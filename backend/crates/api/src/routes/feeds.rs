use application::feed::service::NewFeedElement;
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{get, post},
};
use domain::feed_element::FeedElementType;
use serde::{Deserialize, Serialize};

use crate::error::AppError;
use crate::middleware::AuthUser;
use crate::state::SharedState;
use super::helpers::{PaginationQuery, parse_user_id, parse_uuid};

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

// --- Feed CRUD routes (mounted at /feeds) ------------------------------------

async fn get_feed(
    State(state): State<SharedState>,
    Path(id): Path<String>,
    AuthUser(_claims): AuthUser,
) -> Result<Json<FeedResponse>, AppError> {
    let feed_id = parse_uuid(&id)?;

    let feed = state
        .feed_service
        .get_feed(feed_id)
        .await?;

    Ok(Json(FeedResponse::from(&feed)))
}

async fn update_feed(
    State(state): State<SharedState>,
    Path(id): Path<String>,
    AuthUser(claims): AuthUser,
    Json(body): Json<UpdateFeedRequest>,
) -> Result<Json<FeedResponse>, AppError> {
    let user_id = parse_user_id(&claims.sub)?;
    let feed_id = parse_uuid(&id)?;

    let feed = state
        .feed_service
        .update_feed(feed_id, user_id, &body.display_name, body.description.as_deref())
        .await?;

    Ok(Json(FeedResponse::from(&feed)))
}

async fn delete_feed(
    State(state): State<SharedState>,
    Path(id): Path<String>,
    AuthUser(claims): AuthUser,
) -> Result<StatusCode, AppError> {
    let user_id = parse_user_id(&claims.sub)?;
    let feed_id = parse_uuid(&id)?;

    state
        .feed_service
        .delete_feed(feed_id, user_id)
        .await?;

    Ok(StatusCode::NO_CONTENT)
}

async fn post_to_feed(
    State(state): State<SharedState>,
    Path(id): Path<String>,
    AuthUser(claims): AuthUser,
    Json(body): Json<PostToFeedRequest>,
) -> Result<(StatusCode, Json<FeedItemResponse>), AppError> {
    let user_id = parse_user_id(&claims.sub)?;
    let feed_id = parse_uuid(&id)?;

    let elements: Vec<NewFeedElement> = body
        .elements
        .into_iter()
        .map(|e| {
            let element_type = FeedElementType::from_str(&e.element_type).ok_or_else(|| {
                AppError::BadRequest(format!("Invalid element type: '{}'", e.element_type))
            })?;
            Ok(NewFeedElement {
                element_type,
                content_json: e.content_json,
                position: e.position,
            })
        })
        .collect::<Result<Vec<_>, AppError>>()?;

    let result = state
        .feed_service
        .post_to_feed(feed_id, user_id, elements)
        .await?;

    Ok((StatusCode::CREATED, Json(FeedItemResponse::from(&result))))
}

async fn list_feed_items(
    State(state): State<SharedState>,
    Path(id): Path<String>,
    AuthUser(_claims): AuthUser,
    Query(pagination): Query<PaginationQuery>,
) -> Result<Json<Vec<FeedItemResponse>>, AppError> {
    let feed_id = parse_uuid(&id)?;

    let limit = pagination.limit.clamp(1, 100);
    let items = state
        .feed_service
        .list_feed_items(feed_id, limit, pagination.offset)
        .await?;

    let response: Vec<FeedItemResponse> = items.iter().map(FeedItemResponse::from).collect();
    Ok(Json(response))
}

async fn delete_feed_item(
    State(state): State<SharedState>,
    Path((_, item_id_str)): Path<(String, String)>,
    AuthUser(claims): AuthUser,
) -> Result<StatusCode, AppError> {
    let user_id = parse_user_id(&claims.sub)?;
    let item_id = parse_uuid(&item_id_str)?;

    state
        .feed_service
        .delete_feed_item(item_id, user_id)
        .await?;

    Ok(StatusCode::NO_CONTENT)
}

// --- Org-scoped feed routes (mounted on organizations router) ----------------

pub(crate) async fn list_org_feeds(
    State(state): State<SharedState>,
    Path(id): Path<String>,
    AuthUser(_claims): AuthUser,
) -> Result<Json<Vec<FeedResponse>>, AppError> {
    let org_id = parse_uuid(&id)?;

    let feeds = state
        .feed_service
        .list_feeds_for_entity(domain::entity_feed::EntityType::Org, org_id)
        .await?;

    let response: Vec<FeedResponse> = feeds.iter().map(FeedResponse::from).collect();
    Ok(Json(response))
}

pub(crate) async fn create_org_feed(
    State(state): State<SharedState>,
    Path(id): Path<String>,
    AuthUser(claims): AuthUser,
    Json(body): Json<CreateFeedRequest>,
) -> Result<(StatusCode, Json<FeedResponse>), AppError> {
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
        .await?;

    Ok((StatusCode::CREATED, Json(FeedResponse::from(&feed))))
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

impl From<&domain::feed::Feed> for FeedResponse {
    fn from(f: &domain::feed::Feed) -> Self {
        Self {
            id: f.id.to_string(),
            slug: f.slug.clone(),
            display_name: f.display_name.clone(),
            description: f.description.clone(),
            feed_type: f.feed_type.as_str().to_string(),
        }
    }
}

impl From<&application::feed::service::FeedItemWithElements> for FeedItemResponse {
    fn from(item_with_elements: &application::feed::service::FeedItemWithElements) -> Self {
        Self {
            id: item_with_elements.item.id.to_string(),
            feed_id: item_with_elements.item.feed_id.to_string(),
            author_type: item_with_elements.item.author_type.as_str().to_string(),
            author_id: item_with_elements.item.author_id.to_string(),
            created_at: item_with_elements.item.created_at.to_rfc3339(),
            elements: item_with_elements
                .elements
                .iter()
                .map(FeedElementResponse::from)
                .collect(),
        }
    }
}

impl From<&domain::feed_element::FeedElement> for FeedElementResponse {
    fn from(e: &domain::feed_element::FeedElement) -> Self {
        Self {
            id: e.id.to_string(),
            element_type: e.element_type.as_str().to_string(),
            content_json: e.content_json.clone(),
            position: e.position,
        }
    }
}

