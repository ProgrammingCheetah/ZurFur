> **Revised 2026-04-08** — Updated for org-centric identity, feed-driven content, headless commissions, and plugin-as-org architecture.

# Feature 11: Content Moderation & Trust/Safety

## Overview

Protects the community from harassment, scams, IP theft, and untagged NSFW content. Provides user- and org-level controls (block/mute), a reporting system covering orgs and feed items, DMCA compliance including PDS record takedowns, and automated content flagging. Plugin orgs can be reported and disabled. "Untagged NSFW" detection interacts with the tag system (Feature 8.2). Reports and flags feed into the Platform Admin moderation queue (Feature 13).

## Sub-features

### 11.1 Reporting

**What it is:** Any user can report users, orgs, commission cards, chat messages, feed items, or plugin orgs for policy violations.

**Implementation approach:**
- `reports` table: `id`, `reporter_id`, `target_type` (user/org/commission/message/feed_item), `target_id`, `reason_category`, `description`, `status` (pending/reviewed/actioned/dismissed), `reviewed_by`, `reviewed_at`, `created_at`
- Reason categories: `harassment`, `scam`, `untagged_nsfw`, `copyright`, `impersonation`, `spam`, `plugin_abuse`, `other`
- Reports can target orgs (including plugin orgs) and individual feed items
- Plugin org reports have a dedicated `plugin_abuse` category for misbehaving plugin orgs
- API: `POST /reports` (submit), `GET /admin/reports` (moderation queue — Feature 13)
- Rate limiting: max N reports per user per hour to prevent abuse
- Reporter identity visible to admins but hidden from the reported entity

### 11.2 Block & Mute

**What it is:** User-level and org-level controls to prevent unwanted interaction.

**Implementation approach:**
- `blocks` table: `blocker_type` (user/org), `blocker_id`, `blocked_type` (user/org), `blocked_id`, `type` (block/mute), `created_at`
- Blocks and mutes can target both users and orgs
- **Block (user→user):** Prevents all contact. Hides blocker's content from blocked user and vice versa.
- **Block (user→org):** Hides org's content, prevents commission requests to that org.
- **Block (org→user):** Org members can block a user from commissioning or interacting with the org.
- **Mute:** Silently suppresses notifications from muted user/org. No contact prevention.
- All content queries must check against the block list — add filter to repository queries
- API: `POST /users/:id/block`, `POST /orgs/:id/block`, `POST /users/:id/mute`, `DELETE /users/:id/block`, `GET /me/blocked`

### 11.3 DMCA/Takedown Flow

**What it is:** Formal copyright claim process compliant with DMCA safe harbor. Covers both PostgreSQL-stored content and PDS records.

**Implementation approach:**
- `takedown_requests` table: `id`, `claimant_id`, `target_type` (feed_item/commission/pds_record), `target_id`, `pds_record_uri` (for AT Protocol records), `claim_details`, `claimant_contact`, `sworn_statement`, `status` (filed/content_removed/counter_filed/restored/resolved), `created_at`
- Workflow:
  1. Claimant files takedown (requires sworn statement of ownership)
  2. Content immediately hidden (not deleted) — for PDS records, also issue takedown via AT Protocol admin API
  3. Content owner notified, given 10 business days to file counter-notice
  4. If counter-notice filed → content restored (PDS record re-published) after 14 days unless claimant files legal action
  5. All steps logged with timestamps for legal compliance
- **PDS record takedowns:** Must use AT Protocol admin operations to remove/label records on PDS
- API: `POST /takedowns` (file claim), `POST /takedowns/:id/counter-notice`
- Legal page: DMCA agent contact information (required for safe harbor)

### 11.4 Content Flagging

**What it is:** Automated and manual content flagging that feeds into the moderation queue. "Untagged NSFW" detection integrates with the tag system.

**Implementation approach:**
- `content_flags` table: `id`, `content_type` (feed_item/commission/org_profile), `content_id`, `flag_type` (untagged_nsfw/spam/suspicious), `source` (automated/community), `confidence` (for automated), `status` (pending/reviewed), `created_at`
- **Automated:** On image upload, optionally call external NSFW detection API (e.g., AWS Rekognition, custom model). Flag if NSFW detected but content lacks NSFW tags (interacts with Feature 8.2 tag system)
- **"Untagged NSFW" detection:** Cross-reference automated NSFW confidence score with entity tags — if high NSFW confidence but no NSFW-category tags present, auto-flag
- **Community:** Users flag content → after N independent flags, auto-escalate to moderation queue
- Flag threshold: configurable (e.g., 3 community flags → auto-review)
- Feed posts are flaggable content — moderation queue includes feed items as actionable items
- Feeds into Feature 13.3 moderation queue

## Dependencies

### Requires (must be built first)
- [Feature 1.1](../01-atproto-auth/README.md) — authenticated users
- [Feature 2](../02-identity-profile/README.md) — orgs and profiles to moderate
- [Feature 8.2](../08-search-discovery/README.md) — tag system for NSFW tag detection
- Feed infrastructure — feed items are reportable/flaggable content

### Enables (unlocked after this is built)
- [Feature 13.3](../13-platform-admin/README.md) — moderation queue processes reports and flags
- Safer platform: required before any public launch

## Implementation Phases

### Phase 1: Block/Mute & Reporting
- `blocks` table with user/org targeting + block/mute API
- Block filtering in content queries (add WHERE NOT IN blocked entities)
- `reports` table with org and feed item target types + report submission API
- Report rate limiting
- Crates: domain (Report entity, BlockList), persistence, application, api

### Phase 2: DMCA & Content Flagging
- `takedown_requests` table + full workflow including PDS record takedowns
- DMCA counter-notice flow
- AT Protocol admin API integration for PDS takedowns
- `content_flags` table
- "Untagged NSFW" detection via tag system integration
- Community flagging with threshold escalation
- Automated NSFW detection integration (optional, behind feature flag)
- Feed posts as flaggable content
- Integration with Feature 13 moderation queue

### Phase 3: Post-implementation
- DMCA agent page (legal requirement)
- Plugin org reporting and disable workflow
- Moderation response time SLAs
- Block list performance optimization (caching hot block lists)
- Automated flagging accuracy tracking and tuning
- User/org trust scores (based on report/flag history)
- Transparency report (aggregate moderation statistics)
- Legal review of DMCA compliance including PDS takedown procedures

## Assumptions

- Platform qualifies for DMCA safe harbor (must designate DMCA agent, respond promptly)
- PDS takedowns use AT Protocol admin operations — platform must have admin access to its PDS
- Automated NSFW detection is supplementary — furry art confuses standard classifiers (high false positive rate)
- Block/mute lists are private (blocked entity doesn't know)
- Reporter identity hidden from reported entity but visible to admins
- Community flagging threshold prevents single-user abuse
- Plugin orgs are subject to the same moderation rules as any other org

## Shortcomings & Known Limitations

- **Automated NSFW detection for furry art is unreliable** — species-specific styles confuse standard models. High false positive rate expected.
- **DMCA process requires legal expertise** to implement correctly — mistakes could lose safe harbor protection
- **PDS takedowns add complexity** — must coordinate removal across both PostgreSQL and PDS, handle reconciliation
- **No appeal process** for content removal beyond DMCA counter-notice
- **Block checking on every query** adds performance overhead — needs caching strategy, especially with org-level blocks
- **Coordinated reporting abuse** (mass-reporting to harass a user/org) not addressed beyond rate limiting
- **No proactive content scanning** — only reactive (reports and flags after upload)
- **CSAM detection** (PhotoDNA or equivalent) is legally required in many jurisdictions — not yet integrated
- **Moderation at scale** requires paid moderators — volunteer moderation doesn't sustain
- **Plugin org moderation** needs clear policies — when is a plugin org "misbehaving" vs "functioning as designed"?
