> **Revised 2026-04-08** — Updated for org-centric identity, feed-driven content, headless commissions, and plugin-as-org architecture.

# Feature 12: Dispute Resolution

## Overview

When real money is escrowed and commissions go wrong, there must be a formal process. Disputes are between a user (client) and an org (artist). Either party can file a dispute, which is recorded as an add-on slot on the commission card, freezing fund release. Both sides submit evidence referencing the card's immutable event history. Plugin orgs may be involved as third parties in disputes. Clear-cut cases are auto-resolved; complex ones escalate to platform moderators. All dispute records are private (PostgreSQL only — never published to PDS).

## Sub-features

### 12.1 Dispute Filing

**What it is:** Either party (client user or org) opens a formal dispute on a commission card, freezing pending fund releases. The dispute is an add-on slot on the commission card.

**Implementation approach:**
- `disputes` table: `id`, `commission_id`, `filed_by_user_id`, `filed_by_org_id` (nullable — set if org files), `respondent_user_id`, `respondent_org_id`, `reason_category`, `description`, `status` (open/evidence_gathering/under_review/resolved/escalated), `created_at`, `resolved_at`
- Reason categories: `non_delivery`, `quality_dispute`, `non_payment`, `scope_creep`, `communication_breakdown`, `plugin_malfunction`, `other`
- Filing a dispute:
  1. Creates dispute record
  2. Attaches dispute as an add-on slot on the commission card
  3. Freezes any pending payouts (updates transaction status in Feature 4)
  4. Notifies all card participants and relevant org members (via Feature 9)
- If a plugin org was involved in the commission (e.g., payment processing, asset delivery), they may be named as a third party
- `dispute_third_parties` table: `dispute_id`, `org_id`, `role` (witness/involved_party), `added_at`
- API: `POST /commissions/:id/dispute`
- Limit: one active dispute per commission at a time
- All dispute data is private — PostgreSQL only, never published to PDS

### 12.2 Evidence Submission

**What it is:** Both parties submit evidence, which can reference specific events from the card's immutable history.

**Implementation approach:**
- `dispute_evidence` table: `id`, `dispute_id`, `submitted_by_user_id`, `submitted_by_org_id`, `content` (text explanation), `attachments_json`, `referenced_event_ids` (array of commission event IDs), `created_at`
- Evidence can include: text statements, file attachments, links to specific card events (timestamped proof)
- Both parties see each other's evidence (transparent process)
- Plugin org third parties can submit evidence relevant to their involvement
- Evidence submission deadline: configurable (e.g., 7 days from filing)
- API: `POST /disputes/:id/evidence`, `GET /disputes/:id/evidence`

### 12.3 Resolution Flow

**What it is:** Structured mediation with auto-resolution for clear cases and escalation for complex ones.

**Implementation approach:**
- `dispute_resolutions` table: `id`, `dispute_id`, `resolved_by` (system/moderator_id), `resolution_type`, `description`, `refund_amount_cents`, `created_at`
- Resolution types: `refund_full`, `refund_partial`, `no_refund`, `mutual_agreement`, `escalated_external`
- **Auto-resolution rules:**
  - No delivery past deadline + fully paid → eligible for full refund
  - Partial delivery (some milestones completed) → eligible for partial refund proportional to incomplete milestones
  - Non-payment (invoice overdue > 30 days) → auto-cancel commission
- **Manual review:** Complex cases assigned to moderators via Feature 13.3
- Resolution triggers: unfreeze/refund funds (Feature 4), update dispute add-on slot on card, emit resolution event
- Resolution record references both the client user and the org

### 12.4 Refund & Payout Policies

**What it is:** Transparent refund rules based on milestone completion and deliverables.

**Implementation approach:**
- Platform-wide policies stored as configuration (not per-org):
  - Pre-work cancellation: full refund minus processing fee
  - Mid-work cancellation: refund proportional to incomplete milestones
  - Quality disputes: case-by-case (manual review)
  - Non-delivery: full refund after deadline + grace period
- Refund triggers Stripe refund API (partial or full)
- Org TOS (Feature 10) informs but does not override platform policy
- Policy document: publicly accessible at `/policies/refunds`

## Dependencies

### Requires (must be built first)
- [Feature 1.1](../01-atproto-auth/README.md) — authenticated users
- [Feature 2](../02-identity-profile/README.md) — org entity (disputes involve user vs org)
- [Feature 3](../03-commission-engine/README.md) — commission cards with event history and add-on slots
- [Feature 4](../04-financial-gateway/README.md) — escrowed funds to freeze/release/refund

### Enables (unlocked after this is built)
- Dispute outcomes feed into [Feature 7.3](../07-community-analytics/README.md) risk scores (soft enhancement, not a hard dependency).
- [Feature 13.3](../13-platform-admin/README.md) — complex disputes enter moderation queue

### Also references
- [Feature 10](../10-artist-tos/README.md) — org TOS terms inform dispute context (what was agreed)

## Implementation Phases

### Phase 1: Filing & Evidence
- `disputes` table and domain entity (user vs org structure)
- Dispute as add-on slot on commission card
- Dispute filing API with fund freezing
- `dispute_third_parties` table for plugin org involvement
- `dispute_evidence` table + submission API (supporting both user and org submitters)
- Evidence referencing commission events by ID
- Notification to all participants and relevant org members
- Crates: domain (Dispute entity), persistence, application (dispute use cases), api (dispute routes)

### Phase 2: Resolution & Refunds
- `dispute_resolutions` table
- Auto-resolution rules engine (configurable rule set)
- Manual resolution API for moderators
- Stripe refund integration (partial and full)
- Fund unfreeze on resolution
- Dispute add-on slot update on commission card
- Dispute impact on client/org metrics (Feature 7.3)

### Phase 3: Post-implementation
- Dispute analytics: average resolution time, auto-resolution rate, outcome distribution
- Plugin org third-party evidence workflow
- Escalation path to external mediation (for high-value disputes)
- Dispute prevention: early warning system (flag commissions at risk based on patterns)
- Legal review: ensure dispute process meets jurisdictional requirements
- Documentation: dispute process guide for users and orgs
- Time limit enforcement: auto-close disputes after N days of inactivity

## Assumptions

- Disputes are always user (client) vs org (artist side) — never user vs user or org vs org
- Most disputes can be resolved with evidence from the card's immutable event history
- Platform acts as mediator, not legally binding arbitrator
- Auto-resolution is conservative (only for unambiguous cases)
- Refund policies are platform-wide, not per-org (org TOS informs but doesn't override)
- Small dispute volume initially (< 1% of commissions)
- All dispute data remains private in PostgreSQL — never published to PDS
- Plugin org third-party involvement is optional and case-by-case

## Shortcomings & Known Limitations

- **Not legally binding arbitration** — platform decisions are recommendations. Either party can pursue legal action independently.
- **Auto-resolution rules are simplistic** — can't handle nuanced quality disputes, partial delivery edge cases, or "it's not what I asked for" situations
- **Moderator tooling** (Feature 13) must exist for manual review to work
- **Time limits** for evidence gathering and resolution not enforced in Phase 1
- **International jurisdictions** complicate enforcement — different countries have different consumer protection laws
- **Repeat offender escalation** not automated — needs integration with Feature 7.3 metrics
- **Partial refund calculations for installment plans** are complex and may be incorrect in edge cases
- **No escrow license** — platform-held funds may have legal implications depending on jurisdiction
- **Plugin org disputes** add complexity — determining liability when a plugin org is involved may be ambiguous
- **Org-side representation** in disputes may involve multiple org members — need clear rules on who speaks for the org
