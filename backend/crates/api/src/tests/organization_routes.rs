use axum::http::StatusCode;
use axum_test::TestServer;
use uuid::Uuid;

use super::test_state::{auth_header, issue_test_jwt, test_app_state_with_user, test_server};

// --- Auth guard tests --------------------------------------------------------

#[tokio::test]
async fn create_org_without_token_returns_401() {
    let server = test_server();
    let response = server
        .post("/organizations")
        .json(&serde_json::json!({"slug": "test-org", "display_name": "Test Org"}))
        .await;
    response.assert_status_unauthorized();
}

// --- POST /organizations -----------------------------------------------------

#[tokio::test]
async fn create_org_returns_created_with_owner() {
    let server = test_server();
    let user_id = Uuid::new_v4();
    let token = issue_test_jwt(&user_id, "did:plc:test", Some("test.bsky.social"));
    let (name, value) = auth_header(&token);

    let response = server
        .post("/organizations")
        .json(&serde_json::json!({
            "slug": "my-studio",
            "display_name": "My Studio"
        }))
        .add_header(name, value)
        .await;
    response.assert_status(StatusCode::CREATED);

    let body: serde_json::Value = response.json();
    assert_eq!(body["org"]["slug"], "my-studio");
    assert_eq!(body["org"]["display_name"], "My Studio");
    assert_eq!(body["org"]["is_personal"], false);
    assert_eq!(body["members"][0]["role"], "owner");
    assert_eq!(body["members"][0]["user_id"], user_id.to_string());
}

#[tokio::test]
async fn create_org_with_invalid_slug_returns_400() {
    let server = test_server();
    let user_id = Uuid::new_v4();
    let token = issue_test_jwt(&user_id, "did:plc:test", Some("test.bsky.social"));
    let (name, value) = auth_header(&token);

    let response = server
        .post("/organizations")
        .json(&serde_json::json!({
            "slug": "admin",
            "display_name": "Admin"
        }))
        .add_header(name, value)
        .await;
    response.assert_status(StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn create_org_with_short_slug_returns_400() {
    let server = test_server();
    let user_id = Uuid::new_v4();
    let token = issue_test_jwt(&user_id, "did:plc:test", Some("test.bsky.social"));
    let (name, value) = auth_header(&token);

    let response = server
        .post("/organizations")
        .json(&serde_json::json!({
            "slug": "a",
            "display_name": "A"
        }))
        .add_header(name, value)
        .await;
    response.assert_status(StatusCode::BAD_REQUEST);
}

// --- GET /organizations/:slug ------------------------------------------------

#[tokio::test]
async fn get_org_by_slug() {
    let server = test_server();
    let user_id = Uuid::new_v4();
    let token = issue_test_jwt(&user_id, "did:plc:test", Some("test.bsky.social"));
    let (name, value) = auth_header(&token);

    // Create first
    server
        .post("/organizations")
        .json(&serde_json::json!({
            "slug": "cool-org",
            "display_name": "Cool Org"
        }))
        .add_header(name.clone(), value.clone())
        .await;

    // Fetch by slug
    let response = server
        .get("/organizations/cool-org")
        .add_header(name, value)
        .await;
    response.assert_status_ok();

    let body: serde_json::Value = response.json();
    assert_eq!(body["org"]["slug"], "cool-org");
    assert_eq!(body["members"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn get_nonexistent_org_returns_404() {
    let server = test_server();
    let user_id = Uuid::new_v4();
    let token = issue_test_jwt(&user_id, "did:plc:test", Some("test.bsky.social"));
    let (name, value) = auth_header(&token);

    let response = server
        .get("/organizations/does-not-exist")
        .add_header(name, value)
        .await;
    response.assert_status(StatusCode::NOT_FOUND);
}

// --- PUT /organizations/:slug ------------------------------------------------

#[tokio::test]
async fn update_org_returns_updated() {
    let server = test_server();
    let user_id = Uuid::new_v4();
    let token = issue_test_jwt(&user_id, "did:plc:test", Some("test.bsky.social"));
    let (name, value) = auth_header(&token);

    // Create org
    server
        .post("/organizations")
        .json(&serde_json::json!({"slug": "updatable-org", "display_name": "Old Name"}))
        .add_header(name.clone(), value.clone())
        .await;

    // Update display name
    let response = server
        .put("/organizations/updatable-org")
        .json(&serde_json::json!({"display_name": "New Name"}))
        .add_header(name, value)
        .await;
    response.assert_status_ok();

    let body: serde_json::Value = response.json();
    assert_eq!(body["display_name"], "New Name");
}

// --- DELETE /organizations/:slug ---------------------------------------------

#[tokio::test]
async fn delete_org_returns_204() {
    let server = test_server();
    let user_id = Uuid::new_v4();
    let token = issue_test_jwt(&user_id, "did:plc:test", Some("test.bsky.social"));
    let (name, value) = auth_header(&token);

    server
        .post("/organizations")
        .json(&serde_json::json!({"slug": "delete-org", "display_name": "Delete Me"}))
        .add_header(name.clone(), value.clone())
        .await;

    let response = server
        .delete("/organizations/delete-org")
        .add_header(name, value)
        .await;
    response.assert_status(StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn delete_personal_org_returns_403() {
    let (state, user_id) = test_app_state_with_user();
    let app = crate::router(state);
    let server = TestServer::new(app).unwrap();
    let token = issue_test_jwt(&user_id, "did:plc:testuser", Some("testuser.bsky.social"));
    let (name, value) = auth_header(&token);

    // The pre-created personal org has slug "testuser"
    let response = server
        .delete("/organizations/testuser")
        .add_header(name, value)
        .await;
    response.assert_status(StatusCode::FORBIDDEN);
}

// --- GET /organizations/:id/members ------------------------------------------

#[tokio::test]
async fn list_members_returns_members() {
    let server = test_server();
    let user_id = Uuid::new_v4();
    let token = issue_test_jwt(&user_id, "did:plc:test", Some("test.bsky.social"));
    let (name, value) = auth_header(&token);

    let org_resp = server
        .post("/organizations")
        .json(&serde_json::json!({"slug": "members-org", "display_name": "Members Org"}))
        .add_header(name.clone(), value.clone())
        .await;
    let org_body: serde_json::Value = org_resp.json();
    let org_id = org_body["org"]["id"].as_str().unwrap();

    let response = server
        .get(&format!("/organizations/{org_id}/members"))
        .add_header(name, value)
        .await;
    response.assert_status_ok();

    let body: Vec<serde_json::Value> = response.json();
    assert_eq!(body.len(), 1);
    assert_eq!(body[0]["role"], "owner");
}

// --- POST /organizations/:id/members ----------------------------------------

#[tokio::test]
async fn add_member_returns_created() {
    let server = test_server();
    let user_id = Uuid::new_v4();
    let token = issue_test_jwt(&user_id, "did:plc:test", Some("test.bsky.social"));
    let (name, value) = auth_header(&token);

    let org_resp = server
        .post("/organizations")
        .json(&serde_json::json!({"slug": "add-member-org", "display_name": "Add Member Org"}))
        .add_header(name.clone(), value.clone())
        .await;
    let org_body: serde_json::Value = org_resp.json();
    let org_id = org_body["org"]["id"].as_str().unwrap();

    let new_user_id = Uuid::new_v4();
    let response = server
        .post(&format!("/organizations/{org_id}/members"))
        .json(&serde_json::json!({
            "user_id": new_user_id.to_string(),
            "role": "member",
            "title": "Artist"
        }))
        .add_header(name, value)
        .await;
    response.assert_status(StatusCode::CREATED);

    let body: serde_json::Value = response.json();
    assert_eq!(body["role"], "member");
    assert_eq!(body["title"], "Artist");
}

// --- PUT /organizations/:id/members/:user_id ---------------------------------

#[tokio::test]
async fn update_member_role() {
    let server = test_server();
    let user_id = Uuid::new_v4();
    let token = issue_test_jwt(&user_id, "did:plc:test", Some("test.bsky.social"));
    let (name, value) = auth_header(&token);

    let org_resp = server
        .post("/organizations")
        .json(&serde_json::json!({"slug": "upd-member-org", "display_name": "Upd Member Org"}))
        .add_header(name.clone(), value.clone())
        .await;
    let org_body: serde_json::Value = org_resp.json();
    let org_id = org_body["org"]["id"].as_str().unwrap();

    let target_id = Uuid::new_v4();
    server
        .post(&format!("/organizations/{org_id}/members"))
        .json(&serde_json::json!({"user_id": target_id.to_string(), "role": "member"}))
        .add_header(name.clone(), value.clone())
        .await;

    let response = server
        .put(&format!("/organizations/{org_id}/members/{target_id}"))
        .json(&serde_json::json!({"role": "admin", "title": "Lead Artist"}))
        .add_header(name, value)
        .await;
    response.assert_status_ok();

    let body: serde_json::Value = response.json();
    assert_eq!(body["role"], "admin");
    assert_eq!(body["title"], "Lead Artist");
}

// --- DELETE /organizations/:id/members/:user_id ------------------------------

#[tokio::test]
async fn remove_member_returns_204() {
    let server = test_server();
    let user_id = Uuid::new_v4();
    let token = issue_test_jwt(&user_id, "did:plc:test", Some("test.bsky.social"));
    let (name, value) = auth_header(&token);

    let org_resp = server
        .post("/organizations")
        .json(&serde_json::json!({"slug": "rm-member-org", "display_name": "Rm Member Org"}))
        .add_header(name.clone(), value.clone())
        .await;
    let org_body: serde_json::Value = org_resp.json();
    let org_id = org_body["org"]["id"].as_str().unwrap();

    let target_id = Uuid::new_v4();
    server
        .post(&format!("/organizations/{org_id}/members"))
        .json(&serde_json::json!({"user_id": target_id.to_string(), "role": "member"}))
        .add_header(name.clone(), value.clone())
        .await;

    let response = server
        .delete(&format!("/organizations/{org_id}/members/{target_id}"))
        .add_header(name, value)
        .await;
    response.assert_status(StatusCode::NO_CONTENT);
}
