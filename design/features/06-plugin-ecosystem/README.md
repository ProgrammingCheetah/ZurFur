# Feature 6: The Plugin Ecosystem

## Overview

Zurfur is designed as a modular platform. The community can build, share, and monetize custom tools that hook into the Headless Engine. Plugins range from UI themes and custom Kanban views to workflow automation and external service integrations. The platform also offers premium first-party statistical/analytical plugins.

## Sub-features

### 6.1 User-Generated Plugins

**What it is:** A marketplace where the community uploads custom views, automation scripts, UI themes, and integration bridges.

**Implementation approach:**
- **Plugin types:**
  - **UI Plugins:** WASM modules running in browser sandbox. Receive card/pipeline data, render custom views. Loaded via frontend plugin loader.
  - **Logic Plugins:** Server-side webhook-based. Subscribe to platform events (onCardStateChange, onInvoicePaid, etc.). Zurfur POSTs event payloads to registered webhook URLs.
  - **Integration Plugins:** Specialized logic plugins that bridge external services (Telegram, Discord, Google Calendar, etc.)
- **Tables:** `plugins` (id, author_id, name, description, type, manifest_url, version, price_cents, is_approved, created_at), `plugin_installations` (user_id, plugin_id, config_json, installed_at)
- **Plugin API contract:** Well-defined event types and data shapes (versioned)
- **Marketplace:** Browse, purchase (via Feature 4), install, configure, rate/review
- **Security:** Webhook plugins are isolated (they receive data, can't query the DB). WASM plugins run in browser sandbox.

### 6.2 Native Statistical AI Plugins

**What it is:** Premium first-party analytical tools (non-generative AI). Market price suggestions, queue completion forecasting, profile engagement tracking.

**Implementation approach:**
- Internal plugins with privileged API access (not sandboxed like community plugins)
- **Price suggestions:** Aggregate commission pricing data by tag/category, suggest competitive pricing
- **Queue forecasting:** Average time per pipeline stage × current queue depth = estimated completion
- **Engagement tracking:** View counts, conversion rates (profile view → commission request)
- Subscription or one-time purchase via Feature 4 payment infrastructure
- Data aggregation jobs run as background tasks

## Dependencies

### Requires (must be built first)
- [Feature 1.1](../01-atproto-auth/README.md) — authenticated users
- [Feature 3](../03-commission-engine/README.md) — commission events to hook into
- [Feature 4](../04-financial-gateway/README.md) — marketplace purchases for paid plugins

### Enables (unlocked after this is built)
- [Feature 5.2](../05-omnichannel-comms/README.md) — chat bridge plugins
- [Feature 7.2](../07-community-analytics/README.md) — gamification can be extended via plugins
- Third-party innovation: features the platform doesn't prioritize can be community-built

## Implementation Phases

### Phase 1: Webhook Plugin Framework
- `plugins` and `plugin_installations` tables
- Event dispatch system: on platform events, POST to registered webhook URLs
- HMAC-signed payloads for webhook security
- Plugin registration API: `POST /plugins`, `POST /plugins/:id/install`
- Event types: `commission.created`, `commission.state_changed`, `invoice.paid`, `commission.completed`
- Retry with exponential backoff for failed webhook deliveries
- Crates: domain (Plugin entity, event types), persistence, application (event dispatcher), api (plugin management routes, webhook dispatch)

### Phase 2: Marketplace & UI Plugins
- Plugin marketplace: browse, search, purchase, rate
- WASM plugin loader specification for frontend
- Plugin sandboxing and permissions model
- Revenue sharing: platform takes marketplace cut (via Feature 4)
- Plugin review/approval process (manual initially)

### Phase 3: Post-implementation
- Native statistical plugins (6.2) — built as first-party plugins
- Plugin analytics: installation counts, usage metrics, crash reports
- Plugin versioning and update notifications
- Community plugin development documentation and SDK
- Security audit: penetration testing on plugin boundaries
- Rate limiting per plugin (prevent one plugin from overwhelming the API)

## Assumptions

- Webhook-based plugins are simpler and should ship first (WASM is Phase 2)
- WASM is viable for browser-side UI plugins (requires frontend framework support)
- Plugin sandboxing is sufficient to prevent cross-user data leakage
- Plugin marketplace revenue model follows standard app store percentages
- Community will actually build plugins (chicken-and-egg: need users first)

## Shortcomings & Known Limitations

- **Plugin security is the biggest risk:** Malicious webhook endpoints could exfiltrate event data. WASM plugins could have sandbox escapes.
- **WASM UI plugins require a stable API contract** that's hard to change without breaking existing plugins
- **Plugin versioning and backward compatibility** is inherently complex
- **No plugin review/approval automation** — manual review doesn't scale
- **Plugin performance monitoring** not addressed — a slow webhook handler shouldn't block platform events
- **Revenue sharing model** not finalized (percentage, payout frequency, minimum threshold)
- **This is a late-stage feature** — core platform must be stable and have users before a plugin ecosystem is viable
