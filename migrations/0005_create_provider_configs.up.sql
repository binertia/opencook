-- Create provider_configs table with provider_kind enum

-- Add migration script here

CREATE TYPE provider_kind AS ENUM (
    'openai', 'anthropic', 'azure_openai', 'google_gemini',
    'cohere', 'mistral', 'groq', 'custom', 'bedrock'
);

CREATE TABLE IF NOT EXISTS provider_configs (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id          UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    name            TEXT NOT NULL,
    kind            provider_kind NOT NULL,
    api_base        TEXT,
    api_key_enc     BYTEA NOT NULL,
    default_headers JSONB NOT NULL DEFAULT '{}',
    config          JSONB NOT NULL DEFAULT '{}',
    priority        INTEGER NOT NULL DEFAULT 0,
    status          TEXT NOT NULL DEFAULT 'active'
                        CHECK (status IN ('active', 'inactive', 'error')),
    last_error_at   TIMESTAMPTZ,
    last_error_msg  TEXT,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    deleted_at      TIMESTAMPTZ
);

CREATE UNIQUE INDEX IF NOT EXISTS uk_provider_configs_org_name ON provider_configs(org_id, name) WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_provider_configs_org_id ON provider_configs(org_id) WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_provider_configs_kind ON provider_configs(kind);


