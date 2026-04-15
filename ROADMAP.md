# Zurfur — Roadmap

> **Updated 2026-04-15**

See `design/features/OVERVIEW.md` for the full feature dependency map and implementation order.

## MVP Critical Path

The minimum path from "user signs up" to "artist gets paid":

```
Auth (1) → Org (2) → Tags (3) → Feeds (2+) → Characters (2.3) → TOS (11) → Commission (4) → Payments (5)
```

### Current Progress

| Step | Feature | Status |
|------|---------|--------|
| 1 | AT Protocol OAuth | Done |
| 2 | Identity & Org Engine (Phase 1-2) | Done |
| 3 | Tag Taxonomy & Attribution | Done |
| 3.5 | Transaction Support (UoW) | Done (pending merge) |
| 4 | **OpenAPI + API Documentation** | Next |
| 5 | Characters (Phase 2.3) | Planned |
| 6 | Organization TOS | Planned |
| 7 | Headless Commission Engine | Planned |
| 8 | Financial Gateway | Planned |

### Entity System Formalization (In Progress)

Defining the core entity interfaces as Rust traits before building more features:
- **Entity** — `fn id(&self) -> Uuid`, base trait for all domain objects
- **FeedOwnable** — can own feeds. All entities implement this (Tags use virtualized queries)
- **Taggable** — can have tags. All entities implement this by default
- **Authorable** — can author feed items (User, Org, System only)

## Post-MVP — Entity System Extensions

Features that emerge from the "everything is an entity" architecture. These are structurally possible today but deferred to avoid over-complicating the MVP.

### Org Hierarchy (Nesting Doll)

Organizations belonging to other organizations — like teams in a company.

```
Furry Art Collective (org)
  └── StudioFox (org, role: member)
        └── Zuri (personal org, role: artist)
              └── Zuri the user (member, role: owner)
```

**What it enables:**
- Artist orgs belonging to studio orgs
- Studios belonging to collectives
- Content bubbling up through feed subscriptions at each level
- Shared permissions flowing down the hierarchy

**Implementation path:**
- Make `organization_member` polymorphic (`member_type` + `member_id` replacing `user_id` FK)
- Or: separate `organization_relationship` typed bridge
- Recursive CTEs for hierarchy traversal, depth limits to prevent N+1

**Why deferred:** Adds complexity to the membership model. MVP only needs User ↔ Org membership.

### Tag Taxonomy (Tags Tagging Tags)

Tags tagging other tags to create hierarchy, synonyms, and relationships.

```
canine (general)
  └── wolf (general, tagged with canine)
  └── fox (general, tagged with canine)
```

**What it enables:**
- Parent/child tag relationships (wolf IS a canine)
- Tag synonyms and grouping
- Hierarchical search (search "canine" finds "wolf" content)
- Meta-categorization (tag "wolf" with "species" metadata tag)

**Implementation path:**
- `EntityKind::Tag` and CHECK constraints already support tag-on-tag relationships
- Recursive CTEs for ancestor/descendant traversal
- Depth limit to bound traversal (3-4 levels sufficient)

**Why deferred:** N+1 traversal complexity. MVP tags work fine as a flat set.

### Tag Feeds (Topic Following)

Tags owning feeds, enabling topic-based content aggregation.

**What it enables:**
- Subscribe to a tag = get a feed of all content with that tag
- Tags subscribing to tags = content propagation through tag hierarchy
- Topic-based discovery feeds

**Implementation path:**
- `EntityKind::Tag` and CHECK constraints already support tag-owned feeds
- Create system feed on tag creation (or lazily on first subscription)
- Existing feed subscription infrastructure handles the rest

**Why deferred:** Requires tag taxonomy to be truly useful. MVP discovery uses direct search.

### Inherited Feed Permissions

Allow entities to manage feeds belonging to child entities through relationship inheritance rather than direct ownership.

**What it enables:**
- Organization manages a Character's gallery feed because the character belongs to the org
- Parent org manages sub-org feeds in the hierarchy model
- Permission inheritance without duplicating ownership

**Implementation path:**
- Feed ownership stays direct (`entity_feed` junction)
- Add permission inheritance layer that resolves "can this user manage this feed?" through entity relationships
- Walks the ownership chain: feed → entity_feed → character → org → org_member → user

**Why deferred:** MVP uses direct ownership. Management permissions flow through org membership checks at the service layer.
