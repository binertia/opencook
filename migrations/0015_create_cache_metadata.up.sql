-- Create cache_metadata table

-- Add migration script here

CREATE TABLE cache_metadata (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id          UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,

    cache_key_hash  TEXT NOT NULL,
    cache_key_preview TEXT,

    model_id        TEXT NOT NULL,
    prompt_preview  TEXT,
    prompt_tokens   INTEGER NOT NULL DEFAULT 0,

    storage_backend TEXT NOT NULL DEFAULT 'redis',
    ttl_seconds     INTEGER NOT NULL DEFAULT 3600,
    expires_at      TIMESTAMPTZ NOT NULL,

    hit_count       INTEGER NOT NULL DEFAULT 0,
    last_hit_at     TIMESTAMPTZ,

    content_hash    TEXT,

    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    deleted_at      TIMESTAMPTZ
);

CREATE UNIQUE INDEX uk_cache_metadata_org_hash ON cache_metadata(org_id, cache_key_hash) WHERE deleted_at IS NULL;
CREATE INDEX idx_cache_metadata_expires ON cache_metadata(expires_at) WHERE deleted_at IS NULL;
CREATE INDEX idx_cache_metadata_org_model ON cache_metadata(org_id, model_id) WHERE deleted_at IS NULL;


