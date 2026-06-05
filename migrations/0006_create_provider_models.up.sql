-- Create provider_models table

-- Add migration script here

CREATE TABLE IF NOT EXISTS provider_models (
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id              UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    provider_config_id  UUID NOT NULL REFERENCES provider_configs(id) ON DELETE CASCADE,
    model_id            TEXT NOT NULL,
    model_name          TEXT NOT NULL,
    aliases             TEXT[] NOT NULL DEFAULT '{}',
    input_cost_per_1k   NUMERIC(18, 8) NOT NULL DEFAULT 0,
    output_cost_per_1k  NUMERIC(18, 8) NOT NULL DEFAULT 0,
    context_window      INTEGER,
    max_tokens          INTEGER,
    supports_streaming  BOOLEAN NOT NULL DEFAULT true,
    supports_tools      BOOLEAN NOT NULL DEFAULT false,
    supports_vision     BOOLEAN NOT NULL DEFAULT false,
    status              TEXT NOT NULL DEFAULT 'active'
                            CHECK (status IN ('active', 'deprecated', 'disabled')),
    config              JSONB NOT NULL DEFAULT '{}',
    created_at          TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT now(),
    deleted_at          TIMESTAMPTZ
);

CREATE UNIQUE INDEX IF NOT EXISTS uk_provider_models_provider_model ON provider_models(provider_config_id, model_id) WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_provider_models_org_id ON provider_models(org_id) WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_provider_models_alias ON provider_models USING GIN (aliases) WHERE deleted_at IS NULL;


