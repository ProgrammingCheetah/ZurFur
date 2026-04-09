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
async fn get_me_without_token_returns_401() {
    let server = test_server();
    let response = server.get("/users/me").await;
    response.assert_status_unauthorized();
}

#[tokio::test]
async fn get_preferences_without_token_returns_401() {
    let server = test_server();
    let response = server.get("/users/me/preferences").await;
    response.assert_status_unauthorized();
}

#[tokio::test]
async fn update_preferences_without_token_returns_401() {
    let server = test_server();
    let response = server
        .put("/users/me/preferences")
        .json(&serde_json::json!({"max_content_rating": "nsfw"}))
        .await;
    response.assert_status_unauthorized();
}

// --- GET /users/me -----------------------------------------------------------

#[tokio::test]
async fn get_me_for_nonexistent_user_returns_404() {
    let server = test_server();
    let user_id = Uuid::new_v4();
    let token = issue_test_jwt(&user_id, "did:plc:ghost", Some("ghost.bsky.social"));
    let (name, value) = auth_header(&token);

    let response = server.get("/users/me").add_header(name, value).await;
    response.assert_status(StatusCode::NOT_FOUND);
}

// --- GET /users/me/preferences -----------------------------------------------

#[tokio::test]
async fn get_preferences_returns_default_sfw() {
    let server = test_server();
    let user_id = Uuid::new_v4();
    let token = issue_test_jwt(&user_id, "did:plc:test", Some("test.bsky.social"));
    let (name, value) = auth_header(&token);

    let response = server
        .get("/users/me/preferences")
        .add_header(name, value)
        .await;
    response.assert_status_ok();

    let body: serde_json::Value = response.json();
    assert_eq!(body["max_content_rating"], "sfw");
}

// --- PUT /users/me/preferences -----------------------------------------------

#[tokio::test]
async fn update_preferences_with_valid_rating() {
    let server = test_server();
    let user_id = Uuid::new_v4();
    let token = issue_test_jwt(&user_id, "did:plc:test", Some("test.bsky.social"));
    let (name, value) = auth_header(&token);

    let response = server
        .put("/users/me/preferences")
        .json(&serde_json::json!({"max_content_rating": "nsfw"}))
        .add_header(name, value)
        .await;
    response.assert_status_ok();

    let body: serde_json::Value = response.json();
    assert_eq!(body["max_content_rating"], "nsfw");
}

#[tokio::test]
async fn update_preferences_with_invalid_rating_returns_400() {
    let server = test_server();
    let user_id = Uuid::new_v4();
    let token = issue_test_jwt(&user_id, "did:plc:test", Some("test.bsky.social"));
    let (name, value) = auth_header(&token);

    let response = server
        .put("/users/me/preferences")
        .json(&serde_json::json!({"max_content_rating": "extreme"}))
        .add_header(name, value)
        .await;
    response.assert_status(StatusCode::BAD_REQUEST);
}
