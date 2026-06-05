-- SSO configuration per organization

CREATE TABLE IF NOT EXISTS sso_configs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    provider_type TEXT NOT NULL CHECK (provider_type IN ('saml', 'oidc')),
    metadata_url TEXT,
    entity_id TEXT,
    certificate TEXT,
    sso_url TEXT,
    client_id TEXT,
    client_secret_enc TEXT,
    idp_issuer TEXT,
    role_attribute TEXT DEFAULT 'role',
    enabled BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (org_id, provider_type)
);

CREATE INDEX IF NOT EXISTS idx_sso_configs_org ON sso_configs (org_id);
CREATE INDEX IF NOT EXISTS idx_sso_configs_enabled ON sso_configs (org_id, enabled);
