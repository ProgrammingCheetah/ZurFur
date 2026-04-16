use axum::http::{HeaderName, HeaderValue, StatusCode};
use axum_test::TestServer;
use uuid::Uuid;

use super::test_state::{issue_test_jwt, test_app_state};
use crate::router;

fn test_server() -> TestServer {
    let state = test_app_state();
    let app = router(state);
    TestServer::new(app).unwrap()
}

fn auth_header(token: &str) -> (HeaderName, HeaderValue) {
    (
        HeaderName::from_static("authorization"),
        HeaderValue::from_str(&format!("Bearer {token}")).unwrap(),
    )
}

// --- Auth guard tests --------------------------------------------------------

#[tokio::test]
async fn create_tag_without_token_returns_401() {
    let server = test_server();
    let response = server
        .post("/tags")
        .json(&serde_json::json!({"category": "metadata", "name": "canine"}))
        .await;
    response.assert_status_unauthorized();
}

#[tokio::test]
async fn get_tag_without_token_returns_401() {
    let server = test_server();
    let response = server.get(&format!("/tags/{}", Uuid::new_v4())).await;
    response.assert_status_unauthorized();
}

// --- POST /tags --------------------------------------------------------------

#[tokio::test]
async fn create_metadata_tag_returns_created() {
    let server = test_server();
    let user_id = Uuid::new_v4();
    let token = issue_test_jwt(&user_id, "did:plc:test", Some("test.bsky.social"));
    let (name, value) = auth_header(&token);

    let response = server
        .post("/tags")
        .json(&serde_json::json!({"category": "metadata", "name": "canine"}))
        .add_header(name, value)
        .await;
    response.assert_status(StatusCode::CREATED);

    let body: serde_json::Value = response.json();
    assert_eq!(body["name"], "canine");
    assert_eq!(body["category"], "metadata");
    assert_eq!(body["is_approved"], false);
}

#[tokio::test]
async fn create_tag_with_immutable_category_returns_400() {
    let server = test_server();
    let user_id = Uuid::new_v4();
    let token = issue_test_jwt(&user_id, "did:plc:test", Some("test.bsky.social"));
    let (name, value) = auth_header(&token);

    let response = server
        .post("/tags")
        .json(&serde_json::json!({"category": "organization", "name": "test"}))
        .add_header(name, value)
        .await;
    response.assert_status(StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn create_tag_with_invalid_category_returns_400() {
    let server = test_server();
    let user_id = Uuid::new_v4();
    let token = issue_test_jwt(&user_id, "did:plc:test", Some("test.bsky.social"));
    let (name, value) = auth_header(&token);

    let response = server
        .post("/tags")
        .json(&serde_json::json!({"category": "invalid", "name": "test"}))
        .add_header(name, value)
        .await;
    response.assert_status(StatusCode::BAD_REQUEST);
}

// --- GET /tags/search --------------------------------------------------------

#[tokio::test]
async fn search_tags_returns_empty_for_no_matches() {
    let server = test_server();
    let user_id = Uuid::new_v4();
    let token = issue_test_jwt(&user_id, "did:plc:test", Some("test.bsky.social"));
    let (name, value) = auth_header(&token);

    let response = server
        .get("/tags/search?q=nonexistent")
        .add_header(name, value)
        .await;
    response.assert_status_ok();

    let body: Vec<serde_json::Value> = response.json();
    assert!(body.is_empty());
}

// --- POST /tags/attach + POST /tags/detach -----------------------------------

#[tokio::test]
async fn attach_tag_to_entity_returns_created() {
    let server = test_server();
    let user_id = Uuid::new_v4();
    let token = issue_test_jwt(&user_id, "did:plc:test", Some("test.bsky.social"));
    let (name, value) = auth_header(&token);

    // Create a tag first
    let create_resp = server
        .post("/tags")
        .json(&serde_json::json!({"category": "metadata", "name": "wolf"}))
        .add_header(name.clone(), value.clone())
        .await;
    let tag: serde_json::Value = create_resp.json();
    let tag_id = tag["id"].as_str().unwrap();

    let org_id = Uuid::new_v4();
    let response = server
        .post("/tags/attach")
        .json(&serde_json::json!({
            "entity_type": "org",
            "entity_id": org_id.to_string(),
            "tag_id": tag_id,
        }))
        .add_header(name, value)
        .await;
    response.assert_status(StatusCode::CREATED);
}

#[tokio::test]
async fn attach_with_invalid_entity_type_returns_400() {
    let server = test_server();
    let user_id = Uuid::new_v4();
    let token = issue_test_jwt(&user_id, "did:plc:test", Some("test.bsky.social"));
    let (name, value) = auth_header(&token);

    let response = server
        .post("/tags/attach")
        .json(&serde_json::json!({
            "entity_type": "invalid",
            "entity_id": Uuid::new_v4().to_string(),
            "tag_id": Uuid::new_v4().to_string(),
        }))
        .add_header(name, value)
        .await;
    response.assert_status(StatusCode::BAD_REQUEST);
}

// --- Tag attachment with various entity types --------------------------------

#[tokio::test]
async fn attach_tag_to_user_entity() {
    let server = test_server();
    let user_id = Uuid::new_v4();
    let token = issue_test_jwt(&user_id, "did:plc:test", Some("test.bsky.social"));
    let (name, value) = auth_header(&token);

    let tag_resp = server
        .post("/tags")
        .json(&serde_json::json!({"category": "metadata", "name": "user-tag"}))
        .add_header(name.clone(), value.clone())
        .await;
    let tag: serde_json::Value = tag_resp.json();
    let tag_id = tag["id"].as_str().unwrap();

    let response = server
        .post("/tags/attach")
        .json(&serde_json::json!({
            "entity_type": "user",
            "entity_id": Uuid::new_v4().to_string(),
            "tag_id": tag_id,
        }))
        .add_header(name, value)
        .await;
    response.assert_status(StatusCode::CREATED);
}

#[tokio::test]
async fn attach_tag_to_feed_entity() {
    let server = test_server();
    let user_id = Uuid::new_v4();
    let token = issue_test_jwt(&user_id, "did:plc:test", Some("test.bsky.social"));
    let (name, value) = auth_header(&token);

    let tag_resp = server
        .post("/tags")
        .json(&serde_json::json!({"category": "general", "name": "feed-tag"}))
        .add_header(name.clone(), value.clone())
        .await;
    let tag: serde_json::Value = tag_resp.json();
    let tag_id = tag["id"].as_str().unwrap();

    let response = server
        .post("/tags/attach")
        .json(&serde_json::json!({
            "entity_type": "feed",
            "entity_id": Uuid::new_v4().to_string(),
            "tag_id": tag_id,
        }))
        .add_header(name, value)
        .await;
    response.assert_status(StatusCode::CREATED);
}

#[tokio::test]
async fn attach_tag_to_tag_entity() {
    let server = test_server();
    let user_id = Uuid::new_v4();
    let token = issue_test_jwt(&user_id, "did:plc:test", Some("test.bsky.social"));
    let (name, value) = auth_header(&token);

    let tag_resp = server
        .post("/tags")
        .json(&serde_json::json!({"category": "metadata", "name": "meta-tag"}))
        .add_header(name.clone(), value.clone())
        .await;
    let tag: serde_json::Value = tag_resp.json();
    let tag_id = tag["id"].as_str().unwrap();

    let response = server
        .post("/tags/attach")
        .json(&serde_json::json!({
            "entity_type": "tag",
            "entity_id": Uuid::new_v4().to_string(),
            "tag_id": tag_id,
        }))
        .add_header(name, value)
        .await;
    response.assert_status(StatusCode::CREATED);
}

// --- GET /tags/entity/:type/:id ---------------------------------------------

#[tokio::test]
async fn list_tags_for_entity_returns_tags() {
    let server = test_server();
    let user_id = Uuid::new_v4();
    let token = issue_test_jwt(&user_id, "did:plc:test", Some("test.bsky.social"));
    let (name, value) = auth_header(&token);

    // Create and attach a tag
    let tag_resp = server
        .post("/tags")
        .json(&serde_json::json!({"category": "general", "name": "entity-list-tag"}))
        .add_header(name.clone(), value.clone())
        .await;
    let tag: serde_json::Value = tag_resp.json();
    let tag_id = tag["id"].as_str().unwrap();

    let entity_id = Uuid::new_v4();
    server
        .post("/tags/attach")
        .json(&serde_json::json!({
            "entity_type": "org",
            "entity_id": entity_id.to_string(),
            "tag_id": tag_id,
        }))
        .add_header(name.clone(), value.clone())
        .await;

    // List tags for entity
    let response = server
        .get(&format!("/tags/entity/org/{entity_id}"))
        .add_header(name, value)
        .await;
    response.assert_status_ok();

    let body: Vec<serde_json::Value> = response.json();
    assert_eq!(body.len(), 1);
    assert_eq!(body[0]["name"], "entity-list-tag");
}
