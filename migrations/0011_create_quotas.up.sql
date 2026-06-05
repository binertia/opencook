-- Create quotas table with quota_period and quota_metric enums

-- Add migration script here

CREATE TYPE quota_period AS ENUM ('minute', 'hour', 'day', 'month', 'total');
CREATE TYPE quota_metric AS ENUM ('requests', 'tokens', 'cost_usd');

CREATE TABLE IF NOT EXISTS quotas (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id          UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    api_key_id      UUID REFERENCES api_keys(id) ON DELETE CASCADE,

    name            TEXT NOT NULL,
    description     TEXT,

    metric          quota_metric NOT NULL,
    period          quota_period NOT NULL,

    limit_value     NUMERIC(18, 4) NOT NULL,
    warning_threshold NUMERIC(5, 2) NOT NULL DEFAULT 80.00,

    applies_to      TEXT NOT NULL DEFAULT 'all'
                        CHECK (applies_to IN ('all', 'api_key', 'model', 'provider')),
    scope_filter    JSONB NOT NULL DEFAULT '{}',

    action          TEXT NOT NULL DEFAULT 'block'
                        CHECK (action IN ('block', 'warn', 'throttle')),

    status          TEXT NOT NULL DEFAULT 'active'
                        CHECK (status IN ('active', 'inactive')),

    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    deleted_at      TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS idx_quotas_org ON quotas(org_id) WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_quotas_org_metric_period ON quotas(org_id, metric, period) WHERE deleted_at IS NULL AND status = 'active';
CREATE INDEX IF NOT EXISTS idx_quotas_api_key ON quotas(api_key_id) WHERE deleted_at IS NULL AND api_key_id IS NOT NULL;


