# AI Gateway — Epic Breakdown

> **Document Version:** 1.0
> **Last Updated:** 2025-01-15
> **Total Epics:** 20
> **Dependency Status:** Valid DAG — no cycles detected

---

## Epic Index

| Epic | Title | Priority | Phase | Effort |
|------|-------|----------|-------|--------|
| [Epic-01](#epic-01-project-bootstrap) | Project Bootstrap | P0 | Phase 1 | 1 week |
| [Epic-02](#epic-02-database-foundation) | Database Foundation | P0 | Phase 1 | 1 week |
| [Epic-03](#epic-03-authentication) | Authentication & Authorization | P0 | Phase 1 | 1.5 weeks |
| [Epic-04](#epic-04-provider-abstraction) | Provider Abstraction Layer | P0 | Phase 1 | 1 week |
| [Epic-05](#epic-05-request-proxy) | Request Proxy & OpenAI-Compatible API | P0 | Phase 1 | 1 week |
| [Epic-06](#epic-06-routing-engine) | Routing Engine | P0 | Phase 2 | 1 week |
| [Epic-07](#epic-07-caching) | Caching Layer (Exact Match) | P0 | Phase 2 | 1 week |
| [Epic-08](#epic-08-quota--billing) | Quota, Rate Limiting & Cost Tracking | P0 | Phase 2 | 1.5 weeks |
| [Epic-09](#epic-09-admin-dashboard) | Admin Dashboard — Foundation | P0 | Phase 3 | 2 weeks |
| [Epic-10](#epic-10-provider-management-ui) | Provider Management UI | P0 | Phase 3 | 1 week |
| [Epic-11](#epic-11-api-key-management-ui) | API Key Management UI | P0 | Phase 3 | 1 week |
| [Epic-12](#epic-12-usage-analytics) | Usage Analytics & Cost Dashboards | P0 | Phase 3 | 1.5 weeks |
| [Epic-13](#epic-13-fallback--reliability) | Fallback, Health Checks & Circuit Breaker | P0 | Phase 2 | 1 week |
| [Epic-14](#epic-14-semantic-caching) | Semantic Caching | P1 | Phase 4 | 2 weeks |
| [Epic-15](#epic-15-webhooks--events) | Webhooks & Event System | P1 | Phase 4 | 1.5 weeks |
| [Epic-16](#epic-16-observability) | Observability — Metrics, Logging & Health | P0 | Phase 1-2 | 1 week |
| [Epic-17](#epic-17-security-hardening) | Security Hardening | P1 | Phase 2-3 | 1 week |
| [Epic-18](#epic-18-deployment--operations) | Deployment & Operations | P0 | Phase 3 | 1 week |
| [Epic-19](#epic-19-smart-routing) | Smart Cost-Aware Routing | P1 | Phase 4 | 1.5 weeks |
| [Epic-20](#epic-20-team-collaboration) | Team Collaboration & SSO | P1 | Phase 4 | 2 weeks |

---

## Epic-01: Project Bootstrap

**Goal:** Establish a working development environment and CI/CD pipeline that any developer can clone and run in minutes.
**Priority:** P0
**Phase:** Phase 1
**Estimated Effort:** 1 week

### Description
Create the monorepo structure with a Rust workspace for the gateway backend and a React/TypeScript project for the admin dashboard. Set up Docker Compose with all required services (PostgreSQL, Redis, gateway, dashboard), implement a CI/CD pipeline with automated testing, linting, and security auditing. Establish coding standards and development tooling.

### Acceptance Criteria
- [ ] `git clone && docker-compose up -d` starts PostgreSQL, Redis, gateway API, and dashboard without errors
- [ ] `cargo test` passes with zero failures in CI
- [ ] `cargo clippy` runs with zero warnings in CI
- [ ] `cargo audit` runs with zero critical advisories in CI
- [ ] Frontend `npm run build` succeeds in CI
- [ ] Hot reload works for both backend (`cargo watch`) and frontend (`npm run dev`)
- [ ] README.md includes setup instructions, architecture overview, and contribution guidelines
- [ ] GitHub issue templates (bug report, feature request) are configured

### Dependencies
None — this is the starting point.

### Technical Scope
- **Files/modules affected:** `/Cargo.toml` (workspace root), `/docker-compose.yml`, `/.github/workflows/`, `/gateway-api/`, `/gateway-web/`, `/migrations/`, `README.md`
- **Key architectural components:** Rust workspace layout, Docker multi-service orchestration, GitHub Actions CI
- **Database changes required:** None (Epic-02 adds migrations)

### Risks
- **Risk:** Docker networking issues between gateway and database services on developer machines. **Mitigation:** Use Docker Compose service discovery; test on Linux, macOS, and WSL2.
- **Risk:** Rust compile times slow developer iteration. **Mitigation:** Use `sccache` in CI and recommend it locally; split crates to minimize recompilation.

---

## Epic-02: Database Foundation

**Goal:** All database schemas, migrations, and access patterns are in place with tenant isolation enforced.
**Priority:** P0
**Phase:** Phase 1
**Estimated Effort:** 1 week

### Description
Implement the complete PostgreSQL schema using a migration framework (sqlx/refinery). Create all 16+ tables including organizations, users, api_keys, provider_configs, provider_models, routing_rules, requests, responses, usage_records, quotas, quota_usage, webhooks, webhook_deliveries, cache_metadata, audit_log, and sessions. Set up row-level security policies, indexes optimized for hot-path queries, connection pooling, and a base repository trait that enforces tenant isolation on every query.

### Acceptance Criteria
- [ ] All 22 migration files run successfully in order (0001-0022) with `sqlx migrate run`
- [ ] Row-level security policies are active on all tenant-scoped tables
- [ ] Connection pool (deadpool/sqlx) is configured with min 5 / max 20 connections
- [ ] Every repository query includes `WHERE org_id = $1` as first filter
- [ ] Index creation scripts execute without error; `EXPLAIN ANALYZE` on hot-path queries shows index usage
- [ ] Soft-delete pattern (`deleted_at IS NULL`) is enforced in all queries via repository layer
- [ ] `updated_at` trigger function applied to all mutable tables
- [ ] Initial monthly partitions created for requests, responses, and webhook_deliveries

### Dependencies
- Epic-01 (Project Bootstrap — repository and CI must exist)

### Technical Scope
- **Files/modules affected:** `/migrations/*.sql`, `/gateway-db/src/` (repository layer, connection pool, RLS helpers)
- **Key architectural components:** Tenant-isolated repository pattern, migration framework, PostgreSQL partitioning
- **Database changes required:** All tables, indexes, triggers, RLS policies, initial partitions

### Risks
- **Risk:** Migration ordering errors or conflicting DDL. **Mitigation:** Each migration in its own transaction; test migrations against a fresh database in CI.
- **Risk:** RLS policy performance overhead on high-write tables. **Mitigation:** Application-level `WHERE org_id` filtering is primary; RLS is defense-in-depth.
- **Risk:** Partition management complexity (creating new monthly partitions). **Mitigation:** Background worker auto-creates partitions 1 month ahead.

---

## Epic-03: Authentication & Authorization

**Goal:** Two independent authentication systems (API keys for consumers, sessions for admins) with RBAC are fully operational.
**Priority:** P0
**Phase:** Phase 1
**Estimated Effort:** 1.5 weeks

### Description
Implement System A (API key authentication) for LLM API consumers and System B (JWT session authentication) for dashboard users. API keys use `sk-gw-{base58}` format with SHA-256 hashing for storage — only the hash is persisted, the full key is shown once on creation. Session auth uses RS256-signed JWTs with 15-minute access tokens and 7-day refresh tokens stored in httpOnly cookies. RBAC provides four roles (owner, admin, member, viewer) with a 30+ permission matrix. Include password hashing with Argon2id, account lockout after 5 failed attempts, and password reset via secure token.

### Acceptance Criteria
- [ ] API key format validation accepts valid `sk-gw-*` keys and rejects malformed keys
- [ ] Authenticated request with valid API key returns 200; invalid key returns 401
- [ ] API key lookup completes in < 1ms p99 (Redis cached) or < 5ms (DB miss)
- [ ] Key revocation propagates within 100ms (Redis cache invalidation + pub/sub event)
- [ ] JWT access token expires after 15 minutes; refresh token rotates on each use
- [ ] Login with valid credentials returns httpOnly cookie; invalid credentials return 401
- [ ] Account locks after 5 consecutive failed login attempts for 30 minutes
- [ ] RBAC middleware returns 403 for permission violations on admin endpoints
- [ ] Password reset flow: token generated, email sent (configurable SMTP), password updated, all sessions revoked
- [ ] Tenant isolation enforced: cross-organization access attempts are logged and rejected with 403

### Dependencies
- Epic-01 (Project Bootstrap)
- Epic-02 (Database Foundation — users, api_keys, sessions tables)

### Technical Scope
- **Files/modules affected:** `/gateway-auth/src/` (api_key validator, session manager, RBAC engine, password hasher), `/gateway-api/src/middleware/` (auth middleware layer)
- **Key architectural components:** Dual auth system, JWT RS256 signing, Argon2id password hashing, Redis-backed session revocation list
- **Database changes required:** `api_keys`, `users`, `organization_members`, `sessions`, `refresh_tokens` tables

### Risks
- **Risk:** JWT key rotation without downtime. **Mitigation:** Support multiple public keys; validate against all active keys during rotation window.
- **Risk:** Timing attacks on API key validation. **Mitigation:** Constant-time hash comparison; always perform dummy hash lookup on miss.
- **Risk:** Session token theft via XSS. **Mitigation:** httpOnly cookies (no JavaScript access); SameSite=Strict CSRF protection.

---

## Epic-04: Provider Abstraction Layer

**Goal:** A trait-based provider system that supports multiple LLM backends with unified configuration and model management.
**Priority:** P0
**Phase:** Phase 1
**Estimated Effort:** 1 week

### Description
Define the `Provider` Rust trait that all LLM backends must implement: `chat_completion`, `chat_completion_stream`, `embeddings`, and `health_check`. Implement the OpenAI adapter as the reference implementation with full request/response transformation to the canonical internal format. Build the model registry with per-model pricing, capabilities (streaming, tools, vision), and aliases. Store provider configurations in PostgreSQL with AES-256-GCM encrypted API keys. Health checks probe each provider periodically and store status in Redis.

### Acceptance Criteria
- [ ] `Provider` trait is defined with all required methods; a new adapter can be added by implementing the trait
- [ ] OpenAI adapter transforms OpenAI-format requests (pass-through) and responses correctly
- [ ] Provider config stored in PostgreSQL with encrypted `api_key_enc` (BYTEA, AES-256-GCM)
- [ ] Model registry supports pricing fields (`input_cost_per_1k`, `output_cost_per_1k`), capabilities, aliases
- [ ] Health check performs HTTP probe to provider; stores `healthy`/`degraded`/`unhealthy` in Redis
- [ ] Provider status readable by routing engine within 30 seconds of health change
- [ ] Configuration cache in Redis with 5-minute TTL; invalidated on config change
- [ ] Factory function `create_provider(config: ProviderConfig) -> Box<dyn Provider>` works for all provider kinds

### Dependencies
- Epic-01 (Project Bootstrap)
- Epic-02 (Database Foundation — provider_configs, provider_models tables)

### Technical Scope
- **Files/modules affected:** `/gateway-providers/src/` (provider trait, OpenAI adapter, factory), `/gateway-db/src/provider_repo.rs`
- **Key architectural components:** Provider trait definition, adapter pattern, encrypted config storage, health check framework
- **Database changes required:** `provider_configs`, `provider_models` tables with indexes

### Risks
- **Risk:** OpenAI API spec edge cases (function calling, response_format, etc.). **Mitigation:** Comprehensive test fixtures covering all request shapes; validate against official SDK test vectors.
- **Risk:** Encryption key management (AES key for provider API keys). **Mitigation:** Derive from `GATEWAY_MASTER_KEY` environment variable; fail closed if key not set.
- **Risk:** Health check false positives (temporary blips mark provider unhealthy). **Mitigation:** Require 3 consecutive failures before marking unhealthy; configurable thresholds per provider.

---

## Epic-05: Request Proxy & OpenAI-Compatible API

**Goal:** The gateway accepts OpenAI-compatible requests and proxies them to configured providers, returning standardized responses with cost metadata.
**Priority:** P0
**Phase:** Phase 1
**Estimated Effort:** 1 week

### Description
Build the core HTTP server using Axum with middleware composition (auth, rate limiting, request logging). Implement `POST /v1/chat/completions` as the primary endpoint with full OpenAI request/response compatibility. Handle request parsing, authentication via API key, single-provider proxying (OpenAI first), response transformation back to OpenAI format, and request logging to PostgreSQL. Add the `/v1/models` endpoint listing all configured models and `/health` + `/ready` health check endpoints.

### Acceptance Criteria
- [ ] `POST /v1/chat/completions` accepts OpenAI-compatible JSON and returns OpenAI-compatible response
- [ ] Drop-in replacement validated: changing base URL in OpenAI Python SDK works without code changes
- [ ] Response includes gateway metadata headers: `X-Gateway-Request-ID`, `X-Gateway-Version`
- [ ] Request logged to PostgreSQL `requests` table with full metadata (tokens, cost, latency, provider)
- [ ] Error responses follow gateway error envelope format: `{ error: { code, message, type, param, status, request_id } }`
- [ ] `GET /v1/models` returns list of all configured models with pricing and capabilities
- [ ] `GET /health` returns 200 when all dependencies (DB, Redis) are reachable
- [ ] `GET /ready` returns 200 only when gateway has loaded all configs and providers are initialized
- [ ] Request body size limited to 10MB; oversized requests return 413

### Dependencies
- Epic-01 (Project Bootstrap)
- Epic-02 (Database Foundation — request logging tables)
- Epic-03 (Authentication — API key validation middleware)
- Epic-04 (Provider Abstraction — OpenAI adapter for proxying)

### Technical Scope
- **Files/modules affected:** `/gateway-api/src/` (router, handlers, middleware), `/gateway-core/src/` (request orchestrator, canonical types)
- **Key architectural components:** Axum router, middleware stack (auth → rate limit → logger → handler), request/response transformation, SSE streaming
- **Database changes required:** `requests` table for logging

### Risks
- **Risk:** Streaming SSE implementation has chunk delivery issues. **Mitigation:** Use Axum's SSE type; propagate client disconnect via cancellation tokens; test with real-time chat UIs.
- **Risk:** Request logging is a bottleneck (synchronous write blocks response). **Mitigation:** Fire-and-forget async logging; use a bounded channel with drop-on-backpressure.
- **Risk:** OpenAI compatibility gaps (tool calling, JSON mode, etc.). **Mitigation:** Maintain compatibility test suite against official SDK examples.

---

## Epic-06: Routing Engine

**Goal:** Intelligent rule-based routing that selects the best provider for each request based on model, health, cost, and configured rules.
**Priority:** P0
**Phase:** Phase 2
**Estimated Effort:** 1 week

### Description
Implement the routing engine that evaluates priority-ordered routing rules to select a provider for each request. Rules match on model name, request type, and custom JSON conditions. Strategies include: `single` (one provider), `fallback` (primary with backup chain), `weighted` (load distribution by weight), and `conditional` (rules-based with JSON conditions). The engine checks provider health before selection and constructs a fallback chain for retry. Rules are cached in Redis with event-driven invalidation.

### Acceptance Criteria
- [ ] Routing rules evaluated in priority order (lower number = higher priority)
- [ ] Model name matches against `match_model` field or wildcard (NULL)
- [ ] Strategy `single` routes to one provider; `fallback` tries chain on failure
- [ ] Health-unaware rules rejected when `require_health_check` is true and provider is unhealthy
- [ ] Routing decision logged to `requests.routing_rule_id` for traceability
- [ ] Configurable `timeout_ms` and `retries` per rule
- [ ] Rule changes propagate to gateway within 60 seconds (cache invalidation)
- [ ] No provider available for requested model returns 503 with `X-Unavailable-Reason: no_healthy_provider`

### Dependencies
- Epic-02 (Database Foundation — routing_rules table)
- Epic-04 (Provider Abstraction — provider health status, model registry)
- Epic-05 (Request Proxy — integration point for routing)

### Technical Scope
- **Files/modules affected:** `/gateway-core/src/router.rs`, `/gateway-db/src/routing_repo.rs`
- **Key architectural components:** Rule evaluation engine, strategy pattern, health-aware selection, fallback chain construction
- **Database changes required:** `routing_rules` table with JSONB `conditions` and `targets` columns

### Risks
- **Risk:** Complex JSON conditions create performance bottleneck. **Mitigation:** Cache compiled conditions; benchmark rule evaluation (< 1ms per rule).
- **Risk:** Routing rule conflicts (multiple rules match). **Mitigation:** Priority ordering; first match wins; log which rule was selected.
- **Risk:** Health status staleness leads to routing to unhealthy providers. **Mitigation:** Health check interval is 30 seconds; emergency provider disable API for immediate override.

---

## Epic-07: Caching Layer (Exact Match)

**Goal:** Two-tier exact-match caching that reduces AI provider costs by serving repeated identical requests from cache.
**Priority:** P0
**Phase:** Phase 2
**Estimated Effort:** 1 week

### Description
Implement a two-tier cache system: L1 is an in-process cache using the `moka` crate (LRU + TTL, 10K entries), and L2 is a Redis-backed cache shared across potential future instances. Cache keys are SHA-256 hashes of normalized request content (model + messages + parameters). The cache only stores responses for deterministic requests (temperature = 0, no tools, no streaming). Cache lookups follow L1 → L2 → provider fallback, with L2 hits promoted to L1. Configurable TTL per model (default 1 hour for GPT-4o, 24 hours for embeddings). PII detection skips caching for requests containing sensitive patterns.

### Acceptance Criteria
- [ ] Identical request (same model, messages, temperature=0) returns cached response on second call
- [ ] L1 cache lookup p99 latency < 0.1ms; L2 cache lookup p99 < 5ms
- [ ] Cache hit returns `X-Cache: HIT` header; cache miss returns `X-Cache: MISS`
- [ ] L2 hit promotes entry to L1 (subsequent requests served from in-process cache)
- [ ] Temperature > 0.1 requests bypass cache (read and write)
- [ ] Streaming requests bypass cache by default (unless `X-Cache-Stream: true`)
- [ ] PII-containing requests (SSN, email, credit card patterns) skip cache
- [ ] Cache entries have configurable TTL per model; expired entries not served
- [ ] Tenant isolation: cache key includes org_id prefix; cross-tenant cache poisoning impossible
- [ ] Cache metrics emitted: hit rate by layer, by model; miss rate; cost saved

### Dependencies
- Epic-01 (Project Bootstrap — Redis and moka available)
- Epic-05 (Request Proxy — integration point for cache check/store)

### Technical Scope
- **Files/modules affected:** `/gateway-cache/src/` (Cache trait, ExactCache L1/L2 implementation, cache key builder, PII detector), `/gateway-core/src/` (cache integration in request lifecycle)
- **Key architectural components:** Two-tier cache (moka L1 + Redis L2), SHA-256 cache key generation, request normalization, TTL management
- **Database changes required:** `cache_metadata` table for analytics

### Risks
- **Risk:** Cache key collisions (different requests, same hash). **Mitigation:** SHA-256 has negligible collision probability; key includes full request content hash.
- **Risk:** Cache pollution from dynamic content (timestamps, UUIDs in prompts). **Mitigation:** Regex-based dynamic content detection skips cache for likely non-cacheable requests.
- **Risk:** Redis memory exhaustion on cache growth. **Mitigation:** Redis `maxmemory-policy allkeys-lru`; per-model TTL defaults; cache size monitoring.

---

## Epic-08: Quota, Rate Limiting & Cost Tracking

**Goal:** Comprehensive usage control with per-key rate limits, per-organization budget quotas, hard budget caps, and accurate cost tracking for every request.
**Priority:** P0
**Phase:** Phase 2
**Estimated Effort:** 1.5 weeks

### Description
Implement Redis-backed sliding window rate limiting with atomic Lua scripts for per-key limits (requests/minute, tokens/minute). Build the quota system with flexible configuration: metrics (requests, tokens, cost_usd), periods (minute, hour, day, month), scopes (all, api_key, model, provider), and actions (block, warn, throttle). Hard budget caps reject requests when the limit is reached. Cost tracking calculates per-request cost from model pricing tables and aggregates into `usage_records` with hourly/daily/monthly periods. The `quota_usage` table tracks real-time consumption with atomic upsert operations.

### Acceptance Criteria
- [ ] Rate limit exceeded returns 429 with `Retry-After` header; p99 check latency < 5ms
- [ ] Sliding window rate limiter uses Redis Lua script (atomic check-and-record)
- [ ] Budget cap exceeded returns 403 with `quota_exceeded` error code; zero requests pass after cap
- [ ] Per-request cost calculated from `provider_models` pricing and actual token usage
- [ ] Cost saved by cache hits tracked separately and recorded as `cost_saved_cents_total` metric
- [ ] `usage_records` table aggregates hourly/daily/monthly with unique constraint preventing double-counting
- [ ] Quota check query (hot path) completes in < 5ms via composite index
- [ ] Quota increment (upsert) is atomic and handles concurrent requests without overcounting
- [ ] Warning threshold (default 80%) allows request but adds `X-Quota-Warning` header
- [ ] Rate limit headers returned on every response: `X-RateLimit-Limit`, `X-RateLimit-Remaining`, `X-RateLimit-Reset`

### Dependencies
- Epic-02 (Database Foundation — quotas, quota_usage, usage_records tables)
- Epic-04 (Provider Abstraction — model pricing data)
- Epic-05 (Request Proxy — quota check middleware integration)
- Epic-07 (Caching — cost savings calculation needs cache hit info)

### Technical Scope
- **Files/modules affected:** `/gateway-quota/src/` (QuotaChecker trait, BillingRecorder trait, RateLimiter, sliding window implementation), `/gateway-db/src/quota_repo.rs`, `/gateway-db/src/usage_repo.rs`
- **Key architectural components:** Sliding window rate limiter (Redis Lua), quota check engine, cost calculator, usage aggregation pipeline
- **Database changes required:** `quotas`, `quota_usage`, `usage_records` tables with composite indexes

### Risks
- **Risk:** Race condition in quota increment causes overcounting. **Mitigation:** `INSERT ... ON CONFLICT DO UPDATE` with atomic increment; never read-modify-write.
- **Risk:** Cost calculation accuracy disputes with customers. **Mitigation:** Validate against actual provider invoices; expose calculation formula in docs; round consistently.
- **Risk:** Rate limiter Lua script fails on Redis cluster. **Mitigation:** Script operates on single key (hashed to one slot); no cross-slot operations.

---

## Epic-09: Admin Dashboard — Foundation

**Goal:** A functional React-based admin UI for gateway management that can be served as static files from the gateway container.
**Priority:** P0
**Phase:** Phase 3
**Estimated Effort:** 2 weeks

### Description
Build the admin dashboard as a React/TypeScript SPA using a component library (shadcn/ui). Implement the complete layout with sidebar navigation, authentication pages (login, password reset), organization settings, and user management. The dashboard communicates with gateway REST APIs (`/api/v1/*`) using a typed API client. Key pages: Dashboard overview with KPI cards, settings for organization configuration, and user invitation/management. All data fetching uses React Query with optimistic updates.

### Acceptance Criteria
- [ ] Dashboard loads at `/admin` route; served as static files from gateway container
- [ ] Login page authenticates via session API; sets httpOnly cookie; redirects to dashboard
- [ ] Dashboard layout has responsive sidebar navigation with all main sections
- [ ] Organization settings page: edit org name, slug, default routing strategy, blocked models
- [ ] User invitation flow: enter email, select role, send invitation link
- [ ] User management table: list members, change roles, remove members (RBAC enforced)
- [ ] RBAC enforced in UI: viewer role sees read-only; admin cannot delete org
- [ ] All API calls include CSRF token; handle 401 by redirecting to login
- [ ] Dashboard shows loading states, empty states, and error states for all data fetches
- [ ] Dark mode support (system preference + manual toggle)

### Dependencies
- Epic-01 (Project Bootstrap — React project setup)
- Epic-03 (Authentication — session API, RBAC permissions)

### Technical Scope
- **Files/modules affected:** `/gateway-web/src/` (React app, pages, components, API client, auth hooks)
- **Key architectural components:** React SPA with React Query, shadcn/ui component library, Zustand or React Context for state management, typed API client (generated from OpenAPI or hand-typed)
- **Database changes required:** None (uses existing tables via API)

### Risks
- **Risk:** Frontend complexity exceeds small-team capacity. **Mitigation:** Use shadcn/ui for pre-built accessible components; copy-paste customization pattern.
- **Risk:** API type drift between backend and frontend. **Mitigation:** Generate TypeScript types from Rust structs (ts-rs) or maintain shared schema.
- **Risk:** Bundle size too large for slow connections. **Mitigation:** Code-split by route; lazy load heavy chart libraries.

---

## Epic-10: Provider Management UI

**Goal:** Full CRUD for provider configurations through the admin dashboard.
**Priority:** P0
**Phase:** Phase 3
**Estimated Effort:** 1 week

### Description
Build the provider management pages in the admin dashboard: a list view showing all configured providers with health status, latency, and error rate; a detail/edit page for each provider; and an "add provider" wizard. The UI supports entering provider API keys (encrypted at rest), selecting models to enable, setting custom base URLs, configuring health check parameters (interval, timeout, model to probe), and setting routing weight and priority. All changes trigger cache invalidation and are reflected within 60 seconds.

### Acceptance Criteria
- [ ] Provider list page shows all providers with real-time health status indicator (green/yellow/red)
- [ ] Add provider wizard: select kind (OpenAI/Anthropic/Gemini/Ollama/Custom), enter API key, configure models
- [ ] Provider detail page: edit API key (shows "***" for existing), enable/disable individual models, edit base URL
- [ ] Health check configuration: interval, timeout, model to use for probe
- [ ] Routing weight and priority editable per provider
- [ ] Provider deletion requires confirmation; blocked if referenced by active routing rules
- [ ] Manual health check trigger button with immediate result display
- [ ] Model list within provider shows pricing, capabilities, status
- [ ] Changes persist and propagate to routing engine within 60 seconds

### Dependencies
- Epic-04 (Provider Abstraction — provider config storage, health checks)
- Epic-06 (Routing Engine — provider weights/priorities affect routing)
- Epic-09 (Admin Dashboard — foundation pages and layout)

### Technical Scope
- **Files/modules affected:** `/gateway-web/src/pages/providers/`, `/gateway-api/src/admin_routes/providers.rs`
- **Key architectural components:** Provider CRUD admin API, encrypted API key handling (show only on input), health status display
- **Database changes required:** None (uses `provider_configs`, `provider_models` tables)

### Risks
- **Risk:** API key exposure in browser DevTools. **Mitigation:** Keys are write-only in API; never returned in GET responses; frontend shows masked values.
- **Risk:** Invalid provider configuration breaks routing. **Mitigation:** Validate config on save (test connectivity); don't allow enabling without successful health check.

---

## Epic-11: API Key Management UI

**Goal:** Create, revoke, and configure gateway API keys through the admin dashboard with full visibility into usage.
**Priority:** P0
**Phase:** Phase 3
**Estimated Effort:** 1 week

### Description
Implement the API key management interface in the admin dashboard. Users can create new keys with descriptive names, assign scopes (chat:write, embeddings:write, models:read, etc.), restrict to specific models, set IP allowlists, configure rate limits (requests/minute, tokens/minute), and set expiration dates. Key creation shows the full key exactly once (with copy button). The list view displays key prefixes, status, last used date, and usage count. Revocation is immediate with a confirmation dialog.

### Acceptance Criteria
- [ ] API key list shows all keys with prefix, name, status (active/revoked/expired), last used, created date
- [ ] Create key form: name, scopes multi-select, allowed models multi-select, IP allowlist CIDR input, rate limits, expiration date
- [ ] Full key displayed exactly once on creation with copy-to-clipboard button; never shown again
- [ ] Revoke key button with confirmation modal; revocation is immediate (sub-100ms propagation)
- [ ] Key status filter: All, Active, Revoked, Expired
- [ ] Each key's usage count and estimated cost visible in list view
- [ ] Rate limit values editable after creation
- [ ] Key creation rate-limited: max 10 keys/minute per organization

### Dependencies
- Epic-03 (Authentication — API key generation, validation, revocation)
- Epic-08 (Quota — rate limit configuration per key)
- Epic-09 (Admin Dashboard — foundation)

### Technical Scope
- **Files/modules affected:** `/gateway-web/src/pages/keys/`, `/gateway-api/src/admin_routes/keys.rs`
- **Key architectural components:** API key CRUD admin API, scope management, IP allowlist validation (CIDR), immediate revocation propagation
- **Database changes required:** None (uses `api_keys` table)

### Risks
- **Risk:** Accidental key revocation breaks production clients. **Mitigation:** Confirmation modal with warning; show last used time; suggest creating new key first.
- **Risk:** Key displayed in browser history or logs. **Mitigation:** Use modal for display; never include key in URL; clear from React state after copy.

---

## Epic-12: Usage Analytics & Cost Dashboards

**Goal:** Comprehensive cost visibility through interactive charts and tables showing per-request, per-provider, per-model, and per-key cost breakdown.
**Priority:** P0
**Phase:** Phase 3
**Estimated Effort:** 1.5 weeks

### Description
Build the usage analytics section of the admin dashboard with interactive charts and data tables. The overview page shows KPI cards (total requests, total cost, cache hit rate, average latency). Cost breakdown pages show spending by provider, by model, by API key, and over time (hourly/daily/monthly trends). Token usage visualization shows input vs output token distribution. Cache analytics displays hit rate trends and cost savings attributed to caching. All data is served from pre-aggregated `usage_records` for sub-100ms load times. Export to CSV/JSON is supported for all data views.

### Acceptance Criteria
- [ ] Overview dashboard loads in < 2 seconds with 4 KPI cards (requests, cost, cache hit %, avg latency)
- [ ] Cost by provider pie/donut chart; cost by model bar chart; clickable to drill down
- [ ] Time series chart: cost and request volume over selected period (24h, 7d, 30d, 90d)
- [ ] API key usage table: sortable by cost, requests, tokens; pagination server-side
- [ ] Cache analytics: exact hit rate, semantic hit rate (if enabled), cost saved, cache size
- [ ] Data accuracy: dashboard cost within 1% of sum of `usage_records.total_cost`
- [ ] Export button on every data view: CSV and JSON formats
- [ ] Date range picker for all time-based views; default to last 7 days
- [ ] Responsive charts that work on mobile and desktop viewports

### Dependencies
- Epic-08 (Quota & Billing — usage_records aggregation, cost calculation)
- Epic-09 (Admin Dashboard — foundation, React Query data fetching)
- Epic-07 (Caching — cache hit metrics, cost savings data)

### Technical Scope
- **Files/modules affected:** `/gateway-web/src/pages/analytics/`, `/gateway-api/src/admin_routes/usage.rs`, `/gateway-api/src/admin_routes/analytics.rs`
- **Key architectural components:** Usage aggregation API, chart components (Recharts or Tremor), data export service, date range filtering
- **Database changes required:** None (uses `usage_records` table)

### Risks
- **Risk:** Large date ranges cause slow queries. **Mitigation:** Pre-aggregated data in `usage_records`; never scan raw `requests` table for analytics.
- **Risk:** Chart library bundle size too large. **Mitigation:** Code-split chart library; use lightweight SVG-based charts where possible.
- **Risk:** Timezone confusion in date range selection. **Mitigation:** All storage in UTC; frontend converts to local timezone; API accepts ISO 8601 datetimes.

---

## Epic-13: Fallback, Health Checks & Circuit Breaker

**Goal:** Automatic provider failover with circuit breaker pattern ensures gateway remains available even when upstream providers fail.
**Priority:** P0
**Phase:** Phase 2
**Estimated Effort:** 1 week

### Description
Implement automatic retry and fallback logic in the request orchestrator. On provider error (5xx, 429, timeout), retry up to 2 times with exponential backoff (1s, 2s). If retries exhaust, attempt the next provider in the fallback chain. Circuit breaker tracks consecutive failures per provider: after 5 failures, mark the provider unhealthy for 60 seconds. Health check probes run every 30 seconds against each configured provider using a lightweight model call. Recovery is attempted automatically when the circuit breaker timeout expires. Request cancellation (client disconnect) propagates to abort the upstream provider request.

### Acceptance Criteria
- [ ] Provider returns 5xx: gateway retries up to 2 times with exponential backoff
- [ ] All retries exhausted: gateway attempts next provider in fallback chain
- [ ] Circuit breaker opens after 5 consecutive failures; provider marked unhealthy
- [ ] Circuit breaker closes after 60 seconds; health check probe attempts recovery
- [ ] Client disconnect (connection dropped) aborts upstream provider request within 500ms
- [ ] Fallback chain exhausted: return 502 with `provider_error` code and details of all attempts
- [ ] Health check history stored for 24 hours; viewable via admin API
- [ ] Provider marked unhealthy is excluded from routing within 30 seconds
- [ ] Each fallback attempt logged with provider name, error, and latency

### Dependencies
- Epic-04 (Provider Abstraction — health check framework, provider selection)
- Epic-05 (Request Proxy — request orchestrator integration point)
- Epic-06 (Routing Engine — fallback chain construction)

### Technical Scope
- **Files/modules affected:** `/gateway-core/src/orchestrator.rs` (retry/fallback logic), `/gateway-providers/src/health.rs` (circuit breaker, health tracker)
- **Key architectural components:** Retry with exponential backoff, circuit breaker state machine, health probe scheduler, cancellation token propagation
- **Database changes required:** None (health state in Redis)

### Risks
- **Risk:** Retry storms during provider outages overwhelm remaining healthy providers. **Mitigation:** Circuit breaker opens quickly; jitter on retry delays; max concurrent retries limit.
- **Risk:** Request cancellation not propagating to upstream leaves orphan connections. **Mitigation:** Use `tokio::select!` with cancellation token; drop HTTP client connection on cancel.
- **Risk:** False circuit breaker trips from transient errors. **Mitigation:** Only count specific error codes (5xx, timeout); 429 (rate limit) does not count toward circuit breaker.

---

## Epic-14: Semantic Caching

**Goal:** Embedding-based semantic caching increases cache hit rates by serving cached responses for semantically equivalent prompts, not just exact matches.
**Priority:** P1
**Phase:** Phase 4
**Estimated Effort:** 2 weeks

### Description
Implement semantic caching using a local ONNX embedding model (all-MiniLM-L6-v2 via fastembed-rs, 384 dimensions). On cache miss, compute an embedding vector of the request prompt. Store the embedding alongside the response in Redis. On subsequent requests, compute the embedding and perform cosine similarity search against stored embeddings. If similarity exceeds the threshold (default 0.92), return the cached response. Use brute-force similarity search (Phase 1) — iterate stored embeddings, compute cosine similarity, return best match above threshold. Semantic cache is separate from exact-match cache; exact match is checked first (faster), semantic is fallback.

### Acceptance Criteria
- [ ] Embedding model loads at startup (~30MB RAM, ~5ms per inference)
- [ ] Semantically equivalent prompts (different wording, same intent) return cached response
- [ ] Cosine similarity threshold configurable per org per model (default 0.92)
- [ ] Semantic cache hit rate > 15% of total requests in production workloads
- [ ] Semantic search latency p99 < 50ms for < 100K cached prompts per tenant:model
- [ ] False positive rate < 2% (manual audit of 100 random semantic hits)
- [ ] Semantic cache entries deduplicated: new entry not stored if existing entry has similarity > 0.97
- [ ] Embedding computation skipped for non-cacheable requests (same rules as exact-match cache)
- [ ] Semantic hits tracked separately in metrics: `cache_hit_l2_semantic_total`

### Dependencies
- Epic-07 (Caching — exact-match cache infrastructure, Redis integration)
- Epic-05 (Request Proxy — cache check/store integration in request lifecycle)

### Technical Scope
- **Files/modules affected:** `/gateway-cache/src/semantic.rs` (SemanticCache trait, embedding integration, similarity search), `/gateway-cache/src/embeddings.rs` (ONNX model wrapper)
- **Key architectural components:** ONNX runtime integration (ort/fastembed-rs), embedding computation, cosine similarity, brute-force search over Redis-stored vectors
- **Database changes required:** `cache_metadata` table extended with `embedding` field

### Risks
- **Risk:** ONNX model loading fails or incompatible with target architecture. **Mitigation:** Download model on first run; validate at startup with dummy inference; graceful degradation (disable semantic cache if model fails).
- **Risk:** Embedding computation adds latency to cache miss path (~10ms). **Mitigation:** Only compute on cache miss; exact-match checked first; batch embedding for multiple concurrent requests.
- **Risk:** Brute-force search becomes slow as cache grows (> 100K entries). **Mitigation:** Document 100K limit; Phase 5 upgrades to HNSW index.

---

## Epic-15: Webhooks & Event System

**Goal:** Real-time event notifications enable customers to integrate gateway events into their operational workflows (Slack alerts, custom automation).
**Priority:** P1
**Phase:** Phase 4
**Estimated Effort:** 1.5 weeks

### Description
Build an event system that generates events for significant gateway occurrences (quota warnings, quota exceeded, provider errors, provider recovered, request failed). Allow organizations to configure webhook endpoints that receive these events as signed HTTP POST requests. Implement webhook delivery with retry logic (exponential backoff, max 3 retries), HMAC-SHA256 signature for payload verification, and a delivery log for debugging. Support custom headers on webhook requests. Webhook status tracked: active, inactive, failing (after consecutive failures).

### Acceptance Criteria
- [ ] Webhook CRUD via admin API: create, list, update, delete webhook endpoints
- [ ] Supported event types: `quota.warning`, `quota.exceeded`, `provider.error`, `provider.recovered`, `request.failed`
- [ ] Webhook payload delivered as signed HTTP POST with `X-Webhook-Signature` HMAC-SHA256 header
- [ ] Retry on delivery failure: 3 attempts with exponential backoff (60s intervals)
- [ ] Delivery log tracks every attempt: request, response status, error message, timing
- [ ] Webhook auto-disabled after 10 consecutive failures; status changes to `failing`
- [ ] Custom headers supported on webhook requests (e.g., `Authorization` for receiving services)
- [ ] Webhook secret encrypted at rest (`secret_enc` BYTEA field)
- [ ] Delivery latency p99 < 5 seconds from event generation to HTTP dispatch

### Dependencies
- Epic-02 (Database Foundation — webhooks, webhook_deliveries tables)
- Epic-08 (Quota — event generation triggers for quota warnings/exceeded)
- Epic-13 (Fallback — provider error/recovered events)

### Technical Scope
- **Files/modules affected:** `/gateway-events/src/` (event bus, webhook dispatcher), `/gateway-db/src/webhook_repo.rs`
- **Key architectural components:** In-process event bus, webhook HTTP client with retry, HMAC-SHA256 signing, delivery worker
- **Database changes required:** `webhooks`, `webhook_deliveries` tables with delivery status indexes

### Risks
- **Risk:** Webhook delivery floods receiving endpoint during incidents. **Mitigation:** Implement delivery rate limiting; batch events where possible; circuit breaker on failing endpoints.
- **Risk:** Webhook signature verification confusing for consumers. **Mitigation:** Provide code examples in Python, JavaScript, and curl; document signing algorithm clearly.
- **Risk:** Delivery worker stalls on slow webhook endpoints. **Mitigation:** Use async HTTP client with timeout; process deliveries concurrently; separate queue per webhook.

---

## Epic-16: Observability

**Goal:** Comprehensive metrics, structured logging, and health endpoints enable operators to monitor and debug the gateway effectively.
**Priority:** P0
**Phase:** Phase 1-2 (infrastructure in Phase 1, metrics pipeline in Phase 2)
**Estimated Effort:** 1 week

### Description
Implement structured JSON logging for all requests, errors, and audit events. Add Prometheus-compatible metrics for cache performance, request latency, provider health, quota usage, and cost tracking. Health check endpoints (`/health` for liveness, `/ready` for readiness) report dependency status. Request correlation via `request_id` propagated through all components. Log output is stdout-only for container compatibility.

### Acceptance Criteria
- [ ] Every HTTP request logged as structured JSON with: timestamp, method, path, status, latency, org_id, key_id, provider, model, tokens, cost
- [ ] Error logs include stack trace (in development), error code, request_id, and context
- [ ] Prometheus metrics endpoint (`/metrics`) exposes: request counters (by status, provider, model), latency histograms, cache hit/miss counters, quota check counters, provider health gauges, cost saved counter
- [ ] `request_id` propagated from edge through all async tasks and logged at every step
- [ ] `/health` returns 200 when gateway process is alive; `/ready` returns 200 only when DB and Redis connections are healthy and configs loaded
- [ ] Log level configurable via `RUST_LOG` environment variable; defaults to `info`
- [ ] No sensitive data in logs (API keys, provider API keys, request/response bodies)
- [ ] Metric `gateway_latency_overhead_ms` (total latency minus provider latency) p99 < 5ms

### Dependencies
- Epic-01 (Project Bootstrap)
- Epic-05 (Request Proxy — request logging integration)

### Technical Scope
- **Files/modules affected:** `/gateway-observability/src/` (logger, metrics, tracing), `/gateway-api/src/middleware/logging.rs`
- **Key architectural components:** Structured JSON logger (tracing + tracing-subscriber), Prometheus metrics registry (metrics crate), health check responder, request ID propagation
- **Database changes required:** None

### Risks
- **Risk:** Metrics cardinality explosion (unique label combinations cause memory growth). **Mitigation:** Limit model name labels to known models; sanitize user-provided strings; drop unknown labels.
- **Risk:** Logging overhead impacts hot-path performance. **Mitigation:** Use lock-free ring buffer for log dispatch; async log writing; sampling for high-volume debug logs.
- **Risk:** Health check endpoint becomes DDoS vector. **Mitigation:** Health checks are cheap (Redis PING + DB SELECT 1); no auth required by design (load balancers need it).

---

## Epic-17: Security Hardening

**Goal:** Production-ready security posture with rate limiting, input validation, audit logging, and secure defaults.
**Priority:** P1
**Phase:** Phase 2-3
**Estimated Effort:** 1 week

### Description
Harden the gateway for production deployment: request payload size limits (10MB), input validation on all request fields, CORS configuration, security headers (HSTS, CSP, X-Frame-Options), request timeout enforcement (30s default), connection limits, and audit logging for all administrative actions. Implement API key IP allowlisting. Add request sanitization to prevent log injection. Run dependency security audits in CI.

### Acceptance Criteria
- [ ] Request body size limited to 10MB; returns 413 for oversized requests
- [ ] All request fields validated: model name exists, temperature in [0, 2], max_tokens positive integer, etc.
- [ ] CORS configured via environment variable; defaults to same-origin
- [ ] Security headers on all responses: `Strict-Transport-Security`, `X-Content-Type-Options: nosniff`, `X-Frame-Options: DENY`, `Content-Security-Policy`
- [ ] Request timeout enforced: 30s default per provider call; returns 504 on timeout
- [ ] API key IP allowlist: requests from non-allowed IPs return 403
- [ ] Audit log entry created for every admin action: key creation, key revocation, provider config change, user role change, settings update
- [ ] `cargo audit` runs in CI and blocks merges on critical advisories
- [ ] No secrets in environment variables logged at startup (masked in logs)

### Dependencies
- Epic-03 (Authentication — IP allowlist on API keys)
- Epic-05 (Request Proxy — middleware layer for security headers, validation)
- Epic-09 (Admin Dashboard — audit log viewer UI)

### Technical Scope
- **Files/modules affected:** `/gateway-api/src/middleware/security.rs`, `/gateway-api/src/validation.rs`, `/gateway-api/src/audit.rs`
- **Key architectural components:** Input validation layer, security middleware, audit logger, IP allowlist checker
- **Database changes required:** `audit_log` table

### Risks
- **Risk:** Security headers break frontend integration. **Mitigation:** CSP is report-only in development; CORS is configurable; test with common frontend frameworks.
- **Risk:** Input validation is too strict and rejects valid OpenAI requests. **Mitigation:** Validate only known fields; pass through unknown fields to provider; test against OpenAI SDK test suite.
- **Risk:** Audit log volume becomes excessive. **Mitigation:** Configurable audit level; exclude read-only operations by default; partition by month.

---

## Epic-18: Deployment & Operations

**Goal:** Production Docker Compose deployment that a single developer can set up, operate, and maintain without specialized DevOps knowledge.
**Priority:** P0
**Phase:** Phase 3
**Estimated Effort:** 1 week

### Description
Polish the Docker Compose setup for production use: optimized Rust build (multi-stage Dockerfile), PostgreSQL with persistent volume and automated backups, Redis with persistence and memory limits, environment variable configuration documentation, database migration automation on container startup, log rotation, health check configuration for container orchestrators, and operational runbooks for common tasks (backup, restore, upgrade, rotate secrets).

### Acceptance Criteria
- [ ] Multi-stage Dockerfile: builder stage (cargo build) + runtime stage (minimal image, no build tools)
- [ ] `docker-compose.prod.yml` with resource limits: gateway (1 CPU, 512MB), PostgreSQL (1 CPU, 1GB), Redis (0.5 CPU, 512MB)
- [ ] Database migrations run automatically on container startup before accepting requests
- [ ] PostgreSQL data persisted in named Docker volume; daily automated backup script
- [ ] Redis configured with `appendonly yes` and `maxmemory 512mb` with `allkeys-lru` eviction
- [ ] Environment variable documentation: all required and optional variables with descriptions and defaults
- [ ] `README.md` deployment guide: from `git clone` to first proxied request in < 10 minutes
- [ ] Upgrade runbook: steps for zero-downtime version upgrade (new container, stop old, start new)
- [ ] Secret rotation runbook: rotate database credentials, gateway master key, JWT signing keys
- [ ] Backup/restore runbook: PostgreSQL dump, Redis RDB, configuration export

### Dependencies
- Epic-01 (Project Bootstrap — Docker Compose foundation)
- Epic-02 (Database Foundation — migration framework)

### Technical Scope
- **Files/modules affected:** `/Dockerfile`, `/docker-compose.yml`, `/docker-compose.prod.yml`, `/scripts/` (backup, restore, upgrade), `/docs/runbooks/`
- **Key architectural components:** Multi-stage Docker build, container startup sequencing, volume management, backup automation
- **Database changes required:** None

### Risks
- **Risk:** Docker image size too large for slow deploys. **Mitigation:** Distroless or Alpine runtime stage; strip symbols from binary; ~50MB final image.
- **Risk:** Database migration fails in production (locked table, long migration). **Mitigation:** Migrations run in transactions; test migration time against production-like data volume; provide rollback procedure.
- **Risk:** Users expect Kubernetes manifests. **Mitigation:** Document that Docker Compose is the supported deployment; community can contribute K8s examples but not officially supported.

---

## Epic-19: Smart Cost-Aware Routing

**Goal:** Automatic routing to the cheapest capable model based on query complexity, reducing costs by 20-40% for eligible requests.
**Priority:** P1
**Phase:** Phase 4
**Estimated Effort:** 1.5 weeks

### Description
Implement intelligent routing that analyzes incoming requests and routes simple queries to cheaper models while preserving quality for complex queries. Use heuristics: message count, token count, presence of code/system prompts, complexity indicators. Maintain a cost database per model. Route simple requests (short prompts, no special requirements) to models like GPT-4o-mini or Claude Haiku instead of GPT-4o or Claude Sonnet. Log routing decisions with estimated cost differential for analytics.

### Acceptance Criteria
- [ ] Simple queries (e.g., "What is 2+2?", < 50 tokens) routed to cheapest capable model
- [ ] Complex queries (code generation, multi-step reasoning) routed to premium models
- [ ] Cost differential logged per routing decision; accessible in analytics dashboard
- [ ] User can override via `X-Gateway-Routing-Strategy: cost` header or request body field
- [ ] Cost-aware routing reduces average request cost by > 20% for eligible traffic
- [ ] Quality preservation: no increase in error rate or user complaints when cost routing enabled
- [ ] Routing decision transparent: `gateway.routing_strategy_applied` in response metadata
- [ ] Fallback to premium model if cheap model returns error or rate limit

### Dependencies
- Epic-06 (Routing Engine — rule-based routing foundation)
- Epic-04 (Provider Abstraction — model cost database)
- Epic-12 (Usage Analytics — cost differential reporting)

### Technical Scope
- **Files/modules affected:** `/gateway-core/src/smart_router.rs` (complexity analyzer, cost estimator, routing decision engine)
- **Key architectural components:** Query complexity heuristic, model capability matrix, cost comparison engine, quality feedback loop
- **Database changes required:** `provider_models` extended with complexity scores

### Risks
- **Risk:** Over-aggressive cost routing degrades output quality. **Mitigation:** Conservative heuristics initially; A/B test with quality metrics; user-adjustable aggressiveness setting.
- **Risk:** Cheap model rate limits cause cascading fallbacks. **Mitigation:** Track cheap model quota; pre-check rate limit before routing; maintain fallback to premium.
- **Risk:** Cost estimation inaccurate (model pricing changes). **Mitigation:** Periodic pricing sync from provider docs; override capability per org.

---

## Epic-20: Team Collaboration & SSO

**Goal:** Multi-user team support with SAML 2.0 SSO enables organizations to manage gateway access through their existing identity provider.
**Priority:** P1
**Phase:** Phase 4
**Estimated Effort:** 2 weeks

### Description
Enhance the organization member system for team collaboration: invite users by email, assign roles (owner/admin/member/viewer), track per-user cost attribution, and implement SAML 2.0 SSO for enterprise customers. SSO uses SAML identity provider metadata for configuration, Just-In-Time user provisioning on first login, and optional SSO-only enforcement (disable password login). Session management for SSO users follows the same JWT pattern as password users.

### Acceptance Criteria
- [ ] Organization supports multiple users with different roles
- [ ] User invitation by email: sends invite link, recipient joins with role assignment
- [ ] Per-user cost attribution visible in dashboard: each user's API key usage aggregated
- [ ] SAML 2.0 SSO configuration: upload IdP metadata XML, configure ACS URL, test login flow
- [ ] JIT provisioning: first SSO login creates user account linked to SAML assertion
- [ ] SSO-only mode: when enabled, password login is disabled for the organization
- [ ] Role changes audited: who changed what role, when, logged to audit log
- [ ] Works with major IdPs: Okta, Azure AD, Google Workspace (tested against sandboxes)
- [ ] SAML certificate rotation supported without downtime

### Dependencies
- Epic-03 (Authentication — session management, RBAC roles)
- Epic-09 (Admin Dashboard — user management UI)

### Technical Scope
- **Files/modules affected:** `/gateway-auth/src/sso.rs` (SAML integration), `/gateway-web/src/pages/settings/sso.tsx`, `/gateway-api/src/admin_routes/sso.rs`
- **Key architectural components:** SAML 2.0 SP implementation (samael crate), IdP metadata parser, JIT user provisioning, SSO session bridge
- **Database changes required:** `organization_members` table (already exists); SAML config stored in `organizations.settings` JSONB

### Risks
- **Risk:** SAML implementation complexity (XML signature validation, certificate management). **Mitigation:** Use battle-tested `samael` crate; extensive unit tests with known-good SAML responses; test against real IdP sandboxes.
- **Risk:** SSO login flow UX is confusing (redirects, SAML artfacts). **Mitigation:** Clear error messages for common failures (certificate expired, assertion rejected); debug mode with detailed logs.
- **Risk:** IdP compatibility issues (different SAML implementations). **Mitigation:** Test against top 3 IdPs (Okta, Azure AD, Google); document known configurations.

---

## Dependency Graph

```
Epic-01 (Bootstrap)
  |
  +-- Epic-02 (Database)
  |     |
  |     +-- Epic-03 (Auth)
  |     |     |
  |     |     +-- Epic-09 (Dashboard) -------- Epic-10 (Provider UI)
  |     |     |                                   |
  |     |     +-- Epic-11 (Key Mgmt UI)          Epic-12 (Analytics)
  |     |     |
  |     +-- Epic-04 (Providers)
  |     |     |
  |     |     +-- Epic-05 (Proxy) --------------- Epic-06 (Routing)
  |     |     |     |                              |
  |     |     |     +-- Epic-13 (Fallback)        Epic-19 (Smart Routing)
  |     |     |     |
  |     |     |     +-- Epic-07 (Cache) ---------- Epic-14 (Semantic Cache)
  |     |     |     |
  |     |     |     +-- Epic-08 (Quota/Billing) -- Epic-15 (Webhooks)
  |     |     |           |
  |     |     |           +---------------------- Epic-12 (Analytics)
  |     |     |
  |     +-- Epic-16 (Observability)
  |     |
  |     +-- Epic-17 (Security)
  |
  +-- Epic-18 (Deployment)

Epic-20 (Team/SSO) depends on Epic-03 and Epic-09
```

## Effort Summary by Phase

| Phase | Epics | Total Effort | Team Size | Calendar Weeks |
|-------|-------|-------------|-----------|----------------|
| Phase 1: Foundation | 01, 02, 03, 04, 05 | 5.5 weeks | 2-3 | 4 |
| Phase 2: Core Gateway | 06, 07, 08, 13, 16, 17 | 6 weeks | 2-3 | 4 |
| Phase 3: Dashboard | 09, 10, 11, 12, 18 | 6.5 weeks | 2-3 | 4 |
| Phase 4: Enterprise | 14, 15, 19, 20 | 7 weeks | 2-3 | ~8 |
| Phase 5: Scale | (future epics) | TBD | 3+ | ~24 |

**Notes:**
- Effort estimates assume 1-3 person team with Rust proficiency
- Parallel work possible: frontend (Epic-09+) and backend (Epic-06, 07, 08) can proceed simultaneously in Phase 2-3
- Epic-16 (Observability) is implemented incrementally across all phases, not as a single block
- Calendar weeks account for parallelization, review, and iteration

---

*This epic breakdown is a living document. Acceptance criteria are the definition of done. Effort estimates should be revisited at the start of each epic based on prior velocity.*
