# Feature 5: Omnichannel Communications

> **Revised 2026-04-08** — Updated for org-centric identity, feed-driven content, headless commissions, and plugin-as-org architecture.

## Overview

Each Commission Card has a chat thread implemented as a feed attached to the commission. Chat is an **add-on slot** on the commission card. Messages are feed items in the commission's chat feed, separate from the formal event feed (keeping the audit trail clean). Omnichannel bridging is handled by **plugin orgs** that subscribe to the chat feed and bridge messages to external services — no separate `chat_bridges` table.

## Sub-features

### 5.1 Commission Chat Feed

**What it is:** A private messaging feed bound to a specific Commission Card. Messages are feed items stored in a dedicated chat feed, separate from the commission's event feed. Chat is rendered as a built-in add-on slot on the commission card.

**Implementation approach:**
- On commission creation, auto-create a `chat` feed attached via `entity_feeds` (`entity_type = 'commission'`), in addition to the commission's event feed
- Chat messages are feed items in this chat feed: `id`, `feed_id`, `sender_id`, `content`, `attachments_json`, `created_at`
- WebSocket endpoint (`/ws/commissions/:id/chat`) for real-time messaging — subscribes to the chat feed
- REST fallback: `POST /commissions/:id/chat`, `GET /commissions/:id/chat` (paginated feed items)
- Only commission participants (org members with appropriate role + client users) can access the chat feed
- File attachments: same S3 flow as commission attachments, linked via `attachments_json`
- Chat is a built-in add-on slot — it uses the same `commission_slots` mechanism as plugins but is auto-configured

### 5.2 Omnichannel Bridge via Plugin Orgs

**What it is:** External platform bridges (Telegram, Discord, Matrix) are implemented as plugin orgs. A plugin org subscribes to a commission's chat feed and bridges messages bidirectionally. No separate `chat_bridges` table — bridging uses the standard feed subscription model.

**Implementation approach:**
- A bridge plugin is an org with feed subscription permissions on the commission's chat feed
- Installing a bridge plugin on a commission = granting the plugin org read+write access to the chat feed (via `commission_slots` with `slot_type = 'server_side'`)
- Inbound: external platform sends webhook to the plugin org's endpoint -> plugin org posts a feed item to the commission's chat feed
- Outbound: plugin org subscribes to the chat feed -> on new feed item, plugin org POSTs to external platform's API
- Plugin orgs implement the bridge logic — Zurfur provides the feed subscription infrastructure, not the bridge code
- Users configure bridge plugins per-commission by adding them as add-on slots
- Rate limiting on feed write operations prevents bridge spam

## Dependencies

### Requires (must be built first)
- [Feature 1.1](../01-atproto-auth/README.md) — authenticated users
- [Feature 2.3](../02-identity-profile/README.md) — feeds infrastructure (chat is a feed)
- [Feature 3.1](../03-commission-engine/README.md) — commission cards must exist
- [Feature 3.3](../03-commission-engine/README.md) — add-on slot mechanism for chat rendering

### Enables (unlocked after this is built)
- [Feature 6](../06-plugin-ecosystem/README.md) — bridge plugins are plugin orgs subscribing to chat feeds
- [Feature 9](../09-notification-system/README.md) — new message notifications

## Implementation Phases

### Phase 1: Chat Feed & Add-On Slot
- Chat feed auto-creation on commission creation (via `entity_feeds`)
- Chat messages as feed items in the chat feed
- REST API: send/list messages per commission chat feed
- WebSocket endpoint for real-time delivery
- Participant-only access control (org members + client users)
- Chat rendered as built-in add-on slot on commission card
- File attachment support (reuse S3 infrastructure)
- Crates: domain (feed item types for chat), persistence (repository), application (send/list use cases), api (REST + WebSocket routes)

### Phase 2: Omnichannel Bridge Plugins
- Define feed subscription contract for bridge plugin orgs
- Plugin org feed subscription grant/revoke on slot add/remove
- Inbound message handling: plugin org posts feed items to chat feed
- Outbound message dispatch: plugin org reads new feed items via subscription
- Telegram bridge reference implementation as a plugin org
- Rate limiting on bridge feed writes

### Phase 3: Post-implementation
- Discord and Matrix bridge plugin implementations
- Message delivery guarantees (retry failed outbound deliveries within plugin orgs)
- Message format normalization across platforms (markdown, images, embeds)
- WebSocket scaling: Redis pub/sub for multi-server deployments
- End-to-end encryption evaluation (currently plaintext)
- Load testing: high-frequency chat during active commissions
- Documentation: bridge plugin development guide, feed subscription contract reference

## Assumptions

- WebSocket support in Axum (`axum::extract::ws`) is sufficient for real-time chat
- Bridge plugins are developed and hosted by plugin org operators — Zurfur provides feed infrastructure, not bot hosting
- Message format is simple: text + optional file attachments (no rich embeds initially)
- Chat messages are feed items — they benefit from the same pagination and storage infrastructure as other feeds
- The chat feed is separate from the commission event feed to keep audit trail clean
- No separate `chat_bridges` or `card_messages` tables — everything is feed items and feed subscriptions
- Chat messages are private-tier data (PostgreSQL only, never published to PDS)

## Shortcomings & Known Limitations

- **No end-to-end encryption:** Messages stored in plaintext in the database
- **WebSocket scaling:** Requires sticky sessions or Redis pub/sub for multi-server deployment
- **External platform rate limits** may throttle bridge plugin message delivery (Telegram: 30 msg/sec, Discord: varies)
- **No message editing or deletion sync** across platforms — edit on Telegram won't update the feed item in Zurfur
- **File attachment bridging** needs size limits and content scanning
- **No read receipts or typing indicators** initially
- **No message threading** — flat feed item list only
- **Bridge plugin reliability** depends on third-party plugin org uptime — Zurfur cannot guarantee bridge availability
- **Feed item schema for chat** must be stable to avoid breaking bridge plugins
