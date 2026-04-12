# Feature 6: The Plugin Ecosystem

> **Revised 2026-04-08** — Updated for org-centric identity, feed-driven content, headless commissions, and plugin-as-org architecture.

## Overview

Plugins are orgs. There is no separate plugin API, plugin registry, or plugin runtime. A plugin org is a regular org that subscribes to feeds, reacts to feed items, and posts feed items back. Server-side plugins operate via feed subscriptions. Client-side plugins (UI) are sandboxed iframes with a defined API surface. Installing a plugin means granting a plugin org feed subscription and write permissions on the target entity's feeds. The plugin marketplace is a directory of plugin orgs.

## Sub-features

### 6.1 Plugin Orgs (Server-Side)

**What it is:** A plugin is an org. Server-side plugins subscribe to feeds (commission feeds, org feeds, chat feeds) and react by posting feed items or triggering external actions. No special plugin infrastructure — plugins use the same feed subscription mechanism as any other org.

**Implementation approach:**
- A plugin org is created like any other org, with a flag: `is_plugin: bool` on the `orgs` table
- Plugin orgs register webhook endpoints for feed event delivery: `plugin_webhooks` table: `org_id`, `webhook_url`, `secret_hash`, `event_types_json`, `created_at`
- When a feed item is posted to a feed the plugin org subscribes to, Zurfur delivers the event to the plugin's webhook URL
- HMAC-signed payloads for webhook security
- Plugin orgs can post feed items back to feeds they have write access to
- Feed subscription permissions are scoped: read-only, read+write, or read+write+admin
- Feed subscriptions are defined in [Feature 2.3](../02-identity-profile/README.md). Plugin orgs use the same `feed_subscriptions` table as any other subscriber.
- Retry with exponential backoff for failed webhook deliveries

### 6.2 Client-Side Plugins (UI Iframes)

**What it is:** Plugins that render UI within commission card add-on slots or org profile pages. Implemented as sandboxed iframes with a defined postMessage API.

**Implementation approach:**
- Plugin org specifies a `ui_url` in its configuration: the iframe source
- When a commission slot references a plugin org with a `ui_url`, the frontend renders a sandboxed iframe
- **postMessage API contract:**
  - `readFeedItems(feedId, pagination)` — read items from a feed the plugin has access to
  - `postFeedItem(feedId, item)` — post an item to a feed the plugin has write access to
  - `getCommissionState(commissionId)` — read commission metadata
  - `renderReady()` — signal the host that the iframe is loaded
- iframe sandbox attributes: `allow-scripts`, `allow-forms`, no `allow-same-origin`, no `allow-top-navigation`
- Plugin UI configuration stored in `commission_slots.config_json`
- Size constraints enforced by the host container (max height, responsive width)

### 6.3 Plugin Installation & Permissions

**What it is:** Installing a plugin = granting a plugin org feed subscription + write permissions on specific feeds. No `plugin_installations` table — installation is represented by feed subscriptions.

**Implementation approach:**
- Install flow: user selects a plugin org from the marketplace -> chooses which feeds to grant access to (e.g., a specific commission's feeds, all commissions on an org) -> creates feed subscription rows (see [Feature 2.3](../02-identity-profile/README.md) for the canonical `feed_subscriptions` definition)
- Uninstall flow: revoke all feed subscriptions for the plugin org on the target feeds, remove any `commission_slots` referencing the plugin org
- Permission scoping: plugin orgs can only access feeds they've been explicitly granted access to
- Org-level installation: grant a plugin org subscription to all current and future feeds on an org (stored as `org_plugin_grants` table: `org_id`, `plugin_org_id`, `default_permissions`, `granted_at`)
- Commission-level installation: grant access to a specific commission's feeds only (via `commission_slots`)
- Users can review and revoke plugin access at any time via org settings

### 6.4 Plugin Marketplace

**What it is:** A directory of plugin orgs. Browse, search, and install plugins. No separate marketplace infrastructure — it queries the `orgs` table filtered by `is_plugin = true`.

**Implementation approach:**
- `plugin_listings` table: `org_id`, `short_description`, `long_description`, `category` (enum: automation/bridge/analytics/ui_theme/workflow), `icon_url`, `screenshots_json`, `price_cents` (0 for free), `is_approved`, `created_at`
- Marketplace API: `GET /plugins` (list/search), `GET /plugins/:org_id` (detail)
- Approval process: manual review before listing is visible (moderation queue)
- Ratings/reviews: `plugin_reviews` table: `id`, `plugin_org_id`, `reviewer_user_id`, `rating` (1-5), `review_text`, `created_at`
- Paid plugins: purchase via Feature 4 payment infrastructure, unlocks `org_plugin_grants`
- Installation counts derived from feed subscriptions count per plugin org (see [Feature 2.3](../02-identity-profile/README.md))

### 6.5 Native Analytical Plugins

**What it is:** Premium first-party analytical tools (non-generative AI). Market price suggestions, queue completion forecasting, profile engagement tracking. Built as plugin orgs with privileged access.

**Implementation approach:**
- First-party plugin orgs created and maintained by Zurfur
- Privileged feed access: can read aggregated data across orgs (with user consent)
- **Price suggestions:** Aggregate commission pricing data by tag/category, suggest competitive pricing
- **Queue forecasting:** Average time per internal state from commission feeds x current queue depth = estimated completion
- **Engagement tracking:** View counts, conversion rates (org profile view -> commission request)
- Subscription or one-time purchase via Feature 4 payment infrastructure
- Data aggregation jobs run as background tasks, results posted as feed items to subscribing orgs

## Dependencies

### Requires (must be built first)
- [Feature 1.1](../01-atproto-auth/README.md) — authenticated users
- [Feature 2.1](../02-identity-profile/README.md) — org model (plugins are orgs)
- [Feature 2.3](../02-identity-profile/README.md) — feeds infrastructure (plugins subscribe to feeds)
- [Feature 3](../03-commission-engine/README.md) — commission feeds and add-on slots
- [Feature 4](../04-financial-gateway/README.md) — marketplace purchases for paid plugins

### Enables (unlocked after this is built)
- [Feature 5.2](../05-omnichannel-comms/README.md) — chat bridge plugins are plugin orgs
- [Feature 7.2](../07-community-analytics/README.md) — gamification can be extended via plugin orgs
- Third-party innovation: features the platform doesn't prioritize can be community-built as plugin orgs

## Implementation Phases

### Phase 1: Feed Subscriptions & Server-Side Plugins
- `is_plugin` flag on `orgs` table
- `feed_subscriptions` table with permission scoping
- `plugin_webhooks` table for webhook endpoint registration
- Event delivery: on new feed item, dispatch to subscribed plugin org webhooks
- HMAC-signed webhook payloads
- Retry with exponential backoff for failed deliveries
- Plugin org registration API: `POST /orgs` (with `is_plugin: true`)
- Feed subscription grant/revoke API
- `org_plugin_grants` table for org-level plugin installation
- Crates: domain (feed subscription entity, webhook delivery), persistence, application (event dispatcher), api (plugin management routes)

### Phase 2: Client-Side Plugins & Marketplace
- Client-side iframe spec: postMessage API contract definition
- iframe sandboxing and host container implementation (frontend)
- `plugin_listings` table and marketplace API
- Plugin approval/moderation workflow
- `plugin_reviews` table and rating system
- Paid plugin purchase flow (via Feature 4)
- Plugin search and discovery

### Phase 3: Post-implementation
- Native analytical plugins (6.5) — built as first-party plugin orgs
- Plugin analytics: subscription counts, webhook delivery metrics, error rates
- Plugin versioning: `plugin_webhooks` supports versioned event schemas
- Community plugin development documentation and SDK
- Security audit: penetration testing on feed subscription boundaries, iframe sandboxing
- Rate limiting per plugin org (prevent one plugin from overwhelming feed infrastructure)
- Plugin org identity verification for trust signals in marketplace

## Assumptions

- Feed subscription-based plugins are simpler and should ship first (iframe UI is Phase 2)
- Plugin orgs use the same org infrastructure as all other orgs — no special runtime
- Feed subscription permissions are sufficient for access control (no need for fine-grained per-field permissions)
- Plugin marketplace follows standard app store approval model (manual initially)
- Community will build plugins once the feed subscription API is stable and documented
- Webhook delivery is async and best-effort with retries — plugins must handle eventual consistency
- Plugin org profiles are public-tier data (PDS). Plugin configuration, webhook URLs, and subscription data are private-tier (PostgreSQL only)

## Shortcomings & Known Limitations

- **Plugin security depends on feed subscription scoping:** If a plugin org is granted overly broad access, it can read/write data it shouldn't. Users must understand permission grants.
- **Client-side iframe plugins require a stable postMessage API contract** that's hard to change without breaking existing plugins
- **Webhook reliability:** Plugin org endpoints may be unreliable. Failed deliveries are retried but eventually dropped.
- **No plugin sandboxing beyond feed permissions:** Server-side plugins receive real data via webhooks. Malicious plugin orgs could exfiltrate data they're subscribed to.
- **No plugin review/approval automation** — manual review doesn't scale
- **Revenue sharing model** for paid plugins not finalized (percentage, payout frequency, minimum threshold)
- **This is a late-stage feature** — core platform must be stable and have users before a plugin ecosystem is viable
- **Plugin org discovery** depends on marketplace curation quality — low-quality plugins may clutter search results
- **No plugin-to-plugin communication** — plugins cannot directly interact with each other, only through shared feeds
