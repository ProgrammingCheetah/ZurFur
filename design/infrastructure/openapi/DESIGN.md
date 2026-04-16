# OpenAPI Infrastructure — Design Document

> **Created 2026-04-15**

## Context

Zurfur has 35 API endpoints across 6 route modules with no API documentation. This design covers adding OpenAPI 3.1 spec generation via `utoipa`, serving interactive docs via Scalar UI, and standardizing response shapes.

**This is split across two phases in the WORKBOARD:**
- **Phase 2A** (this unit): Infrastructure — dependencies, spec endpoint, Scalar UI, error/response schemas
- **Phase 3A-3C** (later units): Route annotations — `#[utoipa::path]` on all handlers, split by module for parallelism

This document covers **Phase 2A only**. Phase 3 units will reference this for patterns.

### Codebase State

The API crate (`backend/crates/api/`) uses Axum with:
- 6 route modules: `auth.rs`, `users.rs`, `organizations.rs`, `feeds.rs`, `tags.rs`, `onboarding.rs`
- Unified `AppError` enum in `error.rs` → JSON `{ "error": "...", "code": "..." }`
- Request/response types defined inline per route file with `Serialize`/`Deserialize`
- Auth via `AuthUser` custom extractor (Bearer JWT)
- `SharedState = Arc<AppState>` passed to all handlers

**No OpenAPI dependency exists.** This is a clean slate.

### Decisions from Questions

| Decision | Answer |
|----------|--------|
| UI | Scalar (modern, clean DX) |
| Spec path | `/api/docs/openapi.json` (spec) + `/api/docs` (UI) |
| Environment | Available in production (public API, decentralization principle) |
| Scope | Annotate all endpoints (Phase 3, not this unit) |
| Static spec | Yes, automated via script (deferred to Phase 3) |
| Grouping | Mirror route modules: Auth, Users, Organizations, Feeds, Tags, Onboarding |
| Response standardization | Formalize current patterns with OpenAPI schemas |
| Testing | Contract testing + client generation (deferred, post-Phase 3) |
| Tag API format | Dictionary in responses, `TYPE:tag` in query params (presentation only) |

---

## Dependencies

Add to workspace `Cargo.toml`:

```toml
[workspace.dependencies]
utoipa = { version = "5", features = ["axum_extras"] }
utoipa-scalar = { version = "0.2", features = ["axum"] }
```

Add to `api/Cargo.toml`:

```toml
[dependencies]
utoipa = { workspace = true }
utoipa-scalar = { workspace = true }
```

**Why these crates:**
- `utoipa` — proc macro OpenAPI spec generation for Rust/Axum. Industry standard.
- `utoipa-scalar` — serves Scalar UI from the generated spec. Modern alternative to Swagger UI.
- `axum_extras` feature enables Axum extractor support in path macros.

---

## Response Standardization

The current response shapes are already clean. Standardization means formally defining them as OpenAPI schemas, not changing the shape.

### Success Responses

Data returned directly (no envelope wrapper):

```json
// Single entity
{ "id": "uuid", "slug": "my-org", "display_name": "My Org" }

// List
[{ "id": "uuid", "name": "wolf" }, { "id": "uuid", "name": "fox" }]

// Empty success
(204 No Content, no body)
```

### Error Responses

All errors use the `ErrorBody` shape already defined in `error.rs`:

```json
{ "error": "Human-readable message", "code": "error_code_string" }
```

Error codes: `bad_request`, `unauthorized`, `forbidden`, `not_found`, `conflict`, `internal_error`, `bad_gateway`.

### OpenAPI Schema for Errors

Add `#[derive(ToSchema)]` to `ErrorBody` in `error.rs`:

```rust
#[derive(Serialize, ToSchema)]
pub struct ErrorBody {
    /// Human-readable error message
    pub error: String,
    /// Machine-readable error code
    pub code: &'static str,
}
```

Make `ErrorBody` public (`pub struct`) so utoipa can reference it from route annotations in Phase 3.

---

## OpenAPI Spec Builder

### New module: `api/src/openapi.rs`

Creates the `OpenApi` spec struct with:
- API title, version, description
- Server URL (from env or default)
- Security scheme (Bearer JWT)
- Tag definitions (grouping for UI)
- Component schemas (ErrorBody, plus request/response types added in Phase 3)

```rust
use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(
    info(
        title = "Zurfur API",
        version = "0.1.0",
        description = "AT Protocol-native art commission platform"
    ),
    tags(
        (name = "Auth", description = "AT Protocol OAuth authentication"),
        (name = "Users", description = "User profile and preferences"),
        (name = "Organizations", description = "Org CRUD, membership, roles"),
        (name = "Feeds", description = "Feed management, items, elements"),
        (name = "Tags", description = "Tag taxonomy, attachment, search"),
        (name = "Onboarding", description = "First-login onboarding flow")
    ),
    components(
        schemas(ErrorBody)
    ),
    security(
        ("bearer_auth" = [])
    ),
    modifiers(&SecurityAddon)
)]
pub struct ApiDoc;

/// Adds Bearer JWT security scheme to the spec.
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
```

### Routes

Two endpoints, nested under `/api/docs`:

```rust
// GET /api/docs/openapi.json → returns the OpenAPI JSON spec
async fn openapi_json() -> Json<utoipa::openapi::OpenApi> {
    Json(ApiDoc::openapi())
}

// GET /api/docs → Scalar UI (serves HTML that loads the spec)
// Uses utoipa_scalar::Scalar
```

### Router Integration

In `api/src/lib.rs`, nest the OpenAPI routes into the main router:

```rust
use utoipa_scalar::{Scalar, Servable};

let app = Router::new()
    // ... existing routes ...
    .merge(Scalar::with_url("/api/docs", ApiDoc::openapi()))
    .route("/api/docs/openapi.json", get(openapi_json))
    .with_state(state);
```

---

## Files to Create/Modify

| File | Change |
|------|--------|
| `Cargo.toml` (root) | Add `utoipa` and `utoipa-scalar` to workspace dependencies |
| `api/Cargo.toml` | Add `utoipa` and `utoipa-scalar` to dependencies |
| `api/src/openapi.rs` (new) | `ApiDoc` struct, security modifier, JSON endpoint |
| `api/src/error.rs` | Add `ToSchema` derive to `ErrorBody`, make it `pub` |
| `api/src/lib.rs` | Import openapi module, merge Scalar UI + spec route into router |

**Files NOT touched** (reserved for Phase 3 units):
- `api/src/routes/*.rs` — route annotations happen in Phase 3A-3C
- `api/src/state.rs` — no changes needed
- `api/src/main.rs` — no changes needed (router built in lib.rs)

---

## Test Plan

### Unit Tests

| Test | What it verifies |
|------|-----------------|
| `openapi_spec_generates` | `ApiDoc::openapi()` returns a valid `OpenApi` struct without panicking |
| `openapi_spec_has_info` | Spec title = "Zurfur API", version = "0.1.0" |
| `openapi_spec_has_tags` | All 6 tag groups present: Auth, Users, Organizations, Feeds, Tags, Onboarding |
| `openapi_spec_has_security_scheme` | Bearer JWT security scheme defined |
| `openapi_spec_has_error_schema` | `ErrorBody` component schema present with `error` and `code` fields |

### E2E Tests

| Test | What it verifies |
|------|-----------------|
| `get_openapi_json_returns_200` | `GET /api/docs/openapi.json` returns 200 with `application/json` |
| `get_openapi_json_valid_spec` | Response body deserializes as valid `utoipa::openapi::OpenApi` |
| `get_scalar_ui_returns_200` | `GET /api/docs` returns 200 with HTML content |
| `openapi_spec_paths_empty_initially` | Before Phase 3, paths object exists but has no entries (no routes annotated yet) |

---

## What Phase 3 Instances Need to Know

When annotating routes (Phase 3A-3C), each instance will:

1. Add `#[derive(ToSchema)]` to request/response types in their route file
2. Add `#[utoipa::path]` to each handler function with:
   - `tag = "GroupName"` matching the tags defined here
   - `request_body` for POST/PUT/PATCH
   - `responses` with status codes and schemas
   - `security(("bearer_auth" = []))` for authenticated endpoints
3. Register their paths in `ApiDoc` by adding to the `#[openapi(paths(...))]` attribute

**Pattern for a route annotation:**
```rust
#[utoipa::path(
    post,
    path = "/organizations",
    tag = "Organizations",
    request_body = CreateOrgRequest,
    responses(
        (status = 201, description = "Organization created", body = OrgResponse),
        (status = 400, description = "Invalid input", body = ErrorBody),
        (status = 401, description = "Not authenticated", body = ErrorBody),
        (status = 409, description = "Slug taken", body = ErrorBody),
    ),
    security(("bearer_auth" = []))
)]
async fn create_org(...) -> Result<..., AppError> { ... }
```

**Tag API presentation** (decided separately):
- Responses use dictionary format: `{ "organization": ["StudioFox"], "metadata": ["ref_sheet"] }`
- Query params use `TYPE:tag` format: `?tags=general:wolf,metadata:ref_sheet`
- This is serialization logic in the route handlers, not an OpenAPI concern. The schemas just document the shapes.

---

## Verification

```bash
cargo build -p api               # Compiles with new dependencies
cargo test -p api                 # All existing tests pass
# Then manually:
just dev                          # Start server
# Visit http://localhost:3000/api/docs — Scalar UI loads
# Visit http://localhost:3000/api/docs/openapi.json — JSON spec returns
```

After Phase 3 annotations complete:
- All 35+ endpoints visible in Scalar UI
- Each endpoint shows request/response schemas
- "Try it" works for unauthenticated endpoints
- Authenticated endpoints show the lock icon

## Deferred

- Static spec file generation (`just openapi-export` script) — after Phase 3
- Contract testing (validate responses match spec in CI) — post-MVP
- Client generation (TypeScript SDK from spec) — post-MVP
- Request validation middleware — not planned
