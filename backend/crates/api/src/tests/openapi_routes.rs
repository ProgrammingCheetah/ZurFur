use super::test_state::test_server;

#[tokio::test]
async fn get_openapi_json_returns_200() {
    let server = test_server();
    let response = server.get("/api/docs/openapi.json").await;
    response.assert_status_ok();
    let content_type = response
        .header("content-type")
        .to_str()
        .unwrap()
        .to_string();
    assert!(
        content_type.contains("application/json"),
        "expected application/json, got: {content_type}"
    );
}

#[tokio::test]
async fn get_openapi_json_valid_spec() {
    let server = test_server();
    let response = server.get("/api/docs/openapi.json").await;
    // Should deserialize as a valid OpenApi struct
    let _spec: utoipa::openapi::OpenApi = response.json();
}

#[tokio::test]
async fn get_scalar_ui_returns_200() {
    let server = test_server();
    let response = server.get("/api/docs").await;
    response.assert_status_ok();
    let content_type = response
        .header("content-type")
        .to_str()
        .unwrap()
        .to_string();
    assert!(
        content_type.contains("text/html"),
        "expected text/html, got: {content_type}"
    );
}

#[tokio::test]
async fn openapi_spec_paths_empty_initially() {
    let server = test_server();
    let spec: utoipa::openapi::OpenApi = server.get("/api/docs/openapi.json").await.json();
    // Before Phase 3 annotations, no paths should be registered
    assert!(
        spec.paths.paths.is_empty(),
        "paths should be empty before route annotations are added"
    );
}
