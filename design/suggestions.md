# Zurfur — Philosophy Review, Suggestions, and Corrections

## Philosophy Assessment

### What's Working

**The "everything is an org with feeds" unification is the right call.** Most platforms accrete special cases as they grow: artists get a separate profile type, studios get a different entity, plugins get their own registry. Zurfur's decision to unify all of these under Organization + Feed eliminates an entire category of future technical debt. This is the platform's strongest architectural decision.

**The headless commission engine is a genuine differentiator.** By refusing to bake workflow assumptions into the schema, Zurfur avoids the trap that kills most project management tools: becoming opinionated about how work should flow. A fursuit maker's pipeline looks nothing like a digital artist's pipeline. The headless model serves both. The add-on slot architecture means the platform can evolve without schema migrations for every new feature.

**Data sovereignty via AT Protocol is more than marketing.** The two-tier data model (public identity on PDS, private transactions in PostgreSQL) is architecturally sound. The repository trait abstraction means the migration from "everything in Postgres" to "public data on PDS" is a persistence-layer change that doesn't touch application logic. This is well-designed.

**Transaction-based monetization aligns incentives.** Zurfur only makes money when artists make money. This avoids the perverse incentives of subscription models (charging artists who aren't earning) or advertising models (optimizing for attention over utility).

### What Needs Scrutiny

**The "super app" framing is a risk.** The design document describes 13 features, a plugin ecosystem, AI analytics, federated data hosting, and community wikis. This is a 5-year vision presented as a 1-year roadmap. The critical path (auth → orgs → tags → feeds → TOS → commissions → payments) is already 7+ features deep before a single commission can be completed. Every feature added to the critical path delays the moment of truth: does anyone actually use this to commission art?

**Recommendation:** Treat the critical path as the only path until end-to-end commissions work. Features like profile customization (CSS injection), character repositories, gamification (XP/badges), and the wiki are growth features, not foundation features. They should be explicitly deprioritized.

**The plugin-as-org model may confuse early users.** "Installing a plugin" meaning "granting an organization subscription access to your feeds" is architecturally elegant but conceptually unfamiliar. Early documentation and UX will need to bridge this gap. Users expect "click install" — they shouldn't need to understand the org/feed model to use plugins.

**Recommendation:** The plugin installation UX should be a one-click flow that abstracts away the feed subscription mechanics. The architecture can be org-based internally, but the user-facing experience should be "install / uninstall / configure."

---

## Corrections and Denials

### Schema Divergence from Design (Correct Now)

The current codebase has diverged from the design document in several places. These should be reconciled:

1. **`commission_status` column on `organization_profiles`** — The design document explicitly states that commission availability should be expressed through tags on the org (e.g., `status:open`), not a dedicated column. The current implementation has a `CommissionStatus` enum (open/closed/waitlist) on the `organization_profiles` table. This column should be removed when the tag system is built and replaced with org-level tags.

2. **`organization_profiles` as a separate table** — The design document puts `bio` and `avatar_url` directly on the `organizations` table. The current implementation has a separate `organization_profiles` table with `bio` and `commission_status`. Consider merging `bio` into `organizations` and dropping the profiles table, since every org will have a bio (it's not optional auxiliary data like customization CSS).

3. **`display_name` nullable on organizations** — This is correct per the design (NULL for personal orgs, resolved from owner's handle at API layer). No change needed.

### Characteristics I Affirm

- **User is atomic.** Correct. Never add feature data to User.
- **Feeds are immutable event logs.** Correct. Feed items have `created_at` only, no `updated_at`. Elements can be edited, but the item structure is append-only.
- **Permissions as BIGINT bitfield.** Correct. Fast, compact, extensible without migration. The `ALL = u64::MAX` pattern with i64 wrapping is well-documented and tested.
- **Soft deletes for organizations and feeds.** Correct. These are aggregate roots that may have downstream references.
- **Hard deletes for feed items, elements, subscriptions.** Correct. These are leaf entities within their aggregate.
- **`content_json` as String, not `serde_json::Value`, in domain.** Correct. Keeps the domain crate dependency-free.
- **TEXT + CHECK constraints over PostgreSQL enums** for extensible types (commission_status, feed_type, etc.). Correct. Easier to extend without migration.
- **PostgreSQL ENUM for stable types** (content_rating). Correct. Performance benefit with no downside for types that won't change.

### Characteristics I Deny or Question

1. **Profile customization (custom CSS) in Phase 2.** This is a security-intensive feature (XSS, CSS injection, resource exhaustion) that provides no value until the platform has users who care about profile aesthetics. It should be Phase 3 or later, not Phase 2. The design document already places it in Phase 2 under "Org customizations table and CSS sanitization module." I suggest moving it to Phase 3 explicitly.

2. **File upload infrastructure in Phase 2.** S3 integration, image resizing, CDN setup, and upload limits are infrastructure work that only matters when character repositories and gallery uploads are being built. This is Phase 3 work. Phase 2 should focus on feed structure and onboarding — content that flows through feeds can be text-only initially.

3. **Frontend work in Phase 2.** The design calls for "onboarding screen, org profile page, feed rendering" in Phase 2. The backend is headless and API-first. Frontend work should follow API stabilization, not run in parallel during foundation-building. Suggest deferring all frontend beyond the auth callback page until the commission critical path is complete in the backend.

---

## Architecture Suggestions

### 1. Missing "artist" Default Role (Fixed)

The `OnboardingRole` enum maps `Artist`/`CrafterMaker` to `default_role_name() = "artist"`, but the original seed data only had owner/admin/mod/member. The onboarding flow would fail at runtime when looking up the "artist" role. Fixed by adding "artist" as a default role (hierarchy level 2, between admin and mod) with `MANAGE_PROFILE | MANAGE_COMMISSIONS | CHAT` permissions. Hierarchy levels for mod and member shifted to 3 and 4 respectively.

### 2. Cursor-Based Pagination for Feeds (Do Before Application Layer)

The current `FeedItemRepository::list_by_feed(feed_id, limit, offset)` uses offset-based pagination. For feeds — which are append-heavy, chronologically ordered, and potentially very large — cursor-based pagination is significantly better:

- **Offset pagination breaks when items are inserted during browsing** (items shift, causing duplicates or skips)
- **Offset pagination degrades at high page numbers** (PostgreSQL must scan and discard all offset rows)
- **Cursor pagination is stable** — "give me 20 items older than this timestamp/ID" always returns the correct window regardless of insertions

**Suggestion:** Change the trait signature to:
```rust
async fn list_by_feed(
    &self,
    feed_id: Uuid,
    limit: i64,
    before: Option<DateTime<Utc>>,  // cursor: items older than this
) -> Result<Vec<FeedItem>, FeedItemError>;
```

And the SQL to:
```sql
SELECT ... FROM feed_items
WHERE feed_id = $1 AND ($2::timestamptz IS NULL OR created_at < $2)
ORDER BY created_at DESC
LIMIT $3
```

This should be done before the application layer builds on top of the current offset-based API.

### 3. Feed Slug Uniqueness Strategy

Feed slugs need to be unique within the context of their owning entity (e.g., an org can't have two feeds named "gallery"). Currently there's no enforcement at any layer. Two options:

**Option A (Application layer):** Before creating a feed for entity X, query `entity_feeds` + `feeds` to check no existing feed with the same slug exists for that entity. This matches the org slug validation pattern but requires a cross-table query.

**Option B (Compound unique index):** Create a materialized view or denormalized column that enables a DB-level constraint. More complex but prevents race conditions.

**Suggestion:** Go with Option A for now. It's consistent with how org slugs are validated and avoids schema complexity. Document the pattern so it's applied consistently when feed creation is wired up in the application layer.

### 4. Commission Events as Feed Elements, Not a New Field

The design document lists specific commission event types (Created, StateChanged, CommentAdded, FileUploaded, InvoiceAttached, PaymentReceived, etc.). Currently, feed items have a generic `AuthorType` but no `event_type` field. When the commission engine is built, there will need to be a way to distinguish event types within the commission feed.

**Two approaches:**
- Add an `event_type` field to `FeedItem` (makes it commission-specific, pollutes the generic feed model)
- Use a `FeedElement` with `element_type: "event"` and `content_json` containing the event type and payload (keeps feeds generic, events are just a content type)

**Suggestion:** The second approach is correct. Events are feed elements of type "event" with structured JSON. The feed model stays generic. The application layer interprets the JSON. This is already implied by the design but should be made explicit.

### 5. Default Role Usage in OrganizationService

The `OrganizationService::create_org` and `create_personal_org` currently hardcode `"owner"` as a string and `Permissions::ALL` as the permission set. Now that `default_roles` exists, the application layer should look up the default role to get permissions instead of hardcoding them.

**Current:**
```rust
self.member_repo.add(org.id, user_id, "owner", None, true, Permissions::new(Permissions::ALL))
```

**Should become:**
```rust
let owner_role = self.default_role_repo.find_by_name("owner").await?;
self.member_repo.add(org.id, user_id, &owner_role.name, None, true, owner_role.default_permissions)
```

This ensures that if default permissions change (e.g., a new permission bit is added), the change is made in one place (the default_roles seed data) rather than everywhere roles are assigned.

**Suggestion:** Wire this up when building the application layer for Phase 2 (onboarding flow). The `OrganizationService` will need access to `DefaultRoleRepository` as a new dependency.

### 6. Onboarding as a Separate Service

The onboarding flow touches multiple domains: User (marking completion), Organization (updating role), Feed (creating commissions feed). This cross-cutting logic should live in its own `OnboardingService` rather than being bolted onto `UserService` or `OrganizationService`.

**Suggestion:** Create `application/src/onboarding/service.rs` with an `OnboardingService` that depends on `UserRepository`, `OrganizationService`, `DefaultRoleRepository`, and `FeedService`. This keeps the existing services focused and makes the onboarding flow a single, testable unit.

### 7. Migration Naming Convention

The current migration timestamps use dates without meaningful sequencing:
- `20250227000001` (Feb 2025)
- `20260331000001` (Mar 2026)
- `20260408000001` (Apr 2026)
- `20260409000001` (Apr 2026)

Two migrations in consecutive days could collide if the `000001` suffix doesn't increment. Consider using a more granular timestamp or sequential numbering within a day.

**Suggestion:** Not critical — SQLx orders by filename lexicographically, and the current naming works. Just be aware if multiple migrations are created on the same day.

### 8. Tag System — Typed Tags & Entity-Backed Identity

The tag system has been redesigned with typed tags. Key design decisions (Apr 2026):

- **Tags have types:** `organization`, `character`, `metadata`, `general` — a constrained enum, not free-form categories
- **Entity-backed tags:** Every org and character auto-gets an immutable tag on creation. The tag's UUID is permanent; display resolves from the entity name. Attribution = attaching an org's tag.
- **Metadata tags** have an optional `category` for faceted search (species, art_style, medium, content_type, status)
- **Attribution via tags:** "This artist made this" = the artist's org tag attached to the commission. No separate participants table.

The tag domain + persistence layer should be built as the next Tier 1 infrastructure piece, before the Commission Engine. The `entity_tags` junction follows the same polymorphic pattern as `entity_feeds`.

---

## Priority Recommendations

### Immediate (This Development Cycle)

1. Finish Feature 2 Phase 2 application layer (onboarding + feed creation on org setup)
2. Build Tag Taxonomy domain + persistence (Feature 8.2) — it's needed by almost everything downstream
3. Switch feed pagination from offset to cursor before the API solidifies

### Next Cycle

4. Feature 10 (Org TOS) — blocking commissions
5. Feature 3 (Commission Engine) — the product's reason to exist
6. Feature 4 (Financial Gateway) — Stripe Connect integration

### Defer

7. Profile customization (CSS) — Phase 3
8. Character repositories — Phase 3
9. File upload infrastructure — Phase 3
10. Gamification (XP/badges) — Phase 3+
11. Plugin marketplace — Phase 3+
12. AI analytics — Phase 4+
13. Community wiki/forums — Phase 4+
14. Federated org hosting — Phase 5+
