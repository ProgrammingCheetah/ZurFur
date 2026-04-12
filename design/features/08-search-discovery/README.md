> **Revised 2026-04-08** — Updated for org-centric identity, feed-driven content, headless commissions, and plugin-as-org architecture.

# Feature 8: Search & Discovery

## Overview

How users find orgs offering commissions. Without discovery, users can only find orgs they already know. This feature provides full-text search across orgs (artist studios, plugin orgs, personal orgs) with faceted filtering driven entirely by the tag system. Tags are Tier 1 foundational infrastructure — other features depend on them. Search indexes from PDS records (public data tier). The "Open Now" feed is a feed view, not a DB query.

## Sub-features

### 8.1 Org Search

**What it is:** Find orgs by tags, art style, species specialty, price range, and availability status. Supports full-text and faceted filtering. Plugin orgs are also searchable.

**Implementation approach:**
- **MVP:** PostgreSQL `tsvector` full-text search on org profile fields (bio, display_name, tags)
- **Upgrade path:** Meilisearch or Elasticsearch for complex faceted search
- Search index: built from PDS records (public data tier) combined with tag associations
- All search facets are driven by tags — no separate columns for species, style, medium, etc.
- Faceted filters: all tag-based (species, art style, medium, content rating, status), plus price range. Commission availability status is a tag on the org (e.g., `status:open`). The "Open Now" feed view filters for orgs with the `status:open` tag.
- Plugin orgs appear in search results alongside artist orgs
- API: `GET /search/orgs?q=&tags=status:open,species:wolf&price_min=&price_max=&type=artist,plugin`

### 8.2 Tag Taxonomy (Tier 1 Infrastructure)

**What it is:** Typed, cross-cutting identity and metadata system. Tier 1 foundational infrastructure — the tag system underpins attribution, search, content classification, moderation, and discovery. Tags is one of the five root aggregates.

**Core design: entity-backed tags + descriptive tags.**

Every organization and character automatically gets an immutable tag on creation. These entity-backed tags serve as permanent identity markers — attribution is just "attach the org's tag." Descriptive tags (metadata/general) are user-created and community-curated.

**Tag types:**
- `organization` — auto-created on org creation, entity-backed (entity_id → org), immutable identity, display resolves from org name. Used for attribution (artist credit, studio credit).
- `character` — auto-created on character creation, entity-backed (entity_id → character), immutable identity, display resolves from character name. Used for character depiction tagging.
- `metadata` — user-created descriptive attributes with optional category (species/art_style/medium/content_type/status). Community-curated. E.g., "canine", "digital art", "status:open".
- `general` — free-form user-created tags without category constraints.

**Implementation approach:**
- `tags` table: `id`, `tag_type` (organization/character/metadata/general), `entity_id` (nullable — set for org/character-backed tags), `name` (display for metadata/general; entity-backed tags resolve from entity), `category` (nullable — for metadata faceted search), `parent_id` (hierarchical), `usage_count`, `is_approved`
- Entity-backed tags: auto-created, never deleted, `entity_id` references the owning org/character
- Status tags: `status:open`, `status:closed`, `status:waitlist` — commission availability expressed as metadata tags on the org
- `entity_tags` junction table: `entity_type` (org/commission/feed_item/character/feed_element), `entity_id`, `tag_id` — universal tag assignment for any entity
- Attribution: attaching an org's tag to a commission = crediting that org as participant
- Tag suggestions: auto-complete from existing approved tags
- Community tag proposals: users suggest new metadata/general tags → moderation queue
- Tag synonyms/aliases for search (e.g., "wolf" matches "canine")
- Tags interact with content moderation: "untagged NSFW" detection relies on tag presence (Feature 11)

### 8.3 Recommendation Engine

**What it is:** Personalized org suggestions based on commission history, feed subscriptions, and character species tags.

**Implementation approach:**
- **Heuristic v1:** "Orgs similar to ones you've commissioned" — find orgs with overlapping tags
- **Collaborative filtering v2:** "Users who commissioned Org A also commissioned Org B"
- Input signals: commission history, feed subscriptions (replaces "followed artists"), character species tags, viewed profiles
- Output: ranked list of recommended orgs
- API: `GET /recommendations/orgs`
- Start simple (tag overlap), upgrade to ML models with sufficient data

### 8.4 "Open Now" Feed View

**What it is:** A feed view projecting orgs currently accepting commissions. Not a direct DB query — it is a projection over org commission feeds.

**Implementation approach:**
- Feed view: filters for orgs with the `status:open` tag
- Filterable by tags, price range, content rating — all tag-driven
- Real-time updates via SSE or polling (WebSocket is overkill for a list)
- API: `GET /feeds/open-now?tags=&price_max=`
- Backed by the `entity_feeds` infrastructure — "Open Now" is just one of many possible feed views

## Dependencies

### Requires (must be built first)
- [Feature 1.1](../01-atproto-auth/README.md) — authenticated users
- [Feature 2](../02-identity-profile/README.md) — org profiles and character profiles to search

### Soft dependencies (enhances but not required)
- [Feature 7.3](../07-community-analytics/README.md) — metrics for ranking and price range filtering. Search works without metrics but ranking improves with them.

### Enables (unlocked after this is built)
- Better user acquisition and org discovery — no direct feature dependency, but critical for platform growth
- Tag infrastructure is consumed by Features 7, 9, 10, 11 (tag-based filtering, content classification, NSFW detection)

## Implementation Phases

### Phase 1: Tag Infrastructure & Basic Search
- `tags` table with `tag_type`, `entity_id`, `name`, `category`, `parent_id`
- Auto-creation of org/character tags on entity creation
- `entity_tags` universal junction table
- Seed data for common metadata tags (species, styles, mediums)
- Tag management API: suggest, approve, assign to any entity
- PostgreSQL tsvector search on org profiles
- Basic search endpoint: `GET /search/orgs`
- "Open Now" feed view endpoint
- Crates: domain (Tag root aggregate), persistence (tag repository, search queries), application (tag + search services), api (search routes)

### Phase 2: Faceted Search & Recommendations
- All faceted filtering driven by tags (no separate columns)
- Search index built from PDS records (public data tier)
- Heuristic recommendation engine (tag overlap)
- Auto-complete for tag search
- Tag synonyms/aliases
- Plugin org search support

### Phase 3: Post-implementation
- Evaluate Meilisearch migration if PostgreSQL search becomes a bottleneck
- Collaborative filtering recommendation model (requires commission volume data)
- Search analytics: track query patterns, zero-result queries, click-through rates
- SEO: public org profiles should be search-engine indexable
- NSFW filtering rigor: ensure content rating tags are applied correctly in all search results
- Tag taxonomy curation process and community guidelines
- PDS index freshness monitoring (ensure search data stays in sync with PDS records)

## Assumptions

- PostgreSQL full-text search is sufficient for MVP (thousands of orgs, not millions)
- Tag taxonomy is manually seeded, community-curated later
- Tags are the sole mechanism for descriptive attributes — no separate columns for species, style, etc.
- Entity-backed tags (org/character) are immutable and permanent — they serve as identity references for attribution
- Recommendation engine starts as heuristics — ML is a future upgrade
- "Open Now" feed view doesn't need sub-second latency — 30s polling is acceptable
- Search indexes primarily from PDS records for public data

## Shortcomings & Known Limitations

- **PostgreSQL tsvector limitations:** No fuzzy matching, limited faceted search performance, no typo tolerance
- **Tag taxonomy requires ongoing curation** — tag pollution (duplicates, irrelevant tags) is inevitable
- **Tags-only approach** means losing structured validation — e.g., can't enforce "exactly one species" without tag rules
- **Recommendation engine with few users** gives poor results — cold start problem
- **No geographic/timezone-based search** — relevant for physical commissions (fursuits)
- **NSFW filtering in search** must be rigorous — legal liability if NSFW content leaks to SFW results
- **Search result ranking is opaque** — could be perceived as unfair without transparency
- **No saved searches or alerts** ("notify me when a wolf org opens commissions") — but feed subscriptions partially cover this
- **PDS index lag:** Search results may be stale if PDS sync is delayed
