# Session Retrospective — 2026-04-08/09

## What We Did

### Feature 2 Phase 1: Identity & Profile Engine (Code)

Built the org-centric identity model end-to-end:

**Submodule 1 (domain-persistence) — PR #5, merged:**
- 5 new domain entities: ContentRating, Organization, OrganizationMember (with Permissions bitfield), OrganizationProfile (CommissionStatus), UserPreferences
- Migration: `content_rating` PG enum, `organizations`, `organization_members`, `organization_profiles`, `user_preferences` tables
- 4 SQLx repository implementations
- Shared `sqlx_utils` module (extracted during Copilot review)
- 12 domain tests

**Submodule 2 (application-api) — PR #6, open for review:**
- `UserService`: get_my_profile (user + personal org + memberships), preferences CRUD
- `OrganizationService`: org CRUD, member management with permission checks, slug validation, personal org creation
- Personal org auto-creation in OAuth callback (with self-healing for returning users)
- 12 API endpoints: 3 user routes + 9 organization routes
- 22 application unit tests + 13 API integration tests = 35 new tests
- Restored e2e OAuth fixes (client-metadata.json endpoint, DNS resolver, CORS, vite config, nginx routes)
- All 79 workspace tests passing

### Architecture Evolution (Design)

During implementation, we redesigned the entire platform architecture through discussion:

1. **Org-centric identity** — User is atomic. All roles/capabilities via org membership. Personal org IS the user's profile.
2. **Feeds as universal content container** — Gallery, portfolio, ref sheets, activity, notifications = feed views. Feed items contain feed elements (text/image/file/event/embed).
3. **Headless commissions** — Artist-defined states via pipeline templates. Boards are projections. Commission card is a shell with add-on slots.
4. **Plugins are orgs** — Subscribe/react/post to feeds. No separate plugin API.
5. **Tags over columns** — Descriptive attributes are tags. Commission availability is a tag (`status:open`).
6. **Two-tier data** — AT Protocol PDS for public, PostgreSQL for private.
7. **Five root aggregates** — User, Organization, Feed, Commission, Tag.
8. **Feed elements** — Feed items split into items + elements for multi-content posts.
9. **Roles as free-text** with `default_roles` reference table (owner/admin/mod/member).
10. **Chat as plugin** — Commission chat is a built-in first-party plugin, not system auto-created.

### Design Document Updates

Revised all 15 design files (design_document.md + OVERVIEW.md + 14 feature READMEs, with tags later extracted to Feature 3):
- v2 design document with new Part 2.5 (Core Domain Architecture)
- Updated Mermaid diagrams throughout
- Three rounds of audits (automated + manual) with all issues resolved
- Verified 14/14 checks pass on final audit

## What Was Pushed

### Branches
- `feature/identity-profile` — base feature branch, up to date with main
- `feature/identity-profile_application-api` — PR #6, contains all code + design doc updates

### PRs
- **PR #5** (merged) — `feature/identity-profile_domain-persistence`: Domain entities + SQLx repos + migration
- **PR #6** (open) — `feature/identity-profile_application-api`: Application services + API routes + tests + e2e fixes + all design doc revisions

### Key Commits on PR #6
1. `feat(application): add UserService and OrganizationService with unit tests`
2. `feat(application): create personal org during OAuth callback`
3. `feat(api): wire services and add user + organization routes`
4. `test(api): add user and organization route integration tests`
5. `docs: add Feature 2 retrospective section`
6. `fix: restore e2e OAuth fixes and add /client-metadata.json endpoint`
7. `docs: revise design document and all feature specs for v2 architecture`
8. `docs: fix all consistency issues from design audit` (×3 rounds)
9. `docs: apply all inconsistency resolutions to design documents`
10. `docs: fix remaining internal_state reference, remove consumed tracking docs`

## Changes to Codebase vs Design Docs

The codebase (Phase 1 implementation) and the design docs are now **out of sync** in expected ways:

| Aspect | Codebase (implemented) | Design Docs (specified) |
|--------|----------------------|------------------------|
| Orgs table | `organizations` with `created_by`, separate `organization_profiles` | `orgs` with `bio`/`avatar_url` inline, no `owner_id` |
| Org members | `organization_members` with `is_owner`, `permissions` BIGINT | Same (spec now matches) |
| Commission status | `commission_status` TEXT on `organization_profiles` | Tags on org (`status:open`), no dedicated column |
| Roles | TEXT field, free-form | TEXT with `default_roles` reference table |
| Feeds | Not yet implemented | Fully specified (feeds, feed_items, feed_elements, entity_feeds, feed_subscriptions) |
| Commission states | Not yet implemented | Artist-defined via pipeline templates |
| Feed elements | Not yet implemented | Specified as split from feed_items |

The codebase needs updates to match the new design in future work (especially removing `commission_status` from `organization_profiles` and adding `bio`/`avatar_url` to the `organizations` table directly).

## Where to Pick Up

### Immediate Next Steps

1. **Review and merge PR #6** — all tests pass, e2e verified with real Bluesky OAuth
2. **Update codebase to match revised design** — several schema changes needed:
   - Move `bio` from `organization_profiles` to `organizations` table
   - Remove `commission_status` from `organization_profiles` (use tags instead)
   - Add `default_roles` table
   - Consider renaming `organizations` → `orgs` and `organization_members` → `org_members` for brevity

### Feature 2 Phase 2 (Next Feature Work)

Per the plan, Phase 2 includes:
- **Onboarding flow** — "What are you?" wizard (one screen, pick primary role)
- **Feed infrastructure** — `feeds`, `feed_items`, `feed_elements`, `entity_feeds`, `feed_subscriptions` tables
- **Default feed creation** on org setup (updates, gallery, activity, conditionally commissions)
- **Main app frontend scaffold** — the actual profile page, feed renderer
- **Blob storage** — MinIO in Docker Compose for avatars/uploads

### Longer Term

- Feature 3 (Tag infrastructure) — Tier 1 foundational, needed before Feature 4
- Feature 11 (Org TOS) — on critical path before commissions
- Feature 4 (Headless Commission Engine) — artist-defined states, boards as projections, add-on slots

---

**Delete this document after consumed by next session.**
