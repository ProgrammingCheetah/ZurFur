use axum::http::StatusCode;
use uuid::Uuid;

use super::test_state::{auth_header, issue_test_jwt, test_server, test_server_with_user};

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
    let auth = auth_header(&token);

    let response = server
        .post("/onboarding/complete")
        .json(&serde_json::json!({"role": "artist"}))
        .add_header(auth.0, auth.1)
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
    let auth = auth_header(&token);

    let response = server
        .post("/onboarding/complete")
        .json(&serde_json::json!({"role": "commissioner_client"}))
        .add_header(auth.0, auth.1)
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
    let auth = auth_header(&token);

    server
        .post("/onboarding/complete")
        .json(&serde_json::json!({"role": "artist"}))
        .add_header(auth.0.clone(), auth.1.clone())
        .await;

    let response = server
        .post("/onboarding/complete")
        .json(&serde_json::json!({"role": "artist"}))
        .add_header(auth.0, auth.1)
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
    let auth = auth_header(&token);

    let response = server
        .post("/onboarding/complete")
        .json(&serde_json::json!({"role": "invalid_role"}))
        .add_header(auth.0, auth.1)
        .await;
    response.assert_status(StatusCode::BAD_REQUEST);
}
