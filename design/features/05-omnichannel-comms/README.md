# Feature 5: Omnichannel Communications

## Overview

Each Commission Card has a private chat thread for casual communication between parties (WIP sharing, quick questions). This chat is separate from the formal event history. An abstraction layer allows external platforms (Telegram, Discord, Matrix) to bridge into card chats via bots.

## Sub-features

### 5.1 Isolated Card Chat

**What it is:** A private messaging thread bound to a specific Commission Card. Messages are stored permanently but don't trigger formal history events (keeping the audit trail clean).

**Implementation approach:**
- `card_messages` table: `id`, `commission_id`, `sender_id`, `content`, `attachments_json`, `created_at`
- WebSocket endpoint (`/ws/cards/:id/chat`) for real-time messaging
- REST fallback: `POST /commissions/:id/messages`, `GET /commissions/:id/messages` (paginated)
- Only commission participants can access the chat (enforced by middleware)
- File attachments: same S3 flow as commission attachments, linked via `attachments_json`

### 5.2 Omnichannel Sync (API Abstraction)

**What it is:** External platform bots can subscribe to a card's chat. Messages bridge bidirectionally — a user can send a Telegram message that appears in the Zurfur card chat, and vice versa.

**Implementation approach:**
- `ChatBridge` trait in domain layer: `send_external(message)`, `receive_external(message)`
- `chat_bridges` table: `id`, `commission_id`, `provider` (telegram/discord/matrix), `external_channel_id`, `webhook_url`, `config_json`, `created_at`
- Inbound: external platform sends webhook to Zurfur → mapped to card message
- Outbound: new card message → POST to external platform's API via stored webhook/bot token
- Integration plugins (Feature 6) implement specific bridges
- Zurfur provides the webhook receiver endpoint; users configure their own bots

## Dependencies

### Requires (must be built first)
- [Feature 1.1](../01-atproto-auth/README.md) — authenticated users
- [Feature 3.1](../03-commission-engine/README.md) — commission cards must exist to chat on

### Enables (unlocked after this is built)
- [Feature 6](../06-plugin-ecosystem/README.md) — chat bridge plugins (Telegram, Discord integrations)
- [Feature 9](../09-notification-system/README.md) — new message notifications

## Implementation Phases

### Phase 1: Card Chat
- `card_messages` table and domain entity
- REST API: send/list messages per commission
- WebSocket endpoint for real-time delivery
- Participant-only access control
- File attachment support (reuse S3 infrastructure)
- Crates: domain (CardMessage entity), persistence (repository), application (send/list use cases), api (REST + WebSocket routes)

### Phase 2: Omnichannel Bridge
- `ChatBridge` trait in domain
- `chat_bridges` table and configuration API
- Inbound webhook receiver endpoint: `POST /webhooks/chat/:bridge_id`
- Outbound message dispatch (on new card message, POST to configured bridges)
- Telegram bridge reference implementation
- Rate limiting on bridge endpoints

### Phase 3: Post-implementation
- Discord and Matrix bridge implementations
- Message delivery guarantees (retry failed outbound deliveries)
- Message format normalization across platforms (markdown, images, embeds)
- WebSocket scaling: Redis pub/sub for multi-server deployments
- End-to-end encryption evaluation (currently plaintext)
- Load testing: high-frequency chat during active commissions

## Assumptions

- WebSocket support in Axum (`axum::extract::ws`) is sufficient for real-time chat
- External platform bots are set up by users — Zurfur provides webhook endpoints, not bot hosting
- Message format is simple: text + optional file attachments (no rich embeds)
- Chat messages are not searchable (no full-text index) — this is casual communication

## Shortcomings & Known Limitations

- **No end-to-end encryption:** Messages stored in plaintext in the database
- **WebSocket scaling:** Requires sticky sessions or Redis pub/sub for multi-server deployment
- **External platform rate limits** may throttle message bridging (Telegram: 30 msg/sec, Discord: varies)
- **No message editing or deletion sync** across platforms — edit on Telegram won't update Zurfur
- **File attachment bridging** needs size limits and content scanning
- **No read receipts or typing indicators** initially
- **No message threading** — flat message list only
