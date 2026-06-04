# Current Implementation State

> **One-page snapshot of what's actually built right now.**
> Update this after every completed task. Keep it under 50 lines.
> Last updated: 2026-06-04

## Phase 1–3: Foundation + Core Gateway + Admin APIs — COMPLETED ✅

### Workspace & Infrastructure
- 9 Rust crates: `gateway-api`, `gateway-core`, `gateway-providers`, `gateway-cache`, `gateway-quota`, `gateway-auth`, `gateway-db`, `gateway-observability`, **`gateway-solo`**
- Docker Compose dev env: postgres (5432), redis (6379), backend, frontend
- Database: PostgreSQL 16 for TEAM mode; **SQLite for SOLO mode** (auto-creates DB file)
- sqlx 0.8.6 with `libsqlite3-sys/bundled`

### Dual-Database Architecture (NEW)
- `DbBackend` enum: `Postgres(PgPool)` | `Sqlite(SqlitePool)`
- `gateway-db/src/types.rs`: `DbDecimal` (wraps `Decimal` for cross-db) + `JsonVec<T>` (wraps `Vec<T>` for cross-db)
- All repos dispatch via `match &self.pool`: `request_repo`, `routing_repo`, `quota_repo`, `quota_usage_repo`, `usage_repo`, `provider_config_repo`, `model_registry`
- SQLite schema: `TEXT` for enums/decimals/JSON, `BLOB` for UUIDs, `INTEGER` for booleans
- Zero `.pg()` calls remain outside `pool.rs`

### Auth
- **Passwords**: Argon2id + zxcvbn strength check
- **JWT**: RS256, access 15min, refresh 7 days
- **API keys**: `sk_gw_{32b58}{6b58_checksum}` = 44 chars. SHA-256 stored
- **RBAC**: 4 roles (owner/admin/member/viewer), 31 permissions
- **Tenant isolation**: Middleware wired to API routes
- **SOLO mode**: No auth required; uses default org `00000000-0000-0000-0000-000000000000`

### Providers
- OpenAI, Anthropic, Gemini, Ollama — all implement `Provider` trait
- Chat completions, streaming (SSE), embeddings, health checks

### API Server (TEAM mode — `gateway-api`)
- Axum on port 8080 with CORS, body limit (10MB), trace layer
- `POST /v1/chat/completions` → mock or real provider call
- `GET /v1/models` → dynamic DB query with static fallback
- Admin APIs: quotas CRUD, usage/cost analytics
- SSE streaming with `LoggingStream` wrapper for DB persistence

### API Server (SOLO mode — `gateway-solo`) (NEW)
- SQLite-backed, no auth, no Redis dependency
- `POST /v1/chat/completions`, `GET /v1/models`, `/health`, `/ready`, `/metrics`
- Quota management: `GET/POST/PUT/DELETE /api/v1/quotas` (user-configurable limits)
- Usage analytics: `GET /api/v1/usage`, `GET /api/v1/costs`
- Routing profiles: `privacy-first`, `balanced`, `speed`, `frugal`, `quality`, `offline`
- Config wizard: `gateway-solo config` → interactive profile selection → writes `gateway-solo.toml`

### Middleware Stack (outer → inner)
CORS → body limit → trace → rate limit → auth → handler

### Rate Limiting
- 6 layers: global, org, key requests, key tokens, provider, IP
- Redis Lua scripts; fail-open on Redis errors

### Quota / Budget Caps
- Pre-request cost estimate against `quotas` + `quota_usage`
- Metrics: requests, tokens, cost_usd; periods: minute, hour, day, month, total
- Actions: block (403), warn (header)

### Routing Engine
- `RoutingRepo` CRUD with priority ordering, wildcard model matching
- Strategies: `single`, `fallback`, `weighted`, `conditional`
- `provider_kind` in `Target` JSON for direct provider mapping
- Fallback chain with circuit breaker per provider

### Caching
- L1: moka (10K entries, 60s TTL)
- L2: Redis (GET/SETEX/SCAN+DEL)
- Cacheable: `temperature == 0.0` and `stream != true`
- `X-Cache: HIT/MISS` header

### Circuit Breaker + Retry
- 3-state breaker per provider (Closed/Open/HalfOpen)
- Retry: exponential backoff + jitter (max 2 retries)
- Health worker polls providers every 30s

### Observability
- `tracing` structured JSON logging
- Prometheus `/metrics` endpoint
- Request logs persisted to `requests` table

## Phase 4: Dashboard & Polish — IN PROGRESS

- TASK-0046: React dashboard scaffolded (Vite + TypeScript + Tailwind + shadcn/ui)
- TASK-0047: API client (ky), auth hooks, Zustand store, login page with Zod validation
- TASK-0048: Dashboard layout with responsive sidebar, header, dark mode toggle, route protection
- TASK-0049: Organization settings page with form validation, routing strategy selector, multi-select for providers/models
- TASK-0050: User management page with member table, role changes, remove confirmation, invite modal
- TASK-0051: Dashboard overview with 4 KPI cards, time range selector, recent requests table, active providers, quick actions
- TASK-0052: Provider list page with health status, latency, error rate, sorting, filtering, manual health checks
- TASK-0053: Add/Edit Provider Wizard with 6 steps, test connection, API key show/hide
- Tasks TASK-0054 through TASK-0100 remain unstarted.

## Known Gaps
1. Provider Config Encryption (TASK-0023) — AES-256-GCM for `api_key_enc`; currently env vars
2. Request Cancellation (TASK-0066) — Abort in-flight on client disconnect
3. Frontend dashboard (TASK-0046+) — React admin SPA
