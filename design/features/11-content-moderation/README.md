# Feature 11: Content Moderation & Trust/Safety

## Overview

Protects the community from harassment, scams, IP theft, and untagged NSFW content. Provides user-level controls (block/mute), a reporting system, DMCA compliance, and automated content flagging. Reports and flags feed into the Platform Admin moderation queue (Feature 13).

## Sub-features

### 11.1 User Reporting

**What it is:** Any user can report profiles, commission cards, chat messages, or gallery content for policy violations.

**Implementation approach:**
- `reports` table: `id`, `reporter_id`, `target_type` (user/commission/message/gallery_item), `target_id`, `reason_category`, `description`, `status` (pending/reviewed/actioned/dismissed), `reviewed_by`, `reviewed_at`, `created_at`
- Reason categories: `harassment`, `scam`, `untagged_nsfw`, `copyright`, `impersonation`, `spam`, `other`
- API: `POST /reports` (submit), `GET /admin/reports` (moderation queue — Feature 13)
- Rate limiting: max N reports per user per hour to prevent abuse
- Reporter identity visible to admins but hidden from the reported user

### 11.2 Block & Mute

**What it is:** User-level controls to prevent unwanted interaction.

**Implementation approach:**
- `user_blocks` table: `blocker_id`, `blocked_id`, `type` (block/mute), `created_at`
- **Block:** Prevents all contact. Hides blocker's content from blocked user and vice versa. Blocked user cannot send commission requests, messages, or view profiles.
- **Mute:** Silently suppresses notifications from muted user. No contact prevention.
- All content queries must check against the user's block list — add filter to repository queries
- API: `POST /users/:id/block`, `POST /users/:id/mute`, `DELETE /users/:id/block`, `GET /me/blocked`

### 11.3 DMCA/Takedown Flow

**What it is:** Formal copyright claim process compliant with DMCA safe harbor.

**Implementation approach:**
- `takedown_requests` table: `id`, `claimant_id`, `target_type`, `target_id`, `claim_details`, `claimant_contact`, `sworn_statement`, `status` (filed/content_removed/counter_filed/restored/resolved), `created_at`
- Workflow:
  1. Claimant files takedown (requires sworn statement of ownership)
  2. Content immediately hidden (not deleted)
  3. Content owner notified, given 10 business days to file counter-notice
  4. If counter-notice filed → content restored after 14 days unless claimant files legal action
  5. All steps logged with timestamps for legal compliance
- API: `POST /takedowns` (file claim), `POST /takedowns/:id/counter-notice`
- Legal page: DMCA agent contact information (required for safe harbor)

### 11.4 Content Flagging

**What it is:** Automated and manual content flagging that feeds into the moderation queue.

**Implementation approach:**
- `content_flags` table: `id`, `content_type`, `content_id`, `flag_type` (untagged_nsfw/spam/suspicious), `source` (automated/community), `confidence` (for automated), `status` (pending/reviewed), `created_at`
- **Automated:** On image upload, optionally call external NSFW detection API (e.g., AWS Rekognition, custom model). Flag if NSFW detected but content_rating is SFW.
- **Community:** Users flag content → after N independent flags, auto-escalate to moderation queue
- Flag threshold: configurable (e.g., 3 community flags → auto-review)
- Feeds into Feature 13.3 moderation queue

## Dependencies

### Requires (must be built first)
- [Feature 1.1](../01-atproto-auth/README.md) — authenticated users
- [Feature 2](../02-identity-profile/README.md) — profiles and content to moderate

### Enables (unlocked after this is built)
- [Feature 13.3](../13-platform-admin/README.md) — moderation queue processes reports and flags
- Safer platform: required before any public launch

## Implementation Phases

### Phase 1: Block/Mute & Reporting
- `user_blocks` table + block/mute API
- Block filtering in content queries (add WHERE NOT IN blocked_users)
- `reports` table + report submission API
- Report rate limiting
- Crates: domain (Report entity, BlockList), persistence, application, api

### Phase 2: DMCA & Content Flagging
- `takedown_requests` table + full workflow
- DMCA counter-notice flow
- `content_flags` table
- Community flagging with threshold escalation
- Automated NSFW detection integration (optional, behind feature flag)
- Integration with Feature 13 moderation queue

### Phase 3: Post-implementation
- DMCA agent page (legal requirement)
- Moderation response time SLAs
- Block list performance optimization (caching hot block lists)
- Automated flagging accuracy tracking and tuning
- User trust scores (based on report/flag history)
- Transparency report (aggregate moderation statistics)
- Legal review of DMCA compliance

## Assumptions

- Platform qualifies for DMCA safe harbor (must designate DMCA agent, respond promptly)
- Automated NSFW detection is supplementary — furry art confuses standard classifiers (high false positive rate)
- Block/mute lists are private (blocked user doesn't know)
- Reporter identity hidden from reported user but visible to admins
- Community flagging threshold prevents single-user abuse

## Shortcomings & Known Limitations

- **Automated NSFW detection for furry art is unreliable** — species-specific styles confuse standard models. High false positive rate expected.
- **DMCA process requires legal expertise** to implement correctly — mistakes could lose safe harbor protection
- **No appeal process** for content removal beyond DMCA counter-notice
- **Block checking on every query** adds performance overhead — needs caching strategy
- **Coordinated reporting abuse** (mass-reporting to harass a user) not addressed beyond rate limiting
- **No proactive content scanning** — only reactive (reports and flags after upload)
- **CSAM detection** (PhotoDNA or equivalent) is legally required in many jurisdictions — not yet integrated
- **Moderation at scale** requires paid moderators — volunteer moderation doesn't sustain
