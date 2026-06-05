-- Create audit_log table (partitioned by created_at range, immutable — no updated_at)

-- Add migration script here

CREATE TYPE audit_action AS ENUM (
    'create', 'update', 'delete', 'login', 'logout',
    'api_key.created', 'api_key.revoked',
    'provider.created', 'provider.updated', 'provider.deleted',
    'quota.exceeded', 'quota.warning',
    'webhook.created', 'webhook.deleted',
    'routing_rule.created', 'routing_rule.updated',
    'settings.updated', 'billing.updated'
);

CREATE TABLE IF NOT EXISTS audit_log (
    id              UUID NOT NULL DEFAULT gen_random_uuid(),
    org_id          UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    user_id         UUID REFERENCES users(id) ON DELETE SET NULL,
    api_key_id      UUID REFERENCES api_keys(id) ON DELETE SET NULL,

    action          audit_action NOT NULL,
    entity_type     TEXT NOT NULL,
    entity_id       TEXT,

    old_values      JSONB,
    new_values      JSONB,
    summary         TEXT NOT NULL,

    ip_address      INET,
    user_agent      TEXT,
    request_id      UUID,

    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    deleted_at      TIMESTAMPTZ
) PARTITION BY RANGE (created_at);

CREATE INDEX IF NOT EXISTS idx_audit_org_created ON audit_log(org_id, created_at DESC) WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_audit_action ON audit_log(org_id, action, created_at DESC) WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_audit_entity ON audit_log(entity_type, entity_id) WHERE deleted_at IS NULL;


