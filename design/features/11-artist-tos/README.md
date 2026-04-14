> **Revised 2026-04-08** — Updated for org-centric identity, feed-driven content, headless commissions, and plugin-as-org architecture.

# Feature 11: Organization Terms of Service (TOS) Management

## Overview

Orgs define their rules, boundaries, refund policies, and expectations via a structured TOS. TOS belongs to orgs, not individual users — all studio members operate under their org's TOS. Each revision is immutably versioned. Clients must explicitly accept the current TOS before submitting a commission request; that acceptance is recorded as an add-on slot on the commission card. Returning clients see a diff of what changed. The active TOS version is published to PDS as an AT Protocol record for public auditability.

## Sub-features

### 11.1 TOS Builder

**What it is:** A structured editor for orgs to create and maintain their terms.

**Implementation approach:**
- `org_tos` table: `id`, `org_id`, `version`, `content_json`, `is_active`, `pds_record_uri`, `created_at`
- `content_json` structure: sections array with `{ "key": "refund_policy", "title": "Refund Policy", "body": "..." }` for standard sections + `custom_sections` array
- Standard section keys: `refund_policy`, `usage_rights`, `turnaround_time`, `communication`, `revisions_policy`, `payment_terms`
- When a new TOS version is published, it is also written to PDS as an AT Protocol record (publicly auditable, tamper-evident)
- API: `POST /orgs/:id/tos` (creates new version), `GET /orgs/:id/tos` (returns active version)
- Any org member with the appropriate role can create/publish TOS versions
- Frontend renders a form-based editor with section templates

### 11.2 TOS Versioning

**What it is:** Immutable snapshots. Each save creates a new version; old versions are never modified.

**Implementation approach:**
- New version = new row with incremented `version` number
- Only one version can be `is_active = true` per org
- Publishing a new version: set old to `is_active = false`, new to `is_active = true`, publish to PDS
- Old versions remain accessible: `GET /orgs/:id/tos/versions`, `GET /orgs/:id/tos/:version`
- PDS records for old versions remain (AT Protocol doesn't delete history)
- Commission cards link to the specific `tos_version_id` that was accepted

### 11.3 Mandatory Acknowledgment

**What it is:** Clients must accept the org's current TOS before submitting a commission request. TOS acceptance is an add-on slot on the commission card.

**Implementation approach:**
- `tos_acceptances` table: `id`, `user_id`, `tos_id` (specific version), `accepted_at`
- Commission creation flow: check if client has accepted the org's current active TOS
  - If not accepted → reject with `tos_required` error + return TOS content
  - If accepted → proceed, record acceptance as an add-on slot on the commission card
- TOS acceptance is a commission card add-on slot: the card shell references which TOS version was accepted, with timestamp
- If TOS changes between commissions, client must re-accept before new commission

### 11.4 TOS Diff View

**What it is:** Visual comparison between TOS versions for returning clients.

**Implementation approach:**
- JSON structural diff between two versions' `content_json`
- Diff output: added sections, removed sections, changed section bodies
- API: `GET /orgs/:id/tos/diff?from=2&to=3`
- Frontend renders additions (green), removals (red), changes (yellow)
- Shown automatically to returning clients when TOS has changed since their last acceptance

## Dependencies

### Requires (must be built first)
- [Feature 1.1](../01-atproto-auth/README.md) — authenticated users
- [Feature 2](../02-identity-profile/README.md) — org entity and membership must exist

### Enables (unlocked after this is built)
- [Feature 4](../04-commission-engine/README.md) — commission intake requires TOS acceptance (as card add-on slot)
- [Feature 13](../13-dispute-resolution/README.md) — TOS terms referenced during dispute resolution

## Implementation Phases

### Phase 1: TOS CRUD & Versioning
- `org_tos` table with versioning logic
- Create/publish/list versions API
- Active version retrieval
- PDS publishing on version activation
- Domain: `OrgTos` entity, `OrgTosRepository` trait
- Crates: domain, persistence, application (TOS management use cases), api (routes)

### Phase 2: Acceptance & Commission Card Integration
- `tos_acceptances` table
- Commission creation gate: require TOS acceptance
- TOS acceptance as add-on slot on commission card
- Acceptance API: `POST /orgs/:id/tos/accept`
- Re-acceptance flow when TOS changes

### Phase 3: Diff & Post-implementation
- JSON diff algorithm for TOS versions
- Diff API endpoint
- Integration tests: TOS change → re-acceptance required → commission flow
- TOS template library (common sections with suggested language)
- Legal review: ensure TOS framework covers platform liability
- PDS record schema registration for TOS lexicon
- Documentation: TOS best practices guide for orgs

## Assumptions

- TOS is per-org, not platform-wide (platform's own TOS is separate)
- All members of an org operate under that org's TOS — no per-member TOS
- Structured JSON is sufficient — orgs don't need full rich text/WYSIWYG
- Version history kept indefinitely (legal requirement for dispute reference)
- One active version per org at any time
- TOS acceptance is per-version, not per-commission (accept once, valid until version changes)
- Active TOS published to PDS is eventually consistent — brief window between PostgreSQL activation and PDS publication

## Shortcomings & Known Limitations

- **No legal template library** — orgs write their own TOS with no guidance initially
- **JSON structure may not cover all edge cases** orgs want to express (e.g., complex conditional terms)
- **Diff view is structural, not semantic** — can't detect meaning changes within a section body
- **No internationalization** for TOS content (org writes in one language)
- **TOS is not programmatically enforceable** — it's a reference document, not a smart contract
- **No TOS search** — clients can't search across orgs' TOS for specific terms
- **Bulk TOS changes** (org updates TOS, invalidates all existing acceptances) could disrupt active commissions
- **PDS publishing failure** could leave TOS active in PostgreSQL but not publicly auditable — needs reconciliation
- **Multi-member orgs** need clear UX for who can edit/publish TOS — role-based permissions required
