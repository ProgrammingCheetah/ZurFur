# Feature 8: Search & Discovery

## Overview

How users find artists. Without discovery, users can only find artists they already know. This feature provides full-text artist search with faceted filtering, a structured tag taxonomy, personalized recommendations, and a real-time "Open Now" feed.

## Sub-features

### 8.1 Artist Search

**What it is:** Find artists by tag, art style, species specialty, price range, and availability status. Supports full-text and faceted filtering.

**Implementation approach:**
- **MVP:** PostgreSQL `tsvector` full-text search on artist profile fields (bio, display_name, tags)
- **Upgrade path:** Meilisearch or Elasticsearch for complex faceted search
- Search index: materialized view or table combining artist profile data + tags + metrics (price range from Feature 7.3)
- Faceted filters: species tags, art style, medium, content rating, commission status, price range
- API: `GET /search/artists?q=&tags=&style=&price_min=&price_max=&status=open`

### 8.2 Tag Taxonomy

**What it is:** Structured, community-curated tag system for species, art style, medium, and content rating.

**Implementation approach:**
- `tags` table: `id`, `name`, `category` (species/style/medium/content_type), `parent_id` (hierarchical), `usage_count`, `is_approved`
- `artist_tags` junction table: `artist_id`, `tag_id`
- `character_tags` junction table: `character_id`, `tag_id`
- Tag suggestions: auto-complete from existing approved tags
- Community tag proposals: users suggest new tags → moderation queue
- Tag synonyms/aliases for search (e.g., "wolf" matches "canine")

### 8.3 Recommendation Engine

**What it is:** Personalized artist suggestions based on commission history, follows, and character species.

**Implementation approach:**
- **Heuristic v1:** "Artists similar to ones you've commissioned" — find artists with overlapping tags
- **Collaborative filtering v2:** "Users who commissioned Artist A also commissioned Artist B"
- Input signals: commission history, followed artists, character species tags, viewed profiles
- Output: ranked list of recommended artists
- API: `GET /recommendations/artists`
- Start simple (tag overlap), upgrade to ML models with sufficient data

### 8.4 "Open Now" Feed

**What it is:** Real-time filterable feed of artists currently accepting commissions.

**Implementation approach:**
- Query: `SELECT FROM artist_profiles WHERE commission_status = 'open' ORDER BY status_changed_at DESC`
- `status_changed_at` field on `artist_profiles` (set when artist toggles status)
- Filterable by tags, price range, content rating
- Real-time updates via SSE or polling (WebSocket is overkill for a list)
- API: `GET /feed/open-artists?tags=&price_max=`

## Dependencies

### Requires (must be built first)
- [Feature 1.1](../01-atproto-auth/README.md) — authenticated users
- [Feature 2](../02-identity-profile/README.md) — artist profiles and character profiles to search
- [Feature 7.3](../07-community-analytics/README.md) — metrics for ranking and price range filtering (soft dependency — search works without it but is better with it)

### Enables (unlocked after this is built)
- Better user acquisition and artist discovery — no direct feature dependency, but critical for platform growth

## Implementation Phases

### Phase 1: Tags & Basic Search
- `tags` table with seed data (common species, styles, mediums)
- `artist_tags` and `character_tags` junction tables
- Tag management API: suggest, approve, assign
- PostgreSQL tsvector search on artist profiles
- Basic search endpoint: `GET /search/artists`
- "Open Now" feed endpoint
- Crates: domain (Tag entity), persistence (search queries), application (search use case), api (search routes)

### Phase 2: Faceted Search & Recommendations
- Faceted filtering (price range, tags, status, content rating)
- Search index optimization (materialized view or dedicated search table)
- Heuristic recommendation engine (tag overlap)
- Auto-complete for tag search
- Tag synonyms/aliases

### Phase 3: Post-implementation
- Evaluate Meilisearch migration if PostgreSQL search becomes a bottleneck
- Collaborative filtering recommendation model (requires commission volume data)
- Search analytics: track query patterns, zero-result queries, click-through rates
- SEO: public artist profiles should be search-engine indexable
- NSFW filtering rigor: ensure content rating filters are applied correctly in all search results
- Tag taxonomy curation process and community guidelines

## Assumptions

- PostgreSQL full-text search is sufficient for MVP (thousands of artists, not millions)
- Tag taxonomy is manually seeded, community-curated later
- Recommendation engine starts as heuristics — ML is a future upgrade
- "Open Now" feed doesn't need sub-second latency — 30s polling is acceptable

## Shortcomings & Known Limitations

- **PostgreSQL tsvector limitations:** No fuzzy matching, limited faceted search performance, no typo tolerance
- **Tag taxonomy requires ongoing curation** — tag pollution (duplicates, irrelevant tags) is inevitable
- **Recommendation engine with few users** gives poor results — cold start problem
- **No geographic/timezone-based search** — relevant for physical commissions (fursuits)
- **NSFW filtering in search** must be rigorous — legal liability if NSFW content leaks to SFW results
- **Search result ranking is opaque** — could be perceived as unfair without transparency
- **No saved searches or alerts** ("notify me when a wolf artist opens commissions")
