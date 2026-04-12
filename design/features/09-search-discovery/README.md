> **Revised 2026-04-12** — Tag taxonomy extracted to Feature 3. Updated feature numbering.

# Feature 9: Search & Discovery

## Overview

How users find orgs offering commissions. Without discovery, users can only find orgs they already know. This feature provides full-text search across orgs (artist studios, plugin orgs, personal orgs) with faceted filtering driven entirely by the tag system ([Feature 3](../03-tag-taxonomy/README.md)). Search indexes from PDS records (public data tier). The "Open Now" feed is a feed view, not a DB query.

## Sub-features

### 9.1 Org Search

**What it is:** Find orgs by tags, art style, species specialty, price range, and availability status. Supports full-text and faceted filtering. Plugin orgs are also searchable.

**Implementation approach:**
- **MVP:** PostgreSQL `tsvector` full-text search on org profile fields (bio, display_name, tags)
- **Upgrade path:** Meilisearch or Elasticsearch for complex faceted search
- Search index: built from PDS records (public data tier) combined with tag associations
- All search facets are driven by tags — no separate columns for species, style, medium, etc.
- Faceted filters: all tag-based (species, art style, medium, content rating, status), plus price range. Commission availability status is a tag on the org (e.g., `status:open`). The "Open Now" feed view filters for orgs with the `status:open` tag.
- Plugin orgs appear in search results alongside artist orgs
- API: `GET /search/orgs?q=&tags=status:open,species:wolf&price_min=&price_max=&type=artist,plugin`

### 9.2 Recommendation Engine

**What it is:** Personalized org suggestions based on commission history, feed subscriptions, and character species tags.

**Implementation approach:**
- **Heuristic v1:** "Orgs similar to ones you've commissioned" — find orgs with overlapping tags
- **Collaborative filtering v2:** "Users who commissioned Org A also commissioned Org B"
- Input signals: commission history, feed subscriptions (replaces "followed artists"), character species tags, viewed profiles
- Output: ranked list of recommended orgs
- API: `GET /recommendations/orgs`
- Start simple (tag overlap), upgrade to ML models with sufficient data

### 9.3 "Open Now" Feed View

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
- [Feature 3](../03-tag-taxonomy/README.md) — tag infrastructure (all search facets are tag-driven)

### Soft dependencies (enhances but not required)
- [Feature 8.3](../08-community-analytics/README.md) — metrics for ranking and price range filtering. Search works without metrics but ranking improves with them.

### Enables (unlocked after this is built)
- Better user acquisition and org discovery — no direct feature dependency, but critical for platform growth

## Implementation Phases

### Phase 1: Basic Search & "Open Now"
- PostgreSQL tsvector search on org profiles
- Basic search endpoint: `GET /search/orgs`
- "Open Now" feed view endpoint
- Crates: domain (search queries), persistence (search repository), application (search service), api (search routes)

### Phase 2: Faceted Search & Recommendations
- All faceted filtering driven by tags (no separate columns)
- Search index built from PDS records (public data tier)
- Heuristic recommendation engine (tag overlap)
- Auto-complete for tag search
- Plugin org search support

### Phase 3: Post-implementation
- Evaluate Meilisearch migration if PostgreSQL search becomes a bottleneck
- Collaborative filtering recommendation model (requires commission volume data)
- Search analytics: track query patterns, zero-result queries, click-through rates
- SEO: public org profiles should be search-engine indexable
- NSFW filtering rigor: ensure content rating tags are applied correctly in all search results
- PDS index freshness monitoring (ensure search data stays in sync with PDS records)

## Assumptions

- PostgreSQL full-text search is sufficient for MVP (thousands of orgs, not millions)
- Recommendation engine starts as heuristics — ML is a future upgrade
- "Open Now" feed view doesn't need sub-second latency — 30s polling is acceptable
- Search indexes primarily from PDS records for public data
- All search facets depend on tag infrastructure (Feature 3) being in place

## Shortcomings & Known Limitations

- **PostgreSQL tsvector limitations:** No fuzzy matching, limited faceted search performance, no typo tolerance
- **Recommendation engine with few users** gives poor results — cold start problem
- **No geographic/timezone-based search** — relevant for physical commissions (fursuits)
- **NSFW filtering in search** must be rigorous — legal liability if NSFW content leaks to SFW results
- **Search result ranking is opaque** — could be perceived as unfair without transparency
- **No saved searches or alerts** ("notify me when a wolf org opens commissions") — but feed subscriptions partially cover this
- **PDS index lag:** Search results may be stale if PDS sync is delayed
