-- Junction table for multi-organization user memberships

CREATE TABLE IF NOT EXISTS user_organizations (
    user_id         UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    org_id          UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    role            TEXT NOT NULL DEFAULT 'member'
                        CHECK (role IN ('owner', 'admin', 'member', 'viewer')),
    joined_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
    created_by      UUID REFERENCES users(id) ON DELETE SET NULL,

    PRIMARY KEY (user_id, org_id)
);

CREATE INDEX IF NOT EXISTS idx_user_organizations_user_id ON user_organizations(user_id);
CREATE INDEX IF NOT EXISTS idx_user_organizations_org_id ON user_organizations(org_id);

-- Migrate existing single-org users into the junction table.
-- Each existing user gets their current org_id as a membership with their current role.
INSERT INTO user_organizations (user_id, org_id, role, joined_at, created_by)
SELECT id, org_id, role, created_at, NULL
FROM users
WHERE deleted_at IS NULL
ON CONFLICT (user_id, org_id) DO NOTHING;
