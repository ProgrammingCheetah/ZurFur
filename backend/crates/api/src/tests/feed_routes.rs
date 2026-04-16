use axum::http::StatusCode;
use axum_test::TestServer;
use uuid::Uuid;

use super::test_state::{auth_header, issue_test_jwt, test_server, test_server_with_user};

async fn create_org_and_feed(
    server: &TestServer,
    auth: &(axum::http::HeaderName, axum::http::HeaderValue),
    org_slug: &str,
    feed_slug: &str,
) -> (String, String) {
    let org_resp = server
        .post("/organizations")
        .json(&serde_json::json!({"slug": org_slug, "display_name": org_slug}))
        .add_header(auth.0.clone(), auth.1.clone())
        .await;
    org_resp.assert_status(StatusCode::CREATED);
    let org_body: serde_json::Value = org_resp.json();
    let org_id = org_body["org"]["id"].as_str().unwrap().to_string();

    let feed_resp = server
        .post(&format!("/organizations/{org_id}/feeds"))
        .json(&serde_json::json!({"slug": feed_slug, "display_name": feed_slug}))
        .add_header(auth.0.clone(), auth.1.clone())
        .await;
    feed_resp.assert_status(StatusCode::CREATED);
    let feed_body: serde_json::Value = feed_resp.json();
    let feed_id = feed_body["id"].as_str().unwrap().to_string();

    (org_id, feed_id)
}

// --- Auth guard tests --------------------------------------------------------

#[tokio::test]
async fn create_feed_without_token_returns_401() {
    let server = test_server();
    let org_id = Uuid::new_v4();
    let response = server
        .post(&format!("/organizations/{org_id}/feeds"))
        .json(&serde_json::json!({"slug": "gallery", "display_name": "Gallery"}))
        .await;
    response.assert_status_unauthorized();
}

// --- POST /organizations/:id/feeds ------------------------------------------

#[tokio::test]
async fn create_custom_feed_returns_created() {
    let (server, user_id) = test_server_with_user();
    let token = issue_test_jwt(&user_id, "did:plc:testuser", Some("testuser.bsky.social"));
    let auth = auth_header(&token);

    let (_, feed_id) = create_org_and_feed(&server, &auth, "feed-org", "gallery").await;

    let feed = server
        .get(&format!("/feeds/{feed_id}"))
        .add_header(auth.0, auth.1)
        .await;
    feed.assert_status_ok();
    let body: serde_json::Value = feed.json();
    assert_eq!(body["slug"], "gallery");
    assert_eq!(body["feed_type"], "custom");
}

#[tokio::test]
async fn create_feed_without_permission_returns_403() {
    let (server, user_id) = test_server_with_user();
    let token = issue_test_jwt(&user_id, "did:plc:testuser", Some("testuser.bsky.social"));
    let auth = auth_header(&token);

    let other_user_id = Uuid::new_v4();
    let other_token = issue_test_jwt(&other_user_id, "did:plc:other", Some("other.bsky.social"));
    let other_auth = auth_header(&other_token);

    let org_resp = server
        .post("/organizations")
        .json(&serde_json::json!({"slug": "perm-org", "display_name": "Perm Org"}))
        .add_header(auth.0, auth.1)
        .await;
    let org_body: serde_json::Value = org_resp.json();
    let org_id = org_body["org"]["id"].as_str().unwrap();

    let response = server
        .post(&format!("/organizations/{org_id}/feeds"))
        .json(&serde_json::json!({"slug": "gallery", "display_name": "Gallery"}))
        .add_header(other_auth.0, other_auth.1)
        .await;
    response.assert_status(StatusCode::FORBIDDEN);
}

// --- GET /feeds/:id ---------------------------------------------------------

#[tokio::test]
async fn get_feed_returns_feed() {
    let (server, user_id) = test_server_with_user();
    let token = issue_test_jwt(&user_id, "did:plc:testuser", Some("testuser.bsky.social"));
    let auth = auth_header(&token);

    let (_, feed_id) = create_org_and_feed(&server, &auth, "get-feed-org", "my-feed").await;

    let response = server
        .get(&format!("/feeds/{feed_id}"))
        .add_header(auth.0, auth.1)
        .await;
    response.assert_status_ok();

    let body: serde_json::Value = response.json();
    assert_eq!(body["slug"], "my-feed");
}

#[tokio::test]
async fn get_nonexistent_feed_returns_404() {
    let server = test_server();
    let user_id = Uuid::new_v4();
    let token = issue_test_jwt(&user_id, "did:plc:test", Some("test.bsky.social"));
    let auth = auth_header(&token);

    let response = server
        .get(&format!("/feeds/{}", Uuid::new_v4()))
        .add_header(auth.0, auth.1)
        .await;
    response.assert_status(StatusCode::NOT_FOUND);
}

// --- PUT /feeds/:id ---------------------------------------------------------

#[tokio::test]
async fn update_feed_returns_updated() {
    let (server, user_id) = test_server_with_user();
    let token = issue_test_jwt(&user_id, "did:plc:testuser", Some("testuser.bsky.social"));
    let auth = auth_header(&token);

    let (_, feed_id) = create_org_and_feed(&server, &auth, "upd-feed-org", "updatable").await;

    let response = server
        .put(&format!("/feeds/{feed_id}"))
        .json(&serde_json::json!({"display_name": "New Name", "description": "new desc"}))
        .add_header(auth.0, auth.1)
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
    let auth = auth_header(&token);

    let (_, feed_id) = create_org_and_feed(&server, &auth, "del-feed-org", "deletable").await;

    let response = server
        .delete(&format!("/feeds/{feed_id}"))
        .add_header(auth.0, auth.1)
        .await;
    response.assert_status(StatusCode::NO_CONTENT);
}

// --- POST /feeds/:id/items + GET /feeds/:id/items ---------------------------

#[tokio::test]
async fn post_to_feed_returns_created() {
    let (server, user_id) = test_server_with_user();
    let token = issue_test_jwt(&user_id, "did:plc:testuser", Some("testuser.bsky.social"));
    let auth = auth_header(&token);

    let (_, feed_id) = create_org_and_feed(&server, &auth, "post-feed-org", "postable").await;

    let response = server
        .post(&format!("/feeds/{feed_id}/items"))
        .json(&serde_json::json!({
            "elements": [
                {"element_type": "text", "content_json": "{\"text\":\"hello\"}", "position": 0}
            ]
        }))
        .add_header(auth.0, auth.1)
        .await;
    response.assert_status(StatusCode::CREATED);

    let body: serde_json::Value = response.json();
    assert_eq!(body["elements"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn list_feed_items_returns_paginated() {
    let (server, user_id) = test_server_with_user();
    let token = issue_test_jwt(&user_id, "did:plc:testuser", Some("testuser.bsky.social"));
    let auth = auth_header(&token);

    let (_, feed_id) = create_org_and_feed(&server, &auth, "list-items-org", "listable").await;

    for i in 0..3 {
        server
            .post(&format!("/feeds/{feed_id}/items"))
            .json(&serde_json::json!({
                "elements": [
                    {"element_type": "text", "content_json": format!("{{\"text\":\"item {i}\"}}"), "position": 0}
                ]
            }))
            .add_header(auth.0.clone(), auth.1.clone())
            .await;
    }

    let response = server
        .get(&format!("/feeds/{feed_id}/items?limit=2&offset=0"))
        .add_header(auth.0, auth.1)
        .await;
    response.assert_status_ok();

    let body: Vec<serde_json::Value> = response.json();
    assert_eq!(body.len(), 2);
}

#[tokio::test]
async fn delete_feed_item_returns_204() {
    let (server, user_id) = test_server_with_user();
    let token = issue_test_jwt(&user_id, "did:plc:testuser", Some("testuser.bsky.social"));
    let auth = auth_header(&token);

    let (_, feed_id) = create_org_and_feed(&server, &auth, "del-item-org", "items-feed").await;

    let item_resp = server
        .post(&format!("/feeds/{feed_id}/items"))
        .json(&serde_json::json!({
            "elements": [
                {"element_type": "text", "content_json": "{\"text\":\"bye\"}", "position": 0}
            ]
        }))
        .add_header(auth.0.clone(), auth.1.clone())
        .await;
    let item_body: serde_json::Value = item_resp.json();
    let item_id = item_body["id"].as_str().unwrap();

    let response = server
        .delete(&format!("/feeds/{feed_id}/items/{item_id}"))
        .add_header(auth.0, auth.1)
        .await;
    response.assert_status(StatusCode::NO_CONTENT);
}
