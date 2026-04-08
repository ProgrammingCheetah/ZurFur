# Feature 2: Identity & Profile Engine

## Overview

Manages user identity, artist roles, profile customization, content rating controls, and character repositories. This is the "who you are" layer — it determines how users present themselves and their characters on the platform.

## Sub-features

### 2.1 Flat Account Hierarchy

**What it is:** Every account is a Base User. The "Artist" role is a togglable extension that unlocks creator tools (pipeline, price sheets, TOS). No separate account types.

**Implementation approach:**
- Add `is_artist: bool` (default false) to the `users` table
- Create `artist_profiles` table: `user_id` (FK), `display_name`, `bio`, `commission_status` (open/closed/waitlist), `created_at`
- Toggle endpoint: `POST /users/me/artist` — enables/disables artist mode
- When toggled on, create `artist_profiles` row; when toggled off, soft-delete (preserve data)

### 2.2 Profile Customization (The Toyhouse Model)

**What it is:** Deep control over profile and character page appearance — colors, CSS, layout — similar to Toyhouse.

**Implementation approach:**
- `profile_customizations` table: `user_id`, `custom_css`, `custom_layout_json`, `updated_at`
- CSS sanitization: allowlist-based parser that strips JS, `url()` with external domains, `@import`, `position: fixed`, etc. Use a Rust CSS parser crate.
- Layout stored as JSON structure (sections, ordering) rendered by frontend
- Character pages have their own `character_customizations` table
- Size limit on custom CSS (e.g., 50KB)

### 2.3 The "Universal Layout" Safety Fallback

**What it is:** A mandatory toggle on every customized profile that reverts to the clean default theme.

**Implementation approach:**
- Purely frontend: query parameter `?universal=true` or persistent viewer preference
- When active, skip loading `custom_css` and `custom_layout_json`
- Always visible as a button/toggle in the profile header
- No backend changes needed beyond serving the flag

### 2.4 SFW/NSFW Viewer Control

**What it is:** Viewer-controlled toggle (default SFW) filtering galleries, portfolios, and active commissions.

**Implementation approach:**
- `content_rating` enum on all displayable content: `sfw`, `questionable`, `nsfw`
- Viewer preference stored in `user_preferences` table: `user_id`, `max_content_rating` (default `sfw`)
- All content queries include `WHERE content_rating <= $viewer_rating`
- Artists tag their own content; community flagging (Feature 11) catches mis-tagged content
- API returns content rating metadata so frontend can pre-filter

### 2.5 Character Repositories

**What it is:** Dedicated sub-profiles for original characters with reference sheets, species info, hex codes, and linked galleries.

**Implementation approach:**
- `characters` table: `id`, `owner_id` (FK users), `name`, `species`, `description`, `hex_codes` (JSON array), `reference_sheets` (JSON array of file URLs), `content_rating`, `is_public`, `created_at`
- `character_gallery_items` table: `character_id`, `artwork_id`, `display_order`
- File storage for reference sheets: S3-compatible storage (MinIO for dev, AWS S3 for prod)
- API: full CRUD on `/characters`, gallery management on `/characters/:id/gallery`
- Characters can be attached to commission requests (Feature 3)

## Dependencies

### Requires (must be built first)
- [Feature 1.1](../01-atproto-auth/README.md) — users must be authenticated
- File storage solution (S3/MinIO) for reference sheets and gallery images

### Enables (unlocked after this is built)
- [Feature 3](../03-commission-engine/README.md) — commissions reference character profiles in requests
- [Feature 8](../08-search-discovery/README.md) — artist profiles and tags are searchable
- [Feature 10](../10-artist-tos/README.md) — TOS is attached to artist profiles

## Implementation Phases

### Phase 1: Core Identity
- `is_artist` toggle on User entity + migration
- `artist_profiles` table and domain entity
- Artist toggle endpoint
- `user_preferences` table (content rating preference)
- Content rating enum shared across codebase

### Phase 2: Characters & Customization
- `characters` table and full CRUD API
- File upload infrastructure (S3 integration in persistence crate)
- Reference sheet upload/management
- `profile_customizations` and `character_customizations` tables
- CSS sanitization module in shared crate

### Phase 3: Post-implementation
- Gallery linking (depends on artwork entities from Feature 3)
- Profile SEO (public profiles should be indexable)
- Profile import from Bluesky (sync display name, avatar, bio)
- Load testing on CSS sanitization (potential DoS vector with complex CSS)
- Documentation: customization guide for users, CSS allowlist reference

## Assumptions

- Custom CSS will not include JavaScript (strict sanitization enforced)
- File storage (S3/MinIO) is available as infrastructure before Phase 2
- The "Artist" toggle is instant — no review or approval process
- Character ownership is single-user (no shared characters)
- Reference sheets are image files (PNG, JPG, WebP) with reasonable size limits

## Shortcomings & Known Limitations

- **CSS sanitization is hard:** XSS risk if the allowlist misses an edge case. Need thorough security review.
- **File storage not yet designed:** S3 integration, upload limits, CDN, image resizing are all unaddressed
- **No profile versioning:** Custom CSS changes are not tracked or reversible
- **Character ownership transfer** not addressed (e.g., selling an OC)
- **No collaborative characters** (shared between multiple users)
- **Species taxonomy** not formalized — free-text field initially, structured tags later
- **No profile analytics** (view counts, engagement) — deferred to Feature 7
