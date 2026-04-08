# Retrospectives

## Shared Guidance (Read This First)

### Project Understanding
- Read `CLAUDE.md` at repo root for architecture, commands, and conventions.
- Read `design/features/OVERVIEW.md` for the feature dependency map and build order.
- Read the specific feature's `design/features/XX-name/README.md` before starting work.
- The critical path is: Auth (1) → Identity/Profile (2) → Artist TOS (10) → Commission Engine (3) → Financial (4) → Notifications (9).

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
