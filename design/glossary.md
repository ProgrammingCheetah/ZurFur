# Zurfur — Glossary & Principles

> **Updated 2026-04-12**

## Architectural Principles

### Aggregates are decoupled at the database level
No aggregate table has a foreign key or column pointing to another aggregate. All cross-aggregate relationships live in separate junction/bridge tables. Application code composes them freely — that's its job.

### Feeds are the universal content container
Gallery, bio, activity, commission history, notifications — all feeds. "Adding a new view" = creating a feed and a template, not new backend endpoints. The frontend is fundamentally a feed renderer.

### Tags over columns
Descriptive attributes (species, art style, commission status, etc.) are tags, not database columns. Only structural fields live on aggregate tables. When tempted to add a column, ask: could this be a tag, a feed item, or a JSONB field?

### User is atomic
The User entity holds only authentication identity: id, did, handle, email, username. No bios, no roles, no feature flags. Everything else lives on organizations.

### Everything is an org
Solo artists, studios, plugins — all organizations. A user's "profile" is their personal org. There is no `is_artist` flag.

### Persistence holds minimal logic
Keep the schema lean. Business rules live in the application layer, not in CHECK constraints or complex column arrangements.

## Domain Concepts

### Root Aggregates
The five independent domain objects. They never reference each other in the schema.

| Aggregate | What it is |
|-----------|-----------|
| **User** | Atomic authentication identity. DID, handle, email. Nothing else. |
| **Organization** | All public-facing state. Personal org = user profile. Studio, plugin = same model. |
| **Feed** | Universal content container. Items contain elements (text/image/file/event/embed). |
| **Tag** | Typed identity and metadata marker. Has a category, a name, a usage count. |
| **Commission** | Minimal shell with artist-defined states. Boards are projections, not owners. |

### Tag Categories
A PostgreSQL ENUM (`tag_category`) describing what the tag IS — not how it's connected. Stable set that grows slowly and intentionally.

| Category | Meaning | Example |
|----------|---------|---------|
| `organization` | Represents an org's identity. Auto-created, immutable. | "Studio Howl" |
| `character` | Represents a character's identity. Auto-created, immutable. | "Foxy the Fox" |
| `metadata` | Descriptive attribute. Community-curated. | "canine", "digital art", "open" |
| `general` | Free-form tag. Default category. | anything user-created |

If faceted search later needs finer granularity (species vs. art_style), the enum gains a new value. Until then, they're all `metadata`.

### Entity-Backed Tags
Tags with category `organization` or `character` that are auto-created when their entity is created and attached via `entity_tag`. They are immutable — cannot be renamed or deleted. The tag itself doesn't know what it's attached to (no `entity_id` field) — the connection lives entirely in `entity_tag`.

### Attribution
"This artist made this" = the artist's personal org tag attached to the commission/artwork. Primary attribution is always the personal org tag (permanent, 1:1 with user). Studio org tags are supplementary context. If the artist leaves the studio, their personal org tag stays.

### Polymorphic Junctions
Tables that connect any aggregate to any other by `(entity_type, entity_id)`. No foreign keys to entity tables — validated at the application layer. Adding a new entity type is just a new string value.

| Junction | Connects | Entity Types |
|----------|----------|-------------|
| `entity_feed` | any entity → Feed | org, character, commission, user |
| `entity_tag` | any entity → Tag | org, commission, feed_item, character, feed_element |

### Typed Bridges
Junction tables that connect exactly two specific aggregates. Separate table, references both.

| Bridge | Connects | Purpose |
|--------|----------|---------|
| `organization_member` | User ↔ Organization | Membership, role, title, permissions |
| `feed_subscription` | Organization → Feed | Following, plugin access |

### System Feeds
Feeds with `feed_type = 'system'` that are auto-created and cannot be deleted. Created during onboarding or org creation.

| Feed | Created when | Purpose |
|------|-------------|---------|
| `updates` | Onboarding | General announcements |
| `gallery` | Onboarding | Artwork, portfolio |
| `bio` | Org creation | Organization bio (edits = new feed items, gives version history) |
| `commissions` | Onboarding (artists only) | Commission openings and updates |

### Roles vs. Titles
Roles are administrative positions: Owner, Admin, Mod, Member. They determine what you *can do*. Titles are cosmetic, self-given display strings: "Lead Character Designer", "Furry Illustrator". They describe what you *are*.

### Personal Orgs
Every user gets one on signup (`is_personal = true`). The personal org IS the user's public profile. The owner is immutable — role cannot be changed, owner cannot be removed.

## Schema Conventions

- **Singular table names**: `organization`, `feed`, `tag` — not plural
- **PG ENUM for stable types**: `tag_category`, `content_rating` — sets that rarely change
- **TEXT + CHECK for extensible types**: entity_type values, feed_type — sets that may grow
- **JSONB for extensible settings**: `user_preference.settings` — avoids migrations for new preferences
- **No `created_by` on aggregates**: Creator is derived from relationships (owner member)
- **Polymorphic junctions use no FK to entity tables**: `entity_id` is validated at the application layer
- **Soft deletes** (`deleted_at`) for aggregates; **hard deletes** for leaf entities and junction rows
