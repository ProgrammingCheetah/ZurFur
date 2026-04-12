> **Revised 2026-04-08** — Updated for org-centric identity, feed-driven content, headless commissions, and plugin-as-org architecture.

# Feature 8: Community & Analytics

## Overview

Drives engagement and informed decision-making. Feed subscriptions alert users when orgs open commission queues. Gamification rewards positive platform interactions, accruing XP and badges to both users and orgs separately. The Strategy Engine surfaces transparent, open-formula analytics so both orgs and clients can make data-driven decisions. Public metrics are published to PDS; private metrics remain PostgreSQL-only.

## Sub-features

### 8.1 Feed Subscriptions

**What it is:** Subscribe to an org's commissions feed. When the org opens their commission queue, subscribers receive push notifications via the feed system.

**Implementation approach:**
- Subscriptions are feed subscriptions via the `entity_feeds` system — subscribing to an org's commissions feed
- When an org updates their availability tags (e.g., adds `status:open` tag) and publishes an availability post to their commissions feed, all subscribers are notified
- "Open Now" is a feed view: a projection that filters for orgs with the `status:open` tag. Commission availability is a tag on the org, not a database column.
- Emit notification events → delivered via Feature 10 (in-app, push, email)
- API: `POST /orgs/:id/feeds/commissions/subscribe`, `DELETE /orgs/:id/feeds/commissions/subscribe`, `GET /me/feed-subscriptions`

### 8.2 Gamification

**What it is:** XP, Badges, and Community Rewards for successful transactions and positive platform interactions. XP and badges accrue to users AND orgs separately.

**Implementation approach:**
- `user_xp` table: `user_id`, `total_xp`, `level`
- `org_xp` table: `org_id`, `total_xp`, `level`
- `xp_events` table: `entity_type` (user/org), `entity_id`, `amount`, `reason`, `source_id`, `created_at`
- `badges` table: `id`, `name`, `description`, `icon_url`, `criteria_json`, `target_type` (user/org/both)
- `user_badges` table: `user_id`, `badge_id`, `awarded_at`
- `org_badges` table: `org_id`, `badge_id`, `awarded_at`
- XP triggers: commission completed (+50 to both user and org), first commission (+100), positive review (+25), streak bonuses
- Badge criteria evaluated on XP events: e.g., org "Complete 10 commissions" → "Veteran Studio" badge; user "Commission from 5 different orgs" → "Patron" badge
- Leaderboards: optional, query top users/orgs by XP within timeframes
- XP and badges are cosmetic — no platform functionality gating

### 8.3 The Strategy Engine (Open Metrics)

**What it is:** Transparent month-to-month statistics. Clients see org turnaround trends and price changes. Orgs see client risk assessments. Public metrics published to PDS as AT Protocol records; private metrics PostgreSQL-only.

**Implementation approach:**
- **Org metrics** (`org_monthly_metrics` table): `org_id`, `month`, `avg_turnaround_days`, `avg_price_cents`, `completion_rate`, `dispute_rate`, `total_commissions`, `total_revenue_cents`
- **Client metrics** (`client_monthly_metrics` table): `user_id`, `month`, `payment_timeliness_score`, `dispute_rate`, `total_commissions`, `avg_rating_given`
- **Two-tier data split:**
  - Public metrics (completion_rate, avg_turnaround, total_commissions) → published to PDS as AT Protocol records
  - Private metrics (revenue_cents, dispute_rate, risk scores) → PostgreSQL only
- Aggregation: monthly background job queries commission events and transactions, computes metrics, inserts into metrics tables, publishes public subset to PDS
- All formulas are documented publicly (no black box algorithms)
- API: `GET /orgs/:id/metrics`, `GET /users/:id/metrics` (with appropriate privacy controls)
- Risk assessment: flag clients with dispute_rate > threshold, display warning to orgs

## Dependencies

### Requires (must be built first)
- [Feature 1.1](../01-atproto-auth/README.md) — authenticated users
- [Feature 2](../02-identity-profile/README.md) — org entity and feed infrastructure
- [Feature 4](../04-commission-engine/README.md) — commission event data to aggregate
- [Feature 5](../05-financial-gateway/README.md) — financial data for pricing analytics
- [Feature 10](../10-notification-system/README.md) — delivery mechanism for subscription notifications (8.1)

### Enables (unlocked after this is built)
- [Feature 9](../09-search-discovery/README.md) — search ranking and filtering by metrics
- [Feature 13](../13-dispute-resolution/README.md) — client risk scores inform dispute context

## Implementation Phases

### Phase 1: Feed Subscriptions & XP
- Feed subscription mechanism via `entity_feeds` infrastructure
- "Open Now" as a feed view (filters for orgs with `status:open` tag)
- Notification emission on org availability tag changes and feed status posts (depends on Feature 10)
- `user_xp`, `org_xp`, `xp_events` tables
- XP award logic triggered by commission lifecycle events, accruing to both user and org
- Level calculation (XP thresholds per level)
- API: feed subscription management, XP/level display on profiles

### Phase 2: Badges & Metrics
- `badges`, `user_badges`, `org_badges` tables
- Badge criteria evaluation engine (rule-based, supporting user and org targets)
- Monthly metrics aggregation job
- `org_monthly_metrics` and `client_monthly_metrics` tables
- Public metrics publishing to PDS
- Metrics API endpoints
- Risk assessment warnings for orgs viewing client profiles

### Phase 3: Post-implementation
- Metrics formula documentation (public page)
- Anti-gaming heuristics (detect fake commissions to boost stats)
- Historical trend visualization data (for frontend charts)
- Privacy review: ensure metric visibility respects user preferences and two-tier data boundaries
- Performance: metrics aggregation job optimization for large datasets
- A/B testing infrastructure for gamification balance tweaks

## Assumptions

- XP/badge values are platform-defined, not user-configurable
- XP and badges accrue to users and orgs independently (a user's personal org XP is separate from their user XP)
- Analytics are aggregated monthly (not real-time dashboards)
- "Open metrics" means formulas are public, but individual data respects privacy settings
- Gamification is cosmetic only — no feature gating behind XP levels
- Monthly aggregation job runs during low-traffic hours
- Public metrics on PDS are eventually consistent with PostgreSQL source

## Shortcomings & Known Limitations

- **Gaming the metrics:** Fake commissions to boost stats are possible — needs anti-fraud heuristics
- **Early platform has insufficient data** for meaningful analytics — metrics become useful at scale
- **Aggregation job performance** degrades with data volume — needs optimization strategy
- **Privacy concerns:** Client dispute rate visibility could be controversial
- **XP balancing** requires iteration — initial values will likely need adjustment
- **No real-time metrics** — monthly aggregation means data is up to 30 days stale
- **Leaderboards** can create toxic competition — consider opt-in only
- **PDS sync lag:** Public metrics on PDS may lag behind PostgreSQL during aggregation windows
- **Dual XP tracking** (user + org) adds complexity — must be clear in UI which XP is being displayed
