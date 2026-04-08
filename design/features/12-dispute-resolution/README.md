# Feature 12: Dispute Resolution

## Overview

When real money is escrowed and commissions go wrong, there must be a formal process. Either party can file a dispute on an active card, freezing fund release. Both sides submit evidence referencing the card's immutable event history. Clear-cut cases are auto-resolved; complex ones escalate to platform moderators.

## Sub-features

### 12.1 Dispute Filing

**What it is:** Either party opens a formal dispute on a commission card, freezing pending fund releases.

**Implementation approach:**
- `disputes` table: `id`, `commission_id`, `filed_by`, `reason_category`, `description`, `status` (open/evidence_gathering/under_review/resolved/escalated), `created_at`, `resolved_at`
- Reason categories: `non_delivery`, `quality_dispute`, `non_payment`, `scope_creep`, `communication_breakdown`, `other`
- Filing a dispute:
  1. Creates dispute record
  2. Emits `DisputeOpened` event on the Commission Card
  3. Freezes any pending payouts (updates transaction status in Feature 4)
  4. Notifies all card participants (via Feature 9)
- API: `POST /commissions/:id/dispute`
- Limit: one active dispute per commission at a time

### 12.2 Evidence Submission

**What it is:** Both parties submit evidence, which can reference specific events from the card's immutable history.

**Implementation approach:**
- `dispute_evidence` table: `id`, `dispute_id`, `submitted_by`, `content` (text explanation), `attachments_json`, `referenced_event_ids` (array of commission event IDs), `created_at`
- Evidence can include: text statements, file attachments, links to specific card events (timestamped proof)
- Both parties see each other's evidence (transparent process)
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
- Resolution triggers: unfreeze/refund funds (Feature 4), emit `DisputeResolved` event on Card

### 12.4 Refund & Payout Policies

**What it is:** Transparent refund rules based on milestone completion and deliverables.

**Implementation approach:**
- Platform-wide policies stored as configuration (not per-artist):
  - Pre-work cancellation: full refund minus processing fee
  - Mid-work cancellation: refund proportional to incomplete milestones
  - Quality disputes: case-by-case (manual review)
  - Non-delivery: full refund after deadline + grace period
- Refund triggers Stripe refund API (partial or full)
- Artist TOS (Feature 10) informs but does not override platform policy
- Policy document: publicly accessible at `/policies/refunds`

## Dependencies

### Requires (must be built first)
- [Feature 1.1](../01-atproto-auth/README.md) — authenticated users
- [Feature 3](../03-commission-engine/README.md) — commission cards with event history (evidence references events)
- [Feature 4](../04-financial-gateway/README.md) — escrowed funds to freeze/release/refund

### Enables (unlocked after this is built)
- [Feature 7.3](../07-community-analytics/README.md) — dispute data feeds client/artist risk scores
- [Feature 13.3](../13-platform-admin/README.md) — complex disputes enter moderation queue

### Also references
- [Feature 10](../10-artist-tos/README.md) — artist TOS terms inform dispute context (what was agreed)

## Implementation Phases

### Phase 1: Filing & Evidence
- `disputes` table and domain entity
- Dispute filing API with fund freezing
- `DisputeOpened` event emission on Card
- `dispute_evidence` table + submission API
- Evidence referencing commission events by ID
- Notification to all participants
- Crates: domain (Dispute entity), persistence, application (dispute use cases), api (dispute routes)

### Phase 2: Resolution & Refunds
- `dispute_resolutions` table
- Auto-resolution rules engine (configurable rule set)
- Manual resolution API for moderators
- Stripe refund integration (partial and full)
- Fund unfreeze on resolution
- `DisputeResolved` event emission on Card
- Dispute impact on client/artist metrics (Feature 7.3)

### Phase 3: Post-implementation
- Dispute analytics: average resolution time, auto-resolution rate, outcome distribution
- Escalation path to external mediation (for high-value disputes)
- Dispute prevention: early warning system (flag commissions at risk based on patterns)
- Legal review: ensure dispute process meets jurisdictional requirements
- Documentation: dispute process guide for artists and clients
- Time limit enforcement: auto-close disputes after N days of inactivity

## Assumptions

- Most disputes can be resolved with evidence from the card's immutable event history
- Platform acts as mediator, not legally binding arbitrator
- Auto-resolution is conservative (only for unambiguous cases)
- Refund policies are platform-wide, not per-artist (artist TOS informs but doesn't override)
- Small dispute volume initially (< 1% of commissions)

## Shortcomings & Known Limitations

- **Not legally binding arbitration** — platform decisions are recommendations. Either party can pursue legal action independently.
- **Auto-resolution rules are simplistic** — can't handle nuanced quality disputes, partial delivery edge cases, or "it's not what I asked for" situations
- **Moderator tooling** (Feature 13) must exist for manual review to work
- **Time limits** for evidence gathering and resolution not enforced in Phase 1
- **International jurisdictions** complicate enforcement — different countries have different consumer protection laws
- **Repeat offender escalation** not automated — needs integration with Feature 7.3 metrics
- **Partial refund calculations for installment plans** are complex and may be incorrect in edge cases
- **No escrow license** — platform-held funds may have legal implications depending on jurisdiction
