# Feature 9: Notification System

## Overview

The platform's event delivery infrastructure. Almost every feature emits notifications — commission state changes, payment confirmations, new messages, artist queue openings. This feature provides the unified system for delivering those notifications via in-app feed, browser push, email digests, and developer webhooks.

## Sub-features

### 9.1 In-App Notification Center

**What it is:** A unified notification feed (bell icon, unread count) categorized by type.

**Implementation approach:**
- `notifications` table: `id`, `user_id`, `type` (commission_update/payment/social/system/message), `title`, `body`, `data_json` (structured payload for deep linking), `read_at`, `created_at`
- API: `GET /notifications` (paginated, filterable by type), `PATCH /notifications/:id/read`, `POST /notifications/read-all`
- Real-time delivery: WebSocket connection per authenticated user, push new notifications as they're created
- Unread count: `SELECT COUNT(*) FROM notifications WHERE user_id = $1 AND read_at IS NULL`

### 9.2 Push Notifications

**What it is:** Browser and mobile push for critical events.

**Implementation approach:**
- Web Push API with VAPID keys (generated once, stored server-side)
- `push_subscriptions` table: `user_id`, `endpoint`, `p256dh_key`, `auth_key`, `created_at`
- Frontend service worker registers push subscription → sends to backend
- Backend sends push via `web-push` crate on critical events
- Push categories: payment received, commission state change, new card message, artist opened commissions

### 9.3 Email Digests

**What it is:** Configurable email summaries aggregating platform activity.

**Implementation approach:**
- `notification_preferences` table: `user_id`, `channel` (push/email/in_app), `category`, `frequency` (immediate/daily/weekly/off)
- Background job: daily/weekly, aggregate unread notifications per user per preference, send email
- Email provider: SendGrid, Postmark, or AWS SES
- Email templates: HTML templates with platform branding
- Unsubscribe link in every email (CAN-SPAM compliance)

### 9.4 Webhook Notifications

**What it is:** Developer-facing API for plugins and external integrations to receive platform events.

**Implementation approach:**
- `webhook_subscriptions` table: `user_id`, `url`, `event_types` (array), `secret` (for HMAC signing), `is_active`, `created_at`
- On matching events: POST JSON payload to webhook URL with `X-Zurfur-Signature` header (HMAC-SHA256)
- Retry: exponential backoff (1s, 5s, 30s, 5m, 30m) on HTTP failures
- `webhook_deliveries` table for delivery tracking: `id`, `webhook_id`, `event_type`, `status`, `attempts`, `last_attempt_at`
- API: `POST /webhooks`, `GET /webhooks`, `DELETE /webhooks/:id`

## Dependencies

### Requires (must be built first)
- [Feature 1.1](../01-atproto-auth/README.md) — authenticated users (know who to notify)
- WebSocket infrastructure (Axum supports this natively)
- Email provider account (SendGrid/SES)

### Enables (unlocked after this is built)
- [Feature 7.1](../07-community-analytics/README.md) — commission subscription notifications delivered through this system
- [Feature 5](../05-omnichannel-comms/README.md) — new message notifications
- [Feature 3](../03-commission-engine/README.md) — commission state change notifications
- [Feature 4](../04-financial-gateway/README.md) — payment notifications
- [Feature 6](../06-plugin-ecosystem/README.md) — webhook notifications power plugin integrations

## Implementation Phases

### Phase 1: In-App Notifications
- `notifications` table and domain entity
- Notification creation service (called by other features when events occur)
- REST API: list, read, read-all
- WebSocket endpoint for real-time push to connected clients
- Notification types enum shared across codebase
- Crates: domain (Notification entity), persistence (repository), application (notification service), api (REST + WebSocket routes)

### Phase 2: Push & Email
- VAPID key generation and storage
- `push_subscriptions` table + registration API
- Web Push sending via `web-push` crate
- `notification_preferences` table
- Email digest background job
- Email provider integration (SendGrid SDK or HTTP API)
- HTML email templates

### Phase 3: Webhooks & Post-implementation
- `webhook_subscriptions` and `webhook_deliveries` tables
- Webhook dispatch with HMAC signing
- Retry logic with exponential backoff
- Webhook management API
- Monitoring: notification delivery success rates, push delivery rates, email bounce rates
- Notification batching: group rapid-fire events (e.g., 10 state changes in 1 minute → 1 notification)
- Documentation: webhook event catalog, email template customization guide

## Assumptions

- WebSocket connections are maintained per authenticated user session
- Email sending is via third-party service (not self-hosted SMTP)
- Push notification permission requested on frontend (not forced)
- Webhook delivery is best-effort with retries (not guaranteed exactly-once)
- Notification volume is manageable (< 1M notifications/day at launch scale)

## Shortcomings & Known Limitations

- **WebSocket scaling:** Multiple server instances need Redis pub/sub or similar for fan-out
- **Push notifications require HTTPS** and service worker (aligns with PWA requirement)
- **Email deliverability:** Spam filters, bounce handling, reputation management are a discipline unto themselves
- **No notification deduplication:** Rapid state changes could spam users — needs batching logic
- **Webhook security:** Relies on HMAC secret — if compromised, webhooks can be spoofed
- **No SMS notifications** (cost and complexity)
- **No notification aggregation UI** ("You have 5 new commission updates" instead of 5 separate notifications)
