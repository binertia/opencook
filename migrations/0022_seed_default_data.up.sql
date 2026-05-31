-- Seed default data

-- Add migration script here

-- Seed system organization for internal use
INSERT INTO organizations (id, name, slug, plan_tier, status)
VALUES (
    '00000000-0000-0000-0000-000000000001',
    'System',
    'system',
    'enterprise',
    'active'
)
ON CONFLICT DO NOTHING;


