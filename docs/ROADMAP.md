# AI Gateway — Product Roadmap

> **Document Version:** 1.0
> **Last Updated:** 2025-01-15
> **Status:** Active — guides all sprint planning and epic prioritization

---

## Overview

This roadmap delivers an open-core AI Gateway that deploys in under 10 minutes on a single VPS via Docker Compose, providing intelligent request routing, semantic caching, and hard budget caps that reduce AI API spend by 30-70%. Built for cost-conscious SMEs (20-500 employees) who need multi-provider AI infrastructure without Kubernetes, DevOps teams, or enterprise budgets.

**North Star Metric:** Monthly AI Spend Avoided (dollar value of provider costs saved by all active deployments).

---

## Architecture & Tech Stack

| Layer | Technology |
|-------|-----------|
| Gateway Backend | Rust (Axum framework), modular monolith |
| Admin Dashboard | React + TypeScript, static SPA |
| Database | PostgreSQL 16 (state), Redis 7 (cache/queues) |
| Deployment | Docker Compose (single VPS) |
| Auth | API keys (System A) + JWT sessions (System B) + RBAC |

---

## Phase 1: Foundation (Weeks 1-4)

**Goal:** Infrastructure in place, first provider working, developer can `git clone && docker-compose up` and route a request.

### Key Deliverables

- **Project bootstrap** (Week 1): Monorepo structure (Rust workspace + React frontend), CI/CD pipeline (GitHub Actions), Docker Compose with PostgreSQL + Redis + gateway + dashboard, dev environment with hot reload, automated code formatting and linting
- **Database foundation** (Week 1-2): Migration framework (sqlx/refinery), all 16+ tables with indexes, row-level security policies, connection pooling, base repository layer with tenant isolation
- **Authentication core** (Week 2): API key generation (`sk-gw-*` format), SHA-256 key hashing and validation, session-based JWT auth (RS256), RBAC permission system (owner/admin/member/viewer), login/logout/password reset
- **Provider abstraction** (Week 3): `Provider` Rust trait, OpenAI adapter (request/response transform), provider config storage (encrypted API keys), health check framework, model registry with pricing
- **Request proxy — first path** (Week 4): `POST /v1/chat/completions` endpoint (OpenAI-compatible), request parsing and validation, auth middleware, single-provider proxy (OpenAI), response serialization with gateway metadata headers, request logging to PostgreSQL

### Success Criteria

| # | Criterion | Target |
|---|-----------|--------|
| 1 | `docker-compose up` starts all services without errors | 100% success rate |
| 2 | First proxied request succeeds (OpenAI-compatible endpoint) | p95 latency < 100ms overhead |
| 3 | API key auth rejects invalid keys, accepts valid keys | 100% accuracy |
| 4 | Request logs stored in PostgreSQL with full metadata | Zero data loss |
| 5 | A new engineer clones repo and runs gateway in under 30 minutes | Measured from `git clone` to first request |

### Dependencies
None — this is the foundation.

### Risk Factors

| Risk | Impact | Mitigation |
|------|--------|------------|
| Rust async ecosystem learning curve (sqlx, Axum) | Delays Week 1-2 | Use well-documented crates; keep patterns simple |
| Docker Compose networking issues on developer machines | Delays onboarding | Test on Linux/macOS/Windows; clear troubleshooting docs |
| OpenAI API spec edge cases | Incompatibility with SDKs | Test with official OpenAI Python/JS SDKs directly |

---

## Phase 2: Core Gateway (Weeks 5-8)

**Goal:** Multi-provider routing, caching, quotas, and streaming working end-to-end. Product is usable as a development gateway.

### Key Deliverables

- **Multi-provider support** (Week 5): Anthropic adapter (Claude message format), Google Gemini adapter, Ollama adapter (local models), provider selection by model name, `/v1/models` endpoint aggregating all providers
- **Streaming proxy** (Week 5): SSE stream passthrough from providers, transparent chunk forwarding, stream cancellation propagation
- **Exact-match caching** (Week 6): L1 in-process cache (moka), L2 Redis cache, SHA-256 cache key generation, cache hit/miss metrics, configurable TTL per model, cache skip for non-deterministic requests (temperature > 0, tools, streaming)
- **Routing engine** (Week 6): Rule-based routing table, priority-ordered rule evaluation, model-matching rules, strategy selection (single/fallback/weighted), health-aware provider selection
- **Quota & rate limiting** (Week 7): Redis-backed sliding window rate limiting, per-key rate limits (requests/minute, tokens/minute), per-organization budget quotas, hard budget caps with auto-cutoff, configurable warning thresholds
- **Usage tracking & cost calculation** (Week 7): Per-request cost calculation from model pricing tables, `usage_records` aggregation table, cost attribution by key/provider/model, background aggregation worker
- **Provider health checks** (Week 8): Periodic health probes to each provider, circuit breaker pattern (fail after 5 errors, 60s recovery), health status in `/v1/models`, automatic fallback to healthy providers

### Success Criteria

| # | Criterion | Target |
|---|-----------|--------|
| 1 | Gateway routes to 3+ providers (OpenAI, Anthropic, Gemini minimum) | All return OpenAI-compatible responses |
| 2 | Cache hit rate on repeated requests | > 5% (baseline) |
| 3 | Budget cap enforcement accuracy | 100% — zero overspend when cap configured |
| 4 | Streaming latency overhead | < 5ms per chunk |
| 5 | Provider failover on error | < 3 retries, fallback within 500ms |
| 6 | Quota exceeded returns clear error with `Retry-After` | Always |

### Dependencies
- Phase 1 (all epics)

### Risk Factors

| Risk | Impact | Mitigation |
|------|--------|------------|
| Provider API format drift (especially Anthropic) | Transform bugs | Isolate transforms in adapter modules; comprehensive test fixtures |
| Redis memory pressure on single VPS | Performance degradation | Monitor with `INFO memory`; set `maxmemory-policy allkeys-lru` |
| Rate limiter race conditions | Over-limit requests | Use Redis Lua scripts for atomic operations |
| Cost calculation accuracy | Customer trust erosion | Unit test every model's pricing; validate against actual provider bills |

---

## Phase 3: Dashboard & Polish (Weeks 9-12)

**Goal:** Admin UI provides full visibility and control. Product is deployable by non-technical users. Ready for public beta.

### Key Deliverables

- **Admin dashboard — foundation** (Week 9): React app served by gateway, login page, dashboard layout with navigation, organization settings page, user invitation and management, RBAC enforcement in UI
- **Request logs viewer** (Week 9): Paginated request log table, filtering by model/provider/status, request detail view (prompt/response), search by trace ID
- **Provider management UI** (Week 10): Add/edit/remove provider configurations, provider API key input (encrypted at rest), model enable/disable toggles, health status visualization, manual health check trigger
- **API key management UI** (Week 10): Create keys with name and scopes, revoke keys, set rate limits per key, key usage overview, copy key on creation (shown once)
- **Usage analytics** (Week 11): Cost dashboard (total, by provider, by model, by key), token usage charts (daily/hourly), request volume graphs, cache hit rate visualization, latency percentiles
- **Budget & alerts** (Week 11): Budget cap configuration UI, alert threshold settings (50%/75%/90%/100%), budget usage progress bar, alert history log
- **Docker Compose polish** (Week 12): Single-command `docker-compose up` with all services, environment variable documentation, SSL/TLS via reverse proxy config, health check endpoints for monitoring, automated database migrations on startup, `README.md` with 10-minute deployment guide
- **Beta preparation** (Week 12): API compatibility validation against OpenAI SDKs, load testing (target: 1000 req/s on 4 vCPU), documentation (deployment, API, configuration), issue templates for GitHub

### Success Criteria

| # | Criterion | Target |
|---|-----------|--------|
| 1 | Non-technical user deploys gateway in < 10 minutes | Measured timed test |
| 2 | Dashboard loads usage data within 2 seconds | p95 page load time |
| 3 | All configuration editable via UI (no YAML required for common tasks) | Zero file editing for basic setup |
| 4 | Cost dashboard accuracy within 1% of actual provider bills | Validated against real invoices |
| 5 | 100 GitHub stars within 90 days of public release | Community adoption metric |

### Dependencies
- Phase 1 (infrastructure)
- Phase 2 (routing, caching, quotas — dashboard needs data)

### Risk Factors

| Risk | Impact | Mitigation |
|------|--------|------------|
| Frontend complexity exceeds small-team capacity | Delayed delivery | Use shadcn/ui component library; defer custom visualizations |
| Large request log tables cause slow dashboard loads | Poor UX | Implement server-side pagination; use `usage_records` aggregates |
| SSL/TLS setup complexity | Fails 10-minute deploy promise | Provide Cloudflare Tunnel option; document Caddy reverse proxy |
| Beta user onboarding friction | Low activation | Add setup wizard in dashboard; video walkthrough |

---

## Phase 4: Enterprise Features (Months 4-6)

**Goal:** Team features, advanced routing, SSO, and deeper observability. Launch Professional tier. Validate cost reduction promise.

### Key Deliverables

- **Semantic caching** (Month 4): Local ONNX embedding model (all-MiniLM-L6-v2), embedding computation on cache miss, cosine similarity search in Redis, configurable similarity threshold (default 0.92), semantic hit tracking and metrics, expected hit rate improvement: 20-40%
- **Smart cost-aware routing** (Month 4): Complexity-based model selection, simple queries routed to cheaper models, cost estimation before routing, routing decision logging with cost differential, expected savings: 20-40% on eligible requests
- **Team / multi-user support** (Month 4): Organization member management UI, role assignment (owner/admin/member/viewer), per-team cost attribution, user activity audit log
- **Webhook events** (Month 5): Event types: quota.warning, quota.exceeded, provider.error, provider.recovered, request.failed, configurable webhook endpoints per org, HMAC-SHA256 signature verification, retry with exponential backoff, webhook delivery log
- **Budget alerts** (Month 5): Configurable alert thresholds per org, email notification support (SMTP), webhook notifications for Slack/Discord/Teams integration, alert history and status tracking
- **Import/export configuration** (Month 5): JSON export of full gateway config, LiteLLM config migration tool, bulk provider import from JSON
- **Advanced analytics** (Month 6): Cost trends over time (7/30/90 day), per-model efficiency comparison, team comparison charts, cost prediction based on usage patterns, anomaly detection (spend spikes)
- **SAML 2.0 SSO** (Month 6): SAML identity provider integration, JIT user provisioning, SSO-only authentication option, session management for SSO users
- **Audit logging** (Month 6): Immutable audit trail for all admin actions, tamper-evident storage pattern, audit log viewer in dashboard, compliance-ready export (CSV/JSON)

### Success Criteria

| # | Criterion | Target |
|---|-----------|--------|
| 1 | Semantic cache hit rate | > 15% of total requests |
| 2 | Average cost reduction across beta deployments | > 30% |
| 3 | Budget enforcement: zero overspend events | 100% accuracy |
| 4 | First 10 paying Professional customers | Revenue validation |
| 5 | PMF survey: 40%+ of CE users "very disappointed" if product disappeared | Sean Ellis test |
| 6 | SAML SSO works with major IdPs (Okta, Azure AD, Google Workspace) | Integration tested |

### Dependencies
- Phase 1 and Phase 2 (all core infrastructure)
- Phase 3 (dashboard framework for new UI features)

### Risk Factors

| Risk | Impact | Mitigation |
|------|--------|------------|
| ONNX embedding model size (~30MB) | Increases Docker image size | Lazy-load model on first use; offer download script |
| SAML complexity | Delivers late or buggy | Use `samael` crate; test against Okta + Azure AD sandbox |
| Email delivery reliability | Alerts not received | Support SMTP + SendGrid; webhook as primary alert channel |
| Semantic cache false positives | Poor user experience | Default conservative threshold (0.92); expose tuning UI |

---

## Phase 5: Scale & SaaS (Months 7-12)

**Goal:** Managed hosting option, advanced routing algorithms, semantic caching at scale. Capture mid-market expansion.

### Key Deliverables

- **Managed SaaS hosting** (Month 7-8): Multi-tenant gateway service, automated provisioning per customer, customer brings own provider API keys, usage-based pricing tier, zero-infrastructure onboarding path
- **HNSW semantic search** (Month 8): In-memory HNSW ANN index, sub-millisecond semantic search, periodic index rebuild from Redis, configurable recall vs speed tradeoff, replaces brute-force similarity at >100K cached prompts
- **Prompt compression** (Month 8): Automatic prompt compression before sending to provider, 20-40% token reduction, compression quality preservation check, per-model compression profiles
- **Request transformation / middleware** (Month 9): Pre-request transformation hooks, header injection per provider, request body modification, post-response transformation, middleware chain ordering
- **Advanced routing** (Month 9): Latency-based provider selection, quality-score routing (model capability matching), A/B test routing (percentage splits), custom routing rules engine with JSON conditions
- **Performance optimization** (Month 10): Response body streaming optimization, connection pool tuning per provider, Redis pipelining for batch cache ops, rkyv zero-copy deserialization for L1, request coalescing (deduplicate in-flight identical requests)
- **Enterprise governance** (Month 10-11): Organization hierarchy (parent/child orgs), cross-organization usage rollup, custom roles and permissions, API key scoping by model/provider/time, data residency controls
- **Ecosystem integrations** (Month 11-12): LangChain integration guide, OpenAI SDK compatibility certification, Vercel AI SDK adapter, n8n workflow node, community plugin framework
- **Scale preparation** (Month 12): Horizontal scaling guide (for large deployments), read replica support for PostgreSQL, Redis Cluster compatibility, CDN integration for dashboard assets, performance benchmarking suite

### Success Criteria

| # | Criterion | Target |
|---|-----------|--------|
| 1 | 50+ paying customers across all tiers | Revenue milestone |
| 2 | $10K MRR | Sustainability threshold |
| 3 | 1,000+ active self-hosted CE deployments | Distribution validation |
| 4 | Semantic cache hit rate (deployments with >100K requests) | > 25% |
| 5 | Gateway appears in 3+ "AI Gateway comparison" articles | Category recognition |
| 6 | Managed SaaS: < 2 min provisioning time | Operational efficiency |

### Dependencies
- All prior phases
- Phase 4 specifically: semantic caching, SSO, webhooks

### Risk Factors

| Risk | Impact | Mitigation |
|------|--------|------------|
| SaaS hosting operational burden | Solo founder overwhelmed | Automate everything; use managed DB (Supabase/Railway); no manual onboarding |
| HNSW index memory usage | OOM on small VPS | Make optional; recommend 8GB+ RAM when enabled |
| Multi-tenancy data isolation bugs | Customer trust loss | Extensive integration tests; RLS policies as defense-in-depth |
| Community growth stalls | Limited distribution | Content marketing ("deploy in 10 min" guides); Hacker News launch |

---

## Cross-Cutting Concerns

### Quality Gates (Every Phase)

| Gate | Requirement |
|------|-------------|
| **Tests** | Unit test coverage > 70% for gateway-core; integration tests for each provider adapter |
| **API compatibility** | OpenAI SDK drop-in replacement validated weekly against Python `openai` and Node `openai` packages |
| **Security** | Dependency audit (`cargo audit`); no secrets in Docker images; API keys hashed at rest |
| **Performance** | Gateway latency overhead < 5ms (p99); single VPS handles 1000 req/s |
| **Observability** | Structured JSON logging; Prometheus-compatible metrics; health/readiness endpoints |

### Anti-Goals (Never)

| Feature | Why Excluded |
|---------|-------------|
| Kubernetes-native deployment | Our differentiator is NOT requiring K8s |
| ChatGPT clone / chat UI | Infrastructure product, not end-user application |
| RAG platform / vector DB | Different category; Pinecone/Weaviate exist |
| Model training / fine-tuning | Gateway routes to providers, doesn't train |
| Workflow automation / agent framework | Competes with LangChain/n8n; scope creep |
| Multi-region cloud deployment | Enterprise-only; divert from core simplicity |

---

## Metrics Dashboard

| Phase | Timeline | Active CE Deployments | Paying Customers | MRR | Avg Cost Reduction | Monthly AI Spend Avoided |
|-------|----------|----------------------|------------------|-----|-------------------|------------------------|
| Phase 1 | Weeks 1-4 | 0 (internal) | 0 | $0 | N/A | $0 |
| Phase 2 | Weeks 5-8 | 5 (alpha) | 0 | $0 | 5-10% | $100 |
| Phase 3 | Weeks 9-12 | 50 (beta) | 0 | $0 | 10% | $1,000 |
| Phase 4 | Months 4-6 | 300 | 10 | $500 | 30% | $10,000 |
| Phase 5 | Months 7-12 | 1,000+ | 50+ | $10,000 | 40% | $100,000 |

---

## Decision Log

| Date | Decision | Rationale |
|------|----------|-----------|
| 2025-01-15 | 5-phase structure | Foundation before features; dashboard needs data; enterprise after PMF validation |
| 2025-01-15 | Semantic caching in Phase 4 (not Phase 2) | Phase 2 needs exact-match cache for baseline cost reduction; semantic is differentiator but adds complexity |
| 2025-01-15 | SaaS hosting in Phase 5 (not earlier) | Validate self-hosted model first; SaaS is expansion revenue, not core |
| 2025-01-15 | HNSW index deferred to Phase 5 | Brute-force similarity is sufficient for <100K cached prompts; most SME deployments won't hit this in Year 1 |

---

*This roadmap is a living document. Priorities shift based on customer feedback, competitive landscape, and technical learnings. Review and update monthly.*
