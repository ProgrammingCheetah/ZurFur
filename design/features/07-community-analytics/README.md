# Feature 7: Community & Analytics

## Overview

Drives engagement and informed decision-making. Commission subscriptions alert users when artists open queues. Gamification rewards positive platform interactions. The Strategy Engine surfaces transparent, open-formula analytics so both artists and clients can make data-driven decisions.

## Sub-features

### 7.1 Commission Subscriptions

**What it is:** Push notifications triggered when a specific artist opens their commission queue.

**Implementation approach:**
- `artist_subscriptions` table: `subscriber_id`, `artist_id`, `created_at`
- When artist toggles `commission_status` to "open" (Feature 2.1), query all subscribers
- Emit notification events → delivered via Feature 9 (in-app, push, email)
- API: `POST /artists/:id/subscribe`, `DELETE /artists/:id/subscribe`, `GET /me/subscriptions`

### 7.2 Gamification

**What it is:** XP, Badges, and Community Rewards for successful transactions and positive platform interactions.

**Implementation approach:**
- `user_xp` table: `user_id`, `total_xp`, `level`
- `xp_events` table: `user_id`, `amount`, `reason`, `source_id`, `created_at`
- `badges` table: `id`, `name`, `description`, `icon_url`, `criteria_json`
- `user_badges` table: `user_id`, `badge_id`, `awarded_at`
- XP triggers: commission completed (+50), first commission (+100), positive review (+25), streak bonuses
- Badge criteria evaluated on XP events: e.g., "Complete 10 commissions" → "Veteran Commissioner" badge
- Leaderboards: optional, query top users by XP within timeframes
- XP and badges are cosmetic — no platform functionality gating

### 7.3 The Strategy Engine (Open Metrics)

**What it is:** Transparent month-to-month statistics. Clients see artist turnaround trends and price changes. Artists see client risk assessments.

**Implementation approach:**
- **Artist metrics** (`artist_monthly_metrics` table): `artist_id`, `month`, `avg_turnaround_days`, `avg_price_cents`, `completion_rate`, `dispute_rate`, `total_commissions`, `total_revenue_cents`
- **Client metrics** (`client_monthly_metrics` table): `user_id`, `month`, `payment_timeliness_score`, `dispute_rate`, `total_commissions`, `avg_rating_given`
- Aggregation: monthly background job queries commission events and transactions, computes metrics, inserts into metrics tables
- All formulas are documented publicly (no black box algorithms)
- API: `GET /artists/:id/metrics`, `GET /users/:id/metrics` (with appropriate privacy controls)
- Risk assessment: flag clients with dispute_rate > threshold, display warning to artists

## Dependencies

### Requires (must be built first)
- [Feature 1.1](../01-atproto-auth/README.md) — authenticated users
- [Feature 3](../03-commission-engine/README.md) — commission event data to aggregate
- [Feature 4](../04-financial-gateway/README.md) — financial data for pricing analytics
- [Feature 9](../09-notification-system/README.md) — delivery mechanism for subscription notifications (7.1)

### Enables (unlocked after this is built)
- [Feature 8](../08-search-discovery/README.md) — search ranking and filtering by metrics
- [Feature 12](../12-dispute-resolution/README.md) — client risk scores inform dispute context

## Implementation Phases

### Phase 1: Subscriptions & XP
- `artist_subscriptions` table + subscribe/unsubscribe API
- Notification emission on artist status change (depends on Feature 9)
- `user_xp`, `xp_events` tables
- XP award logic triggered by commission lifecycle events
- Level calculation (XP thresholds per level)
- API: subscription management, XP/level display on profiles

### Phase 2: Badges & Metrics
- `badges`, `user_badges` tables
- Badge criteria evaluation engine (rule-based)
- Monthly metrics aggregation job
- `artist_monthly_metrics` and `client_monthly_metrics` tables
- Metrics API endpoints
- Risk assessment warnings for artists viewing client profiles

### Phase 3: Post-implementation
- Metrics formula documentation (public page)
- Anti-gaming heuristics (detect fake commissions to boost stats)
- Historical trend visualization data (for frontend charts)
- Privacy review: ensure metric visibility respects user preferences
- Performance: metrics aggregation job optimization for large datasets
- A/B testing infrastructure for gamification balance tweaks

## Assumptions

- XP/badge values are platform-defined, not user-configurable
- Analytics are aggregated monthly (not real-time dashboards)
- "Open metrics" means formulas are public, but individual data respects privacy settings
- Gamification is cosmetic only — no feature gating behind XP levels
- Monthly aggregation job runs during low-traffic hours

## Shortcomings & Known Limitations

- **Gaming the metrics:** Fake commissions to boost stats are possible — needs anti-fraud heuristics
- **Early platform has insufficient data** for meaningful analytics — metrics become useful at scale
- **Aggregation job performance** degrades with data volume — needs optimization strategy
- **Privacy concerns:** Client dispute rate visibility could be controversial
- **XP balancing** requires iteration — initial values will likely need adjustment
- **No real-time metrics** — monthly aggregation means data is up to 30 days stale
- **Leaderboards** can create toxic competition — consider opt-in only
