-- SCIM token storage for per-org SCIM API access

CREATE TABLE IF NOT EXISTS scim_tokens (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    token_hash TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expires_at TIMESTAMPTZ,
    UNIQUE (org_id)
);

CREATE INDEX IF NOT EXISTS idx_scim_tokens_hash ON scim_tokens (token_hash);
CREATE INDEX IF NOT EXISTS idx_scim_tokens_org ON scim_tokens (org_id);
