-- Character table and visibility enum for Feature 2 Phase 2B.

CREATE TYPE character_visibility AS ENUM ('public', 'private', 'controlled', 'unlisted');

CREATE TABLE character (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id          UUID NOT NULL REFERENCES organization(id),
    name            TEXT NOT NULL,
    description     TEXT,
    content_rating  content_rating NOT NULL,
    visibility      character_visibility NOT NULL,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    deleted_at      TIMESTAMPTZ
);

CREATE INDEX idx_character_org_id ON character (org_id);
