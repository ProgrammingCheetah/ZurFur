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
async fn complete_onboarding_without_token_returns_401() {
    let server = test_server();
    let response = server
        .post("/onboarding/complete")
        .json(&serde_json::json!({"role": "artist"}))
        .await;
    response.assert_status_unauthorized();
}

// --- POST /onboarding/complete -----------------------------------------------

#[tokio::test]
async fn complete_onboarding_as_artist_returns_200() {
    let (server, user_id) = test_server_with_user();
    let token = issue_test_jwt(&user_id, "did:plc:testuser", Some("testuser.bsky.social"));
    let (name, value) = auth_header(&token);

    let response = server
        .post("/onboarding/complete")
        .json(&serde_json::json!({"role": "artist"}))
        .add_header(name, value)
        .await;
    response.assert_status_ok();

    let body: serde_json::Value = response.json();
    assert_eq!(body["role"], "artist");
    let feeds = body["feeds_created"].as_array().unwrap();
    assert_eq!(feeds.len(), 4, "artist should get bio, updates, gallery, commissions");
}

#[tokio::test]
async fn complete_onboarding_as_commissioner_returns_200() {
    let (server, user_id) = test_server_with_user();
    let token = issue_test_jwt(&user_id, "did:plc:testuser", Some("testuser.bsky.social"));
    let (name, value) = auth_header(&token);

    let response = server
        .post("/onboarding/complete")
        .json(&serde_json::json!({"role": "commissioner_client"}))
        .add_header(name, value)
        .await;
    response.assert_status_ok();

    let body: serde_json::Value = response.json();
    assert_eq!(body["role"], "commissioner_client");
    let feeds = body["feeds_created"].as_array().unwrap();
    assert_eq!(feeds.len(), 3, "commissioner should get bio, updates, gallery (no commissions)");
}

#[tokio::test]
async fn complete_onboarding_twice_is_idempotent() {
    let (server, user_id) = test_server_with_user();
    let token = issue_test_jwt(&user_id, "did:plc:testuser", Some("testuser.bsky.social"));
    let (name, value) = auth_header(&token);

    // First call
    server
        .post("/onboarding/complete")
        .json(&serde_json::json!({"role": "artist"}))
        .add_header(name.clone(), value.clone())
        .await;

    // Second call — should succeed with 0 feeds created
    let response = server
        .post("/onboarding/complete")
        .json(&serde_json::json!({"role": "artist"}))
        .add_header(name, value)
        .await;
    response.assert_status_ok();

    let body: serde_json::Value = response.json();
    let feeds = body["feeds_created"].as_array().unwrap();
    assert!(feeds.is_empty(), "second onboarding call should create no feeds");
}

#[tokio::test]
async fn complete_onboarding_invalid_role_returns_400() {
    let server = test_server();
    let user_id = Uuid::new_v4();
    let token = issue_test_jwt(&user_id, "did:plc:test", Some("test.bsky.social"));
    let (name, value) = auth_header(&token);

    let response = server
        .post("/onboarding/complete")
        .json(&serde_json::json!({"role": "invalid_role"}))
        .add_header(name, value)
        .await;
    response.assert_status(StatusCode::BAD_REQUEST);
}
