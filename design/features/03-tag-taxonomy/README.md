> **Created 2026-04-12** — Extracted from Feature 8 (Search & Discovery) as standalone Tier 1 infrastructure.

# Feature 3: Tag Taxonomy & Attribution

## Overview

Typed, cross-cutting identity and metadata system. Tier 1 foundational infrastructure — the tag system underpins attribution, search, content classification, moderation, and discovery. Tags is one of the five root aggregates. Every organization and character automatically gets an immutable tag on creation. These entity-backed tags serve as permanent identity markers — attribution is just "attach the org's tag." Descriptive tags (metadata/general) are user-created and community-curated.

## Sub-features

### 3.1 Typed Tags

**What it is:** Tags have a `tag_type` that determines their behavior and constraints.

**Tag types:**
- `organization` — auto-created on org creation, entity-backed (entity_id -> org), immutable identity, display resolves from org name. Used for attribution (artist credit, studio credit).
- `character` — auto-created on character creation, entity-backed (entity_id -> character), immutable identity, display resolves from character name. Used for character depiction tagging.
- `metadata` — user-created descriptive attributes with optional category (species/art_style/medium/content_type/status). Community-curated. E.g., "canine", "digital art", "status:open".
- `general` — free-form user-created tags without category constraints.

**Implementation approach:**
- `tags` table: `id`, `tag_type` (organization/character/metadata/general), `entity_id` (nullable — set for org/character-backed tags), `name` (display for metadata/general; entity-backed tags resolve from entity), `category` (nullable — for metadata faceted search), `parent_id` (hierarchical), `usage_count`, `is_approved`
- Entity-backed tags: auto-created, never deleted, `entity_id` references the owning org/character
- Tag type is a constrained enum, not a free-form string

### 3.2 Entity-Backed Identity & Attribution

**What it is:** Every organization and character auto-gets an immutable tag on creation. The tag's UUID is permanent; display resolves from the entity name. Attribution is a first-class operation: tagging a commission with an artist's org tag IS the attribution.

**Implementation approach:**
- On org creation, auto-create a tag with `tag_type = 'organization'` and `entity_id = org.id`
- On character creation, auto-create a tag with `tag_type = 'character'` and `entity_id = character.id`
- Entity-backed tags are immutable — cannot be renamed, deleted, or reassigned
- Display name resolves from the entity at query time (org name changes propagate automatically)
- "Show me everything by this artist" and "show me all digital art" are the same query shape — just filtered by tag type
- No separate `commission_participants` or `attribution` table needed — attribution = tag attachment

### 3.3 Universal Tag Assignment

**What it is:** Any entity can be tagged via the `entity_tags` junction table — the same polymorphic pattern as `entity_feeds`.

**Implementation approach:**
- `entity_tags` junction table: `entity_type` (org/commission/feed_item/character/feed_element), `entity_id`, `tag_id` — universal tag assignment for any entity
- Status tags: `status:open`, `status:closed`, `status:waitlist` — commission availability expressed as metadata tags on the org
- Same polymorphic pattern as `entity_feeds`

### 3.4 Tag Curation & Governance

**What it is:** Community-curated metadata and general tags with approval workflow, suggestions, and synonym/alias support.

**Implementation approach:**
- Tag suggestions: auto-complete from existing approved tags
- Community tag proposals: users suggest new metadata/general tags -> moderation queue
- Tag synonyms/aliases for search (e.g., "wolf" matches "canine")
- Seed data for common metadata tags (species, styles, mediums)
- Tags interact with content moderation: "untagged NSFW" detection relies on tag presence (Feature 12)

## Dependencies

### Requires (must be built first)
- [Feature 1.1](../01-atproto-auth/README.md) — authenticated users
- [Feature 2](../02-identity-profile/README.md) — org and character entities to back tags

### Enables (unlocked after this is built)
- [Feature 4](../04-commission-engine/README.md) — commission descriptive attributes and content rating via tags
- [Feature 9](../09-search-discovery/README.md) — all search facets are tag-driven
- [Feature 12](../12-content-moderation/README.md) — "untagged NSFW" detection via tag system
- Attribution across the platform — org/character tags as identity markers

## Implementation Phases

### Phase 1: Tag Infrastructure & Entity-Backed Tags
- `tags` table with `tag_type`, `entity_id`, `name`, `category`, `parent_id`
- Tag domain entity as root aggregate
- Auto-creation of org tags on org creation (hook into OrganizationService)
- `entity_tags` universal junction table
- Tag assignment API: attach/detach tags to any entity
- Seed data for common metadata tags (species, styles, mediums)
- Crates: domain (Tag root aggregate), persistence (tag repository), application (tag service), api (tag routes)

### Phase 2: Curation, Search & Attribution
- Auto-creation of character tags on character creation
- Tag suggestion auto-complete from approved tags
- Community tag proposals -> moderation queue
- Tag synonyms/aliases
- Attribution workflow: attach org tag to commission
- Status tags for commission availability (status:open, etc.)

### Phase 3: Post-implementation
- Tag taxonomy curation process and community guidelines
- Tag usage analytics (popular tags, trending tags)
- Tag cleanup tooling (merge duplicates, deprecate stale tags)
- Hierarchical tag browsing UI
- Tag import from external sources (e.g., e621 tag database for species)

## Assumptions

- Tag type is a constrained enum (4 variants), not extensible without migration
- Entity-backed tags (org/character) are immutable and permanent — they serve as identity references for attribution
- Tags are the sole mechanism for descriptive attributes — no separate columns for species, style, etc.
- Tag taxonomy is manually seeded, community-curated later
- Metadata tags with `category` cover the primary faceted search dimensions (species, art_style, medium, content_type, status)
- The `entity_tags` junction table follows the same polymorphic pattern as `entity_feeds`

## Shortcomings & Known Limitations

- **Tag taxonomy requires ongoing curation** — tag pollution (duplicates, irrelevant tags) is inevitable
- **Tags-only approach** means losing structured validation — e.g., can't enforce "exactly one species" without tag rules
- **Entity-backed tag display resolution** adds a JOIN at query time — may need denormalization for search performance
- **No tag ownership** beyond entity-backed tags — metadata/general tags are communal
- **Synonym management** is manual — no automated synonym detection
- **Tag hierarchy** (parent_id) adds query complexity for "find all tags under 'canine'" — needs recursive CTE or materialized path
