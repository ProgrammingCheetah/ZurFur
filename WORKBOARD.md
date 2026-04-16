# Zurfur — Workboard

> **Updated 2026-04-15**

Parallel orchestration board for Claude instances. Each **unit** is a self-contained task an instance picks up, implements, and PRs back.

## Branch Strategy (GitFlow)

```
main                    ← stable, merges from develop at phase gates
  └── develop           ← integration branch, all feature PRs target this
        ├── feature/entity-interfaces      (Phase 1A — in flight)
        ├── feature/integration-tests      (Phase 1B — in flight)
        ├── feature/openapi-infra          (Phase 2A)
        ├── feature/characters-domain      (Phase 2B)
        ├── feature/full-test-suite        (Phase 2C)
        └── ...
```

**Rules:**
- Every unit gets a `feature/` branch off `develop`
- Every unit PRs back to `develop`
- Phase gate: all units in Phase N merged to `develop` → merge `develop` to `main`
- Units within a phase MUST NOT touch the same files (no merge conflicts)

## How to Spin Up an Instance

1. Create worktree: `git worktree add ../zurfur-<name> -b feature/<name> develop`
2. Start Claude in that directory
3. Tell it: "Read `design/infrastructure/<area>/DESIGN.md` (or `design/features/<area>/DESIGN.md`) and implement it. PR to `develop` when done."
4. When done, merge PR, remove worktree: `git worktree remove ../zurfur-<name>`

---

## Phase 0 — Foundation (DONE)

All merged to `main`.

| Unit | Feature | Status |
|------|---------|--------|
| 0A | AT Protocol OAuth | Done |
| 0B | Identity & Org Engine (Phase 1-2) | Done |
| 0C | Tag Taxonomy & Attribution | Done |
| 0D | Transaction Support (UoW) | Done |

---

## Phase 1 — Entity System & Test Foundation (IN FLIGHT)

**Gate:** Both units merge to `develop` before Phase 2 starts.

| Unit | Branch | Design Doc | Files Touched | Status |
|------|--------|-----------|---------------|--------|
| 1A | `feature/entity-interfaces` | `design/infrastructure/entity-interfaces/DESIGN.md` | `domain/src/*`, `persistence/src/repositories/*`, `application/src/*/service.rs`, `api/src/routes/*.rs`, `api/src/tests/*`, new migration | In flight |
| 1B | `feature/integration-tests` | `design/infrastructure/testing/DESIGN.md` | `persistence/Cargo.toml`, `persistence/tests/*` (new), `Justfile`, `CLAUDE.md` | In flight |

**No overlap:** 1A modifies existing domain/persistence/api code. 1B creates new test files only. Safe to run in parallel.

**Note:** 1B's smoke tests use the current `EntityType`/`TaggableEntityType`. After 1A merges, tests will need updating to `EntityKind`. This is expected — 1B sets up infrastructure, Phase 2C updates the tests.

---

## Phase 2 — OpenAPI + Characters + Full Tests (READY AFTER PHASE 1)

**Gate:** All 3 units merge to `develop` before Phase 3 starts.

| Unit | Branch | Design Doc | Files Touched | Depends On |
|------|--------|-----------|---------------|------------|
| 2A | `feature/openapi-infra` | `design/infrastructure/openapi/DESIGN.md` (to be written) | `Cargo.toml` (root), `api/Cargo.toml`, `api/src/openapi/*` (new), `api/src/error.rs`, `api/src/lib.rs` | Phase 1 |
| 2B | `feature/characters-domain` | `design/features/02-identity-profile/PHASE3_DESIGN.md` (needs update) | `domain/src/character.rs` (new), `domain/src/lib.rs`, `persistence/src/repositories/character_repository.rs` (new), `persistence/src/repositories/mod.rs`, `persistence/src/lib.rs`, new migration | Phase 1 (EntityKind) |
| 2C | `feature/full-test-suite` | `design/infrastructure/testing/DESIGN.md` | `persistence/tests/*` (expand existing) | Phase 1 (both) |

**No overlap:** 2A touches API infra files. 2B touches domain + persistence (new files mostly). 2C touches only test files. All three can run simultaneously.

**2A scope:** Add utoipa + utoipa-scalar dependencies, create `/api/docs` and `/api/docs/openapi.json` endpoints, standardize error response schemas, create the `openapi` module. Does NOT annotate routes yet.

**2B scope:** Character entity, migration, repository only. No service layer, no API routes. This is the "domain + persistence" half. The character gallery is a filtered view of the org's feed (not a separate feed) per the entity interfaces design decisions.

**2C scope:** Expand the integration test infrastructure from Phase 1B with the full ~85 test suite. Update tests to use `EntityKind`. Add CHECK constraint validation tests for all 8 entity kinds.

---

## Phase 3 — API Completion + OpenAPI Annotations (READY AFTER PHASE 2)

| Unit | Branch | Design Doc | Files Touched | Depends On |
|------|--------|-----------|---------------|------------|
| 3A | `feature/openapi-annotations-auth-users` | (section in OpenAPI design doc) | `api/src/routes/auth.rs`, `api/src/routes/users.rs`, `api/src/routes/onboarding.rs` | 2A |
| 3B | `feature/openapi-annotations-orgs-feeds` | (section in OpenAPI design doc) | `api/src/routes/organizations.rs`, `api/src/routes/feeds.rs` | 2A |
| 3C | `feature/openapi-annotations-tags` | (section in OpenAPI design doc) | `api/src/routes/tags.rs`, `api/src/routes/helpers.rs` | 2A |
| 3D | `feature/characters-api` | (section in Characters design doc) | `application/src/character/*` (new), `api/src/routes/characters.rs` (new), `api/src/state.rs`, `api/src/main.rs`, `api/src/routes.rs`, `api/src/error.rs` | 2B |
| 3E | `feature/characters-tests` | (section in testing design doc) | `persistence/tests/character_*.rs` (new), `api/src/tests/mock_characters.rs` (new), `api/src/tests/character_routes.rs` (new) | 3D |

**Overlap analysis:**
- 3A, 3B, 3C: each touches different route files. **Safe in parallel.**
- 3D: touches `api/src/state.rs`, `api/src/main.rs`, `api/src/routes.rs`, `api/src/error.rs` — these are shared files. **Cannot run with 3A-3C if they also touch these.** But 3A-3C only touch route handler files, not state/main/routes.rs. **Safe in parallel.**
- 3E: depends on 3D (needs character routes to exist). **Sequential after 3D.**

**Maximum parallelism: 4 instances** (3A + 3B + 3C + 3D simultaneously).

---

## Phase 4 — TOS + Commission Prep (READY AFTER PHASE 3)

| Unit | Branch | Design Doc | Files Touched | Depends On |
|------|--------|-----------|---------------|------------|
| 4A | `feature/org-tos` | `design/features/11-artist-tos/DESIGN.md` (to be written) | `domain/src/tos.rs` (new), persistence, application, api | Phase 3 |
| 4B | `feature/characters-openapi` | (section in OpenAPI design doc) | `api/src/routes/characters.rs` (annotations only) | 3D + 3A pattern |

---

## Design Docs Needed

| Doc | For Unit | Status |
|-----|----------|--------|
| `design/infrastructure/entity-interfaces/DESIGN.md` | 1A | Done |
| `design/infrastructure/testing/DESIGN.md` | 1B, 2C | Done |
| `design/infrastructure/openapi/DESIGN.md` | 2A, 3A-3C | Done |
| `design/features/02-identity-profile/PHASE3_DESIGN.md` | 2B, 3D | Done (updated for gallery shift + EntityKind) |
| `design/features/11-artist-tos/DESIGN.md` | 4A | Not started |

---

## Instance Capacity

| Phase | Max Parallel Instances | Notes |
|-------|----------------------|-------|
| 1 | 2 | Entity interfaces + test infra |
| 2 | 3 | OpenAPI infra + characters domain + full tests |
| 3 | 4 | 3 OpenAPI annotation units + characters API |
| 4 | 2 | TOS + characters OpenAPI |
