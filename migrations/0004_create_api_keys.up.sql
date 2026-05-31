-- Create api_keys table

-- Add migration script here

CREATE TABLE api_keys (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id          UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    user_id         UUID REFERENCES users(id) ON DELETE SET NULL,
    name            TEXT NOT NULL,
    key_hash        TEXT NOT NULL,
    key_prefix      TEXT NOT NULL,
    scopes          TEXT[] NOT NULL DEFAULT ARRAY['ai:write'],
    rate_limit_rps  INTEGER NOT NULL DEFAULT 10,
    status          TEXT NOT NULL DEFAULT 'active'
                        CHECK (status IN ('active', 'revoked', 'expired')),
    expires_at      TIMESTAMPTZ,
    last_used_at    TIMESTAMPTZ,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    deleted_at      TIMESTAMPTZ
);

CREATE UNIQUE INDEX uk_api_keys_key_hash ON api_keys(key_hash) WHERE deleted_at IS NULL;
CREATE INDEX idx_api_keys_org_id ON api_keys(org_id) WHERE deleted_at IS NULL;
CREATE INDEX idx_api_keys_key_prefix ON api_keys(key_prefix) WHERE deleted_at IS NULL;


