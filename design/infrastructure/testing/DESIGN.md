# Testing Infrastructure — Design Document

> **Created 2026-04-15**

## Context

Zurfur has 126 tests across 4 crates, but significant gaps exist:
- **0 database integration tests** — no `sqlx::test`, no real PostgreSQL testing
- **14 repository implementations untested** — all persistence layer code is untested
- **Missing HTTP route tests** — feeds and onboarding routes have no API tests
- **No domain entity validation tests** — 7 entities have no tests

This document defines every test needed to close these gaps and establishes the testing patterns going forward.

### Current Test Inventory

| Layer | Tests | Coverage |
|-------|-------|----------|
| API (routes) | 30 `#[tokio::test]` | Auth, users, orgs, tags tested. **Feeds and onboarding missing.** |
| Application (services) | 50 `#[test]` + `#[tokio::test]` | All 6 services tested with mocks. |
| Domain (entities) | 31 `#[test]` | Enum round-trips, permissions. **Entity structs untested.** |
| Shared (JWT, config) | 8 `#[test]` | Good coverage. |
| Persistence (repos) | 7 `#[tokio::test]` | Only OAuth in-memory store. **All 14 SQLx repos untested.** |

### Codebase Architecture

```
backend/crates/
  domain/        # Pure entities, traits, errors — unit tests only
  shared/        # JWT, config — unit tests only
  persistence/   # SQLx repos, migrations — integration tests (sqlx::test)
  application/   # Services — unit tests with mocks
  api/           # Routes, handlers — e2e tests with TestServer
```

Strict dependency direction: `api` → `application` + `persistence` → `domain` + `shared`.

### Test Infrastructure

**Existing patterns:**
- Mock repos use `Mutex<Vec<T>>` for thread-safe in-memory storage
- `axum_test::TestServer` for HTTP route testing
- `test_app_state()` builder wires all mocks into `AppState`
- `issue_test_jwt()` creates valid tokens for authenticated tests

**New infrastructure needed:**
- `sqlx::test` for database integration tests in `persistence/tests/`
- Shared test helpers for creating entities (users, orgs, tags, feeds)

---

## Testing Approach

### `sqlx::test` Setup

`sqlx::test` creates a fresh PostgreSQL database per test, runs all migrations, and drops the DB after. It requires `DATABASE_URL` pointing to a PostgreSQL instance.

Add to `persistence/Cargo.toml` under `[dev-dependencies]`:
```toml
sqlx = { workspace = true, features = ["runtime-tokio", "postgres"] }
```

Test files go in `persistence/tests/` as integration tests:

```rust
#[sqlx::test(migrations = "migrations")]
async fn test_name(pool: PgPool) {
    // pool is connected to a fresh, migrated database
}
```

### Shared Test Helpers

Create `persistence/tests/common/mod.rs` with helper functions for creating test entities:

```rust
pub async fn create_test_user(pool: &PgPool) -> User { ... }
pub async fn create_test_org(pool: &PgPool, owner_id: Uuid) -> Organization { ... }
pub async fn create_test_feed(pool: &PgPool, entity_kind: EntityKind, entity_id: Uuid) -> Feed { ... }
pub async fn create_test_tag(pool: &PgPool, category: TagCategory, name: &str) -> Tag { ... }
```

These directly insert via SQLx (not through repos) to avoid coupling test setup to the code under test.

---

## Test Plan — Domain Crate

### Existing (31 tests — keep as-is)

All enum round-trip tests, permission bitfield tests, role tests, content rating ordering. These remain unchanged.

### New Unit Tests

File: `domain/src/entity.rs` (new, part of entity interfaces work)

| Test | What it verifies |
|------|-----------------|
| `entity_kind_round_trip` | All 8 `EntityKind` variants: `as_str()` → `from_str()` |
| `entity_kind_from_str_unknown` | Unknown strings return `None` |
| `entity_kind_display_values` | String values match DB CHECK constraints: `"user"`, `"org"`, `"character"`, `"commission"`, `"feed"`, `"tag"`, `"feed_item"`, `"feed_element"` |

File: `domain/src/user.rs`

| Test | What it verifies |
|------|-----------------|
| `user_entity_kind` | `User` returns `EntityKind::User` from `entity_kind()` |
| `user_entity_id` | `User` returns correct UUID from `id()` |
| `user_is_authorable` | `User` returns `AuthorType::User` from `author_type()` |
| `user_taggable_validate_default` | Default `validate_tag()` returns `Ok(())` |

File: `domain/src/organization.rs`

| Test | What it verifies |
|------|-----------------|
| `org_entity_kind` | `Organization` returns `EntityKind::Org` |
| `org_entity_id` | `Organization` returns correct UUID from `id()` |
| `org_is_authorable` | `Organization` returns `AuthorType::Org` |

File: `domain/src/feed.rs`

| Test | What it verifies |
|------|-----------------|
| `feed_entity_kind` | `Feed` returns `EntityKind::Feed` |
| `feed_not_authorable` | `Feed` does NOT implement `Authorable` (compile-time — negative trait bound test) |

File: `domain/src/tag.rs`

| Test | What it verifies |
|------|-----------------|
| `tag_entity_kind` | `Tag` returns `EntityKind::Tag` |

File: `domain/src/feed_item.rs`

| Test | What it verifies |
|------|-----------------|
| `feed_item_entity_kind` | `FeedItem` returns `EntityKind::FeedItem` |

File: `domain/src/feed_element.rs`

| Test | What it verifies |
|------|-----------------|
| `feed_element_entity_kind` | `FeedElement` returns `EntityKind::FeedElement` |

File: `domain/src/entity.rs` (trait bounds compile test)

| Test | What it verifies |
|------|-----------------|
| `trait_bounds_compile` | Functions accepting `impl Taggable`, `impl FeedOwnable`, `impl Authorable` accept correct types. Compile-time verification. |

---

## Test Plan — Persistence Crate (Integration Tests)

All tests use `#[sqlx::test(migrations = "migrations")]` with a real PostgreSQL database.

### File: `persistence/tests/common/mod.rs`

Shared helpers — not tests themselves.

### File: `persistence/tests/user_repository.rs`

| Test | What it verifies |
|------|-----------------|
| `create_user_from_atproto` | Insert user with DID, handle, email. Verify all fields round-trip. |
| `find_user_by_id` | Create user, find by UUID. |
| `find_user_by_did` | Create user, find by DID string. |
| `find_user_by_email` | Create user, find by email. |
| `find_nonexistent_user_returns_none` | Random UUID returns `None`. |
| `update_handle` | Change handle, verify updated. |
| `mark_onboarding_completed` | Set `onboarding_completed_at`, verify non-null. |
| `create_duplicate_did_fails` | Second user with same DID fails (unique constraint). |

### File: `persistence/tests/organization_repository.rs`

| Test | What it verifies |
|------|-----------------|
| `create_org` | Insert org with slug, display_name, is_personal. Verify fields. |
| `create_org_with_owner` | Action method: creates org + owner member atomically. Verify both rows. |
| `create_org_with_owner_rollback` | If member creation fails, org is NOT created (transaction atomicity). |
| `find_org_by_id` | Create org, find by UUID. |
| `find_org_by_slug` | Create org, find by slug. |
| `find_personal_org` | Create personal org for user, find via `find_personal_org(user_id)`. |
| `update_display_name` | Change display name, verify updated. |
| `soft_delete_org` | Set `deleted_at`, verify org not found by normal queries. |
| `duplicate_slug_fails` | Second org with same slug fails (unique constraint). |

### File: `persistence/tests/organization_member_repository.rs`

| Test | What it verifies |
|------|-----------------|
| `add_member` | Add user to org with role, title, permissions. Verify fields. |
| `find_by_org_and_user` | Lookup specific membership. |
| `list_by_org` | All members of an org. |
| `list_by_user` | All orgs a user belongs to. |
| `update_role_and_title` | Change role from member to admin. |
| `update_permissions` | Change permission bitfield. |
| `remove_member` | Delete membership row. |
| `add_duplicate_member_fails` | Same user+org fails (unique constraint). |

### File: `persistence/tests/feed_repository.rs`

| Test | What it verifies |
|------|-----------------|
| `create_feed` | Insert feed with slug, display_name, feed_type. Verify fields. |
| `create_and_attach_feed` | Action method: creates feed + entity_feed atomically. |
| `create_and_attach_rollback` | If entity_feed fails, feed is NOT created (transaction). |
| `find_feed_by_id` | Create feed, find by UUID. |
| `update_feed` | Change display_name and description. |
| `soft_delete_custom_feed` | Custom feed can be soft deleted. |
| `soft_delete_system_feed_fails` | System feed deletion rejected. |
| `list_by_ids` | Fetch multiple feeds by UUID array. |
| `duplicate_slug_per_entity_allowed` | Two different entities can have feeds with same slug. |

### File: `persistence/tests/entity_feed_repository.rs`

| Test | What it verifies |
|------|-----------------|
| `attach_feed_to_entity` | Create entity_feed junction row. |
| `find_by_feed_id` | Lookup entity that owns a feed. |
| `list_by_entity` | All feeds owned by an entity. |
| `detach_feed` | Remove junction row. |
| `attach_already_attached_fails` | Double-attach rejected (PK constraint). |
| `all_entity_kinds_accepted` | All 8 `EntityKind` values pass the CHECK constraint. |
| `invalid_entity_type_rejected` | Unknown string rejected by CHECK constraint. |

### File: `persistence/tests/tag_repository.rs`

| Test | What it verifies |
|------|-----------------|
| `create_tag` | Insert tag with category, name, is_approved. Verify fields. |
| `create_and_attach_tag` | Action method: create tag + entity_tag + set usage_count=1 atomically. |
| `find_tag_by_id` | Create tag, find by UUID. |
| `find_by_name_and_category` | Exact name+category lookup. |
| `list_by_category` | Tags in category, ordered by usage_count desc. |
| `search_by_name` | Prefix search, case-insensitive. |
| `update_tag` | Change name and is_approved. |
| `increment_usage_count` | Atomic increment. |
| `decrement_usage_count` | Atomic decrement, floored at 0. |
| `delete_tag` | Hard delete. |
| `attach_and_increment` | Attach tag to entity, verify usage_count incremented. |
| `detach_and_decrement` | Detach tag from entity, verify usage_count decremented. |
| `duplicate_name_category_fails` | Same (name, category) rejected (unique constraint). |
| `attach_tag_twice_fails` | Same entity+tag rejected (PK constraint). |

### File: `persistence/tests/entity_tag_repository.rs`

| Test | What it verifies |
|------|-----------------|
| `attach_tag_to_entity` | Create entity_tag junction row. |
| `detach_tag` | Remove junction row. |
| `list_by_entity` | All tags for an entity. |
| `list_by_tag` | All entities with a specific tag (reverse lookup). |
| `attach_duplicate_fails` | Double-attach rejected (composite PK). |
| `all_entity_kinds_accepted` | All 8 `EntityKind` values pass the CHECK constraint. |
| `invalid_entity_type_rejected` | Unknown string rejected by CHECK constraint. |

### File: `persistence/tests/feed_item_repository.rs`

| Test | What it verifies |
|------|-----------------|
| `create_feed_item` | Insert item with feed_id, author_type, author_id. |
| `create_with_elements` | Action method: item + elements atomically. Verify positions. |
| `find_by_id` | Create item, find by UUID. |
| `list_by_feed` | Items in feed, paginated (limit/offset). |
| `delete_item` | Delete item. |
| `list_by_feed_empty` | Empty feed returns empty vec. |
| `list_by_feed_pagination` | Create 5 items, verify limit=2 offset=2 returns items 3-4. |

### File: `persistence/tests/feed_element_repository.rs`

| Test | What it verifies |
|------|-----------------|
| `create_element` | Insert element with type, content_json, position. |
| `find_by_id` | Create element, find by UUID. |
| `list_by_feed_item` | All elements for an item, ordered by position. |
| `update_content` | Change content_json. |
| `delete_element` | Delete element. |

### File: `persistence/tests/user_preferences_repository.rs`

| Test | What it verifies |
|------|-----------------|
| `get_default_preferences` | No row = default empty JSONB. |
| `set_and_get_preferences` | Store JSON, retrieve it. |
| `update_preferences` | Overwrite existing preferences. |

### File: `persistence/tests/feed_subscription_repository.rs`

| Test | What it verifies |
|------|-----------------|
| `create_subscription` | Subscribe org to feed with permission level. |
| `list_by_feed` | All subscribers to a feed. |
| `list_by_subscriber` | All feeds an org subscribes to. |
| `update_permission` | Change subscription permission level. |
| `delete_subscription` | Unsubscribe. |
| `duplicate_subscription_fails` | Same org+feed rejected. |

### File: `persistence/tests/default_role_repository.rs`

| Test | What it verifies |
|------|-----------------|
| `list_default_roles` | All 4 system roles exist after migration. |
| `find_by_name` | Find role by name string. |
| `hierarchy_ordering` | Owner (0) < Admin (1) < Mod (2) < Member (3). |

### File: `persistence/tests/migration_tests.rs`

| Test | What it verifies |
|------|-----------------|
| `migrations_run_cleanly` | All migrations apply to empty DB without error. |
| `migrations_idempotent` | Running migrations twice doesn't fail. |

---

## Test Plan — Application Crate

### Existing (50 tests — keep as-is)

Auth, org, user, feed, tag, onboarding service tests with mocks. All remain unchanged except for `EntityType`/`TaggableEntityType` → `EntityKind` updates.

### New Unit Tests

File: `application/src/tag/service.rs`

| Test | What it verifies |
|------|-----------------|
| `tag_service_calls_validate_tag` | Service calls `validate_tag()` before repo — mock entity returning `Err` prevents tagging. |
| `tag_service_calls_validate_untag` | Service calls `validate_untag()` before repo. |
| `tag_service_uses_entity_kind` | Service passes correct `EntityKind` discriminator to repo. |

File: `application/src/feed/service.rs`

| Test | What it verifies |
|------|-----------------|
| `feed_service_calls_validate_feed_creation` | Service calls `validate_feed_creation()` before repo. |
| `feed_service_uses_entity_kind` | Service passes correct `EntityKind` discriminator to repo. |

---

## Test Plan — API Crate (E2E Tests)

### Existing (30 tests — keep as-is)

Auth routes (11), user routes (8), tag routes (8), org routes (7). All remain unchanged.

### New E2E Tests

File: `api/src/tests/feed_routes.rs` (new)

| Test | What it verifies |
|------|-----------------|
| `create_feed_without_token_returns_401` | Auth guard on POST /orgs/:id/feeds. |
| `create_custom_feed_returns_created` | POST /orgs/:id/feeds creates feed and attaches to org. |
| `create_feed_without_permission_returns_403` | Non-member cannot create feeds. |
| `get_feed_returns_feed` | GET /feeds/:id returns feed details. |
| `get_nonexistent_feed_returns_404` | GET /feeds/:id with random UUID. |
| `update_feed_returns_updated` | PUT /feeds/:id changes display_name. |
| `delete_custom_feed_returns_204` | DELETE /feeds/:id soft-deletes custom feed. |
| `delete_system_feed_returns_400` | DELETE /feeds/:id rejects system feed deletion. |
| `post_to_feed_returns_created` | POST /feeds/:id/items creates item with elements. |
| `list_feed_items_returns_paginated` | GET /feeds/:id/items respects limit/offset. |
| `delete_feed_item_returns_204` | DELETE /feeds/:id/items/:item_id removes item. |

File: `api/src/tests/onboarding_routes.rs` (new)

| Test | What it verifies |
|------|-----------------|
| `complete_onboarding_without_token_returns_401` | Auth guard on POST /onboarding/complete. |
| `complete_onboarding_as_artist_returns_200` | Onboarding with artist role succeeds. |
| `complete_onboarding_as_commissioner_returns_200` | Onboarding with commissioner role succeeds. |
| `complete_onboarding_twice_is_idempotent` | Second call doesn't fail or recreate feeds. |
| `complete_onboarding_invalid_role_returns_400` | Unknown role string rejected. |

File: existing `api/src/tests/tag_routes.rs` (additions)

| Test | What it verifies |
|------|-----------------|
| `attach_tag_to_user_entity` | Tag attachment with `entity_type: "user"` works. |
| `attach_tag_to_feed_entity` | Tag attachment with `entity_type: "feed"` works. |
| `attach_tag_to_tag_entity` | Tag attachment with `entity_type: "tag"` works. |
| `list_tags_for_entity_returns_tags` | GET /tags/entity/:type/:id returns attached tags. |

File: existing `api/src/tests/organization_routes.rs` (additions)

| Test | What it verifies |
|------|-----------------|
| `update_org_returns_updated` | PUT /organizations/:slug updates display_name. |
| `delete_org_returns_204` | DELETE /organizations/:slug soft-deletes. |
| `delete_personal_org_returns_400` | Cannot delete personal org. |
| `list_members_returns_members` | GET /organizations/:id/members returns member list. |
| `add_member_returns_created` | POST /organizations/:id/members adds member. |
| `update_member_role` | PUT /organizations/:id/members/:user_id changes role. |
| `remove_member_returns_204` | DELETE /organizations/:id/members/:user_id removes member. |

---

## Summary

| Level | Existing | New | Total |
|-------|----------|-----|-------|
| Domain (unit) | 31 | ~15 | ~46 |
| Application (unit) | 50 | 5 | 55 |
| Persistence (integration) | 7 | ~85 | ~92 |
| API (e2e) | 30 | ~23 | ~53 |
| Shared (unit) | 8 | 0 | 8 |
| **Total** | **126** | **~128** | **~254** |

The biggest investment is persistence integration tests (~85 new tests). These close the most critical gap — SQL queries, CHECK constraints, transactions, and migrations running against a real PostgreSQL database.

---

## Implementation Notes

### For the implementation instance

1. **Start with `persistence/tests/common/mod.rs`** — the shared helpers. Every integration test needs these.
2. **Then `persistence/tests/entity_feed_repository.rs` and `persistence/tests/entity_tag_repository.rs`** — these verify the CHECK constraint expansion from the entity interfaces migration.
3. **Then remaining persistence tests** — one file per repository.
4. **Then new API tests** — feed routes and onboarding routes.
5. **Domain tests are quick wins** — entity trait implementation tests are simple.

### Environment

`DATABASE_URL` must point to a running PostgreSQL instance. The existing Docker Compose setup works:

```bash
just up                    # Start PostgreSQL
cargo test --workspace     # Runs all tests including sqlx::test
```

`sqlx::test` creates temporary databases automatically — no manual setup needed. Each test gets isolation.

### Branch

Create from `main` after the entity interfaces work is merged:

```bash
git checkout -b feature/testing-infrastructure
```
