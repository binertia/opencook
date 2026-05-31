-- Create webhooks table with webhook_event enum

-- Add migration script here

CREATE TYPE webhook_event AS ENUM (
    'request.completed', 'request.failed',
    'quota.warning', 'quota.exceeded',
    'provider.error', 'provider.recovered'
);

CREATE TABLE webhooks (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id          UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,

    name            TEXT NOT NULL,
    url             TEXT NOT NULL,
    secret_enc      BYTEA,

    events          webhook_event[] NOT NULL DEFAULT '{}',

    custom_headers  JSONB NOT NULL DEFAULT '{}',

    max_retries     INTEGER NOT NULL DEFAULT 3,
    retry_interval_seconds INTEGER NOT NULL DEFAULT 60,
    timeout_seconds INTEGER NOT NULL DEFAULT 30,

    status          TEXT NOT NULL DEFAULT 'active'
                        CHECK (status IN ('active', 'inactive', 'failing')),

    last_delivered_at   TIMESTAMPTZ,
    last_failure_at     TIMESTAMPTZ,
    consecutive_failures INTEGER NOT NULL DEFAULT 0,

    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    deleted_at      TIMESTAMPTZ
);

CREATE INDEX idx_webhooks_org ON webhooks(org_id) WHERE deleted_at IS NULL;
CREATE INDEX idx_webhooks_status ON webhooks(status) WHERE deleted_at IS NULL;
CREATE INDEX idx_webhooks_events ON webhooks USING GIN (events);


