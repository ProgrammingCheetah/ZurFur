> **Revised 2026-04-08** — Updated for org-centric identity, feed-driven content, headless commissions, and plugin-as-org architecture.

# Feature 13: Platform Administration

## Overview

Internal tooling for the team running Zurfur. Admin roles live on users (not orgs) — admins are platform operators, not org members. Supports separate user suspensions and org suspensions, plugin org moderation (disable feed subscriptions, delist from search), AT Protocol admin operations (PDS takedowns, record labeling), financial auditing, a centralized moderation queue (including feed posts as actionable content), and system health monitoring. Required before public launch but not before private beta.

## Sub-features

### 13.1 User & Org Management Dashboard

**What it is:** View, suspend, ban user accounts and orgs, and audit activity. User suspensions and org suspensions are separate actions.

**Implementation approach:**
- Admin role on User entity (not on orgs): `role` field (user/moderator/finance/admin) — admins are platform operators
- **User suspensions:** `user_suspensions` table: `id`, `user_id`, `reason`, `suspended_by`, `suspended_at`, `expires_at` (null = permanent)
- **Org suspensions:** `org_suspensions` table: `id`, `org_id`, `reason`, `suspended_by`, `suspended_at`, `expires_at` (null = permanent)
- User suspension effects: blocked from login, content hidden from public, active commissions flagged
- Org suspension effects: org profile hidden, commission queue closed, feed posts hidden, members notified, active commissions flagged for review
- `admin_audit_log` table: `id`, `admin_id`, `action`, `target_type` (user/org/plugin_org/feed_post/pds_record), `target_id`, `details_json`, `created_at` — tracks all admin actions for accountability
- Admin API (role-gated):
  - Users: `GET /admin/users`, `POST /admin/users/:id/suspend`, `POST /admin/users/:id/unsuspend`, `GET /admin/users/:id/activity`
  - Orgs: `GET /admin/orgs`, `POST /admin/orgs/:id/suspend`, `POST /admin/orgs/:id/unsuspend`, `GET /admin/orgs/:id/activity`
- Activity view: recent commissions, reports filed/received, transactions, login history, org memberships

### 13.2 Financial Auditing

**What it is:** Transaction logs, payout tracking, fee reconciliation for the operations team.

**Implementation approach:**
- Read-only views over Feature 4 transaction/payout data
- Dashboard aggregations: daily revenue, total payouts, fee collection, chargeback rate, pending payouts
- Reconciliation: compare Stripe records with platform records, flag discrepancies
- Export: CSV download of transactions for accounting
- Admin API: `GET /admin/finance/summary`, `GET /admin/finance/transactions`, `GET /admin/finance/payouts`, `GET /admin/finance/export`
- Role-gated: only `finance` and `admin` roles

### 13.3 Moderation Queue

**What it is:** Centralized queue for reviewing reports (Feature 11), disputes (Feature 12), content flags, and feed posts. Feed posts are actionable content in the moderation queue.

**Implementation approach:**
- Unified view joining: `reports`, `disputes`, `takedown_requests`, `content_flags`
- **Actionable content types:** user profiles, org profiles, commission cards, feed posts, plugin orgs
- Priority sorting: DMCA/takedowns > safety reports > content flags > quality disputes
- Assignment: moderators claim items from the queue
- `moderation_assignments` table: `moderation_item_type`, `moderation_item_id`, `moderator_id`, `claimed_at`, `resolved_at`
- Actions per item type:
  - Reports (users): dismiss, warn user, suspend user, remove content
  - Reports (orgs): dismiss, warn org, suspend org, remove content
  - Reports (plugin orgs): dismiss, disable feed subscriptions, delist from search, suspend org
  - Feed posts: remove post, flag for tag update, warn poster
  - Disputes: view evidence, issue resolution
  - Takedowns: follow DMCA workflow (including PDS record takedowns)
  - Flags: confirm/dismiss flag, update content tags
- Admin API: `GET /admin/moderation`, `POST /admin/moderation/:type/:id/claim`, `POST /admin/moderation/:type/:id/resolve`

### 13.4 Plugin Org Moderation

**What it is:** Dedicated moderation actions for plugin orgs that misbehave or violate platform policies.

**Implementation approach:**
- Plugin org moderation actions:
  - **Disable feed subscriptions:** Plugin org can no longer subscribe to feeds (cuts off data access)
  - **Delist from search:** Plugin org hidden from search results (Feature 8) but not fully suspended
  - **Suspend:** Full org suspension (see 13.1)
  - **Revoke:** Permanent ban — all feed subscriptions terminated, org marked as revoked
- Plugin org status tracked on org entity: `plugin_status` (active/restricted/delisted/suspended/revoked)
- Graduated enforcement: warning → delist → disable subscriptions → suspend → revoke
- All plugin moderation actions logged in `admin_audit_log`

### 13.5 AT Protocol Admin Operations

**What it is:** Administrative operations on AT Protocol PDS records — takedowns, record labeling, and content management.

**Implementation approach:**
- PDS takedowns: remove or label records on the platform's PDS instance
- Record labeling: apply AT Protocol labels (e.g., `!warn`, `!takedown`, `nsfw`) to records
- Bulk operations: label/takedown multiple records matching criteria
- PDS sync status: view records that are out of sync between PostgreSQL and PDS
- Admin API: `POST /admin/pds/takedown`, `POST /admin/pds/label`, `GET /admin/pds/sync-status`
- Integrates with Feature 11.3 DMCA workflow for PDS record takedowns

### 13.6 System Health & Metrics

**What it is:** API performance, error rates, infrastructure health dashboards.

**Implementation approach:**
- Prometheus metrics endpoint: `GET /metrics`
- Key metrics:
  - HTTP: request count, latency histogram (p50, p95, p99), error rate by status code
  - WebSocket: active connections, messages/sec
  - Database: pool utilization, query latency, connection count
  - PDS: sync lag, record count, takedown queue depth
  - Business: active commissions, daily signups, payment volume, feed post volume
- Middleware: Axum layer that records request duration and status
- Infrastructure: PostgreSQL pg_stat, connection pool metrics from SQLx
- Visualization: Grafana dashboards (external, not built into Zurfur)
- Alerting: Grafana alerts or PagerDuty integration for SLA breaches

## Dependencies

### Requires (must be built first)
- [Feature 1.1](../01-atproto-auth/README.md) — authentication + admin role
- [Feature 2](../02-identity-profile/README.md) — org entity for org suspensions
- [Feature 4](../04-financial-gateway/README.md) — transaction data to audit (for 13.2)
- [Feature 11](../11-content-moderation/README.md) — reports and flags to review (for 13.3)
- [Feature 12](../12-dispute-resolution/README.md) — disputes to arbitrate (for 13.3)
- Feed infrastructure — feed posts are actionable content in moderation queue

### Enables (unlocked after this is built)
- Operational capability — **required before public launch**
- Platform safety: moderation queue is the enforcement arm of Features 11 and 12
- Plugin ecosystem governance: moderation controls for plugin orgs

## Implementation Phases

### Phase 1: Admin Role & User/Org Management
- Add `role` field to User entity + migration (admin roles on users, not orgs)
- Role-based middleware (reject non-admin requests to /admin/* routes)
- `user_suspensions` table + suspend/unsuspend API
- `org_suspensions` table + suspend/unsuspend API
- `admin_audit_log` table — log every admin action with target_type support for users, orgs, feed posts, PDS records
- Basic user and org listing and activity views
- Crates: domain (role enum, suspension entities), persistence, application (admin use cases), api (admin routes + role middleware)

### Phase 2: Moderation Queue, Plugin Moderation & Financial Dashboard
- Moderation queue: unified view including feed posts as actionable content, claim/resolve workflow
- `moderation_assignments` table
- Priority sorting logic
- Plugin org moderation actions (delist, disable subscriptions, suspend, revoke)
- Financial summary endpoints (read-only aggregations)
- CSV export
- Integration with Feature 11 reports and Feature 12 disputes

### Phase 3: AT Protocol Admin & Post-implementation
- PDS takedown and record labeling API
- PDS sync status monitoring
- Prometheus metrics middleware for Axum (including PDS metrics)
- Database and connection pool metrics
- Business metrics collection
- Grafana dashboard templates (shipped as JSON, imported by ops team)
- Alerting configuration documentation
- Admin UI: decide between custom frontend or off-the-shelf admin framework (e.g., AdminJS, React Admin)
- Moderation guidelines documentation for moderators
- Incident response playbook (what to do when metrics go red)

## Assumptions

- Admin roles are on users, not orgs — admins are platform operators, not affiliated with any org
- Small admin team initially (< 5 people) — tooling doesn't need to scale to hundreds of moderators
- Admin interface is a separate frontend, not embedded in the user-facing app
- Roles: `admin` (full access), `moderator` (reports + disputes + plugin moderation), `finance` (transactions) — simple RBAC
- Prometheus + Grafana is the monitoring stack (industry standard, widely supported)
- Admin audit log is append-only and never deleted
- Platform has admin-level access to its PDS instance for takedown operations
- User suspensions and org suspensions are independent — suspending a user does not automatically suspend their personal org (but may be desired in some cases)

## Shortcomings & Known Limitations

- **Admin dashboard is a separate frontend** that needs to be built or chosen (off-the-shelf vs custom)
- **No audit log for admin-on-admin actions** (who changed another admin's role)
- **Moderation quality** depends on human moderators — no automated moderation decisions beyond Feature 11's auto-flagging
- **No automated escalation** if moderation queue grows too large — manual monitoring required
- **Financial auditing is read-only** — no tools for manual transaction corrections or adjustments
- **System health metrics don't cover business KPIs** (user retention, commission conversion rate) — those need a separate analytics pipeline
- **No multi-tenancy** for admin roles — can't restrict a moderator to only certain content categories
- **Admin API rate limiting** not addressed — a compromised admin account could exfiltrate data rapidly
- **User vs org suspension interaction** is ambiguous — need clear policy for whether suspending a user cascades to their personal org
- **PDS admin operations** require careful coordination — a bad takedown could affect AT Protocol federation
- **Plugin org moderation graduation** (warning → delist → suspend → revoke) needs clear criteria to avoid arbitrary enforcement
