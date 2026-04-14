# Zurfur — Domain Relationship Map

> **Updated 2026-04-12**

## Core Principle

Aggregates are decoupled at the database level. No aggregate table references another. All cross-aggregate relationships live in separate junction/bridge tables.

```mermaid
erDiagram
    %% ═══════════════════════════════════════
    %% ROOT AGGREGATES
    %% ═══════════════════════════════════════

    user {
        UUID id PK
        TEXT did UK
        TEXT handle
        TEXT email
        TEXT username
        TIMESTAMPTZ onboarding_completed_at
    }

    organization {
        UUID id PK
        TEXT slug UK
        TEXT display_name "nullable"
        BOOL is_personal
    }

    feed {
        UUID id PK
        TEXT slug
        TEXT display_name
        TEXT feed_type "system | custom"
    }

    tag {
        UUID id PK
        tag_category category "organization | character | metadata | general"
        TEXT name
        INT usage_count
        BOOL is_approved
    }

    commission {
        UUID id PK
        TEXT current_state
        TEXT title
    }

    %% ═══════════════════════════════════════
    %% TYPED BRIDGES
    %% ═══════════════════════════════════════

    organization_member {
        UUID id PK
        UUID org_id FK
        UUID user_id FK
        TEXT role "owner | admin | mod | member"
        TEXT title "nullable"
        BIGINT permissions
    }

    feed_subscription {
        UUID id PK
        UUID feed_id FK
        UUID subscriber_org_id FK
        TEXT permissions "read | read_write | admin"
    }

    %% ═══════════════════════════════════════
    %% POLYMORPHIC JUNCTIONS
    %% ═══════════════════════════════════════

    entity_feed {
        UUID feed_id PK_FK
        TEXT entity_type "org | character | commission | user"
        UUID entity_id
    }

    entity_tag {
        TEXT entity_type PK "org | commission | feed_item | character | feed_element"
        UUID entity_id PK
        UUID tag_id PK_FK
    }

    %% ═══════════════════════════════════════
    %% AGGREGATE EXTENSIONS
    %% ═══════════════════════════════════════

    user_preference {
        UUID user_id PK_FK
        JSONB settings
    }

    %% ═══════════════════════════════════════
    %% OWNED CHILDREN
    %% ═══════════════════════════════════════

    feed_item {
        UUID id PK
        UUID feed_id FK
        TEXT author_type "user | org | system"
        UUID author_id
    }

    feed_element {
        UUID id PK
        UUID feed_item_id FK
        TEXT element_type "text | image | file | event | embed"
        TEXT content_json
        INT position
    }

    %% ═══════════════════════════════════════
    %% RELATIONSHIPS
    %% ═══════════════════════════════════════

    user ||--o{ organization_member : ""
    organization ||--o{ organization_member : ""

    feed ||--o{ feed_subscription : ""
    organization ||--o{ feed_subscription : ""

    feed ||--|| entity_feed : ""
    tag ||--o{ entity_tag : ""

    user ||--o| user_preference : ""

    feed ||--o{ feed_item : ""
    feed_item ||--o{ feed_element : ""
```

## Connection Patterns

| Pattern | Example | Coupling |
|---------|---------|----------|
| Polymorphic junction | `entity_feed`, `entity_tag` | None — type + ID, no FK to entity tables |
| Typed bridge | `organization_member`, `feed_subscription` | Separate table, references both aggregates |

## Notes

- **Bio is a feed.** Organization has no profile table. The bio lives in a system feed (`bio`) created when the org is created. Edits are new feed items — gives us version history for free.
- **Tag category is a PG ENUM**, not TEXT. Values: `organization`, `character`, `metadata`, `general`. Defaults to `general`. This is intrinsic to the tag — what the tag IS, not how it's connected.
- **User preferences are JSONB.** One `settings` column. Extensible without migration.
- **Auth tables** (`atproto_session`, `refresh_token`, `default_role`) omitted for clarity — they are owned children of the User aggregate.
