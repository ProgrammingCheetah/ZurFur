-- Add AT Protocol identity fields to users table
ALTER TABLE users
    ADD COLUMN did TEXT UNIQUE,
    ADD COLUMN handle TEXT,
    ALTER COLUMN email DROP NOT NULL;


-- Store AT Protocol OAuth sessions (access/refresh tokens per user).
-- gen_random_uuid() is built-in since PostgreSQL 13 (no pgcrypto needed).
-- NOTE: access_token/refresh_token are stored as plaintext for now.
-- Production should use application-level encryption (envelope encryption / KMS).
CREATE TABLE IF NOT EXISTS atproto_sessions (
    id UUID PRIMARY KEY NOT NULL DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    did TEXT NOT NULL,
    access_token TEXT NOT NULL,
    refresh_token TEXT,
    expires_at TIMESTAMPTZ NOT NULL,
    pds_url TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT uq_atproto_sessions_user_id UNIQUE (user_id)
);

CREATE INDEX idx_atproto_sessions_did ON atproto_sessions (did);

CREATE TRIGGER atproto_sessions_updated_at
    BEFORE UPDATE ON atproto_sessions
    FOR EACH ROW
    EXECUTE PROCEDURE update_updated_at_column();

-- Store hashed refresh tokens for Zurfur session management
CREATE TABLE IF NOT EXISTS refresh_tokens (
    id UUID PRIMARY KEY NOT NULL DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    token_hash TEXT NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE UNIQUE INDEX idx_refresh_tokens_token_hash ON refresh_tokens (token_hash);
CREATE INDEX idx_refresh_tokens_user_id ON refresh_tokens (user_id);
