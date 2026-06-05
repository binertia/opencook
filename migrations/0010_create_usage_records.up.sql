-- Create usage_records table (partitioned by period_start range)

-- Add migration script here

CREATE TABLE IF NOT EXISTS usage_records (
    id              UUID NOT NULL DEFAULT gen_random_uuid(),
    org_id          UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    api_key_id      UUID REFERENCES api_keys(id) ON DELETE SET NULL,
    provider_config_id UUID REFERENCES provider_configs(id) ON DELETE SET NULL,
    provider_model_id  UUID REFERENCES provider_models(id) ON DELETE SET NULL,

    period          TEXT NOT NULL CHECK (period IN ('hourly', 'daily', 'monthly')),
    period_start    TIMESTAMPTZ NOT NULL,

    request_count   INTEGER NOT NULL DEFAULT 0,
    request_success INTEGER NOT NULL DEFAULT 0,
    request_error   INTEGER NOT NULL DEFAULT 0,

    prompt_tokens       BIGINT NOT NULL DEFAULT 0,
    completion_tokens   BIGINT NOT NULL DEFAULT 0,
    total_tokens        BIGINT NOT NULL DEFAULT 0,

    input_cost      NUMERIC(18, 8) NOT NULL DEFAULT 0,
    output_cost     NUMERIC(18, 8) NOT NULL DEFAULT 0,
    total_cost      NUMERIC(18, 8) NOT NULL DEFAULT 0,

    latency_ms_p50  INTEGER,
    latency_ms_p90  INTEGER,
    latency_ms_p99  INTEGER,
    latency_ms_avg  INTEGER,

    cache_hits      INTEGER NOT NULL DEFAULT 0,
    cache_misses    INTEGER NOT NULL DEFAULT 0,

    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    deleted_at      TIMESTAMPTZ,

    UNIQUE(org_id, api_key_id, provider_config_id, provider_model_id, period, period_start)
) PARTITION BY RANGE (period_start);

CREATE INDEX IF NOT EXISTS idx_usage_org_period ON usage_records(org_id, period, period_start DESC) WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_usage_org_model ON usage_records(org_id, provider_model_id, period_start DESC) WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_usage_period_start ON usage_records(period_start DESC) WHERE deleted_at IS NULL;


