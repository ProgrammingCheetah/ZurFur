use axum::http::{HeaderName, HeaderValue, StatusCode};
use axum_test::TestServer;
use uuid::Uuid;

use super::test_state::{issue_test_jwt, test_app_state, test_app_state_with_user};
use crate::router;

fn test_server() -> TestServer {
    let state = test_app_state();
    let app = router(state);
    TestServer::new(app).unwrap()
}

fn test_server_with_user() -> (TestServer, Uuid) {
    let (state, user_id) = test_app_state_with_user();
    let app = router(state);
    (TestServer::new(app).unwrap(), user_id)
}

fn auth_header(token: &str) -> (HeaderName, HeaderValue) {
    (
        HeaderName::from_static("authorization"),
        HeaderValue::from_str(&format!("Bearer {token}")).unwrap(),
    )
}

// --- Auth guard tests --------------------------------------------------------

#[tokio::test]
async fn create_feed_without_token_returns_401() {
    let server = test_server();
    let org_id = Uuid::new_v4();
    let response = server
        .post(&format!("/organizations/{org_id}/feeds"))
        .json(&serde_json::json!({
            "slug": "gallery",
            "display_name": "Gallery"
        }))
        .await;
    response.assert_status_unauthorized();
}

// --- POST /organizations/:id/feeds ------------------------------------------

#[tokio::test]
async fn create_custom_feed_returns_created() {
    let (server, user_id) = test_server_with_user();
    let token = issue_test_jwt(&user_id, "did:plc:testuser", Some("testuser.bsky.social"));
    let (name, value) = auth_header(&token);

    // First create an org so we have an org_id with membership
    let org_resp = server
        .post("/organizations")
        .json(&serde_json::json!({
            "slug": "feed-org",
            "display_name": "Feed Org"
        }))
        .add_header(name.clone(), value.clone())
        .await;
    org_resp.assert_status(StatusCode::CREATED);
    let org_body: serde_json::Value = org_resp.json();
    let org_id = org_body["org"]["id"].as_str().unwrap();

    // Create a feed on that org
    let response = server
        .post(&format!("/organizations/{org_id}/feeds"))
        .json(&serde_json::json!({
            "slug": "gallery",
            "display_name": "Gallery"
        }))
        .add_header(name, value)
        .await;
    response.assert_status(StatusCode::CREATED);

    let body: serde_json::Value = response.json();
    assert_eq!(body["slug"], "gallery");
    assert_eq!(body["display_name"], "Gallery");
    assert_eq!(body["feed_type"], "custom");
}

#[tokio::test]
async fn create_feed_without_permission_returns_403() {
    let (server, user_id) = test_server_with_user();
    let token = issue_test_jwt(&user_id, "did:plc:testuser", Some("testuser.bsky.social"));
    let (name, value) = auth_header(&token);

    // Use the pre-created personal org — create a second user with no membership
    let other_user_id = Uuid::new_v4();
    let other_token = issue_test_jwt(&other_user_id, "did:plc:other", Some("other.bsky.social"));
    let (other_name, other_value) = auth_header(&other_token);

    // Create org as the first user
    let org_resp = server
        .post("/organizations")
        .json(&serde_json::json!({
            "slug": "perm-org",
            "display_name": "Perm Org"
        }))
        .add_header(name, value)
        .await;
    let org_body: serde_json::Value = org_resp.json();
    let org_id = org_body["org"]["id"].as_str().unwrap();

    // Try to create feed as non-member
    let response = server
        .post(&format!("/organizations/{org_id}/feeds"))
        .json(&serde_json::json!({
            "slug": "gallery",
            "display_name": "Gallery"
        }))
        .add_header(other_name, other_value)
        .await;
    response.assert_status(StatusCode::FORBIDDEN);
}

// --- GET /feeds/:id ---------------------------------------------------------

#[tokio::test]
async fn get_feed_returns_feed() {
    let (server, user_id) = test_server_with_user();
    let token = issue_test_jwt(&user_id, "did:plc:testuser", Some("testuser.bsky.social"));
    let (name, value) = auth_header(&token);

    // Create org + feed
    let org_resp = server
        .post("/organizations")
        .json(&serde_json::json!({"slug": "get-feed-org", "display_name": "Get Feed Org"}))
        .add_header(name.clone(), value.clone())
        .await;
    let org_body: serde_json::Value = org_resp.json();
    let org_id = org_body["org"]["id"].as_str().unwrap();

    let feed_resp = server
        .post(&format!("/organizations/{org_id}/feeds"))
        .json(&serde_json::json!({"slug": "my-feed", "display_name": "My Feed"}))
        .add_header(name.clone(), value.clone())
        .await;
    let feed_body: serde_json::Value = feed_resp.json();
    let feed_id = feed_body["id"].as_str().unwrap();

    // Fetch it
    let response = server
        .get(&format!("/feeds/{feed_id}"))
        .add_header(name, value)
        .await;
    response.assert_status_ok();

    let body: serde_json::Value = response.json();
    assert_eq!(body["slug"], "my-feed");
    assert_eq!(body["display_name"], "My Feed");
}

#[tokio::test]
async fn get_nonexistent_feed_returns_404() {
    let server = test_server();
    let user_id = Uuid::new_v4();
    let token = issue_test_jwt(&user_id, "did:plc:test", Some("test.bsky.social"));
    let (name, value) = auth_header(&token);

    let response = server
        .get(&format!("/feeds/{}", Uuid::new_v4()))
        .add_header(name, value)
        .await;
    response.assert_status(StatusCode::NOT_FOUND);
}

// --- PUT /feeds/:id ---------------------------------------------------------

#[tokio::test]
async fn update_feed_returns_updated() {
    let (server, user_id) = test_server_with_user();
    let token = issue_test_jwt(&user_id, "did:plc:testuser", Some("testuser.bsky.social"));
    let (name, value) = auth_header(&token);

    // Create org + feed
    let org_resp = server
        .post("/organizations")
        .json(&serde_json::json!({"slug": "upd-feed-org", "display_name": "Upd Feed Org"}))
        .add_header(name.clone(), value.clone())
        .await;
    let org_body: serde_json::Value = org_resp.json();
    let org_id = org_body["org"]["id"].as_str().unwrap();

    let feed_resp = server
        .post(&format!("/organizations/{org_id}/feeds"))
        .json(&serde_json::json!({"slug": "updatable", "display_name": "Old Name"}))
        .add_header(name.clone(), value.clone())
        .await;
    let feed_body: serde_json::Value = feed_resp.json();
    let feed_id = feed_body["id"].as_str().unwrap();

    // Update
    let response = server
        .put(&format!("/feeds/{feed_id}"))
        .json(&serde_json::json!({"display_name": "New Name", "description": "new desc"}))
        .add_header(name, value)
        .await;
    response.assert_status_ok();

    let body: serde_json::Value = response.json();
    assert_eq!(body["display_name"], "New Name");
}

// --- DELETE /feeds/:id ------------------------------------------------------

#[tokio::test]
async fn delete_custom_feed_returns_204() {
    let (server, user_id) = test_server_with_user();
    let token = issue_test_jwt(&user_id, "did:plc:testuser", Some("testuser.bsky.social"));
    let (name, value) = auth_header(&token);

    // Create org + feed
    let org_resp = server
        .post("/organizations")
        .json(&serde_json::json!({"slug": "del-feed-org", "display_name": "Del Feed Org"}))
        .add_header(name.clone(), value.clone())
        .await;
    let org_body: serde_json::Value = org_resp.json();
    let org_id = org_body["org"]["id"].as_str().unwrap();

    let feed_resp = server
        .post(&format!("/organizations/{org_id}/feeds"))
        .json(&serde_json::json!({"slug": "deletable", "display_name": "Delete Me"}))
        .add_header(name.clone(), value.clone())
        .await;
    let feed_body: serde_json::Value = feed_resp.json();
    let feed_id = feed_body["id"].as_str().unwrap();

    // Delete
    let response = server
        .delete(&format!("/feeds/{feed_id}"))
        .add_header(name, value)
        .await;
    response.assert_status(StatusCode::NO_CONTENT);
}

// --- POST /feeds/:id/items + GET /feeds/:id/items ---------------------------

#[tokio::test]
async fn post_to_feed_returns_created() {
    let (server, user_id) = test_server_with_user();
    let token = issue_test_jwt(&user_id, "did:plc:testuser", Some("testuser.bsky.social"));
    let (name, value) = auth_header(&token);

    // Create org + feed
    let org_resp = server
        .post("/organizations")
        .json(&serde_json::json!({"slug": "post-feed-org", "display_name": "Post Feed Org"}))
        .add_header(name.clone(), value.clone())
        .await;
    let org_body: serde_json::Value = org_resp.json();
    let org_id = org_body["org"]["id"].as_str().unwrap();

    let feed_resp = server
        .post(&format!("/organizations/{org_id}/feeds"))
        .json(&serde_json::json!({"slug": "postable", "display_name": "Postable Feed"}))
        .add_header(name.clone(), value.clone())
        .await;
    let feed_body: serde_json::Value = feed_resp.json();
    let feed_id = feed_body["id"].as_str().unwrap();

    // Post to feed
    let response = server
        .post(&format!("/feeds/{feed_id}/items"))
        .json(&serde_json::json!({
            "elements": [
                {"element_type": "text", "content_json": "{\"text\":\"hello\"}", "position": 0}
            ]
        }))
        .add_header(name, value)
        .await;
    response.assert_status(StatusCode::CREATED);

    let body: serde_json::Value = response.json();
    assert_eq!(body["elements"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn list_feed_items_returns_paginated() {
    let (server, user_id) = test_server_with_user();
    let token = issue_test_jwt(&user_id, "did:plc:testuser", Some("testuser.bsky.social"));
    let (name, value) = auth_header(&token);

    // Create org + feed
    let org_resp = server
        .post("/organizations")
        .json(&serde_json::json!({"slug": "list-items-org", "display_name": "List Items Org"}))
        .add_header(name.clone(), value.clone())
        .await;
    let org_body: serde_json::Value = org_resp.json();
    let org_id = org_body["org"]["id"].as_str().unwrap();

    let feed_resp = server
        .post(&format!("/organizations/{org_id}/feeds"))
        .json(&serde_json::json!({"slug": "listable", "display_name": "Listable Feed"}))
        .add_header(name.clone(), value.clone())
        .await;
    let feed_body: serde_json::Value = feed_resp.json();
    let feed_id = feed_body["id"].as_str().unwrap();

    // Post 3 items
    for i in 0..3 {
        server
            .post(&format!("/feeds/{feed_id}/items"))
            .json(&serde_json::json!({
                "elements": [
                    {"element_type": "text", "content_json": format!("{{\"text\":\"item {i}\"}}"), "position": 0}
                ]
            }))
            .add_header(name.clone(), value.clone())
            .await;
    }

    // List with pagination
    let response = server
        .get(&format!("/feeds/{feed_id}/items?limit=2&offset=0"))
        .add_header(name, value)
        .await;
    response.assert_status_ok();

    let body: Vec<serde_json::Value> = response.json();
    assert_eq!(body.len(), 2);
}

#[tokio::test]
async fn delete_feed_item_returns_204() {
    let (server, user_id) = test_server_with_user();
    let token = issue_test_jwt(&user_id, "did:plc:testuser", Some("testuser.bsky.social"));
    let (name, value) = auth_header(&token);

    // Create org + feed + item
    let org_resp = server
        .post("/organizations")
        .json(&serde_json::json!({"slug": "del-item-org", "display_name": "Del Item Org"}))
        .add_header(name.clone(), value.clone())
        .await;
    let org_body: serde_json::Value = org_resp.json();
    let org_id = org_body["org"]["id"].as_str().unwrap();

    let feed_resp = server
        .post(&format!("/organizations/{org_id}/feeds"))
        .json(&serde_json::json!({"slug": "items-feed", "display_name": "Items Feed"}))
        .add_header(name.clone(), value.clone())
        .await;
    let feed_body: serde_json::Value = feed_resp.json();
    let feed_id = feed_body["id"].as_str().unwrap();

    let item_resp = server
        .post(&format!("/feeds/{feed_id}/items"))
        .json(&serde_json::json!({
            "elements": [
                {"element_type": "text", "content_json": "{\"text\":\"bye\"}", "position": 0}
            ]
        }))
        .add_header(name.clone(), value.clone())
        .await;
    let item_body: serde_json::Value = item_resp.json();
    let item_id = item_body["id"].as_str().unwrap();

    // Delete the item
    let response = server
        .delete(&format!("/feeds/{feed_id}/items/{item_id}"))
        .add_header(name, value)
        .await;
    response.assert_status(StatusCode::NO_CONTENT);
}
