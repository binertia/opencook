-- Create routing_rules table with routing_strategy enum

-- Add migration script here

CREATE TYPE routing_strategy AS ENUM (
    'fallback', 'weighted', 'conditional', 'single'
);

CREATE TABLE IF NOT EXISTS routing_rules (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id          UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    name            TEXT NOT NULL,
    description     TEXT,
    strategy        routing_strategy NOT NULL DEFAULT 'single',
    priority        INTEGER NOT NULL DEFAULT 0,
    match_model     TEXT,
    match_tags      TEXT[] NOT NULL DEFAULT '{}',
    conditions      JSONB NOT NULL DEFAULT '{}',
    targets         JSONB NOT NULL DEFAULT '[]',
    timeout_ms      INTEGER NOT NULL DEFAULT 30000,
    retries         INTEGER NOT NULL DEFAULT 1,
    status          TEXT NOT NULL DEFAULT 'active'
                        CHECK (status IN ('active', 'inactive', 'draft')),
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    deleted_at      TIMESTAMPTZ
);

CREATE UNIQUE INDEX IF NOT EXISTS uk_routing_rules_org_name ON routing_rules(org_id, name) WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_routing_rules_org_priority ON routing_rules(org_id, priority) WHERE deleted_at IS NULL AND status = 'active';
CREATE INDEX IF NOT EXISTS idx_routing_rules_match_model ON routing_rules(org_id, match_model) WHERE deleted_at IS NULL AND status = 'active';
CREATE INDEX IF NOT EXISTS idx_routing_rules_conditions ON routing_rules USING GIN (conditions) WHERE deleted_at IS NULL;


