-- Feature 2 Phase 2: Feeds, Default Roles, Onboarding
--
-- ARCHITECTURE DECISIONS:
--
--   Feed system: feeds are the universal content container. Ownership is
--   polymorphic via entity_feeds (an org, character, commission, or user
--   can own feeds). System feeds (type='system') are undeletable.
--
--   entity_feeds: polymorphic join using entity_type TEXT + entity_id UUID.
--   Not a foreign key — validated at the application layer. This avoids
--   multi-table FK complexity while keeping the schema flexible.
--
--   feed_elements.content_json: TEXT not JSONB, because the domain layer
--   treats it as an opaque string. Validation happens at the application
--   layer before persistence.
--
--   default_roles: seeded with 4 system rows. Permissions use the same
--   BIGINT bitfield as organization_members. Owner gets -1 (all bits set
--   via i64 wrapping to u64::MAX in Rust).
--
--   onboarding_completed_at: nullable timestamp on users. NULL = onboarding
--   pending. This is a platform lifecycle field, not a feature flag.

-- 1. Add onboarding_completed_at to users
ALTER TABLE users
    ADD COLUMN onboarding_completed_at TIMESTAMPTZ;

-- 2. Default roles (system seed data)
CREATE TABLE default_roles (
    id                  UUID    PRIMARY KEY NOT NULL DEFAULT gen_random_uuid(),
    name                TEXT    NOT NULL UNIQUE,
    default_permissions BIGINT  NOT NULL DEFAULT 0,
    hierarchy_level     INT     NOT NULL DEFAULT 0
);

INSERT INTO default_roles (name, default_permissions, hierarchy_level) VALUES
    ('owner',  -1, 0),    -- all bits set (u64::MAX via i64 wrapping)
    ('admin',  63, 1),    -- all 6 current permission bits (0b111111)
    ('mod',    9, 2),     -- MANAGE_PROFILE (1) | CHAT (8)
    ('member', 8, 3);     -- CHAT only

-- 3. Feeds
CREATE TABLE feeds (
    id           UUID        PRIMARY KEY NOT NULL DEFAULT gen_random_uuid(),
    slug         TEXT        NOT NULL,
    display_name TEXT        NOT NULL,
    description  TEXT,
    feed_type    TEXT        NOT NULL DEFAULT 'custom'
        CONSTRAINT chk_feeds_feed_type CHECK (feed_type IN ('system', 'custom')),
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    deleted_at   TIMESTAMPTZ
);

CREATE TRIGGER feeds_updated_at
    BEFORE UPDATE ON feeds
    FOR EACH ROW
    EXECUTE PROCEDURE update_updated_at_column();

-- 4. Entity feeds (polymorphic join — each feed belongs to exactly one entity)
CREATE TABLE entity_feeds (
    feed_id     UUID NOT NULL REFERENCES feeds(id) ON DELETE CASCADE,
    entity_type TEXT NOT NULL
        CONSTRAINT chk_entity_feeds_entity_type CHECK (entity_type IN ('org', 'character', 'commission', 'user')),
    entity_id   UUID NOT NULL,
    PRIMARY KEY (feed_id)
);

-- Fast lookup: all feeds for a given entity
CREATE INDEX idx_entity_feeds_entity
    ON entity_feeds (entity_type, entity_id);

-- 5. Feed items
CREATE TABLE feed_items (
    id          UUID        PRIMARY KEY NOT NULL DEFAULT gen_random_uuid(),
    feed_id     UUID        NOT NULL REFERENCES feeds(id) ON DELETE CASCADE,
    author_type TEXT        NOT NULL
        CONSTRAINT chk_feed_items_author_type CHECK (author_type IN ('user', 'org', 'system')),
    author_id   UUID        NOT NULL,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_feed_items_feed
    ON feed_items (feed_id, created_at DESC);

-- 6. Feed elements
CREATE TABLE feed_elements (
    id            UUID    PRIMARY KEY NOT NULL DEFAULT gen_random_uuid(),
    feed_item_id  UUID    NOT NULL REFERENCES feed_items(id) ON DELETE CASCADE,
    element_type  TEXT    NOT NULL
        CONSTRAINT chk_feed_elements_element_type CHECK (element_type IN ('text', 'image', 'file', 'event', 'embed')),
    content_json  TEXT    NOT NULL,
    position      INT     NOT NULL DEFAULT 0
);

CREATE INDEX idx_feed_elements_item
    ON feed_elements (feed_item_id, position);

-- 7. Feed subscriptions
CREATE TABLE feed_subscriptions (
    id                  UUID        PRIMARY KEY NOT NULL DEFAULT gen_random_uuid(),
    feed_id             UUID        NOT NULL REFERENCES feeds(id) ON DELETE CASCADE,
    subscriber_org_id   UUID        NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    permissions         TEXT        NOT NULL DEFAULT 'read'
        CONSTRAINT chk_feed_subscriptions_permissions CHECK (permissions IN ('read', 'read_write', 'admin')),
    granted_at          TIMESTAMPTZ NOT NULL DEFAULT now(),
    granted_by_user_id  UUID        NOT NULL REFERENCES users(id)
);

-- One subscription per (feed, org) pair
CREATE UNIQUE INDEX uq_feed_subscriptions_feed_org
    ON feed_subscriptions (feed_id, subscriber_org_id);

-- Lookup by feed (list_by_feed queries)
CREATE INDEX idx_feed_subscriptions_feed
    ON feed_subscriptions (feed_id);

-- Lookup by subscriber org (list_by_subscriber queries)
CREATE INDEX idx_feed_subscriptions_subscriber
    ON feed_subscriptions (subscriber_org_id);
