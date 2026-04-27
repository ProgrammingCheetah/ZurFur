//! OpenAPI spec generation and documentation endpoints.
//!
//! Serves the OpenAPI 3.1 JSON spec at `/api/docs/openapi.json` and
//! interactive Scalar UI at `/api/docs`. Route annotations are added
//! in Phase 3 — this module provides the infrastructure only.

use std::sync::LazyLock;

use axum::Json;
use utoipa::OpenApi;

use crate::error::ErrorBody;

static OPENAPI_SPEC: LazyLock<utoipa::openapi::OpenApi> = LazyLock::new(ApiDoc::openapi);

#[derive(OpenApi)]
#[openapi(
    info(
        title = "Zurfur API",
        version = env!("CARGO_PKG_VERSION"),
        description = "AT Protocol-native art commission platform"
    ),
    tags(
        (name = "Auth", description = "AT Protocol OAuth authentication"),
        (name = "Users", description = "User profile and preferences"),
        (name = "Organizations", description = "Org CRUD, membership, roles"),
        (name = "Feeds", description = "Feed management, items, elements"),
        (name = "Onboarding", description = "First-login onboarding flow")
    ),
    components(
        schemas(ErrorBody)
    ),
    // Security scheme is defined (via SecurityAddon) but NOT applied globally.
    // Phase 3 route annotations apply security(("bearer_auth" = [])) per-operation,
    // so public endpoints like /auth/start don't incorrectly show as auth-required.
    modifiers(&SecurityAddon)
)]
pub struct ApiDoc;

struct SecurityAddon;

impl utoipa::Modify for SecurityAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        let components = openapi.components.get_or_insert_with(Default::default);
        components.add_security_scheme(
            "bearer_auth",
            utoipa::openapi::security::SecurityScheme::Http(
                utoipa::openapi::security::HttpBuilder::new()
                    .scheme(utoipa::openapi::security::HttpAuthScheme::Bearer)
                    .bearer_format("JWT")
                    .build(),
            ),
        );
    }
}

pub async fn openapi_json() -> Json<utoipa::openapi::OpenApi> {
    Json(OPENAPI_SPEC.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn openapi_spec_generates() {
        let _spec = ApiDoc::openapi();
    }

    #[test]
    fn openapi_spec_has_info() {
        let spec = ApiDoc::openapi();
        assert_eq!(spec.info.title, "Zurfur API");
        assert_eq!(spec.info.version, env!("CARGO_PKG_VERSION"));
    }

    #[test]
    fn openapi_spec_has_tags() {
        let spec = ApiDoc::openapi();
        let tags = spec.tags.expect("tags should be present");
        let tag_names: Vec<&str> = tags.iter().map(|t| t.name.as_str()).collect();
        for expected in ["Auth", "Users", "Organizations", "Feeds", "Onboarding"] {
            assert!(tag_names.contains(&expected), "missing tag: {expected}");
        }
    }

    #[test]
    fn openapi_spec_has_security_scheme() {
        let spec = ApiDoc::openapi();
        let components = spec.components.as_ref().expect("components should be present");
        let schemes = &components.security_schemes;
        assert!(
            schemes.contains_key("bearer_auth"),
            "bearer_auth security scheme should be defined"
        );
        assert!(
            spec.security.is_none(),
            "global security should not be set; applied per-operation in Phase 3"
        );
    }

    #[test]
    fn openapi_spec_has_error_schema() {
        let spec = ApiDoc::openapi();
        let components = spec.components.as_ref().expect("components should be present");
        assert!(
            components.schemas.contains_key("ErrorBody"),
            "ErrorBody schema should be defined"
        );
        // Verify the schema has the expected fields
        let spec_json = serde_json::to_value(&spec).expect("spec should serialize");
        let error_schema = &spec_json["components"]["schemas"]["ErrorBody"];
        let properties = &error_schema["properties"];
        assert!(properties.get("error").is_some(), "ErrorBody should have 'error' field");
        assert!(properties.get("code").is_some(), "ErrorBody should have 'code' field");
    }
}
