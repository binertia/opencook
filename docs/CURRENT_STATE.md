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
- **Tenant isolation**: Middleware wired to API routes
- **API key validation**: Middleware active on `/v1/*` routes. Validates format, attaches `AuthContext`

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

### Middleware Stack (outer → inner)
CORS → body limit → trace → rate limit → auth → handler
(Quota check moved into orchestrator — needs request body for cost estimation)

### Rate Limiting (TASK-0041 ✅)
- 6 layers: global, org, key requests, key tokens, provider, IP
- Redis Lua scripts for atomic check-and-record
- Fail-open on Redis errors
- Headers: `X-RateLimit-Limit`, `X-RateLimit-Remaining`, `X-RateLimit-Reset`

### Quota / Budget Caps (TASK-0042 ✅)
- Quota engine checks pre-request against `quotas` + `quota_usage` tables
- Supports metrics: requests, tokens, cost_usd; periods: minute, hour, day, month, total
- Actions: block (403), warn (`X-Quota-Warning` header)
- Atomic increment via `UPDATE` (get_or_create uses SELECT + INSERT for NULL handling)

### Request Orchestrator (TASK-0044 ✅)
- `gateway-core/src/orchestrator.rs`: full request lifecycle coordination
- Pre-request: quota check with cost estimation (`max_tokens × pricing`)
- Provider call via closure (avoids circular `gateway-core` ↔ `gateway-providers` dependency)
- Post-request: request record persisted to DB (`requests` table) with actual tokens + cost
- Post-request: quota usage incremented asynchronously (`tokio::spawn`)
- Mock fallback when `OPENAI_API_KEY` unset
- Hardcoded pricing for common models (pending TASK-0024 model registry)

### SSE Streaming (TASK-0028 ✅)
- `POST /v1/chat/completions` with `stream: true` returns `text/event-stream`
- Handler branches: `stream: true` → SSE, otherwise → JSON
- Mock streaming: yields word-by-word chunks + `[DONE]` when no API key
- Real streaming: forwards provider SSE via `LoggingStream` wrapper
- `LoggingStream`: wraps `ReceiverStream`, updates `requests` table on completion/disconnect
- Keep-alive pings every 15s

### Caching — Key Builder & Cacheability (TASK-0036 ✅)
- `gateway-cache/src/key_builder.rs`: deterministic SHA-256 cache key from normalized request
- Includes: model, messages (canonical JSON), temperature, max_tokens, top_p, penalties, stop, seed, tools, response_format
- `gateway-cache/src/cacheable.rs`: cacheability rules
  - `temperature == 0.0` only
  - `stream == false` only
  - Blocks dynamic content: ISO timestamps, UUIDs, template vars (`{current_time}`, `{user_id}`)
- `CacheKey` struct with `redis_key` (`cache:{org_id}:{model}:{hash}`)
- `CachedResponse` struct for stored entries

### Caching — L1 In-Process (TASK-0037 ✅)
- `gateway-cache/src/l1_cache.rs`: `moka::future::Cache` wrapper
- Default: 10K entries, 60s TTL, LRU eviction
- Methods: `get`, `insert`, `invalidate`, `invalidate_all`, `stats`
- Thread-safe via `Arc<AtomicU64>` hit/miss counters
- 17 unit tests covering insert/get, TTL expiry, LRU eviction, concurrency

### Caching — L2 Redis + Two-Tier (TASK-0038 ✅)
- `gateway-cache/src/l2_cache.rs`: Redis-backed cache
  - `get`: Redis GET + JSON deserialize
  - `insert`: Redis SETEX + JSON serialize
  - `invalidate`: Redis DEL
  - `invalidate_pattern`: Redis SCAN + DEL
  - Errors are non-fatal (logged, request continues)
- `gateway-cache/src/two_tier.rs`: unified L1 + L2 cache
  - `get`: L1 → L2 → None (L2 hit promotes to L1 asynchronously)
  - `insert`: L2 first, then L1
  - `invalidate`: both tiers
  - `invalidate_pattern`: L2 SCAN+DEL + L1 invalidate_all

### Caching — Orchestrator Integration (TASK-0039 ✅)
- Cache check in `gateway-api/src/routes/chat.rs` **before** provider call
- Cacheable: `temperature == 0.0` and `stream != true`
- Cache hit: returns cached `ChatCompletionResponse` with `X-Cache: HIT` header
- Cache hit: logs zero-cost request to DB (`cache_hit=true`)
- Cache miss: proceeds to orchestrator, stores response in cache after success
- Streaming requests bypass cache
- Cache errors are non-fatal (logged, request continues)

### Observability
- `tracing` structured logging active
- Request logging: persisted to `requests` table via `RequestRepo`

## Phase 2: Core Gateway — In Progress

### Next Tasks
- TASK-0022: Anthropic, Gemini, Ollama provider adapters
- TASK-0031–0033: Routing engine
- TASK-0045: Quota and Budget Admin API

### Providers (TASK-0022 ✅)
- OpenAI adapter ✅ — full implementation with streaming
- Anthropic adapter ✅ — request/response transform, streaming, health check
- Gemini adapter ✅ — request/response transform, streaming, health check
- Ollama adapter ✅ — request/response transform, streaming, embeddings, health check

## Known Gaps
1. ~~Auth middleware not wired to `/v1/*` routes~~ ✅
2. ~~Request logging not persisted to DB~~ ✅
3. ~~Rate limiter not implemented~~ ✅
4. ~~Quota engine not implemented~~ ✅
5. ~~Tenant isolation middleware not applied to routes~~ ✅
6. ~~SSE streaming not yet exposed via HTTP endpoint~~ ✅
7. ~~Cache key builder / cacheability rules~~ ✅
8. ~~L1 in-process cache (moka)~~ ✅
9. ~~L2 Redis cache + two-tier integration~~ ✅
10. ~~Cache wired into request handler~~ ✅
11. ~~Anthropic adapter~~ ✅
12. ~~Gemini adapter~~ ✅
13. ~~Ollama adapter~~ ✅
