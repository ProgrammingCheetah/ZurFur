# OpenAPI Route Annotations — Design Document

> **Created 2026-04-15**

## Context

Phase 2A adds the OpenAPI infrastructure (utoipa, Scalar UI, spec endpoint, error schemas). This document covers **Phase 3A-3C** — annotating all existing route handlers with `#[utoipa::path]` and adding `#[derive(ToSchema)]` to request/response types.

Split into 3 units for parallelism, one per route file group. Each unit touches different files — safe to run simultaneously.

### Prerequisites

- **Phase 2A (OpenAPI Infrastructure)** must be merged — provides `utoipa` dependency, `ApiDoc` struct, `ErrorBody` schema, security scheme.

### What Phase 2A provides

After Phase 2A merges, the codebase will have:
- `utoipa` and `utoipa-scalar` in workspace dependencies
- `api/src/openapi.rs` with `ApiDoc` struct defining tags (Auth, Users, Organizations, Feeds, Tags, Onboarding) and Bearer JWT security scheme
- `ErrorBody` with `#[derive(ToSchema)]` in `api/src/error.rs`
- Scalar UI at `/api/docs` and spec at `/api/docs/openapi.json`
- The `ApiDoc` struct has a `#[openapi(paths(...))]` attribute that each unit will add paths to

---

## Unit 3A: Auth + Users + Onboarding

**Branch:** `feature/openapi-annotations-auth-users`

**Files touched:**
- `api/src/routes/auth.rs`
- `api/src/routes/users.rs`
- `api/src/routes/onboarding.rs`
- `api/src/openapi.rs` (add paths to `ApiDoc`)

### auth.rs — 5 endpoints

Add `#[derive(ToSchema)]` to: `StartLoginRequest`, `StartLoginResponse`, `CallbackQuery`, `CallbackResponse`, `RefreshRequest`, `RefreshResponse`, `MeResponse`.

| Handler | Method | Path | Tag | Auth | Responses |
|---------|--------|------|-----|------|-----------|
| `start_login` | POST | `/auth/start` | Auth | No | 200 StartLoginResponse, 400 ErrorBody, 502 ErrorBody |
| `callback` | POST | `/auth/callback` | Auth | No | 200 CallbackResponse, 400 ErrorBody, 502 ErrorBody |
| `refresh` | POST | `/auth/refresh` | Auth | No | 200 RefreshResponse, 400 ErrorBody |
| `me` | GET | `/auth/me` | Auth | Yes | 200 MeResponse, 401 ErrorBody |
| `logout` | POST | `/auth/logout` | Auth | Yes | 204, 401 ErrorBody |

### users.rs — 3 endpoints

Add `#[derive(ToSchema)]` to: `UserProfileResponse`, `PersonalOrgResponse`, `MembershipResponse`, `PreferencesResponse`, `UpdatePreferencesRequest`.

Note: these types are currently `struct` (not `pub`). They may need `pub(crate)` for utoipa to access them, or the `ToSchema` derive handles non-pub types within the same crate.

| Handler | Method | Path | Tag | Auth | Responses |
|---------|--------|------|-----|------|-----------|
| `get_me` | GET | `/users/me` | Users | Yes | 200 UserProfileResponse, 401 ErrorBody, 404 ErrorBody |
| `get_preferences` | GET | `/users/me/preferences` | Users | Yes | 200 PreferencesResponse, 401 ErrorBody |
| `update_preferences` | PUT | `/users/me/preferences` | Users | Yes | 200 PreferencesResponse, 400 ErrorBody, 401 ErrorBody |

### onboarding.rs — 1 endpoint

Add `#[derive(ToSchema)]` to: `CompleteOnboardingRequest`, `OnboardingResponse`.

`OnboardingResponse` contains `Vec<FeedResponse>` — `FeedResponse` lives in `feeds.rs`. It needs `ToSchema` too, but that's 3B's territory. Import it as `pub(crate)` or add `ToSchema` to `FeedResponse` in this unit (coordinate: if 3B is running in parallel, only one should add it — **this unit should NOT touch feeds.rs**). Instead, reference `FeedResponse` as an opaque schema using `#[schema(value_type = Vec<Object>)]` or coordinate with 3B.

| Handler | Method | Path | Tag | Auth | Responses |
|---------|--------|------|-----|------|-----------|
| `complete_onboarding` | POST | `/onboarding/complete` | Onboarding | Yes | 200 OnboardingResponse, 400 ErrorBody, 401 ErrorBody, 404 ErrorBody |

### Register paths in ApiDoc

Add all 9 paths to `#[openapi(paths(...))]` in `api/src/openapi.rs`:
```rust
paths(
    routes::auth::start_login,
    routes::auth::callback,
    routes::auth::refresh,
    routes::auth::me,
    routes::auth::logout,
    routes::users::get_me,
    routes::users::get_preferences,
    routes::users::update_preferences,
    routes::onboarding::complete_onboarding,
)
```

Handler functions may need `pub(crate)` visibility for utoipa to reference them in the paths list.

---

## Unit 3B: Organizations + Feeds

**Branch:** `feature/openapi-annotations-orgs-feeds`

**Files touched:**
- `api/src/routes/organizations.rs`
- `api/src/routes/feeds.rs`
- `api/src/openapi.rs` (add paths to `ApiDoc`)

### organizations.rs — 8 endpoints

Add `#[derive(ToSchema)]` to: `OrgResponse`, `MemberResponse`, `OrgDetailResponse`, `CreateOrgRequest`, `UpdateOrgRequest`, `AddMemberRequest`, `UpdateMemberRequest`.

| Handler | Method | Path | Tag | Auth | Responses |
|---------|--------|------|-----|------|-----------|
| `create_org` | POST | `/organizations` | Organizations | Yes | 201 OrgDetailResponse, 400 ErrorBody, 401 ErrorBody, 409 ErrorBody |
| `get_org` | GET | `/organizations/{id_or_slug}` | Organizations | No | 200 OrgDetailResponse, 404 ErrorBody |
| `update_org` | PUT | `/organizations/{id_or_slug}` | Organizations | Yes | 200 OrgResponse, 401 ErrorBody, 403 ErrorBody, 404 ErrorBody |
| `delete_org` | DELETE | `/organizations/{id_or_slug}` | Organizations | Yes | 204, 401 ErrorBody, 403 ErrorBody |
| `list_members` | GET | `/organizations/{id}/members` | Organizations | No | 200 Vec<MemberResponse>, 404 ErrorBody |
| `add_member` | POST | `/organizations/{id}/members` | Organizations | Yes | 201 MemberResponse, 401 ErrorBody, 403 ErrorBody, 409 ErrorBody |
| `update_member` | PUT | `/organizations/{id}/members/{user_id}` | Organizations | Yes | 200 MemberResponse, 401 ErrorBody, 403 ErrorBody, 404 ErrorBody |
| `remove_member` | DELETE | `/organizations/{id}/members/{user_id}` | Organizations | Yes | 204, 401 ErrorBody, 403 ErrorBody |

### feeds.rs — 6 endpoints

Add `#[derive(ToSchema)]` to: `FeedResponse`, `FeedItemResponse`, `FeedElementResponse`, `CreateFeedRequest`, `UpdateFeedRequest`, `PostToFeedRequest`, `NewElementRequest`.

| Handler | Method | Path | Tag | Auth | Responses |
|---------|--------|------|-----|------|-----------|
| `get_feed` | GET | `/feeds/{id}` | Feeds | No | 200 FeedResponse, 404 ErrorBody |
| `update_feed` | PUT | `/feeds/{id}` | Feeds | Yes | 200 FeedResponse, 401 ErrorBody, 403 ErrorBody, 404 ErrorBody |
| `delete_feed` | DELETE | `/feeds/{id}` | Feeds | Yes | 204, 401 ErrorBody, 403 ErrorBody |
| `post_to_feed` | POST | `/feeds/{id}/items` | Feeds | Yes | 201 FeedItemResponse, 401 ErrorBody, 403 ErrorBody |
| `list_feed_items` | GET | `/feeds/{id}/items` | Feeds | No | 200 Vec<FeedItemResponse>, 404 ErrorBody |
| `delete_feed_item` | DELETE | `/feeds/{id}/items/{item_id}` | Feeds | Yes | 204, 401 ErrorBody, 403 ErrorBody |

Also: `list_org_feeds` and `create_org_feed` are org-scoped but defined in feeds.rs and called from organizations.rs. Annotate these too:

| Handler | Method | Path | Tag | Auth | Responses |
|---------|--------|------|-----|------|-----------|
| `list_org_feeds` | GET | `/organizations/{id}/feeds` | Feeds | No | 200 Vec<FeedResponse> |
| `create_org_feed` | POST | `/organizations/{id}/feeds` | Feeds | Yes | 201 FeedResponse, 401 ErrorBody, 403 ErrorBody, 409 ErrorBody |

### Register paths in ApiDoc

Add all 16 paths to `api/src/openapi.rs`.

---

## Unit 3C: Tags

**Branch:** `feature/openapi-annotations-tags`

**Files touched:**
- `api/src/routes/tags.rs`
- `api/src/routes/helpers.rs`
- `api/src/openapi.rs` (add paths to `ApiDoc`)

### tags.rs — 10 endpoints

Add `#[derive(ToSchema)]` to: `TagResponse`, `CreateTagRequest`, `UpdateTagRequest`, `AttachTagRequest`, `DetachTagRequest`, `SearchQuery`.

| Handler | Method | Path | Tag | Auth | Responses |
|---------|--------|------|-----|------|-----------|
| `create_tag` | POST | `/tags` | Tags | Yes | 201 TagResponse, 400 ErrorBody, 401 ErrorBody, 409 ErrorBody |
| `search_tags` | GET | `/tags/search` | Tags | No | 200 Vec<TagResponse> |
| `list_by_category` | GET | `/tags/category/{category}` | Tags | No | 200 Vec<TagResponse> |
| `attach_tag` | POST | `/tags/attach` | Tags | Yes | 201, 400 ErrorBody, 401 ErrorBody, 409 ErrorBody |
| `detach_tag` | POST | `/tags/detach` | Tags | Yes | 204, 400 ErrorBody, 401 ErrorBody, 404 ErrorBody |
| `list_entity_tags` | GET | `/tags/entity/{entity_type}/{entity_id}` | Tags | No | 200 Vec<TagResponse>, 400 ErrorBody |
| `get_tag` | GET | `/tags/{id}` | Tags | Yes | 200 TagResponse, 401 ErrorBody, 404 ErrorBody |
| `update_tag` | PUT | `/tags/{id}` | Tags | Yes | 200 TagResponse, 400 ErrorBody, 401 ErrorBody, 403 ErrorBody |
| `delete_tag` | DELETE | `/tags/{id}` | Tags | Yes | 204, 401 ErrorBody, 403 ErrorBody |
| `approve_tag` | POST | `/tags/{id}/approve` | Tags | Yes | 200 TagResponse, 401 ErrorBody, 404 ErrorBody |

### helpers.rs

Add `#[derive(ToSchema)]` to `PaginationQuery` if used as a documented query param.

### Register paths in ApiDoc

Add all 10 paths to `api/src/openapi.rs`.

---

## Coordination Between Units

All three units add paths to `api/src/openapi.rs`. To avoid merge conflicts:

**Option A (preferred):** Each unit adds paths to the `ApiDoc` struct in a clearly separated block:
```rust
#[openapi(
    paths(
        // 3A: Auth + Users + Onboarding
        routes::auth::start_login,
        // ...
        // 3B: Organizations + Feeds
        routes::organizations::create_org,
        // ...
        // 3C: Tags
        routes::tags::create_tag,
        // ...
    ),
    // ...
)]
```

Since the `paths()` attribute is a comma-separated list, the merge should auto-resolve as long as each unit adds to different lines.

**Option B:** If conflicts are likely, run 3A first, then 3B and 3C after 3A merges.

---

## Pattern for Annotations

Every handler follows this pattern:

```rust
/// Short description of what this endpoint does.
#[utoipa::path(
    post,
    path = "/organizations",
    tag = "Organizations",
    request_body = CreateOrgRequest,
    responses(
        (status = 201, description = "Organization created", body = OrgResponse),
        (status = 400, description = "Invalid input", body = ErrorBody),
        (status = 401, description = "Not authenticated", body = ErrorBody),
    ),
    security(("bearer_auth" = []))
)]
async fn create_org(...) -> Result<..., AppError> { ... }
```

For handlers that don't require auth, omit the `security()` line.

For path parameters:
```rust
#[utoipa::path(
    get,
    path = "/organizations/{id_or_slug}",
    tag = "Organizations",
    params(
        ("id_or_slug" = String, Path, description = "Organization slug or UUID")
    ),
    responses(...)
)]
```

For query parameters:
```rust
#[utoipa::path(
    get,
    path = "/tags/search",
    tag = "Tags",
    params(
        ("q" = String, Query, description = "Search prefix"),
        ("limit" = i64, Query, description = "Max results")
    ),
    responses(...)
)]
```

---

## Test Plan

### Per-unit tests (add to existing test files)

| Test | What it verifies |
|------|-----------------|
| `openapi_spec_has_auth_paths` (3A) | Spec contains all 5 auth paths |
| `openapi_spec_has_user_paths` (3A) | Spec contains all 3 user paths |
| `openapi_spec_has_onboarding_paths` (3A) | Spec contains onboarding path |
| `openapi_spec_has_org_paths` (3B) | Spec contains all 8 org paths |
| `openapi_spec_has_feed_paths` (3B) | Spec contains all 8 feed paths |
| `openapi_spec_has_tag_paths` (3C) | Spec contains all 10 tag paths |
| `all_schemas_present` (any unit) | All request/response types appear as component schemas |

### E2E test (after all 3 merge)

| Test | What it verifies |
|------|-----------------|
| `openapi_spec_complete` | Total path count matches expected (35+) |
| `scalar_ui_shows_all_groups` | All 6 tag groups have at least one endpoint |

---

## Verification

After all 3 units merge:

```bash
cargo build -p api               # Compiles
cargo test -p api                 # All tests pass
just dev                          # Start server
# Visit http://localhost:3000/api/docs — all 35 endpoints visible
# Each endpoint shows request/response schemas
# Auth endpoints show the lock icon
```
