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
2. **Authenticate** — Validate API key (`gk_live_*` format), SHA-256 hash lookup, load org context
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
| [ADR-003](adr/ADR-003-authentication.md) | Dual Auth Systems | System A: API keys (`gk_live_*`, SHA-256 hashed, Redis-cached). System B: JWT sessions (RS256, httpOnly cookies, 15-min expiry). | API key auth adds <1ms overhead. Self-contained deployment with no external IdP dependency. |
| [ADR-004](adr/ADR-004-rate-limiting.md) | Token Bucket + Sliding Window | Redis-backed token bucket for req/s + sliding window for tokens/min. 6 layers (global, org, key, tokens, provider, IP). Lua scripts for atomicity. | Prevents runaway costs from misconfigured clients. Fail-closed: if Redis down, requests blocked (configurable). |
| [ADR-005](adr/ADR-005-tenant-model.md) | Organization-Based Tenancy | `org_id` column on every tenant-scoped table. Application-level `WHERE` clauses + PostgreSQL RLS as defense-in-depth. Cache keys prefixed with tenant ID. | Cross-tenant data leakage would be catastrophic. Dual-layer isolation (app + RLS) means two failures required for breach. |
| [ADR-006](adr/ADR-006-observability.md) | Built-In Observability | Structured JSON logging (`tracing`), Prometheus `/metrics` endpoint, built-in dashboard analytics. No external SaaS dependency. | Single-node system doesn't need distributed tracing. `request_id` correlation sufficient. Zero external dependencies. |
| [ADR-007](adr/ADR-007-fallback-strategy.md) | Circuit Breaker + Fallback | 3-state circuit breaker (CLOSED → OPEN → HALF_OPEN). Health checks every 30s. Configurable fallback chains. Retry 2x with exp backoff. | Provider outages must not cause customer downtime. Automatic recovery without human intervention. |
| [ADR-008](adr/ADR-008-ollama-support.md) | Ollama as First-Class Provider | Implements `Provider` trait. Configurable base URL (default `localhost:11434`). 300s timeout, 20-conn pool. No caching (local inference is "free"). | Data privacy + zero API cost for capable workloads. Gateway is client of Ollama, not its operator. |

---

## 4. Implementation Order

### Phase 1: Foundation (Weeks 1-4)

| Order | Task | Epic | What It Unlocks |
|-------|------|------|-----------------|
| 1 | TASK-0001: Initialize Rust workspace | Epic-01 | Everything |
| 2 | TASK-0002: Docker Compose dev environment | Epic-01 | Team can run stack locally |
| 3 | TASK-0006: Migration framework + connection pool | Epic-02 | Database access |
| 4 | TASK-0007 through TASK-0011: All 22 migrations | Epic-02 | Full schema |
| 5 | TASK-0012: Password hashing + registration | Epic-03 | User accounts |
| 6 | TASK-0014: API key generation + storage | Epic-03 | API authentication |
| 7 | TASK-0013: JWT session auth | Epic-03 | Dashboard login |
| 8 | TASK-0015: API key validation middleware | Epic-03 | Request auth pipeline |
| 9 | TASK-0016: RBAC permission system | Epic-03 | Authorization |
| 10 | TASK-0019: Tenant isolation enforcement | Epic-03 | Security boundary |
| 11 | TASK-0020: Provider trait + canonical types | Epic-04 | Multi-provider support |
| 12 | TASK-0021: OpenAI adapter | Epic-04 | First provider working |
| 13 | TASK-0026: Axum server + middleware stack | Epic-05 | HTTP server |
| 14 | TASK-0027: `POST /v1/chat/completions` | Epic-05 | Core API endpoint |
| 15 | TASK-0029: Request logging | Epic-05 | Observability |
| 16 | TASK-0030: `GET /v1/models` + health endpoints | Epic-05 | Discovery + monitoring |

**Phase 1 exit criteria:** `docker-compose up` starts all services. OpenAI-compatible endpoint proxies requests. API key auth works. Request logs stored. New engineer runs gateway in <30 min from `git clone`.

### Phase 2: Core Gateway (Weeks 5-8)

| Order | Task | Epic | What It Unlocks |
|-------|------|------|-----------------|
| 1 | TASK-0022: Anthropic, Gemini, Ollama adapters | Epic-04 | Multi-provider routing |
| 2 | TASK-0028: SSE streaming | Epic-05 | Chat UI support |
| 3 | TASK-0036: Cache key builder + rules | Epic-07 | Caching foundation |
| 4 | TASK-0037: L1 in-process cache (moka) | Epic-07 | Sub-ms cache lookups |
| 5 | TASK-0038: L2 Redis cache + two-tier integration | Epic-07 | Shared cache |
| 6 | TASK-0039: Cache integration into orchestrator | Epic-07 | Cost reduction active |
| 7 | TASK-0031: Routing rule data model | Epic-06 | Rule-based routing |
| 8 | TASK-0032: Rule evaluation engine | Epic-06 | Provider selection logic |
| 9 | TASK-0033: Routing engine integration | Epic-06 | Intelligent routing |
| 10 | TASK-0041: Sliding window rate limiter (Redis Lua) | Epic-08 | Rate limiting |
| 11 | TASK-0042: Quota engine + budget caps | Epic-08 | Hard budget enforcement |
| 12 | TASK-0044: Rate limiting + quota integration | Epic-08 | Complete usage control |
| 13 | TASK-0064: Retry logic + exponential backoff | Epic-13 | Reliability |
| 14 | TASK-0065: Circuit breaker | Epic-13 | Auto-failover |
| 15 | TASK-0066: Request cancellation + fallback chain | Epic-13 | Full fallback |
| 16 | TASK-0067: Health check background worker | Epic-13 | Provider health monitoring |

**Phase 2 exit criteria:** Routes to 3+ providers. Cache hit rate >5%. Budget caps 100% accurate (zero overspend). Streaming latency overhead <5ms. Provider failover <3 retries.

### Phase 3: Dashboard & Polish (Weeks 9-12)

| Order | Task | Epic |
|-------|------|------|
| 1 | TASK-0046: React + Vite + shadcn/ui setup | Epic-09 |
| 2 | TASK-0047: API client, auth hooks, login page | Epic-09 |
| 3 | TASK-0048: Dashboard layout + sidebar | Epic-09 |
| 4 | TASK-0055: Serve dashboard as static files | Epic-09 |
| 5 | TASK-0051: Dashboard overview with KPI cards | Epic-09 |
| 6 | TASK-0052: Provider list with health status | Epic-10 |
| 7 | TASK-0053: Add/edit provider wizard | Epic-10 |
| 8 | TASK-0056: API key list page | Epic-11 |
| 9 | TASK-0057: API key creation (show once) | Epic-11 |
| 10 | TASK-0058: Key revocation + edit | Epic-11 |
| 11 | TASK-0060: Cost dashboard with charts | Epic-12 |
| 12 | TASK-0063: Budget configuration + alert UI | Epic-12 |
| 13 | TASK-0087: Production Dockerfile + Docker Compose | Epic-18 |
| 14 | TASK-0089: Zero-downtime deployment | Epic-18 |
| 15 | TASK-0099: End-to-end integration tests | Cross-Cutting |
| 16 | TASK-0100: Documentation + release checklist | Cross-Cutting |

**Phase 3 exit criteria:** Non-technical user deploys in <10 min (timed test). Dashboard loads in <2s. All config editable via UI. 100 GitHub stars within 90 days.

### Phase 4: Enterprise Ready (Months 4-6)

| Order | Task | Epic |
|-------|------|------|
| 1 | TASK-0068: Semantic caching (pgvector) | Epic-14 |
| 2 | TASK-0069: Semantic cache integration | Epic-14 |
| 3 | TASK-0092: Cost-optimized routing | Epic-19 |
| 4 | TASK-0093: Latency-based routing | Epic-19 |
| 5 | TASK-0072: Webhook CRUD + delivery | Epic-15 |
| 6 | TASK-0073: Webhook event publisher + retry | Epic-15 |
| 7 | TASK-0074: Budget alert webhooks | Epic-15 |
| 8 | TASK-0095: Multi-organization support | Epic-20 |
| 9 | TASK-0084: Audit log system | Epic-17 |
| 10 | TASK-0096: SAML 2.0 + OIDC SSO | Epic-20 |
| 11 | TASK-0098: Audit log dashboard | Epic-20 |

**Phase 4 exit criteria:** Semantic cache hit rate >15%. Average cost reduction >30%. Zero overspend events. First 10 paying customers. PMF survey: 40%+ "very disappointed" if product disappeared.

### Critical Path

```
TASK-0001 → TASK-0006 → TASK-0007 → TASK-0008 → TASK-0009 → TASK-0010 → TASK-0011
    → TASK-0012 → TASK-0013 → TASK-0014 → TASK-0015 → TASK-0019
    → TASK-0020 → TASK-0021 → TASK-0023 → TASK-0024
    → TASK-0026 → TASK-0027 → TASK-0029 → TASK-0030
    → TASK-0041 → TASK-0042 → TASK-0044
    → TASK-0046 → TASK-0047 → TASK-0048 → TASK-0051 → TASK-0055
```

If any task on this path is delayed, the MVP ship date moves.

### What Can Run in Parallel

- **Frontend dashboard** (Epic-09 tasks from TASK-0046 onward) can start once TASK-0026 (Axum server) and TASK-0013 (JWT sessions) are done
- **Provider adapters** (TASK-0022 Anthropic/Gemini/Ollama) can be built in parallel after TASK-0021 (OpenAI adapter)
- **Caching layer** (Epic-07) can be built in parallel with routing engine (Epic-06)
- **Observability** (Epic-16) can be built in parallel with most other work
- **Security hardening** (Epic-17) should be ongoing but Epic-17 tasks are parallelizable after core middleware exists

---

## 5. Major Risks

### Top 10 Risks

| Rank | Risk | Severity | Mitigation | Owner |
|------|------|----------|------------|-------|
| 1 | Budget cap fails to stop spending → financial loss for customers | Critical | Pre-request cost estimation + post-request atomic deduction. 100% test coverage for quota edge cases. See ADR-004. | Backend Lead |
| 2 | Cross-tenant data leak | Critical | 6-layer isolation (auth, API gateway, app, DB RLS, cache prefix, logs). See ADR-005, SECURITY.md. | Security Lead |
| 3 | Rust async ecosystem complexity (sqlx, Axum) slows early dev | High | Use well-documented crates. Keep patterns simple. Monolith, not microservices. See TECH_STACK.md. | Backend Lead |
| 4 | Cost calculation disputes erode customer trust | High | Unit test every model's pricing. Validate against actual provider invoices. Expose formula in docs. | Backend Lead |
| 5 | Semantic cache false positives → wrong answers | High | Conservative threshold (0.92). Default semantic cache OFF. Expose tuning UI. See ADR-002. | Backend Lead |
| 6 | Competitor (OpenRouter, LiteLLM) matches our differentiator | Medium | Deployment simplicity + self-hosting are architectural moats. Data compounding strengthens over time. See VISION.md. | Product Lead |
| 7 | Redis memory exhaustion on single VPS | Medium | `maxmemory-policy allkeys-lru`. Per-model TTL defaults. Cache size monitoring. See CACHE.md. | Platform Lead |
| 8 | Solo founder bandwidth constraint | High | Monolith understood in <1 day. PostgreSQL + Redis only. No distributed systems. Architecture doc <1 day read. | CTO |
| 9 | Streaming SSE implementation has chunk delivery issues | Medium | Use Axum's SSE type. Propagate cancellation. Test with real chat UIs. See API_SPEC.md. | Backend Lead |
| 10 | Frontend complexity exceeds small-team capacity | Medium | shadcn/ui components. Copy-paste customization. Defer custom visualizations. See EPICS.md Epic-09. | Frontend Lead |

### Security Risks (Top 5)

| ID | Risk | Threat | Mitigation |
|----|------|--------|------------|
| T-004 | Cross-tenant data breach | Missing `WHERE org_id` clause | 6-layer isolation. Code review: grep for queries without org_id. See SECURITY.md |
| T-009 | Authentication bypass | JWT alg:none attack, weak signing | RS256 only. Reject all other algorithms. See AUTH.md, ADR-003 |
| T-007 | Financial destruction from budget failure | Runaway script, misconfigured client | Hard budget caps with pre-request check. 429 at budget limit. See ADR-004 |
| T-003 | API key exposure | Database breach reveals plaintext keys | SHA-256 hash only — no plaintext storage. Keys shown once at creation. See AUTH.md |
| T-005 | SSRF to internal network | Malicious provider URL | URL whitelist. IP blocklist. DNS resolution before request. See SECURITY.md |

### Technical Risks (Top 5)

| ID | Risk | Mitigation |
|----|------|------------|
| R-001 | Rust compile times slow iteration | `sccache` in CI. Split crates. See EPIC-01 |
| R-002 | Provider API format drift | Isolated adapter modules. Comprehensive test fixtures. See ADR-001 |
| R-003 | Rate limiter race conditions | Redis Lua scripts for atomic operations. See ADR-004 |
| R-004 | Migration ordering errors | Each migration in own transaction. Test against fresh DB in CI. See EPIC-02 |
| R-005 | Large request log tables slow dashboard | Server-side pagination. `usage_records` aggregates. Background materialized views. See EPIC-09 |

### Business Risks (Top 3)

| ID | Risk | Mitigation |
|----|------|------------|
| B-001 | OpenRouter adds cost optimization | Differentiate on deployment simplicity + self-hosting. OpenRouter cannot offer data residency. See VISION.md |
| B-002 | LiteLLM improves deployment experience | Rust performance advantage. Single-command deployment moat vs Python. See VISION.md |
| B-003 | Free-to-paid conversion <2% | CE must be genuinely useful. Premium features are operational (SSO, audit, analytics) — not developer features. See MONETIZATION.md |

---

## 6. Success Criteria

### MVP Success Criteria (Month 3)

- [ ] `docker-compose up` starts all services without errors (100% success rate)
- [ ] First proxied request succeeds with p95 latency overhead <100ms
- [ ] API key auth rejects invalid keys, accepts valid keys (100% accuracy)
- [ ] Request logs stored in PostgreSQL with full metadata (zero data loss)
- [ ] Routes to OpenAI, Anthropic, Gemini, Ollama with OpenAI-compatible responses
- [ ] Cache hit rate on repeated identical requests >5%
- [ ] Budget cap enforcement: zero overspend events when cap configured
- [ ] 100+ GitHub stars within 90 days of public release

### V1 Success Criteria (Month 6)

- [ ] Semantic cache hit rate >15% of total requests
- [ ] Average cost reduction across beta deployments >30%
- [ ] Zero budget overspend events (100% enforcement accuracy)
- [ ] First 10 paying Professional customers
- [ ] PMF survey: 40%+ of CE users "very disappointed" if product disappeared
- [ ] Dashboard loads all pages in <2 seconds (p95)
- [ ] SAML SSO works with Okta, Azure AD, Google Workspace

### Production Readiness Checklist

- [ ] Security controls: TLS 1.3, RBAC, tenant isolation, API key hashing, input validation, CORS, CSRF
- [ ] Secrets: Docker Secrets only, never env vars. AES-256-GCM for provider API keys
- [ ] Performance: Gateway overhead <5ms (p99), single VPS handles 1000 req/s
- [ ] Reliability: Circuit breaker + fallback working. Health checks every 30s. Zero-downtime deploy
- [ ] Observability: Structured JSON logging, Prometheus `/metrics`, health/ready endpoints
- [ ] Testing: Unit coverage >70% for gateway-core. Integration tests for each provider. E2E tests pass
- [ ] Deployment: Production Dockerfile. Docker Compose with all services. README with 10-min guide
- [ ] Database: All migrations reversible. RLS policies active. Indexes on hot-path queries
- [ ] Cache: Redis `maxmemory-policy allkeys-lru`. Per-model TTL configured. Tenant isolation verified
- [ ] Auth: Argon2id password hashing. RS256 JWT. httpOnly cookies. Rate limiting on login

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

## 8. Rules for Implementation Swarms

1. **Never reference the original specification** — all content has been improved and superseded by this knowledge base
2. **Challenge any decision that seems wrong** — ADRs document why decisions were made; context may have changed
3. **Every feature must earn its place** — check `PRODUCT.md` for priority (P0 = must have, P1 = differentiator, P2 = growth, P3 = future)
4. **Security is not optional** — check `SECURITY.md` checklist before shipping any feature
5. **Simplicity over cleverness** — when in doubt, choose the simpler approach. Boring technology is a strategic advantage
6. **If a document contradicts another, this handoff document takes precedence** — but flag the inconsistency
7. **Update documents when decisions change** — keep the knowledge base living. Stale docs are worse than no docs

---

*Document version: 1.0*
*Generated from: VISION.md, PRODUCT.md, MARKET.md, ARCHITECTURE.md, TECH_STACK.md, API_SPEC.md, DATABASE.md, AUTH.md, CACHE.md, SECURITY.md, ROADMAP.md, EPICS.md, COMPETITORS.md, MONETIZATION.md, 8 ADRs, tasks/INDEX.md*
*Last updated: 2025-01-15*
