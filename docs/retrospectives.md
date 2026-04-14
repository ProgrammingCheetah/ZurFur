# Retrospectives

## Shared Guidance (Read This First)

### Project Understanding
- Read `CLAUDE.md` at repo root for architecture, commands, and conventions.
- Read `design/features/OVERVIEW.md` for the feature dependency map and build order.
- Read the specific feature's `design/features/XX-name/README.md` before starting work.
- The critical path is: Auth (1) → Identity/Profile (2) → Tags (3) → TOS (11) → Commission Engine (4) → Financial (5) → Notifications (10).
- Read `design/glossary.md` for domain concepts, architectural principles, and schema conventions.

### Branching & PR Workflow
- Branch structure: `main` ← `feature/auth` ← `feature/auth_submodule-name`
- The feature branch (e.g., `feature/auth`) is the base. Submodule branches use underscore: `feature/auth_submodule-name`.
- Git ref conflict: you CANNOT have `feature/auth` and `feature/auth/something` — use `feature/auth_something` instead.
- After each submodule, push, create PR via `gh pr create`, then STOP and wait for user review.
- After merge: checkout feature branch, pull, delete merged submodule branch, continue with next submodule.
- Keep backend and frontend commits separate unless tightly coupled.

### Commit Discipline
- Commits must be clean, descriptive, and logically grouped.
- Layer commits bottom-up when possible: domain → persistence → application → api.
- Test commits can follow implementation commits in the same PR.
- Always include `Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>`.

### Testing Requirements
- Every submodule PR MUST include extensive tests for both backend and frontend.
- Use `#[cfg(test)] mod tests` within each crate, not separate test crates.
- Mock repositories via trait implementations for unit tests (use `#[async_trait]` on mocks to match trait definitions that use `#[async_trait]`).
- Use `axum-test` for API integration tests with mock `AppState`.
- Rust 2024 edition: traits defined with `#[async_trait]` require `#[async_trait]` on mock impls too — bare `async fn` in impl causes lifetime mismatches.

### Automated Reviewer Patterns (Copilot/Cursor Bugbot)
Automated reviewers re-review the full diff on every push, regenerating comments on already-fixed issues. To prevent this:
- **Document architectural decisions inline** with comments explaining WHY, not just what.
- Every intentional deviation from convention needs a comment (e.g., POST instead of GET for OAuth callback, plaintext storage as dev-only).
- Copilot will repeatedly flag: plaintext secrets in DB, hardcoded config, missing rate limiting, missing structured logging. Add TODO comments acknowledging these as tracked items.
- Don't chase zero comments — after 2-3 rounds of fixes, remaining comments are almost always repeats. Move on.

### Common Pitfalls
- `env::set_var` / `env::remove_var` are `unsafe` in Rust 2024 edition.
- `v[..N]` byte slicing on strings can panic on non-ASCII — use `split_once()` or `get(..N)`.
- HTTP `Authorization` header scheme is case-insensitive per RFC 6750.
- `DELETE ... RETURNING` is the correct pattern for atomic take (prevents TOCTOU race on refresh tokens).
- `gen_random_uuid()` is built-in since PostgreSQL 13 — no pgcrypto needed (we target PG16).
- When deriving public keys from private keys, use `to_public()` — never store private key material in fields labeled "public".

---

## Feature 1: AT Protocol OAuth Authentication

**PR #1** — `feature/auth_infra`: Infrastructure (Docker, Nginx, Justfile, .env.example, config module)
**PR #2** — `feature/auth_oauth-backend`: Full OAuth backend (domain, persistence, application, API, tests)

### What Went Well
- Layered commit structure (domain → persistence → application → api) made review clear.
- Mock repository pattern worked cleanly for unit testing AuthService without a DB.
- `axum-test` integration tests caught real routing issues.
- Atomic `take_by_hash` (DELETE...RETURNING) elegantly solved the refresh token TOCTOU race.

### What Didn't Go Well
- **Initial commit was too large.** All uncommitted Feature 1 work was sitting on main. Had to stash and selectively unstash. In future: commit frequently to feature branches, don't accumulate.
- **Tests were an afterthought.** User had to explicitly ask for tests after the first submodule was already merged. Now a standing requirement.
- **Automated reviewer churn.** Copilot generated 11+ comments per review, many repeats. Spent ~4 rounds fixing issues that kept being re-flagged. Solution: add architectural decision comments upfront, and after 2-3 fix rounds, declare remaining comments as repeats and move on.
- **Tab-delimited state storage.** Initially stored `"did\thandle"` as a string in OAuthStateStore — brittle and correctly flagged. Fixed by introducing `OAuthStateData` struct. Lesson: design clean APIs from the start; hacks get caught.
- **Bearer parsing bug.** My own fix (`v[..7]`) introduced a panic on non-ASCII. Always use `split_once()` or bounds-checked slicing for string parsing.

### Key Security Fixes Applied
1. Private key stored in public key field → `to_public()` derivation
2. Double identity resolution → single resolution, pass Document
3. TOCTOU race on refresh → atomic `take_by_hash`
4. Tokens exposed via GET → changed to POST
5. Handle always None → propagated from identity resolution
6. DID treated as handle → check `starts_with("did:")`
7. Stale handle on re-login → `update_handle` when changed
8. jwt_config exposed → made private, added `verify_access_token()`
9. OAuth request deleted before exchange → moved to after success
10. No TTL on state store → 10-min expiry with eviction
11. Empty handle → 400 → early validation
12. InternalError leaks details → generic client messages, log server-side
13. P-256 key length not validated → 32-byte assert on startup
14. Bearer case-sensitive → case-insensitive via `split_once`
15. DID format not validated → require `did:` prefix
16. expires_in unbounded → clamped to 1s–1yr
17. DidMismatch → 401 → changed to 502 (server-side issue)

### Tracked for Future (Not This PR)
- Application-level encryption for AT Protocol tokens at rest
- Structured logging (tracing crate) replacing eprintln
- CORS origins from environment variables
- Rate limiting on auth endpoints
- `iss` claim validation in OAuth callback (mix-up attack prevention)
- Redis-backed OAuthStateStore for production

---

## Feature 2: Identity & Profile Engine (Phase 1)

**PR #5** — `feature/identity-profile_domain-persistence`: Domain entities, repository traits, migration, SQLx implementations
**PR #6** — `feature/identity-profile_application-api`: Application services, API routes, tests, personal org auto-creation

### Key Architectural Decisions

1. **Org-centric identity model**: Every user gets a personal org on signup. The personal org IS the user's public profile. All roles, titles, bios, and capabilities are expressed through org membership — User entity stays atomic (identity only). This replaces the original `is_artist` flag approach.

2. **User is atomic**: User holds only `{id, did, handle, email, username}`. No feature flags, bios, or roles were added. This principle must be maintained as new features are added. All capabilities attach to organizations, not users.

3. **Two-tier data architecture**: Public identity data (orgs, profiles, memberships) will eventually live on the user's AT Protocol PDS. Private transaction data (commissions, payments, disputes) stays in Zurfur's PostgreSQL. Repository trait abstraction enables swapping from SqlxRepo to PdsRepo without touching application or API layers.

4. **display_name is nullable**: Personal orgs store NULL — resolved from owner's username/handle at the API layer. Avoids duplicating handle data that syncs from Bluesky.

5. **PG enum for content_rating, TEXT + CHECK for commission_status**: content_rating is stable; commission_status will grow (paused, by_request, etc.).

6. **Permissions as BIGINT bitfield**: `ALL = u64::MAX` future-proofs new bit positions without migration. Faster than JSONB.

7. **No transactions across repos**: `create_org` + `add_owner` are two separate repo calls. MVP trade-off — UoW pattern deferred to Feature 4.

8. **Slug validation at service layer**: Format rules + reserved word list enforced in Rust, DB only enforces uniqueness. Partial unique index on slug (excludes soft-deleted rows).

9. **Partial unique index for personal orgs**: `uq_organizations_personal` enforces at most one non-deleted personal org per user at the DB level.

10. **`sqlx_utils` shared module**: `is_unique_violation()` and `violated_constraint()` extracted to avoid duplication across repositories.

11. **Route pattern `/{id_or_slug}`**: GET/PUT/DELETE on `/organizations/:param` share one route — handler disambiguates by attempting UUID parse first, then slug lookup.

### What Went Well
- Org-centric model emerged from design discussion and is much more flexible than the original `is_artist` flag approach
- Two-tier data architecture (AT Protocol + PostgreSQL) was validated by researching how Bluesky handles DMs
- Repository trait abstraction makes future PDS integration a persistence-layer-only change
- Permission bitfield is clean and extensible
- Personal org auto-creation with self-healing for returning users

### What Didn't Go Well
- Route conflict between `/{slug}` and `/{id}` wasn't caught until tests ran — Axum treats them as the same wildcard pattern. Fixed by merging into single `/{id_or_slug}`.
- Mock repos are duplicated across application tests and API tests (same pattern as Feature 1). Consider a shared test-utils crate if this continues to grow.
- `NoOpProfileRepo` in auth service is a code smell — needed because `OrganizationService::create_personal_org` takes a profile repo it doesn't use. Could be refactored with a standalone function instead.

### Tracked for Future (Not This PR)
- AT Protocol Lexicon definitions for public org/profile data (Phase 1.5)
- PDS write-through and indexer (Phase 1.5)
- Onboarding "What are you?" flow that sets initial title on personal org
- Team org invite/join flows
- Org focus tags (art, fursuits, coding)
- UoW/transaction pattern for multi-repo operations
- Shared test-utils crate for mock repositories

---

## Feature 2: Identity & Profile Engine (Phase 2)

**PR #10** — `feature/identity-profile_application-api`: Role enum refactor, OnboardingService, FeedService, API routes

### Key Changes
1. **Role enum refactor**: Replaced free-text `role` + `is_owner: bool` with `Role` enum (Owner/Admin/Mod/Member). `is_owner()` is a derived method. "Artist" is a title, not a role.
2. **OnboardingService**: Idempotent "what are you?" wizard creating system feeds (updates, gallery, [commissions]) on personal org. Separate from org creation.
3. **FeedService**: Feed CRUD with permission checks via org membership. Post to feed creates item + elements.
4. **`NoOpProfileRepo` removed** in Feature 3 (was a code smell flagged in retrospective above).

---

## Feature 3: Tag Taxonomy & Attribution

**PR #13** — `feature/tag-taxonomy_schema-cleanup`: Schema cleanup (singular tables, drop profile, JSONB prefs)
**PR #14** — `feature/tag-taxonomy_domain-persistence`: Tag + EntityTag domain and persistence
**PR #15** — `feature/tag-taxonomy_application-api`: TagService, API routes, org tag auto-creation

### Key Architectural Decisions

1. **Aggregates are decoupled at the database level.** No aggregate table references another. All cross-aggregate relationships live in junction tables (`entity_feed`, `entity_tag`, `organization_member`, `feed_subscription`). Application code composes them freely. This is the platform's most important architectural constraint.

2. **Tag is fully decoupled.** No `entity_id` field — a tag doesn't know what it's attached to. The connection lives entirely in `entity_tag`. An org's identity tag is just a tag that happens to be auto-created and attached via `entity_tag`.

3. **Tag category is a PG ENUM** (organization, character, metadata, general). Describes what the tag IS, not how it's connected. Defaults to general. If faceted search later needs finer granularity (species, art_style), the enum gains a new value.

4. **Bio is a feed.** `organization_profiles` table dropped. The org's bio lives in a "bio" system feed — edits are new feed items, giving version history for free.

5. **User preferences as JSONB.** Single `settings` column instead of typed columns. Extensible without migration.

6. **`created_by` dropped from organization.** Creator = owner member in `organization_member`. Aggregates don't reference each other.

7. **Singular table names.** Convention going forward. All tables renamed except `users` (PG reserved word).

8. **Attribution follows the artist, not the org.** Primary attribution = personal org tag (permanent, 1:1 with user). Studio org tag is supplementary. If artist leaves studio, their personal org tag stays.

9. **Org tag auto-creation at orchestration layer.** OrganizationService does NOT create tags. The API handler (create_org) and OnboardingService call TagService — same way OnboardingService creates feeds.

10. **`TaggableEntityType` is separate from `EntityType`.** `entity_feed` supports org/character/commission/user. `entity_tag` supports org/commission/feed_item/character/feed_element. Different sets, no coupling.

### What Went Well
- Design discussion before implementation led to significant simplifications: Tag went from 8 fields (`id, tag_type, entity_id, name, category, parent_id, usage_count, is_approved`) to 5 (`id, category, name, usage_count, is_approved`).
- The "no aggregate references another" principle emerged from iterative discussion and now governs all future design decisions. This is the most impactful outcome of the session.
- Glossary (`design/glossary.md`) created as a single reference document for domain concepts and conventions. Replaces scattered memory files for architectural knowledge.
- Domain relationship chart (`design/domain_relationships.md`) with ER diagram makes the architecture scannable.
- Schema cleanup (table renames, profile drop, JSONB prefs) was bundled with tag work — one migration covers both, avoiding two separate schema-breaking changes.
- Review feedback from Copilot was substantive and all 9 comments were valid. Fixed in one commit.

### What Didn't Go Well
- **Schema cleanup commit was massive.** 33 files changed — every SQL query in every repository needed a table name update. Mechanically simple but high blast radius. Could have been a separate PR, but bundling avoided two rounds of schema migration pain.
- **`find_personal_org` lost its DB constraint.** Dropping `created_by` eliminated the `uq_organizations_personal` unique index. The one-personal-org-per-user rule is now application-layer only. A bug could create duplicates.
- **Mock repository duplication continues.** MockOrgRepo exists in 4 files (org service tests, auth service tests, onboarding service tests, API test mocks). Each copy has to be updated when the trait changes. A shared test-utils crate would fix this.
- **No authorization on tag routes.** All tag endpoints accept any authenticated user. Update/delete/approve should be role-gated. Noted for when the permission system matures.
- **Org tag auto-creation not in auth flow.** Personal org tags are only created during onboarding or when a non-personal org is created via API. Personal orgs created during the auth `complete_login` flow don't get their org tag yet — needs a follow-up.

### Breaking Changes (API)
- `PUT /organizations/:id/profile` removed (bio is a feed now)
- `PUT /users/me/preferences` body changed from `{max_content_rating: "sfw"}` to `{settings: {...}}`
- `OrganizationService::new()` signature: 3→2 args (dropped `profile_repo`)
- `UserService::new()` signature: 5→4 args (dropped `org_profile_repo`)
- `OrganizationRepository::create()` signature: dropped `created_by` parameter

### Tracked for Future (Not This PR)
- Org tag auto-creation in AuthService `complete_login` (personal org flow)
- Authorization checks on tag routes (role-gated approve/update/delete)
- Character tag auto-creation (when Character entity is built, Feature 2 Phase 3)
- Tag synonyms/aliases for search (Phase 2)
- Shared test-utils crate for mock repositories (systemic issue since Feature 1)
- `find_personal_org` DB-level uniqueness (consider partial unique index on `organization_member`)
