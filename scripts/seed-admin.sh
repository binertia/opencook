#!/usr/bin/env bash
# Seed the first admin user into the gateway database.
# Usage: ./scripts/seed-admin.sh [EMAIL] [PASSWORD] [ORG_NAME]
#
# Requires: psql (PostgreSQL client) or docker compose

set -euo pipefail

EMAIL="${1:-admin@example.com}"
PASSWORD="${2:-AdminPass123!}"
ORG_NAME="${3:-Default Org}"

# Pre-computed Argon2id hash for "AdminPass123!" (m=65536,t=3,p=4)
# Generated with: cargo run -p gateway-auth --example hash_password
DEFAULT_HASH='$argon2id$v=19$m=65536,t=3,p=4$noEqSB8UJ7ffQP8qih7p/Q$xTYwSc1Ay/MhQ3x+B3xX/Z/g91JzMdFhBpBf+5zlIrM'

# Allow overriding the hash via env var
PASSWORD_HASH="${ADMIN_PASSWORD_HASH:-$DEFAULT_HASH}"

if command -v psql >/dev/null 2>&1; then
  echo "Seeding admin user via local psql..."
  psql "${DATABASE_URL:-postgres://gateway:gateway_dev_password@localhost:5432/gateway_dev}" -c "
INSERT INTO organizations (id, name, slug, status, settings, billing_email, plan_tier)
VALUES ('00000000-0000-0000-0000-000000000001', '$ORG_NAME', 'default-org', 'active', '{}', NULL, 'free')
ON CONFLICT DO NOTHING;

INSERT INTO users (id, org_id, email, password_hash, display_name, role, status)
VALUES (
  '00000000-0000-0000-0000-000000000002',
  '00000000-0000-0000-0000-000000000001',
  '$EMAIL',
  '$PASSWORD_HASH',
  'Admin User',
  'owner',
  'active'
)
ON CONFLICT DO NOTHING;
"
else
  echo "psql not found. Trying via Docker Compose..."
  docker compose -f docker-compose.dev.yml exec -T postgres psql -U gateway -d gateway_dev -c "
INSERT INTO organizations (id, name, slug, status, settings, billing_email, plan_tier)
VALUES ('00000000-0000-0000-0000-000000000001', '$ORG_NAME', 'default-org', 'active', '{}', NULL, 'free')
ON CONFLICT DO NOTHING;

INSERT INTO users (id, org_id, email, password_hash, display_name, role, status)
VALUES (
  '00000000-0000-0000-0000-000000000002',
  '00000000-0000-0000-0000-000000000001',
  '$EMAIL',
  '$PASSWORD_HASH',
  'Admin User',
  'owner',
  'active'
)
ON CONFLICT DO NOTHING;
"
fi

echo "Admin user seeded: $EMAIL / $PASSWORD"
