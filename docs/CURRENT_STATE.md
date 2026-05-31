# Current Implementation State

> **One-page snapshot of what's actually built right now.**
> Update this after every completed task. Keep it under 50 lines.
> Last updated: 2026-05-31

## Phase 1: Foundation — 16/16 tasks done ✅

### Workspace & Infrastructure
- 8 Rust crates: `gateway-api`, `gateway-core`, `gateway-providers`, `gateway-cache`, `gateway-quota`, `gateway-auth`, `gateway-db`, `gateway-observability`
- Docker Compose dev env: postgres (5432), redis (6379), backend, frontend
- Database: `gateway_dev` with all 22 migrations applied
- sqlx 0.8 (upgraded from 0.7 to fix Rust future-incompatibility warning)

### Database
- Full schema: orgs, users, api_keys, provider_configs, provider_models, routing_rules, requests/responses/usage_records (partitioned), quotas/quota_usage, webhooks, audit_log, sessions
- RLS policies active. Connection pool sets `app.org_id` via `after_connect`
- Partitioned tables have no parent-level PK (Postgres requirement)

### Auth
- **Passwords**: Argon2id + zxcvbn strength check
- **JWT**: RS256, access 15min, refresh 7 days. Tests use dynamic RSA keys
- **API keys**: `sk_gw_{32b58}{6b58_checksum}` = 44 chars. SHA-256 stored. Prefix = first 8 chars
- **RBAC**: 4 roles (owner/admin/member/viewer), 31 permissions
- **Tenant isolation**: Middleware exists, `org_id` extractor exists. NOT yet wired to API routes
- **API key validation**: Stub exists (`AuthContext` model). NOT yet applied as middleware to routes

### Providers
- `Provider` trait in `gateway-providers` with canonical OpenAI types in `gateway-core`
- OpenAI adapter: chat_completion, chat_completion_stream (SSE), embeddings, health_check
- Circular dependency resolved: `gateway-providers` → `gateway-core` for types

### API Server
- Axum on port 8080 with CORS, body limit (10MB), trace layer
- `GET /health` → `{"status":"ok"}`
- `GET /ready` → `{"status":"ready"}` (checks DB)
- `GET /v1/models` → static model list with gateway metadata
- `POST /v1/chat/completions` → mock response if no OPENAI_API_KEY; real provider call if set
- Response includes `gateway` metadata field: `{ provider, latency_ms, cache_hit }`

### Observability
- `tracing` structured logging active
- Request logging: console-only stub. NOT persisted to DB yet

## Phase 2: Core Gateway — Starting

### Next Task
- TASK-0041: Rate limiter (Redis Lua, sliding window)

## Known Gaps (Block Phase 1 Completion)
1. Auth middleware not wired to `/v1/*` routes
2. Request logging not persisted to DB
3. Rate limiter not implemented
4. Quota engine not implemented
5. Tenant isolation middleware not applied to routes
