-- Feature 3 Phase 1: Schema cleanup + Tag infrastructure
--
-- ARCHITECTURE DECISIONS:
--
--   1. All tables renamed from plural to singular (except `users` — PG reserved word).
--   2. `created_by` dropped from organization — creator is derived from the owner
--      member in organization_member. Aggregates never reference each other.
--   3. `organization_profiles` dropped — bio becomes a system feed on the org.
--      Commission status becomes a tag.
--   4. `user_preferences` → single JSONB settings column. Extensible without migration.
--   5. Tag: root aggregate with category PG ENUM. Fully decoupled — no entity_id.
--   6. entity_tag: polymorphic junction (same pattern as entity_feed).

-- ═══════════════════════════════════════════════════════════════════
-- 1. Drop tables/constraints that reference columns we're removing
-- ═══════════════════════════════════════════════════════════════════

DROP TABLE organization_profiles;
DROP INDEX IF EXISTS uq_organizations_personal;

-- ═══════════════════════════════════════════════════════════════════
-- 2. Rename tables to singular convention (except users)
-- ═══════════════════════════════════════════════════════════════════

ALTER TABLE organizations RENAME TO organization;
ALTER TABLE organization_members RENAME TO organization_member;
ALTER TABLE feeds RENAME TO feed;
ALTER TABLE entity_feeds RENAME TO entity_feed;
ALTER TABLE feed_items RENAME TO feed_item;
ALTER TABLE feed_elements RENAME TO feed_element;
ALTER TABLE feed_subscriptions RENAME TO feed_subscription;
ALTER TABLE default_roles RENAME TO default_role;
ALTER TABLE user_preferences RENAME TO user_preference;
ALTER TABLE atproto_sessions RENAME TO atproto_session;
ALTER TABLE refresh_tokens RENAME TO refresh_token;

-- ═══════════════════════════════════════════════════════════════════
-- 3. Drop cross-aggregate coupling
-- ═══════════════════════════════════════════════════════════════════

ALTER TABLE organization DROP COLUMN created_by;

-- ═══════════════════════════════════════════════════════════════════
-- 4. User preferences → JSONB
-- ═══════════════════════════════════════════════════════════════════

ALTER TABLE user_preference DROP COLUMN max_content_rating;
ALTER TABLE user_preference ADD COLUMN settings JSONB NOT NULL DEFAULT '{}';

-- ═══════════════════════════════════════════════════════════════════
-- 5. Tag infrastructure
-- ═══════════════════════════════════════════════════════════════════

CREATE TYPE tag_category AS ENUM ('organization', 'character', 'metadata', 'general');

CREATE TABLE tag (
    id           UUID        PRIMARY KEY NOT NULL DEFAULT gen_random_uuid(),
    category     tag_category NOT NULL DEFAULT 'general',
    name         TEXT        NOT NULL,
    usage_count  INT         NOT NULL DEFAULT 0,
    is_approved  BOOLEAN     NOT NULL DEFAULT false,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at   TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TRIGGER tag_updated_at
    BEFORE UPDATE ON tag
    FOR EACH ROW
    EXECUTE PROCEDURE update_updated_at_column();

CREATE UNIQUE INDEX uq_tag_name_category ON tag (name, category);
CREATE INDEX idx_tag_name ON tag (lower(name) text_pattern_ops);

-- ═══════════════════════════════════════════════════════════════════
-- 6. Entity tag junction (polymorphic, same pattern as entity_feed)
-- ═══════════════════════════════════════════════════════════════════

CREATE TABLE entity_tag (
    entity_type TEXT NOT NULL
        CONSTRAINT chk_entity_tag_entity_type
            CHECK (entity_type IN ('org', 'commission', 'feed_item', 'character', 'feed_element')),
    entity_id   UUID NOT NULL,
    tag_id      UUID NOT NULL REFERENCES tag(id) ON DELETE CASCADE,
    PRIMARY KEY (entity_type, entity_id, tag_id)
);

CREATE INDEX idx_entity_tag_entity ON entity_tag (entity_type, entity_id);
CREATE INDEX idx_entity_tag_tag ON entity_tag (tag_id);

-- ═══════════════════════════════════════════════════════════════════
-- 7. Seed common metadata tags
-- ═══════════════════════════════════════════════════════════════════

INSERT INTO tag (category, name, is_approved) VALUES
    -- Species
    ('metadata', 'canine', true),
    ('metadata', 'feline', true),
    ('metadata', 'avian', true),
    ('metadata', 'equine', true),
    ('metadata', 'dragon', true),
    ('metadata', 'protogen', true),
    -- Art styles
    ('metadata', 'digital art', true),
    ('metadata', 'traditional art', true),
    ('metadata', 'pixel art', true),
    ('metadata', 'chibi', true),
    ('metadata', 'toony', true),
    ('metadata', 'realistic', true),
    -- Content types
    ('metadata', 'reference sheet', true),
    ('metadata', 'icon', true),
    ('metadata', 'badge', true),
    ('metadata', 'full illustration', true),
    ('metadata', 'sketch', true),
    -- Status
    ('metadata', 'open', true),
    ('metadata', 'closed', true),
    ('metadata', 'waitlist', true);
