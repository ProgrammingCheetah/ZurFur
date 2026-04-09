> **Revised 2026-04-08** — Updated for org-centric identity, feed-driven content, headless commissions, and plugin-as-org architecture.

# Feature 9: Notification System

## Overview

The platform's event delivery infrastructure. Almost every feature emits notifications — commission state changes, payment confirmations, new messages, org queue openings. This feature provides the unified system for delivering those notifications via in-app notification feed, browser push, email digests, and feed subscriptions. In-app notifications are modeled as a user's private notification feed. Org-level notifications are distributed to relevant org members. The webhook system (9.4) is largely superseded by feed subscriptions for plugin orgs, but remains available for external integrations.

## Sub-features

### 9.1 In-App Notification Feed

**What it is:** A user's private notification feed (bell icon, unread count) categorized by type. Modeled as a feed — the notification center is a feed view over the user's private notification feed.

**Implementation approach:**
- Notifications are posts in the user's private notification feed via `entity_feeds`
- Each notification post has: `type` (commission_update/payment/social/system/message), `title`, `body`, `data_json` (structured payload for deep linking), `read_at`
- API: `GET /me/feeds/notifications` (paginated, filterable by type), `PATCH /notifications/:id/read`, `POST /notifications/read-all`
- Real-time delivery: WebSocket connection per authenticated user, push new feed items as they're created
- Unread count: count of notification feed items where `read_at IS NULL`
- Org-level notifications: when an event targets an org (e.g., new commission request), distribute to relevant org members based on their roles

### 9.2 Push Notifications

**What it is:** Browser and mobile push for critical events.

**Implementation approach:**
- Web Push API with VAPID keys (generated once, stored server-side)
- `push_subscriptions` table: `user_id`, `endpoint`, `p256dh_key`, `auth_key`, `created_at`
- Frontend service worker registers push subscription → sends to backend
- Backend sends push via `web-push` crate on critical events
- Push categories: payment received, commission state change, new card message, org opened commissions

### 9.3 Email Digests

**What it is:** Configurable email summaries aggregating platform activity.

**Implementation approach:**
- `notification_preferences` table: `user_id`, `org_id` (nullable — null for personal prefs, org_id for org-specific prefs), `channel` (push/email/in_app), `category`, `frequency` (immediate/daily/weekly/off)
- Users can configure notification preferences per org membership (e.g., mute notifications for one org but not another)
- Background job: daily/weekly, aggregate unread notification feed items per user per preference, send email
- Email provider: SendGrid, Postmark, or AWS SES
- Email templates: HTML templates with platform branding
- Unsubscribe link in every email (CAN-SPAM compliance)

### 9.4 Webhook Notifications (Legacy Path)

**What it is:** Developer-facing API for external integrations to receive platform events. Largely superseded by feed subscriptions for plugin orgs — plugin orgs subscribe to feeds directly and react to feed items. Webhooks remain for external systems that cannot be modeled as orgs.

**Implementation approach:**
- `webhook_subscriptions` table: `user_id`, `url`, `event_types` (array), `secret` (for HMAC signing), `is_active`, `created_at`
- On matching events: POST JSON payload to webhook URL with `X-Zurfur-Signature` header (HMAC-SHA256)
- Retry: exponential backoff (1s, 5s, 30s, 5m, 30m) on HTTP failures
- `webhook_deliveries` table for delivery tracking: `id`, `webhook_id`, `event_type`, `status`, `attempts`, `last_attempt_at`
- API: `POST /webhooks`, `GET /webhooks`, `DELETE /webhooks/:id`
- **Plugin orgs should prefer feed subscriptions** over webhooks — they can subscribe/react/post to feeds natively

## Dependencies

### Requires (must be built first)
- [Feature 1.1](../01-atproto-auth/README.md) — authenticated users (know who to notify)
- [Feature 2](../02-identity-profile/README.md) — org membership (for org-level notification distribution)
- Feed infrastructure from `entity_feeds` system
- WebSocket infrastructure (Axum supports this natively)
- Email provider account (SendGrid/SES)

### Enables (unlocked after this is built)
- [Feature 7.1](../07-community-analytics/README.md) — feed subscription notifications delivered through this system
- [Feature 5](../05-omnichannel-comms/README.md) — new message notifications
- [Feature 3](../03-commission-engine/README.md) — commission state change notifications
- [Feature 4](../04-financial-gateway/README.md) — payment notifications

## Implementation Phases

### Phase 1: In-App Notification Feed
- Private notification feed per user via `entity_feeds`
- Notification creation service (called by other features when events occur)
- Org-level notification distribution to relevant members
- REST API: list, read, read-all
- WebSocket endpoint for real-time push to connected clients
- Notification types enum shared across codebase
- Crates: domain (notification feed types), persistence (feed repository), application (notification service), api (REST + WebSocket routes)

### Phase 2: Push, Email & Preferences
- VAPID key generation and storage
- `push_subscriptions` table + registration API
- Web Push sending via `web-push` crate
- `notification_preferences` table with `org_id` support
- Email digest background job
- Email provider integration (SendGrid SDK or HTTP API)
- HTML email templates

### Phase 3: Webhooks & Post-implementation
- `webhook_subscriptions` and `webhook_deliveries` tables
- Webhook dispatch with HMAC signing (for external integrations only)
- Retry logic with exponential backoff
- Webhook management API
- Documentation: note that plugin orgs should use feed subscriptions, not webhooks
- Monitoring: notification delivery success rates, push delivery rates, email bounce rates
- Notification batching: group rapid-fire events (e.g., 10 state changes in 1 minute → 1 notification)

## Assumptions

- WebSocket connections are maintained per authenticated user session
- Email sending is via third-party service (not self-hosted SMTP)
- Push notification permission requested on frontend (not forced)
- Webhook delivery is best-effort with retries (not guaranteed exactly-once)
- Notification volume is manageable (< 1M notifications/day at launch scale)
- Plugin orgs use feed subscriptions as their primary notification mechanism
- Org-level notifications are distributed to members with relevant roles (not all members)
- Notification data is private-tier (PostgreSQL only, never published to PDS)

## Shortcomings & Known Limitations

- **WebSocket scaling:** Multiple server instances need Redis pub/sub or similar for fan-out
- **Push notifications require HTTPS** and service worker (aligns with PWA requirement)
- **Email deliverability:** Spam filters, bounce handling, reputation management are a discipline unto themselves
- **No notification deduplication:** Rapid state changes could spam users — needs batching logic
- **Webhook security:** Relies on HMAC secret — if compromised, webhooks can be spoofed
- **No SMS notifications** (cost and complexity)
- **No notification aggregation UI** ("You have 5 new commission updates" instead of 5 separate notifications)
- **Org notification distribution rules** may be complex — need clear defaults for which roles receive which notification types
- **Feed subscription vs webhook overlap** may confuse developers — need clear documentation on when to use each
