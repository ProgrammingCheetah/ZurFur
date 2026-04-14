# Feature 4: The Headless Commission Engine (The "Card")

> **Revised 2026-04-08** — Updated for org-centric identity, feed-driven content, headless commissions, and plugin-as-org architecture.

## Overview

The core product of Zurfur. Commissions are headless data objects with internal state only. The backend manages state and events; **boards are separate projection entities** that map commissions to visual layouts (Kanban columns, lists, calendars). A commission card is a shell with **add-on slots** — server-side slots are feed participants (plugin orgs subscribing to commission feeds), client-side slots are sandboxed iframes. Every action is an immutable event in the commission's feed, creating a single source of truth for all parties.

## Sub-features

### 4.1 Headless Commission State

**What it is:** Each commission has internal state only. The state is not tied to any visual representation. States are artist-defined per pipeline template (free-text, e.g., 'inbox', 'sketching', 'lineart', 'coloring', 'review', 'delivered'). The system does not enforce a fixed set of states.

**Implementation approach:**
- **Domain model:** `Commission` aggregate root
- **Tables:** `commissions` table: `id`, `org_id`, `pipeline_template_id` (FK pipeline_templates), `current_state` (TEXT — validated against template), `title`, `description`, `created_at`, `completed_at`, `deleted_at`
- Pipeline templates define: `valid_states` (array of strings), `transitions` (map of from→to), `terminal_states` (subset of valid_states). The system only distinguishes active vs terminal for payment release, deadline tracking, and dispute eligibility.
- State transitions are validated against the pipeline template's `transitions` map
- All state mutations emit events to the commission's feed (see 3.2)
- The commission entity has no feed field. Feeds are attached via `entity_feeds` (entity_type='commission') — same decoupled pattern as all other entities.
- The commission itself knows nothing about boards, columns, or visual layout

### 4.2 Commission Feed (Event History)

**What it is:** The commission's feed is its event history. `commission_events` is a feed attached to the commission via `entity_feeds`. Events include state changes, comments, file uploads, invoice references, deadline changes, participant additions, etc.

**Implementation approach:**
- Commission gets a system feed auto-created on commission creation, attached via `entity_feeds` (`entity_type = 'commission'`)
- `commission_events` are feed items in this feed, each with an `event_type` enum: `Created`, `StateChanged`, `CommentAdded`, `FileUploaded`, `InvoiceAttached`, `PaymentReceived`, `DeadlineSet`, `DeadlineMissed`, `ParticipantAdded`, `Completed`, `Cancelled`, `DisputeOpened`. Cancellation is a terminal state defined in the pipeline template, not a special system event. An artist who wants cancellation adds 'cancelled' to their template's `terminal_states`.
- Feed items carry structured `payload_json` for event-specific data
- Current state in `commissions` table is a materialized cache derived from events
- Plugin orgs can subscribe to the commission feed to react to events (see Feature 7)

### 4.3 Add-On Slots

**What it is:** Commission cards are shells with defined slots for extensibility. Server-side slots are feed subscriptions (plugin orgs that subscribe to the commission's feed and can post back). Client-side slots are sandboxed iframes rendered alongside the card UI.

**Implementation approach:**
- `commission_slots` table: `id`, `commission_id`, `slot_type` (enum: server_side/client_side), `plugin_org_id` (FK orgs), `config_json`, `position` (display order), `created_at`
- Server-side slots: the plugin org gets a feed subscription to the commission feed with read+write permissions. The plugin can post feed items (e.g., invoice widgets, status updates, automated messages).
- Client-side slots: the frontend renders an iframe pointing to a URL from `config_json`. The iframe communicates via postMessage API with a defined contract (read feed items, post feed items, render UI).
- Built-in slots: chat (Feature 6), invoices (Feature 5), file attachments — these are implemented as first-party add-ons using the same slot mechanism.
- Artists (org members with artist role) configure which slots appear on their commissions via pipeline templates.

### 4.4 Board Projections

**What it is:** Boards are separate entities that project commissions into visual layouts. A board does not own commissions — it maps them to columns and positions. Multiple boards can display the same commission. Boards belong to orgs.

**Implementation approach:**
- `boards` table: `id`, `org_id`, `name`, `board_type` (enum: kanban/list/calendar), `config_json`, `created_at`
- `board_columns` table: `id`, `board_id`, `name`, `position`, `color`, `config_json`
- `board_cards` table: `commission_id`, `board_id`, `column_id`, `position` (sort order within column)
- Moving a card between columns is a board operation, not a commission state change. Board layout is purely organizational.
- A commission's internal state (`in_progress`, `awaiting_input`, etc.) can be used to auto-sort or filter within a board, but the board does not dictate state.
- Default board auto-created per org with columns mapped to internal states.
- API: `POST /orgs/:id/boards`, `GET /boards/:id`, `PATCH /boards/:id/cards/:commission_id` (move card)

### 4.5 Shapeless Data Attachments

**What it is:** Cards accept arbitrary file attachments — high-res artwork, PDFs, reference sheets, original intake form JSON.

**Implementation approach:**
- `commission_attachments` table: `id`, `commission_id`, `uploader_id`, `filename`, `mime_type`, `size_bytes`, `storage_key` (S3 path), `metadata_json`, `created_at`
- S3-compatible storage (shared with Feature 2's file storage)
- Upload via presigned URLs (client uploads directly to S3, backend records metadata)
- Virus/malware scanning on upload (ClamAV or similar)
- File uploads emit a `FileUploaded` event to the commission feed

### 4.6 Deadline & Time Tracking

**What it is:** Automated triggers that flag cards and track turnaround analytics.

**Implementation approach:**
- Fields on `commissions`: `started_at`, `deadline`, `completed_at`
- Background job (tokio interval task): query for commissions where `deadline < now() AND current_state NOT IN terminal_states`. Emit `DeadlineMissed` event to the commission feed.
- Turnaround analytics derived from `started_at` to `completed_at` per commission
- Stage-level timing: calculate time spent in each state from `StateChanged` event timestamps in the feed

### 4.7 Multi-Party Collaboration

**What it is:** Many-to-many participation. Artist-side participants are orgs (a studio org can have multiple members working on one commission). Client-side participants are users directly. All participants have shared visibility into the commission feed.

**Implementation approach:**
- `commission_participants` table: `commission_id`, `participant_type` (enum: org/user), `participant_id`, `role` (artist/client/collaborator), `added_at`
- Artist-side: references `org_id` — all members of that org with appropriate role can access the commission
- Client-side: references `user_id` — individual clients
- Multi-party = multiple orgs on the same commission (e.g., two artist studios collaborating)
- All participants can view the commission feed and its chat slot
- Permission model: artist-role orgs can change state, client-role users can approve/pay, collaborators are read-only + chat
- Commission creation allows tagging multiple participant orgs/users

### 4.8 Pipeline Templates

**What it is:** Reusable templates that define default slots, board column mappings, and intake form structure for new commissions. Templates belong to orgs (not users).

**Implementation approach:**
- `pipeline_templates` table: `id`, `org_id`, `name`, `valid_states_json` (array of state names), `transitions_json` (map: state → allowed next states), `terminal_states_json` (array — subset of valid_states), `default_slots_json`, `default_board_columns_json`, `intake_form_json`, `is_default`, `created_at`
- When a commission is created from a template, `current_state` is set to the first state in `valid_states_json`. State transitions are validated against `transitions_json`.
- When a commission is created from a template, it auto-creates the configured slots and adds the card to the org's default board with the appropriate column mapping
- Default template provided for new orgs with artist role
- API: `POST /orgs/:id/pipeline-templates`, `GET /orgs/:id/pipeline-templates`

## Dependencies

### Requires (must be built first)
- [Feature 1.1](../01-atproto-auth/README.md) — authenticated users
- [Feature 2.1](../02-identity-profile/README.md) — org model and org roles
- [Feature 2.3](../02-identity-profile/README.md) — feeds infrastructure (commission feeds are feeds)
- [Feature 3](../03-tag-taxonomy/README.md) — Tag infrastructure for commission descriptive attributes and content rating
- [Feature 11](../11-artist-tos/README.md) — TOS acceptance required before commission submission
- File storage (S3/MinIO) for attachments

### Soft dependencies
- [Feature 2.7](../02-identity-profile/README.md) — character profiles can be attached to commission requests, but a commission can exist without a character reference

### Enables (unlocked after this is built)
- [Feature 5](../05-financial-gateway/README.md) — invoices are add-on slots on cards
- [Feature 6](../06-omnichannel-comms/README.md) — chat is an add-on slot on cards
- [Feature 7](../07-plugin-ecosystem/README.md) — plugin orgs subscribe to commission feeds
- [Feature 1.2](../01-atproto-auth/README.md) — cross-post commission openings to Bluesky
- [Feature 8.3](../08-community-analytics/README.md) — analytics from feed event data
- [Feature 13](../13-dispute-resolution/README.md) — disputes reference commission feed history

## Implementation Phases

### Phase 1: Headless Card & Feeds
- `Commission` domain entity with internal state enum
- `commissions` table with internal state
- Commission feed auto-creation via `entity_feeds`
- Commission events as feed items with event type and payload
- Commission CRUD: create, read, list
- State transition validation and `StateChanged` event emission
- `commission_participants` table with org/user participant types
- API: `POST /commissions`, `GET /commissions/:id`, `GET /commissions` (list), `POST /commissions/:id/transition`
- Crates: domain (entities), persistence (repositories), application (use cases), api (routes)

### Phase 2: Boards, Slots & Templates
- `boards`, `board_columns`, `board_cards` tables
- Board CRUD and card movement API
- Default board auto-creation per org
- `commission_slots` table and slot management API
- `pipeline_templates` table and template-based commission creation
- `commission_attachments` table + S3 upload flow
- Deadline tracking + background job for overdue detection
- Intake form system via pipeline templates
- API: board management, slot configuration, file upload, deadline setting

### Phase 3: Post-implementation
- Event replay/projection tooling for debugging
- Performance: index optimization on commission feed for large histories
- Snapshotting strategy for commissions with 100+ events
- Integration tests: full commission lifecycle (create, state transitions, complete)
- Board view types beyond Kanban (list, calendar)
- Client-side slot iframe sandboxing and postMessage API spec
- Documentation: internal state reference, slot API contract, board projection model

## Assumptions

- Artist-defined states via pipeline templates cover all workflow variations. The system only needs to know active vs terminal.
- Boards are lightweight projections — losing a board does not lose commission data
- Add-on slots use the same feed subscription mechanism as plugin orgs
- File storage infrastructure (S3) exists when Phase 2 begins
- Pipeline templates cover 80% of artist workflows; custom slot configuration handles the rest
- The `commissions` materialized state is kept in sync via application-level logic, not database triggers
- Artist-side participants are always orgs; client-side participants are always users
- Commission data is private-tier (PostgreSQL only). Completed commissions can be optionally published to PDS as gallery records (the gallery feed serves as the portfolio)

## Shortcomings & Known Limitations

- **Event replay can be slow** for old commissions with many feed items — may need snapshotting
- **No conflict resolution** for simultaneous state changes — last-write-wins
- **Internal state is intentionally minimal** — complex workflow logic must be handled by board projections or plugin slots, which adds complexity for power users
- **No undo/rollback** of events (immutable by design) — mistakes require compensating events
- **No archival strategy** for completed commissions (cold storage, data retention)
- **Intake forms** are embedded in pipeline templates, which is simpler but less flexible than a standalone form builder
- **No draft commissions** — once created, a card is active
- **Board sync across multiple views** not addressed — if a commission appears on two boards, moving it on one does not affect the other
- **Client-side slot security** (iframe sandboxing, origin restrictions) needs thorough security review
- **Search within commission feeds** is not indexed — finding a specific event in a long history may be slow
