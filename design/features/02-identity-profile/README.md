# Feature 2: Identity & Profile Engine

> **Revised 2026-04-08** — Updated for org-centric identity, feed-driven content, headless commissions, and plugin-as-org architecture.

## Overview

Manages user identity, org membership, profile customization, content rating controls, and character repositories. Identity follows an org-centric model: the **User** is atomic (authentication identity only). All roles, titles, bios, and public-facing profiles live on **orgs**. Every user gets a personal org on first login. There is no `is_artist` flag — "artist" is an org role. Characters belong to orgs, not users directly. Galleries and portfolios are feed views attached to orgs and characters.

## Sub-features

### 2.1 Org-Centric Identity Model

**What it is:** The User entity is minimal — it holds only authentication data (DID, handle, tokens). All public identity (display name, bio, avatar, roles, commission status) lives on the org. Every user automatically receives a personal org. Artist functionality is unlocked by assigning the "artist" role on an org, not by toggling a flag on the user.

**Implementation approach:**
- `users` table remains minimal: `id`, `did`, `handle`, `created_at`, `deleted_at`
- `orgs` table: `id`, `owner_id` (FK users), `slug`, `display_name`, `bio`, `avatar_url`, `is_personal` (bool), `created_at`, `deleted_at`
- `org_members` table: `org_id`, `user_id`, `role` (enum: owner/artist/collaborator/member), `title` (free-text), `joined_at`
- On user creation (first login), auto-create a personal org (`is_personal = true`) and add the user as owner
- `commission_status` (open/closed/waitlist) lives on the org, not a separate artist profile
- No `is_artist` column. No `artist_profiles` table. Artist is a role on `org_members`.
- Endpoint: `POST /orgs/:id/members/:user_id/role` — assign/change role
- Personal org slug defaults to the user's AT Protocol handle

### 2.2 Onboarding Flow

**What it is:** On first login only, the user picks one primary role (Artist, Crafter/Maker, Commissioner/Client, Coder/Developer). This sets the initial org role on their personal org. Title is a separate free-text field the user can set independently.

**Implementation approach:**
- `onboarding_role` enum: `artist`, `crafter_maker`, `commissioner_client`, `coder_developer`
- On first login, if the user has no completed onboarding, redirect to onboarding screen
- `POST /onboarding` — accepts `{ role, title? }`, sets `org_members.role` on personal org, optionally sets `org_members.title`
- `users` table gets `onboarding_completed_at` (nullable timestamp) — null means onboarding pending
- Role can be changed later via org settings; onboarding just sets the initial value
- If role is `artist` or `crafter_maker`, the personal org gets a `commissions` default feed created (see 2.6)

### 2.3 Default Feeds per Org

**What it is:** Every org gets system feeds auto-created on setup. System feeds are undeletable. Additional feeds can be created by users.

**Implementation approach:**
- `feeds` table: `id`, `slug`, `display_name`, `feed_type` (enum: system/custom), `created_at`
- `entity_feeds` table (polymorphic join): `feed_id`, `entity_type` (enum: org/character/commission), `entity_id`
- On org creation, auto-create system feeds attached via `entity_feeds`:
  - `updates` (always) — general announcements
  - `gallery` (always) — artwork and portfolio items
  - `activity` (always) — auto-generated activity log
  - `commissions` (only if org has artist/crafter role) — commission openings and status
- System feeds (`feed_type = 'system'`) cannot be deleted or renamed
- Custom feeds: `POST /orgs/:id/feeds` — user-created, deletable
- Following an org = subscribing to its feeds (no separate follows table)

### 2.4 Profile Customization (The Toyhouse Model)

**What it is:** Deep control over profile and character page appearance — colors, CSS, layout — similar to Toyhouse. Customization applies to org profiles.

**Implementation approach:**
- `org_customizations` table: `org_id`, `custom_css`, `custom_layout_json`, `updated_at`
- CSS sanitization: allowlist-based parser that strips JS, `url()` with external domains, `@import`, `position: fixed`, etc. Use a Rust CSS parser crate.
- Layout stored as JSON structure (sections, ordering) rendered by frontend
- Character pages have their own `character_customizations` table
- Size limit on custom CSS (e.g., 50KB)

### 2.5 The "Universal Layout" Safety Fallback

**What it is:** A mandatory toggle on every customized profile that reverts to the clean default theme.

**Implementation approach:**
- Purely frontend: query parameter `?universal=true` or persistent viewer preference
- When active, skip loading `custom_css` and `custom_layout_json`
- Always visible as a button/toggle in the profile header
- No backend changes needed beyond serving the flag

### 2.6 SFW/NSFW Viewer Control

**What it is:** Viewer-controlled toggle (default SFW) filtering galleries, portfolios, and active commissions.

**Implementation approach:**
- `content_rating` enum on all displayable content: `sfw`, `questionable`, `nsfw`
- Viewer preference stored in `user_preferences` table: `user_id`, `max_content_rating` (default `sfw`)
- All content queries include `WHERE content_rating <= $viewer_rating`
- Content creators tag their own content; community flagging (Feature 11) catches mis-tagged content
- API returns content rating metadata so frontend can pre-filter

### 2.7 Character Repositories

**What it is:** Dedicated sub-profiles for original characters with reference sheets, species info, and linked galleries. Characters belong to orgs (not users directly). A character's gallery is a feed. Reference sheets are feed items tagged "ref_sheet".

**Implementation approach:**
- `characters` table: `id`, `org_id` (FK orgs), `name`, `description`, `content_rating`, `is_public`, `created_at`
- Species, hex codes, art style, and other structured data stored as **tags** (not columns). Only `description` is free-text.
- `tags` table: `id`, `namespace` (species/color/art_style/ref_sheet/etc.), `value`, `created_at`
- `entity_tags` table (polymorphic): `tag_id`, `entity_type` (character/feed_item/org), `entity_id`
- On character creation, auto-create a `gallery` feed and a `ref_sheets` feed attached via `entity_feeds`
- Reference sheets are feed items in the character's `ref_sheets` feed, tagged with `ref_sheet` namespace
- Gallery items are feed items in the character's `gallery` feed
- File storage for images: S3-compatible storage (MinIO for dev, AWS S3 for prod)
- API: full CRUD on `/orgs/:id/characters`, feed management on `/characters/:id/feeds`
- Characters can be attached to commission requests (Feature 3)

## Dependencies

### Requires (must be built first)
- [Feature 1.1](../01-atproto-auth/README.md) — users must be authenticated
- File storage solution (S3/MinIO) for reference sheets and gallery images

### Enables (unlocked after this is built)
- [Feature 3](../03-commission-engine/README.md) — commissions reference character profiles and org roles
- [Feature 8](../08-search-discovery/README.md) — org profiles and tags are searchable
- [Feature 10](../10-artist-tos/README.md) — TOS is attached to org profiles

## Implementation Phases

### Phase 1: Org Model & API (Done)
- `orgs` table, `org_members` table, domain entities
- Personal org auto-creation on user registration
- Org CRUD endpoints
- Role assignment endpoint
- `user_preferences` table (content rating preference)
- Content rating enum shared across codebase

### Phase 2: Onboarding, Frontend & Feeds
- Onboarding flow: role selection endpoint, `onboarding_completed_at` tracking
- `feeds` table, `entity_feeds` table, domain entities
- Default feed creation on org setup (updates, gallery, activity, commissions)
- Feed CRUD API for custom feeds
- Feed subscription model (following = feed subscription)
- File upload infrastructure (S3 integration in persistence crate)
- `org_customizations` table and CSS sanitization module in shared crate
- Frontend: onboarding screen, org profile page, feed rendering

### Phase 3: Characters, Tags & Feed Items
- `characters` table and full CRUD API scoped to orgs
- `tags` and `entity_tags` tables — species, colors, art style as tags
- Character feed auto-creation (gallery + ref_sheets feeds)
- Reference sheet upload as feed items tagged "ref_sheet"
- `character_customizations` table
- Gallery linking via character gallery feed
- Profile SEO (public profiles should be indexable)
- Profile import from Bluesky (sync display name, avatar, bio to personal org)
- Load testing on CSS sanitization (potential DoS vector with complex CSS)
- Documentation: customization guide for users, CSS allowlist reference

## Assumptions

- Custom CSS will not include JavaScript (strict sanitization enforced)
- File storage (S3/MinIO) is available as infrastructure before Phase 3
- Role changes are instant — no review or approval process
- Character ownership is per-org (org members with appropriate role can manage characters)
- Reference sheets are image files (PNG, JPG, WebP) with reasonable size limits
- Every user gets exactly one personal org; additional orgs (studios, groups) are created manually
- Tags are reusable across entities — the same "wolf" species tag applies to any character
- Onboarding role selection is a one-time convenience; the role can be changed later

## Shortcomings & Known Limitations

- **CSS sanitization is hard:** XSS risk if the allowlist misses an edge case. Need thorough security review.
- **File storage not yet designed:** S3 integration, upload limits, CDN, image resizing are all unaddressed
- **No profile versioning:** Custom CSS changes are not tracked or reversible
- **Character ownership transfer** not addressed (e.g., transferring an OC between orgs)
- **No collaborative characters** across orgs (shared between multiple orgs)
- **Tag taxonomy governance** not formalized — who creates canonical tags, how to handle duplicates/synonyms
- **No profile analytics** (view counts, engagement) — deferred to Feature 7
- **Org member permissions** beyond role are not granular — no per-feed or per-character access control yet
- **Feed pagination and performance** at scale not yet addressed
