# Feature 2 Phase 3: Characters — API + Service Layer

> **Created 2026-04-15**

## Context

Phase 2B adds the Character entity, migration, and repository to the domain and persistence crates. This document covers **Phase 3D** — the application service layer and API routes.

### Prerequisites

- **Phase 2B (Characters Domain)** must be merged — provides `Character`, `CharacterVisibility`, `CharacterError`, `CharacterRepository`, `SqlxCharacterRepository`, and the `character` table migration.
- **Phase 1A (Entity Interfaces)** — provides `EntityKind::Character`, entity traits.

### What Phase 2B provides

After Phase 2B merges:
- `domain/src/character.rs` — `Character` struct, `CharacterVisibility` enum, `CharacterError`, `CharacterRepository` trait
- `Character` implements `Entity`, `Taggable`, `FeedOwnable` traits
- `persistence/src/repositories/character_repository.rs` — `SqlxCharacterRepository`
- Migration creating `character_visibility` PG ENUM and `character` table
- No service, no routes, no AppState wiring yet

### Codebase Patterns

**Service pattern** — see `application/src/organization/service.rs`:
- Service struct holds `Arc<dyn Repository>` for each dependency
- Public async methods with permission checks
- Service-level error enum with `thiserror`
- `From<ServiceError> for AppError` in api crate

**Route pattern** — see `api/src/routes/organizations.rs`:
- Request/response types with Serde derives at top of file
- Handlers extract `State`, `AuthUser`, `Path`, `Json`
- `parse_user_id(&claims.sub)` for auth
- Return `Result<Json<T>, AppError>` or `Result<StatusCode, AppError>`

**Org resolution** — slug-first, UUID fallback (see PHASE3_DESIGN.md):
```
Try slug lookup → Found? Use org
                → Not found? Parse as UUID → Find by ID → 404 if not found
```

---

## Phase 3D Scope — Service + API

### New: `application/src/character/mod.rs`

Module declaration.

### New: `application/src/character/service.rs`

**CharacterServiceError:**
```rust
pub enum CharacterServiceError {
    NotFound,
    Forbidden,
    InvalidInput(String),
    Internal(String),
}
```

**CharacterService:**
```rust
pub struct CharacterService {
    character_repo: Arc<dyn CharacterRepository>,
    org_repo: Arc<dyn OrganizationRepository>,
    member_repo: Arc<dyn OrganizationMemberRepository>,
}
```

**Methods:**

`create_character(user_id, org_id, name, description, content_rating, visibility)`:
1. Verify org exists via `org_repo.find_by_id(org_id)`
2. Verify user is org member with `MANAGE_PROFILE` via `member_repo.find_by_org_and_user(org_id, user_id)`
3. Validate name: non-empty, max 256 chars (`.trim()`, check `.is_empty()`, check `.len() > 256`)
4. Call `character_repo.create(org_id, name, description, content_rating, visibility)`
5. Return `Character`

`get_character(character_id, viewer_user_id: Option<Uuid>)`:
1. Fetch character via `character_repo.find_by_id(character_id)`
2. If not found → `NotFound`
3. Apply visibility rules:
   - `Public` or `Unlisted` → return character
   - `Private` or `Controlled` → check if viewer is org member. If no viewer or not member → `NotFound` (404, not 403 — don't leak existence)

`list_org_characters(org_id, viewer_user_id: Option<Uuid>, limit, offset, content_rating_filter, tag_ids_filter)`:
1. Clamp limit to max 100
2. If viewer is an org member → include all visibilities
3. If viewer is not a member or anonymous → include only `Public`
4. Call `character_repo.list_by_org(org_id, limit, offset, content_rating_filter, tag_ids_filter)` with appropriate visibility filter
5. Return `Vec<Character>`

`update_character(user_id, character_id, name, description, content_rating, visibility)`:
1. Fetch character to get `org_id`
2. Verify membership + `MANAGE_PROFILE`
3. Validate name if provided (same rules as create)
4. Call `character_repo.update(character_id, name, description, content_rating, visibility)`

`delete_character(user_id, character_id)`:
1. Fetch character to get `org_id`
2. Verify membership + `MANAGE_PROFILE`
3. Call `character_repo.soft_delete(character_id)`

**Org resolution helper** — `resolve_org(org_repo, id_or_slug: &str) -> Result<Organization, ...>`:
Try `org_repo.find_by_slug(id_or_slug)` first. If `None`, try parsing as UUID and `org_repo.find_by_id(uuid)`. If both fail, return `NotFound`. This helper should be extracted to a shared location (e.g., `application/src/common.rs`) since org routes will use it too. If extracting feels like scope creep, implement it as a private function in the character service for now.

### Modified: `application/src/lib.rs`

Add `pub mod character;`

### New: `api/src/routes/characters.rs`

**Request types:**
```rust
#[derive(Deserialize)]
struct CreateCharacterRequest {
    name: String,
    description: Option<String>,
    content_rating: String,  // parsed to ContentRating
    visibility: String,      // parsed to CharacterVisibility
}

#[derive(Deserialize)]
struct UpdateCharacterRequest {
    name: Option<String>,
    description: Option<String>,
    content_rating: Option<String>,
    visibility: Option<String>,
}

#[derive(Deserialize)]
struct ListCharactersQuery {
    #[serde(default = "default_limit")]
    limit: i64,
    #[serde(default)]
    offset: i64,
    content_rating: Option<String>,
    tags: Option<String>,  // comma-separated UUIDs
}
```

**Response type:**
```rust
#[derive(Serialize)]
struct CharacterResponse {
    id: String,
    org_id: String,
    name: String,
    description: Option<String>,
    content_rating: String,
    visibility: String,
    created_at: String,  // ISO 8601
    updated_at: String,
}
```

**Org-scoped routes (`/orgs/:org_id_or_slug/characters`):**

| Handler | Method | Path | Auth | Description |
|---------|--------|------|------|-------------|
| `create_character` | POST | `/orgs/{org}/characters` | Yes (MANAGE_PROFILE) | Create character. After creation, auto-create character tag via `tag_service.create_entity_tag(TagCategory::Character, EntityKind::Character, char.id, &name)` — best-effort, log on failure. |
| `list_characters` | GET | `/orgs/{org}/characters` | Optional | List characters with visibility filtering. Parse query params for filters. |

**Top-level routes (`/characters`):**

| Handler | Method | Path | Auth | Description |
|---------|--------|------|------|-------------|
| `get_character` | GET | `/characters/{id}` | Optional | Get character by UUID. Visibility-gated. |
| `update_character` | PATCH | `/characters/{id}` | Yes (MANAGE_PROFILE) | Update character fields. |
| `delete_character` | DELETE | `/characters/{id}` | Yes (MANAGE_PROFILE) | Soft delete. |

**Router structure:**
```rust
// Org-scoped (nested under /organizations in routes.rs)
pub fn org_router() -> Router<SharedState> {
    Router::new()
        .route("/", post(create_character).get(list_characters))
}

// Top-level
pub fn router() -> Router<SharedState> {
    Router::new()
        .route("/{id}", get(get_character).patch(update_character).delete(delete_character))
}
```

### Modified: `api/src/routes.rs`

Add character routes:
```rust
.nest("/characters", characters::router())
```

And nest org-scoped character routes under organizations — this requires modifying `organizations.rs` to nest `/organizations/{id_or_slug}/characters` → `characters::org_router()`. Pattern: add `.nest("/{id_or_slug}/characters", characters::org_router())` to the org router.

### Modified: `api/src/state.rs`

Add `character_service: CharacterService` to `AppState`.

### Modified: `api/src/main.rs`

Instantiate `SqlxCharacterRepository` from pool, build `CharacterService` with `character_repo` + `org_repo` + `member_repo`, add to `AppState`.

### Modified: `api/src/error.rs`

Add `From<CharacterServiceError> for AppError`:
- `NotFound` → `AppError::NotFound`
- `Forbidden` → `AppError::Forbidden`
- `InvalidInput(msg)` → `AppError::BadRequest`
- `Internal(msg)` → `AppError::Internal`

### New: `api/src/tests/mock_characters.rs`

Mock `CharacterRepository` following the existing mock pattern (`Mutex<Vec<Character>>`).

### Modified: `api/src/tests/mod.rs`

Add `mod mock_characters;`

### Modified: `api/src/tests/test_state.rs`

Wire `MockCharacterRepo` into `test_app_state()`, build `CharacterService`.

---

## Test Plan

### Unit Tests (application)

| Test | What it verifies |
|------|-----------------|
| `create_character_succeeds` | Valid permissions + input → Character returned |
| `create_character_without_permission_fails` | User without MANAGE_PROFILE → Forbidden |
| `create_character_non_member_fails` | Non-member → Forbidden |
| `create_character_empty_name_fails` | Empty/whitespace name → InvalidInput |
| `create_character_name_too_long_fails` | >256 chars → InvalidInput |
| `get_character_public_visible` | Public character returned to anonymous viewer |
| `get_character_unlisted_visible` | Unlisted character returned to anonymous viewer |
| `get_character_private_member_visible` | Private character visible to org member |
| `get_character_private_nonmember_hidden` | Private character → NotFound for non-member |
| `get_character_controlled_treated_as_private` | Controlled → same behavior as private |
| `list_characters_member_sees_all` | Org member sees all visibilities |
| `list_characters_nonmember_sees_public_only` | Non-member sees only public |
| `update_character_succeeds` | Valid permissions → updated |
| `update_character_forbidden` | Non-member → Forbidden |
| `delete_character_succeeds` | Soft delete → character not found |

### E2E Tests (API)

| Test | What it verifies |
|------|-----------------|
| `create_character_returns_201` | POST /orgs/:slug/characters → 201 + CharacterResponse |
| `create_character_without_token_returns_401` | Auth guard |
| `create_character_missing_content_rating_returns_400` | Required field validation |
| `list_characters_returns_200` | GET /orgs/:slug/characters → 200 + array |
| `get_character_by_id_returns_200` | GET /characters/:id → 200 + CharacterResponse |
| `get_nonexistent_character_returns_404` | GET /characters/:random_uuid → 404 |
| `update_character_returns_200` | PATCH /characters/:id → 200 + updated fields |
| `delete_character_returns_204` | DELETE /characters/:id → 204 |
| `deleted_character_returns_404` | GET /characters/:id after delete → 404 |
| `org_resolution_by_slug` | POST /orgs/my-studio/characters works |
| `org_resolution_by_uuid` | POST /orgs/:uuid/characters works |
| `character_tag_auto_created` | After creation, GET /tags/entity/character/:id returns the identity tag |

---

## Files Summary

| File | Change |
|------|--------|
| `application/src/character/mod.rs` | New — module declaration |
| `application/src/character/service.rs` | New — CharacterService with 5 methods |
| `application/src/lib.rs` | Add `pub mod character;` |
| `api/src/routes/characters.rs` | New — 5 route handlers + request/response types |
| `api/src/routes.rs` | Nest `/characters` router |
| `api/src/routes/organizations.rs` | Nest `/{id_or_slug}/characters` under org routes |
| `api/src/state.rs` | Add `character_service` to AppState |
| `api/src/main.rs` | Instantiate CharacterService |
| `api/src/error.rs` | Add `From<CharacterServiceError> for AppError` |
| `api/src/tests/mock_characters.rs` | New — MockCharacterRepo |
| `api/src/tests/mod.rs` | Add mock_characters module |
| `api/src/tests/test_state.rs` | Wire MockCharacterRepo |

## Verification

```bash
cargo test --workspace          # All tests pass
cargo build --workspace         # No warnings
just dev                        # Start server
# Then test manually:
# POST /orgs/my-studio/characters with JSON body → 201
# GET /characters/:id → 200
# PATCH /characters/:id → 200
# DELETE /characters/:id → 204
```
