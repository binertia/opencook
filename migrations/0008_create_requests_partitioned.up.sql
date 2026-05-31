-- Create requests table (partitioned by created_at range)

-- Add migration script here

CREATE TYPE request_status AS ENUM (
    'pending', 'processing', 'success', 'error', 'timeout', 'cancelled'
);

CREATE TABLE requests (
    id                  UUID NOT NULL DEFAULT gen_random_uuid(),
    org_id              UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    api_key_id          UUID REFERENCES api_keys(id) ON DELETE SET NULL,
    user_id             UUID REFERENCES users(id) ON DELETE SET NULL,
    provider_config_id  UUID REFERENCES provider_configs(id) ON DELETE SET NULL,
    provider_model_id   UUID REFERENCES provider_models(id) ON DELETE SET NULL,
    routing_rule_id     UUID REFERENCES routing_rules(id) ON DELETE SET NULL,

    trace_id            TEXT NOT NULL,
    parent_trace_id     TEXT,

    method              TEXT NOT NULL DEFAULT 'POST',
    path                TEXT NOT NULL,
    model_requested     TEXT,
    model_routed        TEXT,

    request_headers     JSONB NOT NULL DEFAULT '{}',
    request_body        TEXT,
    request_body_truncated BOOLEAN NOT NULL DEFAULT false,

    requested_at        TIMESTAMPTZ NOT NULL DEFAULT now(),
    gateway_received_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    provider_sent_at    TIMESTAMPTZ,
    provider_responded_at TIMESTAMPTZ,
    completed_at        TIMESTAMPTZ,

    latency_gateway_ms  INTEGER,
    latency_provider_ms INTEGER,
    latency_total_ms    INTEGER,

    prompt_tokens       INTEGER NOT NULL DEFAULT 0,
    completion_tokens   INTEGER NOT NULL DEFAULT 0,
    total_tokens        INTEGER NOT NULL DEFAULT 0,

    input_cost          NUMERIC(18, 8) NOT NULL DEFAULT 0,
    output_cost         NUMERIC(18, 8) NOT NULL DEFAULT 0,
    total_cost          NUMERIC(18, 8) NOT NULL DEFAULT 0,

    status              request_status NOT NULL DEFAULT 'pending',
    status_code         INTEGER,
    error_code          TEXT,
    error_message       TEXT,
    metadata            JSONB NOT NULL DEFAULT '{}',

    cache_hit           BOOLEAN NOT NULL DEFAULT false,
    cache_key_hash      TEXT,

    created_at          TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT now(),
    deleted_at          TIMESTAMPTZ
) PARTITION BY RANGE (created_at);

CREATE INDEX idx_requests_org_created ON requests(org_id, created_at DESC) WHERE deleted_at IS NULL;
CREATE INDEX idx_requests_api_key_created ON requests(api_key_id, created_at DESC) WHERE deleted_at IS NULL;
CREATE INDEX idx_requests_trace_id ON requests(trace_id) WHERE deleted_at IS NULL;
CREATE INDEX idx_requests_status ON requests(status) WHERE deleted_at IS NULL;
CREATE INDEX idx_requests_model ON requests(org_id, model_routed, created_at DESC) WHERE deleted_at IS NULL;
CREATE INDEX idx_requests_cache ON requests(org_id, cache_key_hash) WHERE cache_hit = true AND deleted_at IS NULL;
CREATE INDEX idx_requests_metadata ON requests USING GIN (metadata) WHERE deleted_at IS NULL;

-- Create initial partitions for current and next month
DO $$
DECLARE
    this_month DATE := date_trunc('month', CURRENT_DATE);
    next_month DATE := date_trunc('month', CURRENT_DATE + interval '1 month');
    next2_month DATE := date_trunc('month', CURRENT_DATE + interval '2 months');
    part_name TEXT;
BEGIN
    part_name := 'requests_y' || to_char(this_month, 'YYYY') || 'm' || to_char(this_month, 'MM');
    EXECUTE format('CREATE TABLE IF NOT EXISTS %I PARTITION OF requests FOR VALUES FROM (%L) TO (%L)',
        part_name, this_month, next_month);

    part_name := 'requests_y' || to_char(next_month, 'YYYY') || 'm' || to_char(next_month, 'MM');
    EXECUTE format('CREATE TABLE IF NOT EXISTS %I PARTITION OF requests FOR VALUES FROM (%L) TO (%L)',
        part_name, next_month, next2_month);
END;
$$;


