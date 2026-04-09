use axum::http::{HeaderName, HeaderValue, StatusCode};
use axum_test::TestServer;
use uuid::Uuid;

use super::helpers::{issue_test_jwt, test_app_state};
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
    assert_eq!(body["members"][0]["is_owner"], true);
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
