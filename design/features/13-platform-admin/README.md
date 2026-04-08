# Feature 13: Platform Administration

## Overview

Internal tooling for the team running Zurfur. User management, financial auditing, a centralized moderation queue, and system health monitoring. Required before public launch but not before private beta.

## Sub-features

### 13.1 User Management Dashboard

**What it is:** View, suspend, ban accounts, and audit user activity.

**Implementation approach:**
- Admin role on User entity: `role` field (user/moderator/finance/admin)
- `user_suspensions` table: `id`, `user_id`, `reason`, `suspended_by`, `suspended_at`, `expires_at` (null = permanent)
- Suspension effects: blocked from login, content hidden from public, active commissions flagged
- `admin_audit_log` table: `id`, `admin_id`, `action`, `target_type`, `target_id`, `details_json`, `created_at` — tracks all admin actions for accountability
- Admin API (role-gated): `GET /admin/users`, `POST /admin/users/:id/suspend`, `POST /admin/users/:id/unsuspend`, `GET /admin/users/:id/activity`
- Activity view: recent commissions, reports filed/received, transactions, login history

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

**What it is:** Centralized queue for reviewing reports (Feature 11), disputes (Feature 12), and content flags.

**Implementation approach:**
- Unified view joining: `reports`, `disputes`, `takedown_requests`, `content_flags`
- Priority sorting: DMCA/takedowns > safety reports > content flags > quality disputes
- Assignment: moderators claim items from the queue
- `moderation_assignments` table: `moderation_item_type`, `moderation_item_id`, `moderator_id`, `claimed_at`, `resolved_at`
- Actions per item type:
  - Reports: dismiss, warn user, suspend user, remove content
  - Disputes: view evidence, issue resolution
  - Takedowns: follow DMCA workflow
  - Flags: confirm/dismiss flag, update content rating
- Admin API: `GET /admin/moderation`, `POST /admin/moderation/:type/:id/claim`, `POST /admin/moderation/:type/:id/resolve`

### 13.4 System Health & Metrics

**What it is:** API performance, error rates, infrastructure health dashboards.

**Implementation approach:**
- Prometheus metrics endpoint: `GET /metrics`
- Key metrics:
  - HTTP: request count, latency histogram (p50, p95, p99), error rate by status code
  - WebSocket: active connections, messages/sec
  - Database: pool utilization, query latency, connection count
  - Business: active commissions, daily signups, payment volume
- Middleware: Axum layer that records request duration and status
- Infrastructure: PostgreSQL pg_stat, connection pool metrics from SQLx
- Visualization: Grafana dashboards (external, not built into Zurfur)
- Alerting: Grafana alerts or PagerDuty integration for SLA breaches

## Dependencies

### Requires (must be built first)
- [Feature 1.1](../01-atproto-auth/README.md) — authentication + admin role
- [Feature 4](../04-financial-gateway/README.md) — transaction data to audit (for 13.2)
- [Feature 11](../11-content-moderation/README.md) — reports and flags to review (for 13.3)
- [Feature 12](../12-dispute-resolution/README.md) — disputes to arbitrate (for 13.3)

### Enables (unlocked after this is built)
- Operational capability — **required before public launch**
- Platform safety: moderation queue is the enforcement arm of Features 11 and 12

## Implementation Phases

### Phase 1: Admin Role & User Management
- Add `role` field to User entity + migration
- Role-based middleware (reject non-admin requests to /admin/* routes)
- `user_suspensions` table + suspend/unsuspend API
- `admin_audit_log` table — log every admin action
- Basic user listing and activity view
- Crates: domain (role enum, suspension entity), persistence, application (admin use cases), api (admin routes + role middleware)

### Phase 2: Moderation Queue & Financial Dashboard
- Moderation queue: unified view, claim/resolve workflow
- `moderation_assignments` table
- Priority sorting logic
- Financial summary endpoints (read-only aggregations)
- CSV export
- Integration with Feature 11 reports and Feature 12 disputes

### Phase 3: System Health & Post-implementation
- Prometheus metrics middleware for Axum
- Database and connection pool metrics
- Business metrics collection
- Grafana dashboard templates (shipped as JSON, imported by ops team)
- Alerting configuration documentation
- Admin UI: decide between custom frontend or off-the-shelf admin framework (e.g., AdminJS, React Admin)
- Moderation guidelines documentation for moderators
- Incident response playbook (what to do when metrics go red)

## Assumptions

- Small admin team initially (< 5 people) — tooling doesn't need to scale to hundreds of moderators
- Admin interface is a separate frontend, not embedded in the user-facing app
- Roles: `admin` (full access), `moderator` (reports + disputes), `finance` (transactions) — simple RBAC
- Prometheus + Grafana is the monitoring stack (industry standard, widely supported)
- Admin audit log is append-only and never deleted

## Shortcomings & Known Limitations

- **Admin dashboard is a separate frontend** that needs to be built or chosen (off-the-shelf vs custom)
- **No audit log for admin-on-admin actions** (who changed another admin's role)
- **Moderation quality** depends on human moderators — no automated moderation decisions beyond Feature 11's auto-flagging
- **No automated escalation** if moderation queue grows too large — manual monitoring required
- **Financial auditing is read-only** — no tools for manual transaction corrections or adjustments
- **System health metrics don't cover business KPIs** (user retention, commission conversion rate) — those need a separate analytics pipeline
- **No multi-tenancy** for admin roles — can't restrict a moderator to only certain content categories
- **Admin API rate limiting** not addressed — a compromised admin account could exfiltrate data rapidly
