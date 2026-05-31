# SWARM HANDOFF DOCUMENT

> This is the single entry point for all implementation swarms.
> Read this document first, then follow the pointers to specific documents.
> Do NOT read the original specification — all relevant content has been extracted,
> improved, and condensed here.

---

## 1. Product Vision (30-Second Summary)

### What We Are Building
An open-core AI Gateway that deploys in under 10 minutes on a single VPS via Docker Compose. It provides intelligent request routing, semantic caching, and hard budget caps that reduce AI API spend by 30-70%. Built for cost-conscious SMEs who need multi-provider AI infrastructure without Kubernetes or DevOps expertise.

### Who We Serve
**Primary persona:** The Cost-Conscious CTO at 20-100 person tech companies spending $500-$10,000/month on AI APIs. **Secondary persona:** The Agency Technical Lead managing AI for 5-20 clients who needs per-client cost attribution and data residency. Explicitly NOT for Fortune 500 (requires SAML, SOC 2), Kubernetes shops, or teams spending <$100/month on AI.

### Why They Buy
1. **Cut AI spend 30-70%** — Smart routing + semantic caching reduces bills with zero platform markup on inference (target: 40% average cost reduction).
2. **Deploy in <10 minutes on a $20 VPS** — `docker-compose up` after `git clone`. No Kubernetes, no DevOps. Only product in the market that achieves this.
3. **Hard budget caps that actually stop spending** — Auto-cutoff when budget reached. Validated gap: no competitor offers affordable hard caps (Helicone has alerts only; Portkey has no hard caps at lower tiers).

### What Winning Looks Like
**North Star Metric:** Monthly AI Spend Avoided (dollar value saved by all active deployments). **1-year targets:** 1,000 active CE deployments, 50 paying customers, $10K MRR. **Winning condition:** 40%+ of free-tier users deploy to production within 7 days.

---

## 2. Architecture at a Glance

### Stack

| Layer | Technology | Purpose |
|-------|-----------|---------|
| Language | Rust 1.78+ | High-performance, memory-safe backend |
| Web Framework | Axum 0.7 | HTTP server, routing, middleware, SSE streaming |
| HTTP Client | reqwest 0.12 | Outbound requests to LLM providers |
| Async Runtime | tokio 1.37 | Async runtime, thousands of concurrent connections |
| Database | PostgreSQL 16 | Persistent data (API keys, quotas, billing, configs) |
| Cache | Redis 7.2 | Ephemeral data (response cache, rate limits, health) |
| Frontend | React 18 + TypeScript 5.4 | Admin dashboard SPA |
| Bundler | Vite 5.2 | Fast dev server and production build |
| UI Library | shadcn/ui + Radix UI | Accessible, customizable components |
| Styling | Tailwind CSS 3.4 | Utility-first CSS |
| Reverse Proxy | Caddy 2.8 | Automatic HTTPS, static file serving |
| Orchestration | Docker Compose 2.27 | Single-command deployment |
| Auth | API Keys + JWT (RS256) | Dual auth system (System A for APIs, System B for dashboard) |
| L1 Cache | moka 0.12 | In-process cache (sub-microsecond lookups) |
| Embeddings | fastembed-rs v3 | Local ONNX for semantic caching |

### Component Diagram

```
                        +-----------+
                        |   Caddy   |  TLS termination, static files
                        +-----+-----+
                              |
+-----------------------------v------------------------------------+
|                      GATEWAY-API (Axum)                          |
|  Auth MW -> Rate Limiter -> Request Logger -> Router -> Handler  |
+----------------------------+-------------------------------------+
                             |
+--------------------v-------v-------------------------------------+
|                  GATEWAY-CORE                                    |
|  Parse -> Auth -> Rate Limit -> Quota Check -> Cache Check       |
|    -> Provider Select -> Transform -> Call Provider              |
|    -> Transform Response -> Store Cache -> Update Quota          |
+----+------+---------+---------------------+----------------------+
     |      |         |                     |
+----v-+ +--v------+ +--v---------------+  +--v-------------+
|Providers| Cache   | | Quota & Billing  |  | Auth           |
| -OpenAI | L1:moka | | -Rate limiting   |  | -API key auth  |
| -Anthro | L2:Redis| | -Budget caps     |  | -JWT sessions  |
| -Gemini |         | | -Cost tracking   |  | -RBAC          |
| -Ollama |         | | -Usage records   |  |                |
+--------+ +---------+ +-----------------+  +----------------+
     |
+----v---------+   +-----------------+
|  PostgreSQL  |   |     Redis       |
|  (persistent)|   |   (ephemeral)   |
+--------------+   +-----------------+
     |
+----v---------------------------------+
|         REACT ADMIN DASHBOARD        |
|  Logs | Analytics | Keys | Providers |
|  Budgets | Users | Settings           |
+--------------------------------------+
```

### Request Lifecycle

1. **Parse & Validate** — Deserialize request, extract headers, validate body (10MB limit)
2. **Authenticate** — Validate API key (`sk_gw_*` format), SHA-256 hash lookup, load org context
3. **Rate Limit Check** — Token bucket + sliding window (Redis Lua scripts), 6 layers checked
4. **Quota/Budget Check** — Pre-request cost estimate against budget cap; reject with 429 if exceeded
5. **Cache Check** — L1 (moka, <0.1ms) → L2 exact (Redis) → L2 semantic (embedding similarity)
6. **Provider Selection** — Route to healthy provider based on strategy (cost/latency/priority/fallback)
7. **Request Transform** — Convert OpenAI-format to provider-native format
8. **Call Provider** — HTTP request with retry (2x exp backoff) and circuit breaker; stream SSE if requested
9. **Response Transform** — Normalize to OpenAI-compatible response with `gateway` metadata
10. **Store Cache** — Write response to L1 + L2 (async, non-blocking)
11. **Update Quota & Record Usage** — Deduct actual tokens, record cost to PostgreSQL (async)
12. **Respond** — Return JSON with headers: `X-Cache`, `X-Provider`, `X-Quota-Remaining`

### Deployment Target

**Single VPS. Docker Compose. <10 minutes.**

```bash
git clone <repo>
cd ai-gateway
docker compose up -d
# Gateway, PostgreSQL, Redis, and dashboard all start
# First request in under 10 minutes from git clone
```

Services: gateway API (Rust binary), PostgreSQL 16, Redis 7.2, React dashboard (static files served by Caddy). Resource requirements: 2 vCPU, 4GB RAM, 20GB disk minimum. Runs on any VPS provider (Hetzner, DigitalOcean, Linode, AWS Lightsail).

---

## 3. Critical Architecture Decisions (ADRs)

| ADR | Title | Decision Summary | Why It Matters |
|-----|-------|-----------------|----------------|
| [ADR-001](adr/ADR-001-provider-abstraction.md) | Provider Abstraction | All providers behind a unified `Provider` trait; OpenAI-compatible canonical format. New provider = new trait impl, no core changes. | Enables adding providers without touching routing, caching, or billing code. Isolates provider-specific complexity. |
| [ADR-002](adr/ADR-002-cache-strategy.md) | Two-Tier Cache (Exact + Semantic) | L1 in-process (moka) + L2 Redis. Exact-match (SHA-256) catches 5-15%; semantic (embedding similarity) catches 25-50%. Ollama responses NOT cached. | Cost reduction is the #1 value prop. Semantic caching is a key differentiator — no competitor offers it affordably. |
| [ADR-003](adr/ADR-003-authentication.md) | Dual Auth Systems | System A: API keys (`sk_gw_*`, SHA-256 hashed, Redis-cached). System B: JWT sessions (RS256, httpOnly cookies, 15-min expiry). | API key auth adds <1ms overhead. Self-contained deployment with no external IdP dependency. |
| [ADR-004](adr/ADR-004-rate-limiting.md) | Token Bucket + Sliding Window | Redis-backed token bucket for req/s + sliding window for tokens/min. 6 layers (global, org, key, tokens, provider, IP). Lua scripts for atomicity. | Prevents runaway costs from misconfigured clients. Fail-closed: if Redis down, requests blocked (configurable). |
| [ADR-005](adr/ADR-005-tenant-model.md) | Organization-Based Tenancy | `org_id` column on every tenant-scoped table. Application-level `WHERE` clauses + PostgreSQL RLS as defense-in-depth. Cache keys prefixed with tenant ID. | Cross-tenant data leakage would be catastrophic. Dual-layer isolation (app + RLS) means two failures required for breach. |
| [ADR-006](adr/ADR-006-observability.md) | Built-In Observability | Structured JSON logging (`tracing`), Prometheus `/metrics` endpoint, built-in dashboard analytics. No external SaaS dependency. | Single-node system doesn't need distributed tracing. `request_id` correlation sufficient. Zero external dependencies. |
| [ADR-007](adr/ADR-007-fallback-strategy.md) | Circuit Breaker + Fallback | 3-state circuit breaker (CLOSED → OPEN → HALF_OPEN). Health checks every 30s. Configurable fallback chains. Retry 2x with exp backoff. | Provider outages must not cause customer downtime. Automatic recovery without human intervention. |
| [ADR-008](adr/ADR-008-ollama-support.md) | Ollama as First-Class Provider | Implements `Provider` trait. Configurable base URL (default `localhost:11434`). 300s timeout, 20-conn pool. No caching (local inference is "free"). | Data privacy + zero API cost for capable workloads. Gateway is client of Ollama, not its operator. |

---

## 4. Implementation Status

### Phase 1: Foundation — COMPLETED ✅

| Order | Task | Status | Notes |
|-------|------|--------|-------|
| 1 | TASK-0001: Initialize Rust workspace | ✅ Done | 8 crates: gateway-api, gateway-core, gateway-providers, gateway-cache, gateway-quota, gateway-auth, gateway-db, gateway-observability |
| 2 | TASK-0002: Docker Compose dev environment | ✅ Done | postgres, redis, backend, frontend services. Dev DB: `gateway_dev` |
| 3 | TASK-0006: Migration framework + connection pool | ✅ Done | sqlx 0.9, connection pool with `SET app.org_id` in `after_connect` |
| 4 | TASK-0007–0011: All 22 migrations | ✅ Done | Full schema + seed data applied. Partitioned tables: requests, responses, usage_records, webhook_deliveries, audit_log |
| 5 | TASK-0012: Password hashing + registration | ✅ Done | Argon2id + zxcvbn strength validation |
| 6 | TASK-0013: JWT session auth | ✅ Done | RS256, access 15min, refresh 7 days. Tests use dynamically generated RSA keys |
| 7 | TASK-0014: API key generation + storage | ✅ Done | Format: `sk_gw_{32_base58_chars}{6_base58_checksum}` = 44 chars. SHA-256 hash stored. Prefix is first 8 chars |
| 8 | TASK-0015: API key validation middleware | ✅ Done | `AuthContext` model with stub validation. Full middleware not yet wired into router |
| 9 | TASK-0016: RBAC permission system | ✅ Done | 4 roles (owner/admin/member/viewer), 31 permissions |
| 10 | TASK-0019: Tenant isolation enforcement | ✅ Done | `tenant_isolation_middleware` + `org_id` path extractor. NOT yet applied to API routes |
| 11 | TASK-0020: Provider trait + canonical types | ✅ Done | `Provider` trait in `gateway-providers`. Canonical OpenAI types in `gateway-core` |
| 12 | TASK-0021: OpenAI adapter | ✅ Done | chat_completion, chat_completion_stream (SSE), embeddings, health_check |
| 13 | TASK-0026: Axum server + middleware stack | ✅ Done | CORS, body limit (10MB), trace layer, health/ready endpoints. Server verified running on :8080 |
| 14 | TASK-0027: `POST /v1/chat/completions` | ✅ Done | Mock response when OPENAI_API_KEY unset; real provider call when set. Returns OpenAI-compatible JSON with `gateway` metadata |
| 15 | TASK-0029: Request logging | ✅ Done | Stub logging (prints to console). Full DB persistence not yet implemented |
| 16 | TASK-0030: `GET /v1/models` + health endpoints | ✅ Done | Static model list with gateway metadata. `/health` and `/ready` working |

**Phase 1 exit criteria progress:**
- ✅ `docker-compose up` starts all services without errors
- ✅ OpenAI-compatible endpoint returns valid responses
- ⚠️ API key auth works (generation + validation implemented, middleware not yet wired to routes)
- ❌ Request logs stored in PostgreSQL (stub only)

### Phase 2: Core Gateway — IN PROGRESS

| Order | Task | Status | Blockers |
|-------|------|--------|----------|
| 1 | TASK-0041: Sliding window rate limiter (Redis Lua) | 🔄 Next | None |
| 2 | TASK-0042: Quota engine + budget caps | ⏳ Pending | TASK-0041 |
| 3 | TASK-0044: Rate limiting + quota integration | ⏳ Pending | TASK-0041, TASK-0042 |
| 4 | TASK-0022: Anthropic, Gemini, Ollama adapters | ⏳ Pending | None (parallelizable) |
| 5 | TASK-0028: SSE streaming | ⏳ Pending | None (parallelizable) |
| 6 | TASK-0036: Cache key builder + rules | ⏳ Pending | None (parallelizable) |
| 7 | TASK-0037: L1 in-process cache (moka) | ⏳ Pending | None (parallelizable) |
| 8 | TASK-0038: L2 Redis cache + two-tier integration | ⏳ Pending | TASK-0036, TASK-0037 |
| 9 | TASK-0039: Cache integration into orchestrator | ⏳ Pending | TASK-0038 |
| 10 | TASK-0031: Routing rule data model | ⏳ Pending | None |
| 11 | TASK-0032: Rule evaluation engine | ⏳ Pending | TASK-0031 |
| 12 | TASK-0033: Routing engine integration | ⏳ Pending | TASK-0032 |
| 13 | TASK-0064: Retry logic + exponential backoff | ⏳ Pending | None |
| 14 | TASK-0065: Circuit breaker | ⏳ Pending | TASK-0064 |
| 15 | TASK-0066: Request cancellation + fallback chain | ⏳ Pending | TASK-0065 |
| 16 | TASK-0067: Health check background worker | ⏳ Pending | None |

### Phase 3: Dashboard & Polish — NOT STARTED

See original plan in section 4 of archived handoff. Tasks TASK-0046 through TASK-0100 remain unstarted.

### Critical Path (Updated)

```
TASK-0001 → TASK-0006 → TASK-0007 → TASK-0008 → TASK-0009 → TASK-0010 → TASK-0011
    → TASK-0012 → TASK-0013 → TASK-0014 → TASK-0015 → TASK-0019
    → TASK-0020 → TASK-0021
    → TASK-0026 → TASK-0027 → TASK-0029 → TASK-0030  ✅ COMPLETED TO HERE
    → TASK-0041 → TASK-0042 → TASK-0044
    → TASK-0046 → TASK-0047 → TASK-0048 → TASK-0051 → TASK-0055
```

**Next task on critical path:** TASK-0041 (Rate limiter)

### What Can Run in Parallel

- **Frontend dashboard** (Epic-09 tasks from TASK-0046 onward) can start once TASK-0026 (Axum server) and TASK-0013 (JWT sessions) are done ✅
- **Provider adapters** (TASK-0022 Anthropic/Gemini/Ollama) can be built in parallel after TASK-0021 (OpenAI adapter) ✅
- **Caching layer** (Epic-07) can be built in parallel with routing engine (Epic-06) ✅
- **Observability** (Epic-16) can be built in parallel with most other work ✅
- **Security hardening** (Epic-17) should be ongoing but Epic-17 tasks are parallelizable after core middleware exists ✅

---

## 5. Major Risks

### Top 10 Risks

| Rank | Risk | Severity | Status | Mitigation | Owner |
|------|------|----------|--------|------------|-------|
| 1 | Budget cap fails to stop spending → financial loss for customers | Critical | Open | Pre-request cost estimation + post-request atomic deduction. 100% test coverage for quota edge cases. See ADR-004. | Backend Lead |
| 2 | Cross-tenant data leak | Critical | Mitigated | 6-layer isolation (auth, API gateway, app, DB RLS, cache prefix, logs). See ADR-005, SECURITY.md. Tenant middleware exists but not wired to routes yet. | Security Lead |
| 3 | Rust async ecosystem complexity (sqlx, Axum) slows early dev | High | Mitigated | Patterns established. Monolith, not microservices. sqlx compile-time checked queries working. See TECH_STACK.md. | Backend Lead |
| 4 | Cost calculation disputes erode customer trust | High | Open | Unit test every model's pricing. Validate against actual provider invoices. Expose formula in docs. | Backend Lead |
| 5 | Semantic cache false positives → wrong answers | High | Open | Conservative threshold (0.92). Default semantic cache OFF. Expose tuning UI. See ADR-002. | Backend Lead |
| 6 | Competitor (OpenRouter, LiteLLM) matches our differentiator | Medium | Open | Deployment simplicity + self-hosting are architectural moats. Data compounding strengthens over time. See VISION.md. | Product Lead |
| 7 | Redis memory exhaustion on single VPS | Medium | Open | `maxmemory-policy allkeys-lru`. Per-model TTL defaults. Cache size monitoring. See CACHE.md. | Platform Lead |
| 8 | Solo founder bandwidth constraint | High | Open | Monolith understood in <1 day. PostgreSQL + Redis only. No distributed systems. Architecture doc <1 day read. | CTO |
| 9 | Streaming SSE implementation has chunk delivery issues | Medium | Open | Use Axum's SSE type. Propagate cancellation. Test with real chat UIs. See API_SPEC.md. | Backend Lead |
| 10 | Frontend complexity exceeds small-team capacity | Medium | Open | shadcn/ui components. Copy-paste customization. Defer custom visualizations. See EPICS.md Epic-09. | Frontend Lead |

### Security Risks (Top 5)

| ID | Risk | Threat | Status | Mitigation |
|----|------|--------|--------|------------|
| T-004 | Cross-tenant data breach | Missing `WHERE org_id` clause | Mitigated | 6-layer isolation. Code review: grep for queries without org_id. Tenant middleware exists but not yet applied to API routes. See SECURITY.md |
| T-009 | Authentication bypass | JWT alg:none attack, weak signing | Mitigated | RS256 only. Reject all other algorithms. See AUTH.md, ADR-003 |
| T-007 | Financial destruction from budget failure | Runaway script, misconfigured client | Open | Hard budget caps with pre-request check. 429 at budget limit. Rate limiter (TASK-0041) is prerequisite. See ADR-004 |
| T-003 | API key exposure | Database breach reveals plaintext keys | Mitigated | SHA-256 hash only — no plaintext storage. Keys shown once at creation. See AUTH.md |
| T-005 | SSRF to internal network | Malicious provider URL | Open | URL whitelist. IP blocklist. DNS resolution before request. See SECURITY.md |

### Technical Risks (Top 5)

| ID | Risk | Status | Mitigation |
|----|------|--------|------------|
| R-001 | Rust compile times slow iteration | Mitigated | `sccache` in CI. Split crates. See EPIC-01 |
| R-002 | Provider API format drift | Mitigated | Isolated adapter modules. Comprehensive test fixtures. See ADR-001 |
| R-003 | Rate limiter race conditions | Open | Redis Lua scripts for atomic operations. See ADR-004 |
| R-004 | Migration ordering errors | Mitigated | Each migration in own transaction. Test against fresh DB in CI. See EPIC-02 |
| R-005 | Large request log tables slow dashboard | Open | Server-side pagination. `usage_records` aggregates. Background materialized views. See EPIC-09 |

### Business Risks (Top 3)

| ID | Risk | Mitigation |
|----|------|------------|
| B-001 | OpenRouter adds cost optimization | Differentiate on deployment simplicity + self-hosting. OpenRouter cannot offer data residency. See VISION.md |
| B-002 | LiteLLM improves deployment experience | Rust performance advantage. Single-command deployment moat vs Python. See VISION.md |
| B-003 | Free-to-paid conversion <2% | CE must be genuinely useful. Premium features are operational (SSO, audit, analytics) — not developer features. See MONETIZATION.md |

---

## 6. Success Criteria

### MVP Success Criteria (Month 3)

- [x] `docker-compose up` starts all services without errors (100% success rate)
- [x] First proxied request succeeds with p95 latency overhead <100ms
- [x] API key auth rejects invalid keys, accepts valid keys (generation + validation done; middleware wiring pending)
- [ ] Request logs stored in PostgreSQL with full metadata (zero data loss) — stub only
- [ ] Routes to OpenAI, Anthropic, Gemini, Ollama with OpenAI-compatible responses — OpenAI only
- [ ] Cache hit rate on repeated identical requests >5% — not implemented
- [ ] Budget cap enforcement: zero overspend events when cap configured — not implemented
- [ ] 100+ GitHub stars within 90 days of public release — marketing

### V1 Success Criteria (Month 6)

- [ ] Semantic cache hit rate >15% of total requests
- [ ] Average cost reduction across beta deployments >30%
- [ ] Zero budget overspend events (100% enforcement accuracy)
- [ ] First 10 paying Professional customers
- [ ] PMF survey: 40%+ of CE users "very disappointed" if product disappeared
- [ ] Dashboard loads all pages in <2 seconds (p95)
- [ ] SAML SSO works with Okta, Azure AD, Google Workspace

### Production Readiness Checklist

- [x] Security controls: TLS 1.3 (Caddy), RBAC (4 roles), tenant isolation (middleware exists), API key hashing (SHA-256), input validation (10MB limit), CORS (Any origin in dev), CSRF (not yet)
- [ ] Secrets: Docker Secrets only, never env vars. AES-256-GCM for provider API keys — currently env vars
- [ ] Performance: Gateway overhead <5ms (p99), single VPS handles 1000 req/s — not measured
- [ ] Reliability: Circuit breaker + fallback working. Health checks every 30s. Zero-downtime deploy — not implemented
- [x] Observability: Structured JSON logging (`tracing`), health/ready endpoints. Prometheus `/metrics` — not yet
- [ ] Testing: Unit coverage >70% for gateway-core. Integration tests for each provider. E2E tests pass — partial
- [ ] Deployment: Production Dockerfile. Docker Compose with all services. README with 10-min guide — dev only
- [x] Database: All migrations reversible. RLS policies active. Indexes on hot-path queries
- [ ] Cache: Redis `maxmemory-policy allkeys-lru`. Per-model TTL configured. Tenant isolation verified — not implemented
- [x] Auth: Argon2id password hashing. RS256 JWT. Rate limiting on login — not yet

---

## 7. Document Map

### Core Documents

| Document | Purpose | When to Read |
|----------|---------|-------------|
| `docs/VISION.md` | Strategic positioning, 1-year/3-year vision, competitive dynamics | Before making product decisions |
| `docs/PRODUCT.md` | Feature spec (P0/P1/P2/P3), personas, value props, anti-goals | Before implementing features |
| `docs/MARKET.md` | TAM/SAM/SOM, buyer personas, market trends, pricing landscape | When evaluating market opportunities |
| `docs/ARCHITECTURE.md` | Full architecture: principles, components, request lifecycle, data flows | When designing or modifying system components |
| `docs/TECH_STACK.md` | Technology choices with rationale and rejected alternatives | When evaluating dependencies or adding tools |
| `docs/API_SPEC.md` | OpenAI-compatible API spec, admin API, error formats, SSE streaming | When adding or modifying endpoints |
| `docs/DATABASE.md` | Complete schema, indexes, RLS policies, naming conventions | When making database changes |
| `docs/AUTH.md` | Dual auth system spec: API keys, JWT sessions, RBAC, tenant isolation | When modifying auth or authorization |
| `docs/CACHE.md` | Two-tier caching: exact-match, semantic, TTL, invalidation, PII detection | When modifying caching behavior |
| `docs/SECURITY.md` | Threat model, defense layers, audit logging, security checklist | Before shipping. Periodically for security review |
| `docs/ROADMAP.md` | 5-phase roadmap with deliverables, success criteria, metrics | For sprint planning and prioritization |
| `docs/EPICS.md` | 20 epics with acceptance criteria, technical scope, risks | When starting a new epic |
| `docs/COMPETITORS.md` | 6-competitor deep dive with feature/pricing/deployment matrices | When positioning against competitors |
| `docs/MONETIZATION.md` | Pricing tiers, packaging, conversion strategy, LTV/CAC targets | When making pricing or packaging decisions |

### ADR Documents

| Document | Decision | When to Read |
|----------|----------|-------------|
| `docs/adr/ADR-001-provider-abstraction.md` | Provider trait + OpenAI canonical format | Adding a new provider |
| `docs/adr/ADR-002-cache-strategy.md` | L1 moka + L2 Redis, exact + semantic | Changing caching behavior |
| `docs/adr/ADR-003-authentication.md` | Dual auth: API keys + JWT sessions | Modifying auth systems |
| `docs/adr/ADR-004-rate-limiting.md` | Token bucket + sliding window (Redis Lua) | Changing rate limiting |
| `docs/adr/ADR-005-tenant-model.md` | org_id + RLS for multi-tenancy | Changing tenant isolation |
| `docs/adr/ADR-006-observability.md` | Built-in logging + metrics, no external SaaS | Adding observability features |
| `docs/adr/ADR-007-fallback-strategy.md` | Circuit breaker + health checks + fallback chains | Modifying provider failover |
| `docs/adr/ADR-008-ollama-support.md` | Ollama as first-class Provider trait impl | Modifying local model support |

### Task Documents

| Document | Purpose | When to Read |
|----------|---------|-------------|
| `tasks/INDEX.md` | All 100 tasks with epic assignments, priorities, dependencies | For sprint planning |
| `tasks/TASK-XXXX.md` | Individual task specs (referenced from INDEX.md) | When picking up a task |

### Quick Reference by Topic

| Topic | Documents to Read |
|-------|------------------|
| **Starting a new epic** | `EPICS.md`, relevant ADR, `ARCHITECTURE.md` section |
| **Adding a feature** | `PRODUCT.md` (check priority), `API_SPEC.md`, relevant ADR |
| **Database changes** | `DATABASE.md`, `ARCHITECTURE.md` §3.2.7 (gateway-db crate) |
| **Security review** | `SECURITY.md`, `AUTH.md`, relevant ADR |
| **Deployment** | `TECH_STACK.md` §3-4, `ROADMAP.md` Phase 3 |
| **Caching** | `CACHE.md`, `ADR-002-cache-strategy.md` |
| **Authentication** | `AUTH.md`, `ADR-003-authentication.md` |
| **Rate limiting** | `ADR-004-rate-limiting.md`, `API_SPEC.md` §5 |
| **Observability** | `ADR-006-observability.md`, `SECURITY.md` §6 |
| **Adding a provider** | `ADR-001-provider-abstraction.md`, `ADR-008-ollama-support.md` |
| **Provider failover** | `ADR-007-fallback-strategy.md`, `ARCHITECTURE.md` §4.1 Steps 6-8 |
| **Tenant isolation** | `ADR-005-tenant-model.md`, `SECURITY.md` §2.5 |
| **Frontend dashboard** | `EPICS.md` Epic-09 through Epic-12, `TECH_STACK.md` §1.2 |
| **Tasks** | `tasks/INDEX.md`, then specific `TASK-XXXX.md` files |

---

## 8. Implementation Notes (Current Swarm)

This section captures decisions, workarounds, and gotchas from the current implementation swarm. **Read this before picking up any task.**

### 8.1 API Key Format

**Decision:** Changed from `gk_live_*` to `sk_gw_*` to match OpenAI conventions and improve developer experience.

- Format: `sk_gw_{32_base58_chars}{6_base58_checksum}` = 44 chars total
- Random part: 24 bytes → base58 → truncated to 32 chars
- Checksum: CRC32C of the base58 random-part **string** (not raw bytes), then base58 → truncated to 6 chars
- Why string-based checksum: handles truncation edge cases consistently
- Stored hash: SHA-256 hex of full key string
- Prefix (shown in UI): first 8 chars of full key

### 8.2 Circular Dependency Resolution

**Problem:** `gateway-core` originally depended on `gateway-providers` for types, but `gateway-providers` needed `gateway-core` for error types.

**Resolution:** Canonical OpenAI-compatible types moved to `gateway-core`. `gateway-providers` depends on `gateway-core` for types and errors. `gateway-api` depends on both. This is the correct direction: core owns the contracts, providers implement them.

### 8.3 Partitioned Table Primary Keys

**Problem:** PostgreSQL requires partition columns in PRIMARY KEY / UNIQUE constraints. Parent tables cannot have PKs that don't include the partition key.

**Resolution:** Removed `PRIMARY KEY` from partitioned parent tables: `requests`, `responses`, `usage_records`, `webhook_deliveries`, `audit_log`. Child partition tables have implicit `id` uniqueness via the partition bounds. The `responses.request_id` foreign key to `requests(id)` was also removed — enforce at application level if needed.

### 8.4 Migration Format (sqlx 0.9)

**Critical:** sqlx 0.9 uses separate `.up.sql` and `.down.sql` files. Do NOT use the old combined format with `--! down` separator comments. Each migration needs:
```
migrations/YYYYMMDDHHMMSS_description.up.sql
migrations/YYYYMMDDHHMMSS_description.down.sql
```

### 8.5 RLS as Defense-in-Depth

Application queries MUST include `WHERE org_id = $1`. RLS policies are a safety net, not a substitute for application-level filtering. The connection pool sets `app.org_id` on every connection via `after_connect`, but this is for the default/superuser path only. Per-request queries must pass the actual `org_id`.

### 8.6 Server Middleware Stack Order

Current order (outer → inner):
1. CORS (`CorsLayer::new().allow_origin(Any)`)
2. Body limit (`RequestBodyLimitLayer::new(10 * 1024 * 1024)`)
3. Trace (`TraceLayer::new_for_http()`)

**Note:** Auth middleware is NOT yet applied to API routes. When wiring it, place it AFTER trace/body-limit but BEFORE route handlers. Tenant isolation middleware should be applied per-route or as a layer on the API route group.

### 8.7 Mock vs Real Provider Behavior

The chat completions endpoint checks `OPENAI_API_KEY` env var:
- **Unset/empty** → returns mock response with `gateway.provider = "mock"`
- **Set** → creates OpenAI provider, makes real HTTP request, enriches response with `gateway.latency_ms`

This allows development without API keys. For integration testing, the user has offered to provide a local API server.

### 8.8 Known Gaps (Must Fix Before Phase 1 Complete)

1. **Auth middleware not wired to API routes** — `AuthContext` exists, validation stub exists, but no `Authorization` header checking on `/v1/chat/completions` or `/v1/models`
2. **Request logging is console-only** — no DB persistence to `requests` / `responses` / `usage_records` tables
3. **Rate limiter not implemented** — no Redis Lua scripts, no middleware
4. **Quota engine not implemented** — no budget cap enforcement
5. **Tenant isolation middleware not applied** — exists but not wired to routes

---

## 9. Rules for Implementation Swarms

1. **Never reference the original specification** — all content has been improved and superseded by this knowledge base
2. **Challenge any decision that seems wrong** — ADRs document why decisions were made; context may have changed
3. **Every feature must earn its place** — check `PRODUCT.md` for priority (P0 = must have, P1 = differentiator, P2 = growth, P3 = future)
4. **Security is not optional** — check `SECURITY.md` checklist before shipping any feature
5. **Simplicity over cleverness** — when in doubt, choose the simpler approach. Boring technology is a strategic advantage
6. **If a document contradicts another, this handoff document takes precedence** — but flag the inconsistency
7. **Update documents when decisions change** — keep the knowledge base living. Stale docs are worse than no docs
8. **Update this handoff document after every milestone** — this is the landing page for the next swarm

---

*Document version: 2.0*
*Generated from: VISION.md, PRODUCT.md, MARKET.md, ARCHITECTURE.md, TECH_STACK.md, API_SPEC.md, DATABASE.md, AUTH.md, CACHE.md, SECURITY.md, ROADMAP.md, EPICS.md, COMPETITORS.md, MONETIZATION.md, 8 ADRs, tasks/INDEX.md*
*Last updated: 2026-05-31*
*Current swarm: Phase 1 complete, Phase 2 starting (TASK-0041 next)*
