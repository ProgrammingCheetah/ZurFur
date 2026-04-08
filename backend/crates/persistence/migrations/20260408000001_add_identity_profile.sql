-- Feature 2 Phase 1: Identity & Profile Engine
--
-- ARCHITECTURE DECISIONS:
--
--   Org-centric identity model: every user gets a personal organization on signup.
--   The personal org IS the user's public profile. Users can create additional orgs
--   freely (studios, groups, SFW/NSFW separation). User entity stays atomic —
--   no feature flags, roles, or bios added to the users table.
--
--   content_rating: PostgreSQL ENUM type. The set (sfw/questionable/nsfw) is stable
--   and unlikely to change. Adding a variant is still possible via ALTER TYPE.
--
--   commission_status: TEXT + CHECK constraint (not a PG enum) because this set
--   will likely grow as the platform evolves (e.g., 'paused', 'by_request').
--   TEXT avoids the ALTER TYPE limitations of PG enums.
--
--   display_name on organizations: NULLABLE by design. For personal orgs, NULL
--   means "resolve from the owner's username/handle" at the API layer. This avoids
--   duplicating the user's handle (which syncs from Bluesky) and prevents stale data.
--
--   Permissions: BIGINT bitfield on organization_members. Faster than JSONB for
--   permission checks, compact storage, extensible without migration by defining
--   new bit positions. Owner gets ALL (all bits set = max u64).
--
--   gen_random_uuid(): built-in since PostgreSQL 13. This project targets PG 16.
--
--   update_updated_at_column(): trigger function defined in the first migration
--   (20250227000001_create_users.sql) and reused here.

-- 1. Content rating enum type
CREATE TYPE content_rating AS ENUM ('sfw', 'questionable', 'nsfw');

-- 2. Organizations table
CREATE TABLE IF NOT EXISTS organizations (
    id            UUID        PRIMARY KEY NOT NULL DEFAULT gen_random_uuid(),
    slug          TEXT        NOT NULL,
    display_name  VARCHAR,
    is_personal   BOOLEAN     NOT NULL DEFAULT false,
    created_by    UUID        NOT NULL REFERENCES users(id),
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    deleted_at    TIMESTAMPTZ
);

-- Slug uniqueness only among non-deleted orgs. A deleted org's slug can be reused.
CREATE UNIQUE INDEX uq_organizations_slug
    ON organizations (slug)
    WHERE deleted_at IS NULL;

-- Fast lookup of a user's personal org.
CREATE INDEX idx_organizations_personal
    ON organizations (created_by)
    WHERE is_personal = true AND deleted_at IS NULL;

CREATE TRIGGER organizations_updated_at
    BEFORE UPDATE ON organizations
    FOR EACH ROW
    EXECUTE PROCEDURE update_updated_at_column();

-- 3. Organization members table
CREATE TABLE IF NOT EXISTS organization_members (
    id            UUID        PRIMARY KEY NOT NULL DEFAULT gen_random_uuid(),
    org_id        UUID        NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    user_id       UUID        NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    role          TEXT        NOT NULL DEFAULT 'member',
    title         TEXT,
    is_owner      BOOLEAN     NOT NULL DEFAULT false,
    permissions   BIGINT      NOT NULL DEFAULT 0,
    joined_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at    TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE UNIQUE INDEX uq_organization_members_org_user
    ON organization_members (org_id, user_id);

CREATE INDEX idx_organization_members_user
    ON organization_members (user_id);

CREATE TRIGGER organization_members_updated_at
    BEFORE UPDATE ON organization_members
    FOR EACH ROW
    EXECUTE PROCEDURE update_updated_at_column();

-- 4. Organization profiles table (optional, 0..1 per org)
CREATE TABLE IF NOT EXISTS organization_profiles (
    org_id            UUID        PRIMARY KEY NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    bio               TEXT,
    commission_status TEXT        NOT NULL DEFAULT 'closed'
        CONSTRAINT chk_organization_profiles_commission_status
            CHECK (commission_status IN ('open', 'closed', 'waitlist')),
    created_at        TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at        TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TRIGGER organization_profiles_updated_at
    BEFORE UPDATE ON organization_profiles
    FOR EACH ROW
    EXECUTE PROCEDURE update_updated_at_column();

-- 5. User preferences table (content rating filter)
CREATE TABLE IF NOT EXISTS user_preferences (
    user_id            UUID           PRIMARY KEY NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    max_content_rating content_rating NOT NULL DEFAULT 'sfw',
    created_at         TIMESTAMPTZ    NOT NULL DEFAULT now(),
    updated_at         TIMESTAMPTZ    NOT NULL DEFAULT now()
);

CREATE TRIGGER user_preferences_updated_at
    BEFORE UPDATE ON user_preferences
    FOR EACH ROW
    EXECUTE PROCEDURE update_updated_at_column();
