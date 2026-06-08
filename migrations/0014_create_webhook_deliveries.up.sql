-- Create webhook_deliveries table (partitioned by created_at range)

-- Add migration script here

CREATE TABLE IF NOT EXISTS webhook_deliveries (
    id              UUID NOT NULL DEFAULT gen_random_uuid(),
    org_id          UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    webhook_id      UUID NOT NULL REFERENCES webhooks(id) ON DELETE CASCADE,

    event_type      webhook_event NOT NULL,
    payload         JSONB NOT NULL,

    attempt_number  INTEGER NOT NULL DEFAULT 1,

    request_headers JSONB NOT NULL DEFAULT '{}',
    request_body    TEXT,
    response_status INTEGER,
    response_body   TEXT,
    response_headers JSONB NOT NULL DEFAULT '{}',

    status          TEXT NOT NULL
                        CHECK (status IN ('pending', 'delivered', 'failed', 'expired')),
    error_message   TEXT,

    scheduled_at    TIMESTAMPTZ NOT NULL,
    started_at      TIMESTAMPTZ,
    completed_at    TIMESTAMPTZ,

    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    deleted_at      TIMESTAMPTZ
) PARTITION BY RANGE (created_at);

CREATE INDEX IF NOT EXISTS idx_webhook_deliveries_webhook ON webhook_deliveries(webhook_id, created_at DESC) WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_webhook_deliveries_status ON webhook_deliveries(status) WHERE deleted_at IS NULL AND status IN ('pending', 'failed');
CREATE INDEX IF NOT EXISTS idx_webhook_deliveries_scheduled ON webhook_deliveries(scheduled_at) WHERE status = 'pending' AND deleted_at IS NULL;

-- Create initial partitions for current and next month
DO $$
DECLARE
    this_month DATE := date_trunc('month', CURRENT_DATE);
    next_month DATE := date_trunc('month', CURRENT_DATE + interval '1 month');
    next2_month DATE := date_trunc('month', CURRENT_DATE + interval '2 months');
    part_name TEXT;
BEGIN
    part_name := 'webhook_deliveries_y' || to_char(this_month, 'YYYY') || 'm' || to_char(this_month, 'MM');
    EXECUTE format('CREATE TABLE IF NOT EXISTS %I PARTITION OF webhook_deliveries FOR VALUES FROM (%L) TO (%L)',
        part_name, this_month, next_month);

    part_name := 'webhook_deliveries_y' || to_char(next_month, 'YYYY') || 'm' || to_char(next_month, 'MM');
    EXECUTE format('CREATE TABLE IF NOT EXISTS %I PARTITION OF webhook_deliveries FOR VALUES FROM (%L) TO (%L)',
        part_name, next_month, next2_month);
END;
$$;


