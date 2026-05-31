-- Create responses table (partitioned by created_at range)

-- Add migration script here

CREATE TABLE responses (
    id              UUID NOT NULL DEFAULT gen_random_uuid(),
    org_id          UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    request_id      UUID NOT NULL,

    status_code     INTEGER NOT NULL,
    response_headers JSONB NOT NULL DEFAULT '{}',
    response_body   TEXT,
    response_body_truncated BOOLEAN NOT NULL DEFAULT false,

    prompt_tokens   INTEGER NOT NULL DEFAULT 0,
    completion_tokens INTEGER NOT NULL DEFAULT 0,
    total_tokens    INTEGER NOT NULL DEFAULT 0,

    finish_reason   TEXT,
    model_used      TEXT,
    provider_metadata JSONB NOT NULL DEFAULT '{}',

    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    deleted_at      TIMESTAMPTZ
) PARTITION BY RANGE (created_at);

CREATE INDEX idx_responses_request_id ON responses(request_id) WHERE deleted_at IS NULL;
CREATE INDEX idx_responses_org_created ON responses(org_id, created_at DESC) WHERE deleted_at IS NULL;
CREATE INDEX idx_responses_status ON responses(status_code) WHERE deleted_at IS NULL;

-- Create initial partitions for current and next month
DO $$
DECLARE
    this_month DATE := date_trunc('month', CURRENT_DATE);
    next_month DATE := date_trunc('month', CURRENT_DATE + interval '1 month');
    next2_month DATE := date_trunc('month', CURRENT_DATE + interval '2 months');
    part_name TEXT;
BEGIN
    part_name := 'responses_y' || to_char(this_month, 'YYYY') || 'm' || to_char(this_month, 'MM');
    EXECUTE format('CREATE TABLE IF NOT EXISTS %I PARTITION OF responses FOR VALUES FROM (%L) TO (%L)',
        part_name, this_month, next_month);

    part_name := 'responses_y' || to_char(next_month, 'YYYY') || 'm' || to_char(next_month, 'MM');
    EXECUTE format('CREATE TABLE IF NOT EXISTS %I PARTITION OF responses FOR VALUES FROM (%L) TO (%L)',
        part_name, next_month, next2_month);
END;
$$;


