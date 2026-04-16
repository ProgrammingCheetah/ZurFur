use axum::http::StatusCode;
use uuid::Uuid;

use super::test_state::{auth_header, issue_test_jwt, test_server};

#[tokio::test]
async fn root_returns_hello_world() {
    let server = test_server();
    let response = server.get("/").await;
    response.assert_status_ok();
    response.assert_text("Hello, World!");
}

#[tokio::test]
async fn me_without_token_returns_401() {
    let server = test_server();
    let response = server.get("/auth/me").await;
    response.assert_status_unauthorized();
}

#[tokio::test]
async fn me_with_invalid_token_returns_401() {
    let server = test_server();
    let (name, value) = auth_header("garbage");
    let response = server.get("/auth/me").add_header(name, value).await;
    response.assert_status_unauthorized();
}

#[tokio::test]
async fn me_with_valid_token_returns_claims() {
    let server = test_server();
    let user_id = Uuid::new_v4();
    let token = issue_test_jwt(&user_id, "did:plc:testuser", Some("test.bsky.social"));
    let (name, value) = auth_header(&token);

    let response = server.get("/auth/me").add_header(name, value).await;
    response.assert_status_ok();

    let body: serde_json::Value = response.json();
    assert_eq!(body["user_id"], user_id.to_string());
    assert_eq!(body["did"], "did:plc:testuser");
    assert_eq!(body["handle"], "test.bsky.social");
}

#[tokio::test]
async fn refresh_with_invalid_token_returns_400() {
    let server = test_server();
    let response = server
        .post("/auth/refresh")
        .json(&serde_json::json!({"refresh_token": "nonexistent"}))
        .await;
    // InvalidState maps to 400
    response.assert_status_bad_request();
}

#[tokio::test]
async fn logout_without_token_returns_401() {
    let server = test_server();
    let response = server.post("/auth/logout").await;
    response.assert_status_unauthorized();
}

#[tokio::test]
async fn logout_with_valid_token_returns_204() {
    let server = test_server();
    let user_id = Uuid::new_v4();
    let token = issue_test_jwt(&user_id, "did:plc:testuser", None);
    let (name, value) = auth_header(&token);

    let response = server.post("/auth/logout").add_header(name, value).await;
    response.assert_status(StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn start_login_with_empty_handle_returns_400() {
    let server = test_server();
    let response = server
        .post("/auth/start")
        .json(&serde_json::json!({"handle": ""}))
        .await;
    response.assert_status_bad_request();
}

#[tokio::test]
async fn start_login_with_invalid_handle_format_returns_400() {
    let server = test_server();
    // No dot → not a valid domain-style handle
    let response = server
        .post("/auth/start")
        .json(&serde_json::json!({"handle": "notahandle"}))
        .await;
    response.assert_status_bad_request();
}

#[tokio::test]
#[ignore] // Requires network access (hits plc.directory for identity resolution)
async fn start_login_accepts_did_format() {
    let server = test_server();
    // DID format bypasses handle validation but will fail on identity resolution
    let response = server
        .post("/auth/start")
        .json(&serde_json::json!({"handle": "did:plc:fake"}))
        .await;
    // Fails at identity resolution, not validation
    response.assert_status(StatusCode::BAD_GATEWAY);
}
