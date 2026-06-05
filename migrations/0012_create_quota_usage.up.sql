-- Create quota_usage table

-- Add migration script here

CREATE TABLE IF NOT EXISTS quota_usage (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id          UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    quota_id        UUID NOT NULL REFERENCES quotas(id) ON DELETE CASCADE,
    api_key_id      UUID REFERENCES api_keys(id) ON DELETE CASCADE,

    period_start    TIMESTAMPTZ NOT NULL,
    period_end      TIMESTAMPTZ NOT NULL,

    current_value   NUMERIC(18, 4) NOT NULL DEFAULT 0,

    limit_value     NUMERIC(18, 4) NOT NULL,
    metric          quota_metric NOT NULL,

    exceeded_at     TIMESTAMPTZ,
    warned_at       TIMESTAMPTZ,

    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    deleted_at      TIMESTAMPTZ,

    UNIQUE(org_id, quota_id, api_key_id, period_start)
);

CREATE INDEX IF NOT EXISTS idx_quota_usage_org_quota ON quota_usage(org_id, quota_id, period_start) WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_quota_usage_api_key ON quota_usage(api_key_id) WHERE deleted_at IS NULL AND api_key_id IS NOT NULL;


