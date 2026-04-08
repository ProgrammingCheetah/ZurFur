# Feature 10: Artist Terms of Service (TOS) Management

## Overview

Artists define their rules, boundaries, refund policies, and expectations via a structured TOS. Each revision is immutably versioned. Clients must explicitly accept the current TOS before submitting a commission request, and that acceptance is linked to the Card's audit trail. Returning clients see a diff of what changed.

## Sub-features

### 10.1 TOS Builder

**What it is:** A structured editor for artists to create and maintain their terms.

**Implementation approach:**
- `artist_tos` table: `id`, `artist_id`, `version`, `content_json`, `is_active`, `created_at`
- `content_json` structure: sections array with `{ "key": "refund_policy", "title": "Refund Policy", "body": "..." }` for standard sections + `custom_sections` array
- Standard section keys: `refund_policy`, `usage_rights`, `turnaround_time`, `communication`, `revisions_policy`, `payment_terms`
- API: `POST /me/tos` (creates new version), `GET /artists/:id/tos` (returns active version)
- Frontend renders a form-based editor with section templates

### 10.2 TOS Versioning

**What it is:** Immutable snapshots. Each save creates a new version; old versions are never modified.

**Implementation approach:**
- New version = new row with incremented `version` number
- Only one version can be `is_active = true` per artist
- Publishing a new version: set old to `is_active = false`, new to `is_active = true`
- Old versions remain accessible: `GET /artists/:id/tos/versions`, `GET /artists/:id/tos/:version`
- Commission cards link to the specific `tos_version_id` that was accepted

### 10.3 Mandatory Acknowledgment

**What it is:** Clients must accept the artist's current TOS before submitting a commission request.

**Implementation approach:**
- `tos_acceptances` table: `id`, `user_id`, `tos_id` (specific version), `accepted_at`
- Commission creation flow: check if client has accepted the artist's current active TOS
  - If not accepted → reject with `tos_required` error + return TOS content
  - If accepted → proceed, link `tos_id` to commission card
- Acceptance recorded as a `TosAccepted` event on the Card's audit trail
- If TOS changes between commissions, client must re-accept before new commission

### 10.4 TOS Diff View

**What it is:** Visual comparison between TOS versions for returning clients.

**Implementation approach:**
- JSON structural diff between two versions' `content_json`
- Diff output: added sections, removed sections, changed section bodies
- API: `GET /artists/:id/tos/diff?from=2&to=3`
- Frontend renders additions (green), removals (red), changes (yellow)
- Shown automatically to returning clients when TOS has changed since their last acceptance

## Dependencies

### Requires (must be built first)
- [Feature 1.1](../01-atproto-auth/README.md) — authenticated users
- [Feature 2.1](../02-identity-profile/README.md) — artist role must exist

### Enables (unlocked after this is built)
- [Feature 3](../03-commission-engine/README.md) — commission intake requires TOS acceptance
- [Feature 12](../12-dispute-resolution/README.md) — TOS terms referenced during dispute resolution

## Implementation Phases

### Phase 1: TOS CRUD & Versioning
- `artist_tos` table with versioning logic
- Create/publish/list versions API
- Active version retrieval
- Domain: `ArtistTos` entity, `ArtistTosRepository` trait
- Crates: domain, persistence, application (TOS management use cases), api (routes)

### Phase 2: Acceptance & Integration
- `tos_acceptances` table
- Commission creation gate: require TOS acceptance
- `TosAccepted` event type in commission event system
- Acceptance API: `POST /artists/:id/tos/accept`
- Re-acceptance flow when TOS changes

### Phase 3: Diff & Post-implementation
- JSON diff algorithm for TOS versions
- Diff API endpoint
- Integration tests: TOS change → re-acceptance required → commission flow
- TOS template library (common sections with suggested language)
- Legal review: ensure TOS framework covers platform liability
- Documentation: TOS best practices guide for artists

## Assumptions

- TOS is per-artist, not platform-wide (platform's own TOS is separate)
- Structured JSON is sufficient — artists don't need full rich text/WYSIWYG
- Version history kept indefinitely (legal requirement for dispute reference)
- One active version per artist at any time
- TOS acceptance is per-version, not per-commission (accept once, valid until version changes)

## Shortcomings & Known Limitations

- **No legal template library** — artists write their own TOS with no guidance initially
- **JSON structure may not cover all edge cases** artists want to express (e.g., complex conditional terms)
- **Diff view is structural, not semantic** — can't detect meaning changes within a section body
- **No internationalization** for TOS content (artist writes in one language)
- **TOS is not programmatically enforceable** — it's a reference document, not a smart contract
- **No TOS search** — clients can't search across artists' TOS for specific terms
- **Bulk TOS changes** (artist updates TOS, invalidates all existing acceptances) could disrupt active commissions
