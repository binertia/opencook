# AI Gateway — Architecture Document

**Version:** 1.0  
**Date:** 2025-01-14  
**Classification:** Architecture Decision Record  
**Status:** Draft — Open for Review  

---

## 1. Architectural Principles

### 1.1 Prefer Boring Technology

**Statement:** Choose well-understood, battle-tested tools over novel or exotic ones. The default technology stack should be familiar to any experienced Rust engineer.

**Rationale:** A solo developer or small team cannot afford to debug interactions between bleeding-edge libraries. Boring technology has established patterns, extensive documentation, community knowledge, and fewer surprises in production.

**Applied:** Axum over custom HTTP frameworks; PostgreSQL over distributed databases; Redis over custom caching clusters; Docker Compose over Kubernetes.

---

### 1.2 Single-Node First

**Statement:** Design for a single VPS deployment. Do not introduce distributed-system patterns (consensus, event sourcing, CQRS, service mesh) until horizontal scaling is a proven requirement.

**Rationale:** Distributed systems introduce operational complexity that grows superlinearly with team size. A single-node system with vertical scaling can handle millions of requests per day. The operational surface area must fit in one person's head.

**Applied:** No Kafka, no Kubernetes, no etcd, no Consul. PostgreSQL + Redis on the same host. Async task queues use in-process channels or Redis lists, not external brokers.

---

### 1.3 Database Is the Source of Truth

**Statement:** PostgreSQL is the sole system of record. Redis is a performance layer, not a persistence layer. On restart, all application state must be reconstructable from PostgreSQL.

**Rationale:** Splitting state across multiple persistence systems creates consistency nightmares. Redis can (and will) be wiped. PostgreSQL's ACID guarantees are the foundation for quota accuracy, billing correctness, and audit trails.

**Applied:** All quota balances, API keys, provider configurations, billing records, and audit logs live in PostgreSQL. Redis holds only: (a) request cache entries, (b) rate-limiting counters, (c) ephemeral session data. Redis keys have TTLs and can be recomputed from PostgreSQL.

---

### 1.4 Explicit Over Implicit

**Statement:** All behavior should be traceable from configuration and code. No magic auto-discovery, no hidden middleware chains, no framework conventions that are not immediately obvious.

**Rationale:** When a system breaks at 2 AM, the on-call engineer (who is likely the solo developer) must understand the request path without reverse-engineering framework internals. Every routing decision, every transformation, every fallback must be explicit in code.

**Applied:** Provider selection logic is a visible function, not a plugin system. Middleware order is declared explicitly in the router setup. No procedural macros that hide business logic.

---

### 1.5 Fail Closed, Not Open

**Statement:** When a subsystem cannot make a confident decision, it defaults to the most restrictive safe state. A failed quota check blocks the request. A failed auth check returns 401. An unknown provider returns 400.

**Rationale:** The default failure mode of a gateway must be secure. An "allow by default" policy in access control or quota management leads to cost overruns and data breaches.

**Applied:** All permission checks return `deny` on error. All quota checks return `exceeded` on error. All provider health checks start as `unhealthy` until proven otherwise. Graceful degradation is an explicit code path, not an implicit fallback.

---

### 1.6 Module Boundaries Are API Boundaries

**Statement:** Internal modules communicate through well-defined Rust traits. A module can be replaced without changing other modules if it implements the same trait.

**Rationale:** This enables testing with mocks, swapping implementations (e.g., adding a new LLM provider), and understanding dependencies by reading trait definitions.

**Applied:** Every core subsystem exposes a trait: `Provider`, `Router`, `Cache`, `QuotaChecker`, `AuthValidator`. Concrete implementations live in separate crates. The main application composes them.

---

### 1.7 Operability by One Person

**Statement:** Every operational task — deploy, rollback, rotate secrets, inspect logs, scale up, restore from backup — must be achievable by one engineer in under 15 minutes with only the README and a shell.

**Rationale:** Team size is <5. There is no SRE team, no DevOps specialist, no 24/7 NOC. The system must be debuggable with standard Unix tools: `docker logs`, `psql`, `redis-cli`, `curl`.

**Applied:** Single `docker-compose.yml` for full stack. Health check endpoints on every service. Structured JSON logs to stdout. SQL migrations are plain files, not ORM-generated. All state in named volumes.

---

## 2. System Boundary Diagram

### 2.1 What Is Inside the Gateway

The gateway is a single Rust application (modular monolith) running in one Docker container, plus its directly co-located dependencies:

| Component | Purpose | Data Stored |
|-----------|---------|-------------|
| `gateway-api` (Rust binary) | HTTP server, request lifecycle | None (stateless) |
| PostgreSQL 16 | Persistent data | API keys, quotas, billing records, provider configs, audit logs, organizations, users |
| Redis 7 | Ephemeral data | Request cache, rate-limit counters, provider health status |
| `gateway-web` (React/TS) | Admin dashboard static files | None |
| Nginx (optional) | Reverse proxy, TLS termination, static file serving | None |

### 2.2 What Requests Come In

```
┌─────────────────────────────────────────────────────────────┐
│                         EXTERNAL CLIENTS                      │
├─────────────────────────────────────────────────────────────┤
│ 1. LLM API Consumers                                          │
│    Method: POST /v1/chat/completions                          │
│    Method: POST /v1/embeddings                                │
│    Method: GET  /v1/models                                    │
│    Auth: Bearer <gateway-api-key>                             │
│    Body: OpenAI-compatible JSON                               │
│                                                               │
│ 2. LLM API Consumers (Streaming)                              │
│    Method: POST /v1/chat/completions                          │
│    Header: Accept: text/event-stream                          │
│    Response: Server-Sent Events                               │
│                                                               │
│ 3. Admin Dashboard Users                                      │
│    Method: GET  /admin/* (static assets)                      │
│    Method: GET  /api/admin/* (admin API)                      │
│    Auth: Session cookie (after login)                         │
│                                                               │
│ 4. Health Check Probes                                        │
│    Method: GET /health                                        │
│    Method: GET /ready                                         │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
                    ┌─────────────────┐
                    │  Gateway (Axum) │
                    └─────────────────┘
```

### 2.3 What Calls Go Out

| Destination | Protocol | Purpose |
|-------------|----------|---------|
| OpenAI API | HTTPS + JSON | Chat completions, embeddings, model lists |
| Anthropic API | HTTPS + JSON | Claude chat completions |
| Google Gemini API | HTTPS + JSON | Gemini chat completions |
| Ollama (localhost or LAN) | HTTP + JSON | Local model inference |
| Custom OpenAI-compatible endpoints | HTTPS + JSON | Self-hosted or third-party providers |

### 2.4 What State Is Stored

**PostgreSQL (persistent):**

```
organizations          — tenant isolation boundary
users                  — dashboard users, auth credentials
api_keys               — gateway-issued keys for LLM API consumers
provider_configs       — provider credentials, base URLs, enabled models
models                 — available models with metadata (provider, cost per token)
quotas                 — per-org, per-user, per-key limits
usage_records          — every request with tokens, cost, latency, provider
billing_cycles         — monthly billing aggregation
cache_entries          — semantic cache vectors and responses (optional table)
audit_log              — admin actions, config changes
```

**Redis (ephemeral, reconstructable):**

```
cache:{hash}           — exact-match response cache (TTL: configurable)
semantic_cache:*       — semantic cache entries (TTL: configurable)
ratelimit:{key}:{window} — sliding window counters (TTL: window size)
health:{provider}      — provider health status (TTL: health check interval)
stats:{provider}:{window} — real-time provider stats (TTL: 1 hour)
```

### 2.5 What External Services Are Contacted

All external services are LLM inference providers. There are no dependencies on: identity providers (Auth0, Okta), payment processors (Stripe), observability SaaS (Datadog, Honeycomb), or cloud-specific services (AWS S3, GCP Pub/Sub).

---

## 3. High-Level Component Diagram

### 3.1 Component Overview

```
┌─────────────────────────────────────────────────────────────────────────┐
│                              NGINX (Optional)                             │
│                    TLS termination, static file serving                    │
└─────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────────┐
│                            GATEWAY-API CRATE                              │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐  ┌─────────────────┐ │
│  │   Router    │  │  Auth MW    │  │  Rate Limiter│  │  Request Logger │ │
│  │  (Axum)     │  │  (tower)    │  │  (tower)     │  │  (tower)        │ │
│  └──────┬──────┘  └──────┬──────┘  └──────┬──────┘  └─────────────────┘ │
│         │                │                │                               │
│         └────────────────┼────────────────┘                               │
│                          ▼                                               │
│  ┌─────────────────────────────────────────────────────────────────────┐ │
│  │                         Request Handler                             │ │
│  │              (validates, deserializes, dispatches)                  │ │
│  └─────────────────────────────────┬───────────────────────────────────┘ │
└────────────────────────────────────┼────────────────────────────────────┘
                                     │
                                     ▼
┌─────────────────────────────────────────────────────────────────────────┐
│                            GATEWAY-CORE CRATE                             │
│  ┌─────────────────────────────────────────────────────────────────────┐ │
│  │                    Request Lifecycle Orchestrator                    │ │
│  │                                                                     │ │
│  │  1. Parse request ──▶ 2. Authenticate ──▶ 3. Check quota          │ │
│  │                                                                     │ │
│  │  4. Check cache ──▶ 5. Select provider ──▶ 6. Transform           │ │
│  │                                                                     │ │
│  │  7. Call provider ──▶ 8. Transform response ──▶ 9. Store cache    │ │
│  │                                                                     │ │
│  │  10. Update quota ──▶ 11. Record cost ──▶ 12. Respond            │ │
│  └─────────────────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────────────────┘
        │                    │                    │
        ▼                    ▼                    ▼
┌──────────────┐  ┌─────────────────┐  ┌─────────────────────────────┐
│  GATEWAY-    │  │  GATEWAY-       │  │  GATEWAY-QUOTA CRATE         │
│  PROVIDERS   │  │  CACHE CRATE    │  │                              │
│  CRATE       │  │                 │  │  ┌─────────────────────────┐ │
│              │  │  ┌─────────────┐│  │  │ QuotaChecker trait      │ │
│  ┌─────────┐ │  │  │ ExactCache  ││  │  │ - check_quota()         │ │
│  │Provider │ │  │  │  (Redis)    ││  │  │ - deduct_quota()        │ │
│  │ trait   │ │  │  └─────────────┘││  │  │ - get_usage()           │ │
│  └────┬────┘ │  │  ┌─────────────┐││  │  └─────────────────────────┘ │
│       │      │  │  │SemanticCache│││  │                              │
│  ┌────┴────┐ │  │  │  (Redis)    │││  │  ┌─────────────────────────┐ │
│  │ OpenAI  │ │  │  └─────────────┘││  │  │ BillingRecorder trait   │ │
│  │ Adapter │ │  │                 ││  │  │ - record_usage()        │ │
│  ├─────────┤ │  └─────────────────┘│  │  │ - get_billing_summary() │ │
│  │Anthropic│ │                     │  │  └─────────────────────────┘ │
│  │ Adapter │ │                     │  └─────────────────────────────┘
│  ├─────────┤ │                     │
│  │ Gemini  │ │                     │
│  │ Adapter │ │                     │
│  ├─────────┤ │                     │
│  │ Ollama  │ │                     │
│  │ Adapter │ │                     │
│  ├─────────┤ │                     │
│  │ Custom  │ │                     │
│  │ Adapter │ │                     │
│  └─────────┘ │                     │
└──────────────┘                     │
                                     ▼
┌─────────────────────────────────────────────────────────────────────────┐
│                         GATEWAY-AUTH CRATE                                │
│  ┌─────────────────────────────────────────────────────────────────────┐ │
│  │  AuthValidator trait                                                │ │
│  │  - validate_api_key(key: &str) -> Result<AuthContext, AuthError>   │ │
│  │  - validate_session(token: &str) -> Result<AuthContext, AuthError> │ │
│  │                                                                     │ │
│  │  AuthContext: { org_id, user_id, key_id, permissions[], expiry }   │ │
│  └─────────────────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────────────────┘
        │
        ▼
┌─────────────────────────────────────────────────────────────────────────┐
│                         GATEWAY-DB CRATE                                  │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐  ┌─────────────────┐ │
│  │  org_repo   │  │  key_repo   │  │ usage_repo  │  │  config_repo    │ │
│  └─────────────┘  └─────────────┘  └─────────────┘  └─────────────────┘ │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐  ┌─────────────────┐ │
│  │  user_repo  │  │ quota_repo  │  │ billing_repo│  │  audit_repo     │ │
│  └─────────────┘  └─────────────┘  └─────────────┘  └─────────────────┘ │
└─────────────────────────────────────────────────────────────────────────┘
        │
        ▼
┌─────────────────────────────────────────────────────────────────────────┐
│                      GATEWAY-OBSERVABILITY CRATE                          │
│  ┌─────────────────────────────────────────────────────────────────────┐ │
│  │  Logger trait                                                       │ │
│  │  Metrics trait                                                      │ │
│  │  Tracing integration (OpenTelemetry-compatible, stdout-only)        │ │
│  └─────────────────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────────────────┘
```

### 3.2 Component Details

#### 3.2.1 API Layer (`gateway-api`)

**Responsibilities:**
- HTTP server lifecycle (bind, graceful shutdown)
- Route definitions and URL dispatch
- Middleware stack composition (auth, rate limit, logging, CORS)
- Request deserialization and response serialization
- SSE (Server-Sent Events) streaming for chat completions
- Admin API routes (CRUD for keys, quotas, providers, usage viewing)
- Health check and readiness endpoints

**Key Interfaces (exposes):**
```rust
// No public Rust API — this is the binary crate
// It COMPOSES all other crates and starts the server

// Internal structure:
pub fn app_router(deps: AppDependencies) -> Router;
pub async fn run_server(config: ServerConfig, deps: AppDependencies);
```

**Key Interfaces (consumes):**
- `gateway_core::RequestOrchestrator` — to process LLM requests
- `gateway_auth::AuthValidator` — to authenticate requests
- `gateway_observability::RequestLogger` — to log requests
- `gateway_quota::RateLimiter` — to apply rate limits

**Dependencies:** All other crates (it is the composition root)

**Crate type:** Binary (`main.rs`)

---

#### 3.2.2 Core / Request Lifecycle (`gateway-core`)

**Responsibilities:**
- Define the canonical `ChatCompletionRequest` and `ChatCompletionResponse` types
- Orchestrate the 12-step request lifecycle
- Implement retry and fallback logic between providers
- Transform provider-specific request/response formats to/from the canonical OpenAI format
- Handle streaming responses (SSE aggregation and forwarding)
- Enforce request timeouts and cancellation

**Key Interfaces (exposes):**
```rust
#[async_trait]
pub trait RequestOrchestrator: Send + Sync {
    async fn chat_completion(
        &self,
        request: ChatCompletionRequest,
        context: RequestContext,
    ) -> Result<ChatCompletionResponse, GatewayError>;

    async fn chat_completion_stream(
        &self,
        request: ChatCompletionRequest,
        context: RequestContext,
    ) -> Result<SseStream, GatewayError>;
}

// Canonical request type (OpenAI-compatible)
pub struct ChatCompletionRequest {
    pub model: String,              // "gpt-4o", "claude-3-5-sonnet", etc.
    pub messages: Vec<Message>,
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
    pub stream: Option<bool>,
    // ... other OpenAI fields
}

// Request context (populated by auth layer)
pub struct RequestContext {
    pub org_id: Uuid,
    pub key_id: Uuid,
    pub user_id: Option<Uuid>,
    pub permissions: Vec<Permission>,
    pub request_id: Uuid,         // generated at edge, propagated throughout
}
```

**Key Interfaces (consumes):**
- `gateway_providers::Provider` — to call LLM providers
- `gateway_cache::Cache` — to check/store cached responses
- `gateway_quota::QuotaChecker` — to check and deduct quota
- `gateway_quota::BillingRecorder` — to record usage for billing
- `gateway_auth::AuthContext` — to understand who is calling

**Dependencies:** `gateway-providers`, `gateway-cache`, `gateway-quota`, `gateway-auth`

**Crate type:** Library

---

#### 3.2.3 Provider Abstraction (`gateway-providers`)

**Responsibilities:**
- Define the `Provider` trait that all LLM backends must implement
- Implement provider-specific adapters: OpenAI, Anthropic, Gemini, Ollama, Custom
- Transform provider-native request/response formats to/from the canonical OpenAI format
- Manage provider-specific authentication (API keys, headers)
- Track per-provider health status
- Handle provider-specific streaming formats

**Key Interfaces (exposes):**
```rust
#[async_trait]
pub trait Provider: Send + Sync {
    fn name(&self) -> &str;
    fn supported_models(&self) -> Vec<&str>;
    
    async fn chat_completion(
        &self,
        request: ChatCompletionRequest,
    ) -> Result<ChatCompletionResponse, ProviderError>;

    async fn chat_completion_stream(
        &self,
        request: ChatCompletionRequest,
    ) -> Result<SseStream, ProviderError>;

    async fn embeddings(
        &self,
        request: EmbeddingRequest,
    ) -> Result<EmbeddingResponse, ProviderError>;

    async fn health_check(&self) -> HealthStatus;
}

// Factory function
pub fn create_provider(config: ProviderConfig) -> Box<dyn Provider>;
```

**Key Interfaces (consumes):**
- HTTP client (hyper/reqwest — injected, not hardcoded)
- Provider configuration from database

**Dependencies:** `gateway-core` (for canonical types), `gateway-db` (for provider configs)

**Crate type:** Library

---

#### 3.2.4 Cache Layer (`gateway-cache`)

**Responsibilities:**
- Exact-match response caching (request hash → response)
- Semantic caching (embedding similarity → response) — optional, configurable
- Cache TTL management and eviction
- Cache statistics (hit rate, size)
- Skip-cache directives (honor `x-skip-cache` header)

**Key Interfaces (exposes):**
```rust
#[async_trait]
pub trait Cache: Send + Sync {
    async fn get(&self, key: &CacheKey) -> Result<Option<CachedResponse>, CacheError>;
    async fn put(&self, key: &CacheKey, response: &CachedResponse, ttl: Duration) -> Result<(), CacheError>;
    async fn invalidate(&self, pattern: &str) -> Result<u64, CacheError>;
    async fn stats(&self) -> CacheStats;
}

#[async_trait]
pub trait SemanticCache: Send + Sync {
    async fn get_similar(
        &self,
        embedding: &[f32],
        threshold: f32,
    ) -> Result<Option<CachedResponse>, CacheError>;
    
    async fn put(
        &self,
        embedding: &[f32],
        response: &CachedResponse,
        ttl: Duration,
    ) -> Result<(), CacheError>;
}

pub struct CacheKey {
    pub request_hash: String,       // SHA-256 of normalized request
    pub model: String,
    pub org_id: Uuid,               // per-tenant cache isolation
}
```

**Key Interfaces (consumes):**
- Redis connection pool

**Dependencies:** None (Redis client is an implementation detail)

**Crate type:** Library

---

#### 3.2.5 Quota & Billing (`gateway-quota`)

**Responsibilities:**
- Check quota before request (per-org, per-user, per-key)
- Deduct quota after request (token-based or request-based)
- Record usage for billing (tokens in/out, cost, provider, model)
- Aggregate usage into billing periods
- Enforce budget limits (hard stop vs. alert threshold)
- Provide real-time usage queries for admin dashboard

**Key Interfaces (exposes):**
```rust
#[async_trait]
pub trait QuotaChecker: Send + Sync {
    async fn check_quota(&self, context: &RequestContext, estimated_cost: f64) -> Result<QuotaStatus, QuotaError>;
    async fn deduct_quota(&self, context: &RequestContext, actual_usage: TokenUsage) -> Result<(), QuotaError>;
    async fn get_usage(&self, org_id: Uuid, period: BillingPeriod) -> Result<UsageSummary, QuotaError>;
}

#[async_trait]
pub trait BillingRecorder: Send + Sync {
    async fn record_usage(&self, record: UsageRecord) -> Result<(), BillingError>;
    async fn get_billing_summary(&self, org_id: Uuid, period: BillingPeriod) -> Result<BillingSummary, BillingError>;
}

pub struct UsageRecord {
    pub request_id: Uuid,
    pub org_id: Uuid,
    pub key_id: Uuid,
    pub user_id: Option<Uuid>,
    pub provider: String,
    pub model: String,
    pub tokens_in: u64,
    pub tokens_out: u64,
    pub latency_ms: u64,
    pub cost: f64,                    // computed from model pricing
    pub timestamp: DateTime<Utc>,
    pub cached: bool,
}
```

**Key Interfaces (consumes):**
- PostgreSQL (for quota balances, usage records, billing data)
- Redis (for fast rate-limit counters)
- Model pricing configuration (from `gateway-db`)

**Dependencies:** `gateway-db`

**Crate type:** Library

---

#### 3.2.6 Authentication (`gateway-auth`)

**Responsibilities:**
- Validate gateway-issued API keys (for LLM API consumers)
- Validate admin session tokens (for dashboard users)
- Enforce key permissions and scopes
- Track key usage metadata
- Support key rotation (new key generation, old key deprecation)

**Key Interfaces (exposes):**
```rust
#[async_trait]
pub trait AuthValidator: Send + Sync {
    async fn validate_api_key(&self, key: &str) -> Result<AuthContext, AuthError>;
    async fn validate_session(&self, token: &str) -> Result<AuthContext, AuthError>;
}

pub struct AuthContext {
    pub org_id: Uuid,
    pub key_id: Option<Uuid>,         // set for API key auth
    pub user_id: Option<Uuid>,        // set for session auth
    pub permissions: Vec<Permission>, // ["chat:write", "embeddings:read", "admin"]
    pub rate_limit_tier: RateLimitTier,
}

pub enum Permission {
    ChatWrite,
    EmbeddingsRead,
    ModelsRead,
    Admin,
}
```

**Key Interfaces (consumes):**
- PostgreSQL (for key storage and lookup)

**Dependencies:** `gateway-db`

**Crate type:** Library

---

#### 3.2.7 Observability (`gateway-observability`)

**Responsibilities:**
- Structured JSON logging (request logs, error logs, audit logs)
- Request-level metrics (latency histograms, throughput counters, error rates)
- Provider-level metrics (latency by provider, error rate by provider, cost by provider)
- Per-organization usage dashboards data
- OpenTelemetry-compatible trace context propagation (for future compatibility)
- Log correlation via `request_id`

**Key Interfaces (exposes):**
```rust
pub trait RequestLogger: Send + Sync {
    fn log_request(&self, record: RequestLogRecord);
    fn log_error(&self, error: &GatewayError, context: &RequestContext);
}

pub struct RequestLogRecord {
    pub request_id: Uuid,
    pub timestamp: DateTime<Utc>,
    pub method: String,
    pub path: String,
    pub status_code: u16,
    pub latency_ms: u64,
    pub org_id: Uuid,
    pub key_id: Option<Uuid>,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub tokens_in: Option<u64>,
    pub tokens_out: Option<u64>,
    pub cost: Option<f64>,
    pub cached: bool,
    pub error: Option<String>,
}

// Metrics are emitted via a simple counter/histogram interface
// Backed by a thread-local stats aggregator, flushed to stdout
pub trait Metrics: Send + Sync {
    fn increment_counter(&self, name: &str, labels: &[(&str, &str)]);
    fn record_histogram(&self, name: &str, value: f64, labels: &[(&str, &str)]);
}
```

**Key Interfaces (consumes):**
- None (self-contained; writes to stdout, reads config from environment)

**Dependencies:** None

**Crate type:** Library

---

#### 3.2.8 Admin Dashboard (`gateway-web`)

**Responsibilities:**
- Organization management (settings, members)
- API key management (create, revoke, view usage)
- Provider configuration (add, edit, enable/disable providers)
- Quota and budget management (set limits, view usage)
- Usage analytics (charts, tables, export)
- Request inspector (search, filter logs)
- Model management (view available models, set custom pricing)

**Key Interfaces (exposes):**
- React SPA served as static files
- Admin REST API consumed by the frontend (part of `gateway-api`)

**Key Interfaces (consumes):**
- `GET /api/admin/organizations/:id/usage` — usage data
- `GET /api/admin/organizations/:id/keys` — API keys list
- `POST /api/admin/organizations/:id/keys` — create key
- `GET /api/admin/organizations/:id/providers` — provider configs
- `PUT /api/admin/organizations/:id/providers/:provider` — update config
- `GET /api/admin/organizations/:id/quotas` — quota settings
- `PUT /api/admin/organizations/:id/quotas` — update quotas
- `GET /api/admin/organizations/:id/billing` — billing summary
- `GET /api/admin/requests` — request logs (paginated, filterable)

**Dependencies:** `gateway-api` (for the admin API endpoints)

**Crate type:** Separate frontend project (TypeScript/React), not a Rust crate

---

## 4. Request Lifecycle

### 4.1 Complete Trace: Chat Completion Request

This section traces a single `POST /v1/chat/completions` request through every component. For each step: **Component**, **Action**, **Data In**, **Data Out**.

---

#### Step 1: Request Arrives at API Layer

| Attribute | Value |
|-----------|-------|
| **Component** | `gateway-api::Router` |
| **Action** | Accept HTTP connection, parse request body, extract headers |
| **Data In** | `POST /v1/chat/completions`, headers: `Authorization: Bearer sk-gw-xxx`, body: JSON |
| **Data Out** | `Axum Request` object with deserialized `ChatCompletionRequest` |

Details:
- Router matches `POST /v1/chat/completions` to the `chat_completions_handler`
- Body deserialized into `gateway_core::ChatCompletionRequest`
- `x-request-id` header used if present; otherwise generates UUIDv4
- `Content-Type` validated (must be `application/json`)
- Request body size limited to 10MB

---

#### Step 2: Authentication & Validation

| Attribute | Value |
|-----------|-------|
| **Component** | `gateway-auth::AuthValidator` + `gateway-api::ValidationMiddleware` |
| **Action** | Extract and validate API key; load permissions; reject if invalid |
| **Data In** | `Authorization: Bearer sk-gw-{base58}` header |
| **Data Out** | `AuthContext { org_id, key_id, permissions, rate_limit_tier }` |

Details:
- API key format: `sk-gw-{base58-encoded-random-32-bytes}` (URL-safe, unambiguous)
- Key lookup in PostgreSQL `api_keys` table (indexed on `key_hash`)
- Check key is active (not revoked, not expired)
- Load organization settings (is org active? any org-level blocks?)
- Attach `AuthContext` to request extensions for downstream use
- **Fail closed:** any error → `401 Unauthorized`, no leakage of internal state

---

#### Step 3: Rate Limit Check

| Attribute | Value |
|-----------|-------|
| **Component** | `gateway-quota::RateLimiter` |
| **Action** | Check request rate against configured limits |
| **Data In** | `AuthContext.rate_limit_tier`, `key_id`, `org_id` |
| **Data Out** | `Ok(())` or `Err(RateLimitExceeded { retry_after: Duration })` |

Details:
- Rate limits are tiered: `free` (10/min), `standard` (100/min), `premium` (1000/min)
- Sliding window counter in Redis: `ratelimit:{key_id}:{window}`
- Counter incremented, TTL set to window size
- If limit exceeded → `429 Too Many Requests` with `Retry-After` header
- Rate limit check is **best-effort**; a small race window is acceptable for this use case

---

#### Step 4: Quota / Budget Check

| Attribute | Value |
|-----------|-------|
| **Component** | `gateway-quota::QuotaChecker` |
| **Action** | Check if organization has sufficient budget/quota for this request |
| **Data In** | `org_id`, `key_id`, estimated cost (from model pricing * max_tokens) |
| **Data Out** | `QuotaStatus::Allowed`, `QuotaStatus::Warning(threshold)`, or `QuotaStatus::Exceeded` |

Details:
- Budget checks are **pre-request** (prevent overages) and **post-request** (record actuals)
- Pre-request estimate uses `max_tokens` from request × model's output token price
- If no `max_tokens`, use a default estimate (e.g., 4096 tokens)
- Budget exceeded → `429 Payment Required` (non-standard but semantically correct) or `403 Forbidden`
- Warning threshold (e.g., 80% of budget) allows request but adds `X-Quota-Warning` header

---

#### Step 5: Cache Check

| Attribute | Value |
|-----------|-------|
| **Component** | `gateway-cache::Cache` (exact) + `gateway-cache::SemanticCache` (if enabled) |
| **Action** | Check if an identical (or semantically similar) request was recently cached |
| **Data In** | Normalized `ChatCompletionRequest`, `org_id` |
| **Data Out** | `Some(CachedResponse)` → skip to Step 12, or `None` → continue |

Details:
- Exact cache key: SHA-256 of normalized JSON request body + model + org_id
- Normalization: strip `temperature` if `1.0`, sort object keys, strip `stream: false`
- Semantic cache (optional): compute embedding of request, search Redis for similar vectors
- Cache lookup in Redis: `cache:{hash}` or `semantic_cache:{org}:{embedding_hash}`
- Cache hit → return cached response with `X-Cache: HIT` header, skip provider call
- Cache miss → continue to provider selection, `X-Cache: MISS` header added later
- Cache skipped if `x-skip-cache: true` header present

---

#### Step 6: Provider Selection (Routing)

| Attribute | Value |
|-----------|-------|
| **Component** | `gateway-core::Router` |
| **Action** | Select the best provider for this request based on model, health, cost, latency |
| **Data In** | `model` field from request, provider health status, routing config |
| **Data Out** | `ProviderSelection { provider_name, provider_config, fallback_chain: Vec<String> }` |

Details:
- Model name maps to one or more capable providers (e.g., "gpt-4o" → OpenAI; "claude-3-sonnet" → Anthropic)
- Routing strategies: `fixed` (always use configured provider), `priority` (first healthy), `latency` (lowest recent latency), `cost` (cheapest available)
- Default strategy: `priority` with health-based failover
- Provider health refreshed every 30 seconds (background task, stored in Redis)
- If no healthy provider for requested model → `503 Service Unavailable` with `X-Unavailable-Reason: no_healthy_provider`
- Fallback chain constructed: if primary fails, try next in chain (see Step 8 retry logic)

---

#### Step 7: Request Transformation

| Attribute | Value |
|-----------|-------|
| **Component** | `gateway-providers::{Provider}::transform_request` |
| **Action** | Convert canonical `ChatCompletionRequest` to provider-native format |
| **Data In** | `ChatCompletionRequest` (OpenAI-compatible) |
| **Data Out** | Provider-specific HTTP request body (JSON) |

Details:
- OpenAI adapter: pass through (already OpenAI format)
- Anthropic adapter: convert `messages` to Claude's `messages` format; map `model` to Claude model ID; map `max_tokens` (required for Claude)
- Gemini adapter: convert to Gemini's `contents` format; handle system instructions
- Ollama adapter: convert to Ollama's `/api/chat` format; map model to local model name
- Custom adapter: user-configured URL + header injection + body passthrough
- Temperature, top_p, max_tokens mapped where possible; unsupported parameters dropped with warning logged

---

#### Step 8: Provider Call

| Attribute | Value |
|-----------|-------|
| **Component** | `gateway-providers::{Provider}::chat_completion` |
| **Action** | Send HTTP request to provider API, await response |
| **Data In** | Provider-native request body, provider config (base URL, API key, timeout) |
| **Data Out** | Provider-native response body, HTTP status, headers |

Details:
- HTTP client: `reqwest` with connection pooling, 30-second default timeout
- Timeout is configurable per-provider (e.g., Ollama local models may need longer)
- Streaming: if `stream: true`, provider returns SSE stream; gateway forwards chunks transparently
- Retry logic: on `5xx`, `429`, or timeout, retry up to 2 times with exponential backoff (1s, 2s)
- Fallback: if primary provider exhausted retries, attempt next provider in fallback chain
- Circuit breaker: after 5 consecutive failures, mark provider unhealthy for 60 seconds
- Request cancellation: if client disconnects, abort the provider request (propagate cancellation token)

---

#### Step 9: Response Transformation

| Attribute | Value |
|-----------|-------|
| **Component** | `gateway-providers::{Provider}::transform_response` |
| **Action** | Convert provider-native response to canonical OpenAI-compatible format |
| **Data In** | Provider-native response JSON |
| **Data Out** | `ChatCompletionResponse` (OpenAI-compatible) |

Details:
- All responses normalized to OpenAI's `ChatCompletion` schema:
  ```json
  {
    "id": "chatcmpl-{uuid}",
    "object": "chat.completion",
    "created": 1700000000,
    "model": "gpt-4o",
    "choices": [{ "index": 0, "message": { "role": "assistant", "content": "..." }, "finish_reason": "stop" }],
    "usage": { "prompt_tokens": 10, "completion_tokens": 20, "total_tokens": 30 }
  }
  ```
- Token counts: use provider-reported counts when available; estimate using tiktoken when unavailable
- `model` field in response reflects the **gateway model name**, not the provider's internal name
- Streaming chunks transformed to OpenAI's SSE format: `data: { ... }\n\n`

---

#### Step 10: Cache Store

| Attribute | Value |
|-----------|-------|
| **Component** | `gateway-cache::Cache` |
| **Action** | Store the response for future cache hits |
| **Data In** | `CacheKey` (from Step 5), `ChatCompletionResponse` |
| **Data Out** | `Ok(())` or `Err(CacheError)` (non-fatal) |

Details:
- Store only if: (a) not a streaming request, (b) not a cache hit, (c) response successful (2xx)
- Exact cache: store in Redis with configurable TTL (default: 1 hour)
- Semantic cache (if enabled): compute embedding of request, store embedding + response
- Cache key includes org_id for tenant isolation
- Cache errors are **non-fatal** — logged but do not fail the request

---

#### Step 11: Quota Update & Cost Tracking

| Attribute | Value |
|-----------|-------|
| **Component** | `gateway-quota::QuotaChecker::deduct_quota` + `gateway-quota::BillingRecorder::record_usage` |
| **Action** | Deduct actual token usage from quota; record usage for billing |
| **Data In** | `AuthContext`, actual `TokenUsage { tokens_in, tokens_out }`, provider, model |
| **Data Out** | `UsageRecord` persisted to PostgreSQL |

Details:
- Cost computed: `tokens_in × input_price_per_1k / 1000 + tokens_out × output_price_per_1k / 1000`
- Prices are per-model, configured in the database (`models` table), overridable per-organization
- Usage record inserted into PostgreSQL `usage_records` table (append-only, no updates)
- Quota balance updated in PostgreSQL (atomic decrement)
- Rate limit counter already incremented in Step 3
- All database writes happen **after** the response is ready; if write fails, it is retried async (logged, not blocking)

---

#### Step 12: Response to Client

| Attribute | Value |
|-----------|-------|
| **Component** | `gateway-api::ResponseSerializer` |
| **Action** | Serialize response, add metadata headers, send to client |
| **Data In** | `ChatCompletionResponse`, metadata (cache status, request ID, provider used) |
| **Data Out** | HTTP response with JSON body and headers |

Details:
- Response headers:
  - `X-Request-Id: {uuid}` — for support/debugging
  - `X-Cache: HIT | MISS`
  - `X-Provider: openai | anthropic | gemini | ollama`
  - `X-Model: {gateway_model_name}`
  - `X-Quota-Remaining: {tokens}` — if quota system enabled
  - `X-Processing-Time: {ms}`
- Status code: `200 OK` for success, streamed as `SSE` if `stream: true`
- JSON serialization via `serde_json`, no pretty-printing

---

### 4.2 Request Lifecycle Summary Table

| Step | Component | Action | Critical? | On Failure |
|------|-----------|--------|-----------|------------|
| 1 | `gateway-api` | Accept & parse | Yes | 400 Bad Request |
| 2 | `gateway-auth` | Authenticate | Yes | 401 Unauthorized |
| 3 | `gateway-quota` | Rate limit check | Yes | 429 Too Many Requests |
| 4 | `gateway-quota` | Quota/budget check | Yes | 403 Forbidden / 429 |
| 5 | `gateway-cache` | Cache check | No | Continue (cache miss) |
| 6 | `gateway-core` | Provider selection | Yes | 503 Unavailable |
| 7 | `gateway-providers` | Request transform | Yes | 500 Internal Error |
| 8 | `gateway-providers` | Provider call | Yes | Retry → fallback → 502 |
| 9 | `gateway-providers` | Response transform | Yes | 500 Internal Error |
| 10 | `gateway-cache` | Cache store | No | Log, continue |
| 11 | `gateway-quota` | Quota deduct + billing | No | Retry async, log |
| 12 | `gateway-api` | Respond to client | Yes | N/A (final step) |

---

## 5. Data Flow Diagrams

### 5.1 Normal Request (Happy Path)

```
Client ──POST /v1/chat/completions──▶ gateway-api
                                         │
                                         ▼
                                   [Parse & Validate]
                                         │
                                         ▼
 gateway-auth ◀──Validate API Key──── [Auth Middleware]
     │                                   │
     ▼                                   ▼
  PostgreSQL                       [Rate Limit Check]
                                         │
                                         ▼
 gateway-quota ◀──Check quota───── [Quota Check]
     │                                   │
     ▼                                   ▼
  PostgreSQL                       [Cache Check]
                                         │
                                         ▼ (MISS)
 gateway-cache ◀──Lookup cache──── [Cache Layer]
     │                                   │
     ▼                                   ▼
   Redis                        [Provider Selection]
                                         │
                                         ▼
 gateway-providers ◀──Health────── [Router]
     │                                   │
     ▼                                   ▼
   Redis                        [Transform Request]
                                         │
                                         ▼
                                  [Call Provider]
                                         │
                              HTTP/JSON to OpenAI/Anthropic/etc.
                                         │
                                         ▼
                                  [Transform Response]
                                         │
                                         ▼
                                  [Store Cache]
                                         │
                                         ▼
 gateway-cache ──Store response──▶ [Cache Layer]
     │                                   │
     ▼                                   ▼
   Redis                        [Update Quota & Billing]
                                         │
                                         ▼
 gateway-quota ──Record usage───▶ [Quota/Billing]
     │                                   │
     ▼                                   ▼
  PostgreSQL                     [Serialize Response]
                                         │
                                         ▼
Client ◀────OpenAI-compatible JSON──── [Response]
```

### 5.2 Cache Hit (Shortcut Path)

```
Client ──POST /v1/chat/completions──▶ gateway-api
                                         │
                                         ▼
                                   [Parse & Validate]
                                         │
                                         ▼
                                   [Auth & Rate Limit]
                                         │
                                         ▼
                                   [Quota Check]
                                         │
                                         ▼
 gateway-cache ◀──Lookup cache──── [Cache Check]
     │                                   │
     ▼                                   ▼ (HIT)
   Redis                        [Return Cached Response]
                                         │ (skip provider)
                                         ▼
                                   [Update Quota & Billing]
                                         │ (record as cached=true, cost=0)
                                         ▼
Client ◀────Cached JSON + X-Cache: HIT── [Response]
```

**Key difference:** Steps 6-10 (provider selection through cache store) are skipped entirely. Latency is reduced from hundreds of milliseconds to single-digit milliseconds.

### 5.3 Provider Failure (Fallback Path)

```
Client ──POST /v1/chat/completions──▶ gateway-api
                                         │
                                         ▼
                                   [... auth, quota, cache ...]
                                         │
                                         ▼ (cache miss)
                                   [Provider Selection]
                                         │
                               Primary: OpenAI
                                         │
                                         ▼
                                   [Call OpenAI]
                                         │
                              503 Service Unavailable
                                         │
                                         ▼
                                   [Retry 1: 1s backoff]
                                         │
                              503 Service Unavailable
                                         │
                                         ▼
                                   [Retry 2: 2s backoff]
                                         │
                              Timeout after 30s
                                         │
                                         ▼
                                   [Exhausted primary]
                                         │
                                         ▼
 gateway-providers ──Mark unhealthy──▶ [Circuit Breaker]
     │
     ▼
   Redis
                                         │
                                         ▼
                                   [Try Fallback: Anthropic]
                                         │
                                         ▼
                                   [Transform for Anthropic]
                                         │
                                         ▼
                                   [Call Anthropic]
                                         │
                                         ▼
                                   [Transform Response]
                                         │
                                         ▼
                                   [... cache, quota, respond ...]
                                         │
Client ◀────JSON + X-Provider: anthropic── [Response]
```

**Key behaviors:**
- Circuit breaker: OpenAI marked unhealthy for 60s after 5 consecutive failures
- Fallback chain: OpenAI → Anthropic → Gemini (configurable per-model)
- Client sees single latency spike but eventually gets a response
- If all providers fail → `502 Bad Gateway` with `X-Unavailable-Reason: all_providers_failed`

### 5.4 Budget Exceeded (Rejection Path)

```
Client ──POST /v1/chat/completions──▶ gateway-api
                                         │
                                         ▼
                                   [Auth & Rate Limit] ── OK
                                         │
                                         ▼
 gateway-quota ◀──Check budget─── [Quota Check]
     │                                   │
     ▼                                   ▼
  PostgreSQL                      Budget: $0.00 remaining
                                         │
                                         ▼
                                   [REJECT]
                                         │
Client ◀────403 Forbidden + X-Quota-Reason: budget_exceeded──── [Error Response]
```

**Key behaviors:**
- Rejected **before** any provider call (no provider cost incurred)
- Response body: `{ "error": { "type": "quota_exceeded", "message": "Organization budget exhausted" } }`
- HTTP status: `403 Forbidden` (semantic: server understood request but refuses to authorize)
- Alternative: `429 Too Many Requests` with `Retry-After` if budget resets periodically

### 5.5 Admin Dashboard Query (Read Path)

```
Browser ──GET /admin──▶ Nginx (static files)
                            │
                            ▼
                        [Serve React SPA]
                            │
                            ▼
Browser ◀────index.html + JS/CSS bundles──── [Dashboard UI]

Browser ──GET /api/admin/org/123/usage──▶ gateway-api
                                              │
                                              ▼
                                        [Session Auth]
                                              │
                                              ▼
                                        [Permission Check]
                                              │ (admin role required)
                                              ▼
 gateway-quota ◀──Query usage────── [Admin Handler]
     │
     ▼
  PostgreSQL                          [Aggregate Data]
                                              │
                                              ▼
 gateway-db ◀──SQL queries──────── [Repository Layer]
     │
     ▼
  PostgreSQL
                                              │
                                              ▼
Browser ◀────JSON { usage, cost, charts }──── [API Response]
```

**Key behaviors:**
- Admin API is separate route prefix (`/api/admin/*`) with stricter auth
- All admin endpoints require `Permission::Admin` in the session's `AuthContext`
- Data aggregation done in PostgreSQL (SUM, COUNT, GROUP BY) — not in application code
- Large result sets paginated (cursor-based for time-series data)
- Read-only operations; no provider calls made


---

## 6. Crate Structure

### 6.1 Refined Workspace Layout

The initial proposal is good but requires refinement. Key changes from the initial proposal:

1. **`gateway-core` renamed to `gateway-domain`** — it owns the canonical types and the orchestrator; "domain" is more descriptive than "core"
2. **`gateway-db` merged into `gateway-domain`** — repository traits belong with the types they operate on; SQLx queries live in a `db` module within `gateway-domain`
3. **`gateway-config` added** — a dedicated crate for configuration parsing and validation; every crate depends on it
4. **`gateway-common` added** — shared utilities (error types, middleware, HTTP helpers) that multiple crates need
5. **Frontend is NOT in the workspace** — it is a separate TypeScript project at `web/`

```
gateway/                          # Workspace root
├── Cargo.toml                  # Workspace manifest
├── crates/
│   ├── gateway-config/         # Configuration: env, file, validation
│   ├── gateway-domain/         # Canonical types, traits, orchestrator
│   │   └── src/
│   │       ├── types/          # ChatCompletionRequest, etc.
│   │       ├── traits/         # Provider, Cache, QuotaChecker, etc.
│   │       ├── orchestrator/   # Request lifecycle orchestrator
│   │       └── db/             # Repository traits, migrations
│   ├── gateway-providers/      # Provider adapters
│   │   └── src/
│   │       ├── openai.rs
│   │       ├── anthropic.rs
│   │       ├── gemini.rs
│   │       ├── ollama.rs
│   │       ├── custom.rs
│   │       └── factory.rs
│   ├── gateway-cache/          # Caching logic (exact + semantic)
│   ├── gateway-quota/          # Quota/billing
│   ├── gateway-auth/           # Authentication
│   ├── gateway-observability/  # Logging, metrics
│   └── gateway-common/         # Shared utilities, error types
├── src/
│   └── main.rs                 # Binary: composition root, starts Axum
└── web/                        # React/TypeScript frontend (separate)
    ├── src/
    ├── package.json
    └── vite.config.ts
```

### 6.2 Crate Details

#### 6.2.1 `gateway-config`

**Purpose:** Centralized configuration management. Parses environment variables and TOML config files into validated structs. Used by every other crate.

**Public API:**
```rust
pub struct AppConfig {
    pub server: ServerConfig,
    pub database: DatabaseConfig,
    pub redis: RedisConfig,
    pub providers: Vec<ProviderConfig>,
    pub cache: CacheConfig,
    pub quota: QuotaConfig,
    pub auth: AuthConfig,
    pub observability: ObservabilityConfig,
}

impl AppConfig {
    pub fn from_env() -> Result<Self, ConfigError>;
    pub fn from_file(path: &Path) -> Result<Self, ConfigError>;
    pub fn validate(&self) -> Result<(), Vec<ValidationError>>;
}
```

**Internal Structure:**
```
gateway-config/src/
├── lib.rs              # re-exports
├── sources/            # env, file, defaults
│   ├── env.rs
│   ├── file.rs
│   └── defaults.rs
├── sections/           # one module per config section
│   ├── server.rs
│   ├── database.rs
│   ├── redis.rs
│   ├── provider.rs
│   ├── cache.rs
│   ├── quota.rs
│   ├── auth.rs
│   └── observability.rs
└── validation.rs       # cross-field validation logic
```

**Dependencies:** `serde`, `serde_json`, `toml`, `thiserror`, `tracing`

---

#### 6.2.2 `gateway-domain`

**Purpose:** The heart of the system. Defines all canonical types, the trait contracts between subsystems, the request lifecycle orchestrator, and the database repository traits.

**Public API:**
```rust
// Types (src/types/)
pub use types::chat::*;        // ChatCompletionRequest, ChatCompletionResponse, Message, etc.
pub use types::embedding::*;   // EmbeddingRequest, EmbeddingResponse
pub use types::common::*;      // ModelId, ProviderName, TokenUsage, etc.

// Traits (src/traits/)
pub use traits::provider::*;   // Provider trait
pub use traits::cache::*;      // Cache, SemanticCache traits
pub use traits::quota::*;      // QuotaChecker, BillingRecorder traits
pub use traits::auth::*;       // AuthValidator trait
pub use traits::router::*;     // Router trait
pub use traits::logger::*;     // RequestLogger, Metrics traits

// Orchestrator (src/orchestrator/)
pub use orchestrator::RequestOrchestrator;

// Database (src/db/)
pub use db::repositories::*;   // OrgRepo, KeyRepo, UsageRepo, etc. (traits)
pub use db::migrations;        // sqlx migrations
```

**Internal Structure:**
```
gateway-domain/src/
├── lib.rs
├── types/
│   ├── mod.rs
│   ├── chat.rs           # ChatCompletionRequest, ChatCompletionResponse, Message, FinishReason
│   ├── embedding.rs      # EmbeddingRequest, EmbeddingResponse
│   └── common.rs         # ModelId, ProviderName, TokenUsage, RequestContext, AuthContext
├── traits/
│   ├── mod.rs
│   ├── provider.rs       # Provider trait
│   ├── cache.rs          # Cache, SemanticCache traits
│   ├── quota.rs          # QuotaChecker, BillingRecorder traits
│   ├── auth.rs           # AuthValidator trait
│   ├── router.rs         # Router trait
│   └── logger.rs         # RequestLogger, Metrics traits
├── orchestrator/
│   ├── mod.rs
│   └── lifecycle.rs      # 12-step request lifecycle implementation
└── db/
    ├── mod.rs
    ├── migrations/         # sqlx migrate .sql files
    └── repositories/       # Repository traits (not implementations)
        ├── mod.rs
        ├── org.rs
        ├── key.rs
        ├── usage.rs
        ├── quota.rs
        ├── provider.rs
        └── audit.rs
```

**Key Design Decision:** `gateway-domain` defines repository **traits**, not implementations. Concrete repository implementations (SQLx queries) live in a `db` module within `gateway-domain` but behind the trait interface. This allows:
- Mock repositories for unit testing
- Future database swaps without changing business logic
- Clear separation: traits are the contract, SQLx is the implementation

**Dependencies:** `async-trait`, `serde`, `serde_json`, `uuid`, `chrono`, `thiserror`, `sqlx` (for migration macros and query macros)

---

#### 6.2.3 `gateway-providers`

**Purpose:** Implements the `Provider` trait for each supported LLM backend. Handles request/response transformation and provider-specific HTTP calls.

**Public API:**
```rust
use gateway_domain::traits::provider::Provider;

pub mod adapters {
    pub use super::openai::OpenAiAdapter;
    pub use super::anthropic::AnthropicAdapter;
    pub use super::gemini::GeminiAdapter;
    pub use super::ollama::OllamaAdapter;
    pub use super::custom::CustomAdapter;
}

pub fn create_provider(config: ProviderConfig, http: reqwest::Client) -> Box<dyn Provider>;
```

**Internal Structure:**
```
gateway-providers/src/
├── lib.rs
├── factory.rs            # create_provider() — dispatches to correct adapter
├── common.rs             # Shared HTTP helpers, error mapping, retry logic
├── transform.rs          # Request/response transformation utilities
├── openai.rs             # OpenAI adapter
├── anthropic.rs          # Anthropic/Claude adapter
├── gemini.rs             # Google Gemini adapter
├── ollama.rs             # Ollama/local adapter
└── custom.rs             # Custom OpenAI-compatible endpoint adapter
```

**Dependencies:** `gateway-domain`, `reqwest`, `serde`, `serde_json`, `async-trait`, `thiserror`, `tracing`

---

#### 6.2.4 `gateway-cache`

**Purpose:** Implements exact-match and semantic caching on top of Redis.

**Public API:**
```rust
use gateway_domain::traits::cache::{Cache, CacheKey, CachedResponse, SemanticCache};

pub struct RedisCache {
    // ...
}

impl Cache for RedisCache { /* ... */ }

pub struct RedisSemanticCache {
    // ...
}

impl SemanticCache for RedisSemanticCache { /* ... */ }
```

**Internal Structure:**
```
gateway-cache/src/
├── lib.rs
├── exact.rs              # RedisCache implementation
├── semantic.rs           # RedisSemanticCache implementation
└── key.rs                # CacheKey construction (hashing, normalization)
```

**Dependencies:** `gateway-domain`, `redis` (with `tokio-comp` feature), `serde`, `serde_json`, `sha2`, `tracing`

---

#### 6.2.5 `gateway-quota`

**Purpose:** Implements quota checking, rate limiting, and billing recording.

**Public API:**
```rust
use gateway_domain::traits::quota::{QuotaChecker, BillingRecorder};

pub struct PostgresQuotaChecker {
    // ...
}

impl QuotaChecker for PostgresQuotaChecker { /* ... */ }

pub struct PostgresBillingRecorder {
    // ...
}

impl BillingRecorder for PostgresBillingRecorder { /* ... */ }

pub struct RedisRateLimiter {
    // ...
}
```

**Internal Structure:**
```
gateway-quota/src/
├── lib.rs
├── checker.rs            # QuotaChecker implementation (PostgreSQL-backed)
├── billing.rs            # BillingRecorder implementation
├── rate_limiter.rs       # Redis-backed rate limiter
└── pricing.rs            # Cost calculation (tokens × price per model)
```

**Dependencies:** `gateway-domain`, `sqlx`, `redis`, `chrono`, `tracing`

---

#### 6.2.6 `gateway-auth`

**Purpose:** Implements API key and session token validation.

**Public API:**
```rust
use gateway_domain::traits::auth::AuthValidator;

pub struct PostgresAuthValidator {
    // ...
}

impl AuthValidator for PostgresAuthValidator { /* ... */ }

pub fn hash_api_key(key: &str) -> String;        // Argon2 hash
pub fn verify_api_key(key: &str, hash: &str) -> bool;
```

**Internal Structure:**
```
gateway-auth/src/
├── lib.rs
├── validator.rs          # AuthValidator implementation
├── key.rs                # API key generation, hashing, verification
├── session.rs            # Session token creation and validation
└── permissions.rs        # Permission enum, scope checking
```

**Dependencies:** `gateway-domain`, `sqlx`, `argon2`, `rand`, `chrono`, `tracing`

---

#### 6.2.7 `gateway-observability`

**Purpose:** Structured logging and metrics emission.

**Public API:**
```rust
use gateway_domain::traits::logger::{RequestLogger, Metrics};

pub struct JsonLogger;
impl RequestLogger for JsonLogger { /* ... */ }

pub struct StatsdMetrics;
impl Metrics for StatsdMetrics { /* ... */ }

pub fn init_tracing(config: &ObservabilityConfig);  // Sets up tracing subscriber
```

**Internal Structure:**
```
gateway-observability/src/
├── lib.rs
├── logger.rs             # JsonLogger implementation
├── metrics.rs            # StatsdMetrics implementation
├── tracing_setup.rs      # tracing_subscriber initialization
└── middleware.rs          # Tower middleware for request logging
```

**Dependencies:** `gateway-domain`, `tracing`, `tracing-subscriber`, `serde_json`, `chrono`

---

#### 6.2.8 `gateway-common`

**Purpose:** Shared utilities used by multiple crates. This crate has NO external dependencies beyond basic Rust ecosystem crates. It is the only crate that every other crate can depend on without creating circular dependencies.

**Public API:**
```rust
pub mod errors;            # GatewayError, ProviderError, etc.
pub mod middleware;        # Tower middleware (timeout, request_id)
pub mod http;              # HTTP helpers (SSE streaming, JSON extraction)
pub mod validate;          # Common validation (JSON schema, token counting)
```

**Internal Structure:**
```
gateway-common/src/
├── lib.rs
├── errors.rs             # Centralized error types and HTTP status mapping
├── middleware/
│   ├── mod.rs
│   ├── request_id.rs     # X-Request-Id header handling
│   ├── timeout.rs        # Request timeout wrapper
│   └── cors.rs           # CORS configuration
├── http.rs               # SSE response builder, JSON error responses
└── validate.rs           # Input validation helpers
```

**Dependencies:** `axum`, `http`, `serde`, `serde_json`, `thiserror`, `tower`, `uuid`

**IMPORTANT:** `gateway-common` must NOT depend on any other gateway crate. It is the leaf of the dependency tree.

---

### 6.3 Dependency Graph

```
                    ┌─────────────┐
                    │   gateway   │
                    │   (binary)  │
                    └──────┬──────┘
                           │
        ┌──────────────────┼──────────────────┬──────────────┐
        │                  │                  │              │
        ▼                  ▼                  ▼              ▼
┌───────────────┐  ┌───────────────┐  ┌───────────────┐  ┌───────────────┐
│gateway-domain │  │gateway-providers│ │gateway-cache   │  │gateway-quota  │
│               │  │               │  │               │  │               │
│ types, traits │  │ adapters      │  │ redis impl    │  │ checker,      │
│ orchestrator  │  │ factory       │  │               │  │ billing       │
└───────┬───────┘  └───────┬───────┘  └───────┬───────┘  └───────┬───────┘
        │                  │                  │                  │
        │            ┌─────┘                  │            ┌─────┘
        │            │                        │            │
        ▼            ▼                        ▼            ▼
┌───────────────────────────────────────────────────────────────────────┐
│                           gateway-common                               │
│                     (errors, middleware, http)                         │
└───────────────────────────────────────────────────────────────────────┘
        ▲
        │
┌───────┴───────┐  ┌───────────────┐  ┌───────────────┐
│gateway-auth   │  │gateway-observ │  │gateway-config │
│               │  │               │  │               │
│ validator     │  │ logger,       │  │ env + file    │
│ key mgmt      │  │ metrics       │  │ parsing       │
└───────────────┘  └───────────────┘  └───────────────┘
        │                  │                  │
        └──────────────────┼──────────────────┘
                           │
                           ▼
                    ┌─────────────┐
                    │   External  │
                    │ PostgreSQL  │
                    │    Redis    │
                    │ LLM APIs    │
                    └─────────────┘
```

**Dependency Rules:**
- `gateway-common` has no internal dependencies (leaf crate)
- `gateway-domain` depends only on `gateway-common`
- `gateway-config` depends only on `gateway-common`
- Implementation crates (`gateway-providers`, `gateway-cache`, `gateway-quota`, `gateway-auth`, `gateway-observability`) depend on `gateway-domain` and `gateway-common`
- The binary crate (`gateway`) depends on all implementation crates and composes them
- **NO circular dependencies.** If a cycle emerges, the shared code moves to `gateway-domain` or `gateway-common`.

---

## 7. Technology Choices with Rationale

### 7.1 Web Framework: Axum

**Chosen:** `axum` (Tokio project)  
**Alternatives Considered:** `actix-web`, `rocket`, `poem`, `warp`

**Why Axum:**
- Native `tokio` integration — the entire async ecosystem is built on tokio
- Tower middleware ecosystem — composable, type-safe middleware (`tower-http` for CORS, compression, tracing)
- Request extensions pattern — pass `AuthContext`, `RequestId` through request extensions cleanly
- Streaming SSE support — built-in `Sse` response type for LLM streaming
- Used by AWS, Microsoft, and other major Rust projects — long-term viability
- No custom runtime, no proc-macro magic for routing — explicit and debuggable

**Tradeoffs:**
- Slightly more boilerplate than Rocket for handler definitions (acceptable — explicit > convenient)
- Less mature than actix-web (but catching up rapidly; maintained by the Tokio team)
- No built-in auto-reload (use `cargo-watch` for development)

**Decision:** Use `axum` with `tower` middleware. Handlers are async functions. Router composition is explicit in `main.rs`.

---

### 7.2 Database Access: SQLx (Query Builder)

**Chosen:** `sqlx` (compile-time checked queries)  
**Alternatives Considered:** `diesel` (ORM), `sea-orm` (async ORM), `tokio-postgres` (raw driver)

**Why SQLx:**
- Compile-time query checking against a real database schema — catches SQL errors at build time
- No ORM abstraction layer — write SQL directly, full control over queries
- Async-native with `tokio` runtime
- Migration support built-in (`sqlx migrate`)
- Connection pooling via `sqlx::Pool`
- No code generation step (unlike Diesel) — simpler build pipeline

**Tradeoffs:**
- Requires a running database at compile time (for query checking) or `sqlx prepare` for offline builds
- No ORM convenience methods — must write SQL for CRUD (acceptable — CRUD SQL is simple)
- No automatic relationship loading (acceptable — we will explicitly JOIN where needed)

**Decision:** Use `sqlx` with the `runtime-tokio`, `tls-rustls`, and `chrono` features. All queries are in `.sqlx/` query files or inline with `query_as!`. Migrations are plain `.sql` files in `gateway-domain/src/db/migrations/`.

**Schema management:** `sqlx migrate run` on startup (in `main.rs`). Migrations are versioned and idempotent.

---

### 7.3 HTTP Client: Reqwest

**Chosen:** `reqwest` (async, connection pooling)  
**Alternatives Considered:** `hyper` (low-level), `ureq` (sync)

**Why Reqwest:**
- High-level, ergonomic API for HTTP requests
- Built-in connection pooling (critical for provider calls — reuse TCP connections)
- Timeout support per-request
- Streaming response body support (for SSE)
- JSON serialization/deserialization via `serde_json` integration
- Proxy support (useful for routing through corporate proxies or VPNs)

**Tradeoffs:**
- Heavier dependency tree than raw `hyper` (acceptable — functionality worth it)
- Less control than hyper (but we don't need HTTP/2 server push or custom framing)

**Decision:** Single `reqwest::Client` instance shared across all provider adapters (via `Arc<reqwest::Client>`). Connection pool configured for up to 100 idle connections per host.

---

### 7.4 Serialization: Serde + Serde_json

**Chosen:** `serde` + `serde_json`  
**Alternatives Considered:** `simd-json` (faster), `miniserde` (lighter), `sonic-rs` (fast Rust-native)

**Why Serde:**
- Ecosystem standard — every Rust library supports serde
- Derive macros (`Serialize`, `Deserialize`) eliminate boilerplate
- Custom serializers for provider-specific formats
- JSON is the lingua franca of LLM APIs — serde_json is the right tool

**Tradeoffs:**
- Not the fastest JSON parser (simd-json is ~2x faster on large payloads)
- Slightly slower compile times due to derive macro expansion

**Decision:** Use `serde` + `serde_json` as default. For performance-critical paths (provider response parsing), consider `simd-json` as a drop-in replacement later. Start with serde.

---

### 7.5 Async Runtime: Tokio

**Chosen:** `tokio` (full feature set)  
**Alternatives Considered:** `async-std` (now in maintenance mode)

**Why Tokio:**
- De facto standard for Rust async
- Axum, SQLx, Reqwest, Redis crate all depend on tokio — ecosystem convergence
- Mature, production-tested at scale (Discord, AWS, Cloudflare)
- Rich ecosystem: `tokio::sync`, `tokio::time`, `tokio::signal` for graceful shutdown

**Tradeoffs:**
- Locks us into tokio ecosystem (but so does every other major crate we use)

**Decision:** Tokio with `rt-multi-thread`, `macros`, `signal`, `time`, `sync` features. Runtime configured for 4 worker threads (VPS-sized, adjust via `TOKIO_WORKER_THREADS`).

---

### 7.6 Configuration: envy + toml

**Chosen:** `envy` (env var → struct) + `toml` (file parsing)  
**Alternatives Considered:** `config-rs` (unified config), `dotenvy` + manual parsing, `clap` (CLI only)

**Why envy + toml:**
- `envy`: Zero-boilerplate environment variable deserialization into structs
- `toml`: Human-readable config file for complex nested structures (provider configs)
- Separation: env vars for secrets (API keys, DB passwords) and deployment-specific values; TOML file for structural config (provider definitions, model pricing)

**Tradeoffs:**
- Two config sources to manage (mitigated by `gateway-config` crate unifying them)
- No hot-reloading of config (requires restart — acceptable for single-node)

**Decision:** `AppConfig::from_env()` reads env vars; `AppConfig::from_file(path)` reads TOML. The binary loads both and merges (env vars override file values for the same key). Validation runs after merge, before any connections are opened.

---

### 7.7 Logging & Tracing: tracing + tracing-subscriber

**Chosen:** `tracing` + `tracing-subscriber` (JSON format)  
**Alternatives Considered:** `log` + `env_logger`, `slog`, `fern`

**Why Tracing:**
- Structured spans and events — correlated logs via `request_id`
- JSON output via `tracing-subscriber::fmt::json()` — machine-parseable
- OpenTelemetry-compatible context propagation (future-proof if we add OTLP export)
- Integration with `axum` (request logging middleware), `tower` (per-request spans)
- Async-aware — spans follow execution across await points

**Tradeoffs:**
- Slightly more complex setup than `env_logger` (one-time cost)
- JSON logs are less human-readable in development (mitigated: pretty-print in dev mode via config)

**Decision:** All logs are structured JSON. Every request creates a span with `request_id`, `org_id`, `method`, `path`. Errors are logged with full context. In development, use pretty format; in production, use compact JSON.

---

### 7.8 Testing: built-in + tower-test + wiremock

**Chosen:** `tokio::test` + `tower::ServiceExt` + `wiremock`  
**Alternatives Considered:** `mockall` (mocking), `httptest`, `cucumber` (BDD)

**Why This Stack:**
- `tokio::test`: Async test runtime (built into tokio)
- `tower::ServiceExt`: Test Tower services (middleware) without HTTP
- `wiremock`: Mock HTTP servers for provider adapter tests (verify request format, return mock responses)
- `sqlx::test`: Test with temporary PostgreSQL databases (via `sqlx test` with Testcontainers or local DB)

**Tradeoffs:**
- `mockall` not needed — we use trait implementations for fakes, not mocks (e.g., `FakeProvider`, `InMemoryCache`)
- No end-to-end test framework — use `curl` scripts and the Docker Compose setup for integration tests

**Decision:** Three test tiers:
1. **Unit tests:** In each crate, test pure logic (transformations, cost calculations) with no I/O
2. **Integration tests:** In each crate's `tests/` directory, test with real PostgreSQL and Redis (via Docker)
3. **Smoke tests:** Shell scripts in `scripts/smoke-test.sh` that spin up the full stack and run curl commands

---

### 7.9 Frontend: React + TypeScript + Vite

**Chosen:** React 19 + TypeScript + Vite + TanStack Query + Tailwind CSS  
**Alternatives Considered:** `Next.js` (full framework), `Vue`, `Svelte`, `htmx`

**Why This Stack:**
- React: Largest ecosystem, most hiring pool, familiar to most frontend developers
- TypeScript: Type safety across API boundaries (shared types between frontend and Rust via generated TS from Rust structs)
- Vite: Fast dev server, fast builds, no SSR complexity (we serve a static SPA)
- TanStack Query: Data fetching, caching, and server state management
- Tailwind CSS: Utility-first, no CSS-in-JS runtime cost

**Tradeoffs:**
- React is heavier than htmx/Vue (acceptable — admin dashboard is not performance-critical)
- No SSR — initial load is a blank page + JS bundle (acceptable for internal tool)
- SPA requires client-side routing (use `react-router-dom`)

**Decision:** The frontend is a completely separate project in `web/`. It builds to static files served by the Axum app (or Nginx). It consumes the Admin REST API at `/api/admin/*`. No server-side rendering, no Next.js complexity.

---

### 7.10 Additional Dependencies

| Crate | Purpose | Why |
|-------|---------|-----|
| `uuid` | UUID generation | Standard, serde support, v4 for random IDs |
| `chrono` | Date/time handling | SQLx integration, serde support, timezone-aware |
| `argon2` | Password hashing | OWASP recommended, memory-hard |
| `rand` | Cryptographic randomness | API key generation |
| `sha2` | SHA-256 hashing | Cache key computation |
| `thiserror` | Error type definitions | Derive `Error` trait, less boilerplate than `std::error::Error` |
| `anyhow` | Error handling in application code | `.context()` for error propagation in `main.rs` |
| `tower-http` | HTTP middleware | CORS, compression, request ID, timeout (Axum companion) |
| `metrics` + `metrics-exporter-prometheus` | Metrics | Standard metrics API, optional Prometheus endpoint |
| `redis` | Redis client | `tokio-comp` feature for async, multiplexed connections |

---

## 8. Interfaces & Contracts

This section defines every public trait in the system. These are the contracts that modules use to communicate. Every trait is `Send + Sync` because it will be used across async boundaries, typically behind `Arc<dyn Trait>`.

### 8.1 Provider Trait

```rust
use async_trait::async_trait;
use gateway_domain::types::chat::{ChatCompletionRequest, ChatCompletionResponse};
use gateway_domain::types::embedding::{EmbeddingRequest, EmbeddingResponse};
use serde_json::Value;
use std::pin::Pin;
use tokio_stream::Stream;

/// A stream of Server-Sent Events (SSE) chunks.
pub type SseStream = Pin<Box<dyn Stream<Item = Result<String, ProviderError>> + Send>>;

#[async_trait]
pub trait Provider: Send + Sync + std::fmt::Debug {
    /// Unique provider identifier, e.g., "openai", "anthropic", "ollama".
    fn name(&self) -> &str;

    /// List of model IDs this provider can serve, e.g., ["gpt-4o", "gpt-4o-mini"].
    fn supported_models(&self) -> Vec<&str>;

    /// Synchronous (non-streaming) chat completion.
    /// 
    /// # Contract
    /// - Returns a complete response within the configured timeout.
    /// - On provider error (5xx, 429, timeout), returns `ProviderError` — caller decides retry.
    /// - On authentication error (401), returns `ProviderError::Authentication` — no retry.
    /// - On rate limit (429 with Retry-After), returns `ProviderError::RateLimited` — caller may retry.
    async fn chat_completion(
        &self,
        request: ChatCompletionRequest,
    ) -> Result<ChatCompletionResponse, ProviderError>;

    /// Streaming chat completion. Returns an SSE stream.
    ///
    /// # Contract
    /// - Stream yields OpenAI-formatted SSE chunks: `data: {"choices":[{"delta":{"content":"..."}}]}\n\n`
    /// - Stream ends with `data: [DONE]\n\n`
    /// - If the provider's native format differs, the adapter must translate.
    /// - If the client disconnects, the stream should be dropped (cancellation propagated).
    async fn chat_completion_stream(
        &self,
        request: ChatCompletionRequest,
    ) -> Result<SseStream, ProviderError>;

    /// Text embedding request.
    async fn embeddings(
        &self,
        request: EmbeddingRequest,
    ) -> Result<EmbeddingResponse, ProviderError>;

    /// Lightweight health check. Should complete in < 5 seconds.
    /// Called periodically by a background task.
    async fn health_check(&self) -> HealthStatus;
}

#[derive(Debug, Clone)]
pub enum HealthStatus {
    Healthy,
    Degraded { reason: String },
    Unhealthy { reason: String },
}

#[derive(Debug, thiserror::Error)]
pub enum ProviderError {
    #[error("HTTP error: {status} — {body}")]
    HttpError { status: u16, body: String },

    #[error("Authentication failed: {0}")]
    Authentication(String),

    #[error("Rate limited: retry after {retry_after:?}")]
    RateLimited { retry_after: Option<std::time::Duration> },

    #[error("Timeout after {0:?}")]
    Timeout(std::time::Duration),

    #[error("Request serialization failed: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("HTTP client error: {0}")]
    ClientError(String),

    #[error("Provider returned invalid response: {0}")]
    InvalidResponse(String),

    #[error("Circuit breaker open for provider {0}")]
    CircuitOpen(String),
}
```

**Design Notes:**
- `Provider` is the only trait that adapters implement. One struct per provider (OpenAiAdapter, AnthropicAdapter, etc.).
- `SseStream` is a `Pin<Box<dyn Stream>>` because streaming is inherently dynamic — the adapter controls the chunk production rate.
- `ProviderError` is exhaustive. Every error case maps to a specific HTTP status and retry behavior.
- Health check is a separate method (not derived from recent errors) because some providers have dedicated health endpoints.

---

### 8.2 Cache Trait

```rust
use async_trait::async_trait;
use gateway_domain::types::chat::ChatCompletionResponse;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use uuid::Uuid;

#[async_trait]
pub trait Cache: Send + Sync {
    /// Look up a cached response by key.
    /// Returns `None` on cache miss or cache error (cache is best-effort).
    async fn get(&self, key: &CacheKey) -> Result<Option<CachedResponse>, CacheError>;

    /// Store a response in the cache with the specified TTL.
    async fn put(
        &self,
        key: &CacheKey,
        response: &CachedResponse,
        ttl: Duration,
    ) -> Result<(), CacheError>;

    /// Invalidate cache entries matching a pattern.
    /// Returns the number of entries removed.
    async fn invalidate(&self, pattern: &str) -> Result<u64, CacheError>;

    /// Cache statistics (hits, misses, size, hit rate).
    async fn stats(&self) -> CacheStats;
}

#[async_trait]
pub trait SemanticCache: Send + Sync {
    /// Find a semantically similar cached response.
    /// `threshold`: minimum cosine similarity (0.0 to 1.0, typically 0.95).
    async fn get_similar(
        &self,
        embedding: &[f32],
        threshold: f32,
    ) -> Result<Option<CachedResponse>, CacheError>;

    /// Store a response with its embedding vector.
    async fn put(
        &self,
        embedding: &[f32],
        response: &CachedResponse,
        ttl: Duration,
    ) -> Result<(), CacheError>;
}

#[derive(Debug, Clone, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub struct CacheKey {
    pub request_hash: String,  // SHA-256 of normalized request JSON
    pub model: String,
    pub org_id: Uuid,          // per-tenant cache isolation
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedResponse {
    pub response: ChatCompletionResponse,
    pub cached_at: chrono::DateTime<chrono::Utc>,
    pub model: String,
    pub provider: String,
}

#[derive(Debug, Default)]
pub struct CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub size: u64,       // approximate number of entries
    pub hit_rate: f64,   // hits / (hits + misses)
}

#[derive(Debug, thiserror::Error)]
pub enum CacheError {
    #[error("Redis error: {0}")]
    Redis(String),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Connection pool exhausted")]
    PoolExhausted,
}
```

**Design Notes:**
- Cache errors are non-fatal — `get()` returns `Ok(None)` on error (treat as miss), `put()` errors are logged but don't fail the request.
- `CacheKey` includes `org_id` for multi-tenant cache isolation. Organization A cannot see Organization B's cached responses.
- `SemanticCache` is a separate trait because it requires embedding computation (may call an embedding model or use a local embedding function). It can be disabled if not configured.
- Cache invalidation by pattern uses Redis `SCAN` + `DEL` — acceptable for single-node, not for distributed cache clusters.

---

### 8.3 Router Trait

```rust
use async_trait::async_trait;
use std::collections::HashMap;

/// Strategy for selecting a provider.
pub enum RoutingStrategy {
    /// Always use the first healthy provider in the priority list.
    Priority,
    /// Select the provider with the lowest recent average latency.
    Latency,
    /// Select the provider with the lowest cost per token.
    Cost,
    /// Round-robin across healthy providers.
    RoundRobin,
}

#[async_trait]
pub trait Router: Send + Sync {
    /// Select the best provider for the given model and request context.
    /// Returns the primary provider and an ordered list of fallbacks.
    ///
    /// # Contract
    /// - Returns `Err` only if no provider supports the requested model.
    /// - Returns `Err` only if all capable providers are unhealthy.
    /// - The fallback chain is ordered by the configured strategy.
    async fn select_provider(
        &self,
        model: &str,
        strategy: RoutingStrategy,
    ) -> Result<ProviderSelection, RouterError>;

    /// Report the result of a provider call (for latency tracking and health updates).
    async fn report_result(
        &self,
        provider_name: &str,
        latency: std::time::Duration,
        success: bool,
    );

    /// Get current health status for all providers.
    async fn health_summary(&self) -> HashMap<String, HealthStatus>;
}

#[derive(Debug, Clone)]
pub struct ProviderSelection {
    pub primary: String,
    pub fallbacks: Vec<String>,
    pub estimated_latency_ms: u64,
    pub estimated_cost_per_1k_tokens: f64,
}

#[derive(Debug, thiserror::Error)]
pub enum RouterError {
    #[error("No provider supports model '{0}'")]
    ModelNotSupported(String),

    #[error("All providers for model '{0}' are unhealthy")]
    AllProvidersUnhealthy(String),

    #[error("No providers configured")]
    NoProvidersConfigured,
}
```

**Design Notes:**
- The router is stateful — it maintains latency statistics and health status in Redis.
- `report_result()` is called after every provider call to update rolling average latency.
- Health checks run in a background task (every 30s), not inline with requests.
- Routing strategy is configurable per-model and has a global default.
- The fallback chain enables automatic failover without client involvement.

---

### 8.4 QuotaChecker Trait

```rust
use async_trait::async_trait;
use gateway_domain::types::common::TokenUsage;
use chrono::{DateTime, Utc};
use uuid::Uuid;

#[async_trait]
pub trait QuotaChecker: Send + Sync {
    /// Check if the request should be allowed based on quota/budget.
    /// Called before the provider call.
    async fn check_quota(
        &self,
        context: &RequestContext,
        estimated_cost: f64,
    ) -> Result<QuotaStatus, QuotaError>;

    /// Deduct actual usage after a successful provider call.
    async fn deduct_quota(
        &self,
        context: &RequestContext,
        actual_usage: TokenUsage,
    ) -> Result<(), QuotaError>;

    /// Get usage summary for an organization in a billing period.
    async fn get_usage(
        &self,
        org_id: Uuid,
        period: BillingPeriod,
    ) -> Result<UsageSummary, QuotaError>;
}

#[async_trait]
pub trait BillingRecorder: Send + Sync {
    /// Record a usage event for billing.
    async fn record_usage(&self, record: UsageRecord) -> Result<(), BillingError>;

    /// Get billing summary for an organization.
    async fn get_billing_summary(
        &self,
        org_id: Uuid,
        period: BillingPeriod,
    ) -> Result<BillingSummary, BillingError>;
}

#[derive(Debug, Clone)]
pub enum QuotaStatus {
    Allowed,
    Warning { remaining: f64, threshold: f64 },
    Exceeded { reason: String },
}

#[derive(Debug, Clone)]
pub struct BillingPeriod {
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct UsageSummary {
    pub requests: u64,
    pub tokens_in: u64,
    pub tokens_out: u64,
    pub total_cost: f64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub provider_breakdown: Vec<ProviderUsage>,
}

#[derive(Debug, Clone)]
pub struct ProviderUsage {
    pub provider: String,
    pub requests: u64,
    pub tokens_in: u64,
    pub tokens_out: u64,
    pub cost: f64,
}

#[derive(Debug, Clone)]
pub struct UsageRecord {
    pub request_id: Uuid,
    pub org_id: Uuid,
    pub key_id: Uuid,
    pub user_id: Option<Uuid>,
    pub provider: String,
    pub model: String,
    pub tokens_in: u64,
    pub tokens_out: u64,
    pub latency_ms: u64,
    pub cost: f64,
    pub timestamp: DateTime<Utc>,
    pub cached: bool,
}

#[derive(Debug, Clone)]
pub struct BillingSummary {
    pub period: BillingPeriod,
    pub total_cost: f64,
    pub budget_limit: Option<f64>,
    pub budget_remaining: Option<f64>,
    pub requests: u64,
}

#[derive(Debug, thiserror::Error)]
pub enum QuotaError {
    #[error("Database error: {0}")]
    Database(String),

    #[error("Quota configuration missing for org {0}")]
    ConfigMissing(Uuid),
}

#[derive(Debug, thiserror::Error)]
pub enum BillingError {
    #[error("Database error: {0}")]
    Database(String),
}
```

**Design Notes:**
- `check_quota` runs **before** the provider call (prevention). `deduct_quota` runs **after** (recording).
- `estimated_cost` in `check_quota` prevents overages — the worst case is a small underestimate due to token count differences.
- `UsageRecord` is append-only — never updated, only inserted. This is the audit trail.
- Billing aggregation is done via SQL `SUM`/`GROUP BY` queries, not application code.
- Budget enforcement is **hard stop** by default. A soft-limit mode (alert only) can be configured per-organization.

---

### 8.5 AuthValidator Trait

```rust
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[async_trait]
pub trait AuthValidator: Send + Sync {
    /// Validate an API key (for LLM API consumers).
    /// 
    /// # Contract
    /// - Returns `AuthContext` with permissions if key is valid and active.
    /// - Returns `AuthError::InvalidKey` if key doesn't exist or is malformed.
    /// - Returns `AuthError::RevokedKey` if key was revoked.
    /// - Returns `AuthError::ExpiredKey` if key has expired.
    /// - Returns `AuthError::InactiveOrg` if the organization is suspended.
    async fn validate_api_key(&self, key: &str) -> Result<AuthContext, AuthError>;

    /// Validate a session token (for admin dashboard users).
    async fn validate_session(&self, token: &str) -> Result<AuthContext, AuthError>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthContext {
    pub org_id: Uuid,
    pub key_id: Option<Uuid>,       // set for API key auth
    pub user_id: Option<Uuid>,      // set for session auth
    pub permissions: Vec<Permission>,
    pub rate_limit_tier: RateLimitTier,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Permission {
    ChatWrite,
    EmbeddingsRead,
    ModelsRead,
    Admin,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum RateLimitTier {
    Free,      // 10 req/min
    Standard,  // 100 req/min
    Premium,   // 1000 req/min
    Custom(u32), // user-defined
}

#[derive(Debug, thiserror::Error)]
pub enum AuthError {
    #[error("Invalid API key")]
    InvalidKey,

    #[error("API key has been revoked")]
    RevokedKey,

    #[error("API key expired")]
    ExpiredKey,

    #[error("Organization is inactive")]
    InactiveOrg,

    #[error("Session expired or invalid")]
    InvalidSession,

    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    #[error("Database error: {0}")]
    Database(String),
}
```

**Design Notes:**
- `AuthContext` carries all authorization information needed by downstream components. No additional DB lookups needed after auth.
- `Permission` is an enum, not a string — type-safe permission checks at compile time.
- `RateLimitTier` is resolved at auth time and carried in `AuthContext` — rate limiter doesn't need its own DB lookup.
- Session tokens are JWTs signed with a server-side secret (HS256). Token expiry is checked via JWT `exp` claim + DB revocation list.
- API keys are stored as Argon2id hashes — the full key is shown only once at creation time.

---

### 8.6 RequestLogger & Metrics Traits

```rust
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use uuid::Uuid;

#[async_trait]
pub trait RequestLogger: Send + Sync {
    /// Log a completed request. Called after the response is sent.
    fn log_request(&self, record: RequestLogRecord);

    /// Log an error that occurred during request processing.
    fn log_error(&self, error: &GatewayError, context: &RequestContext);

    /// Log an audit event (admin action, configuration change).
    fn log_audit(&self, event: AuditEvent);
}

#[async_trait]
pub trait Metrics: Send + Sync {
    /// Increment a counter metric.
    fn increment_counter(&self, name: &str, labels: &[(&str, &str)]);

    /// Record a value in a histogram (for latency distributions).
    fn record_histogram(&self, name: &str, value: f64, labels: &[(&str, &str)]);

    /// Set a gauge value (for current state, e.g., active connections).
    fn set_gauge(&self, name: &str, value: f64, labels: &[(&str, &str)]);
}

#[derive(Debug, Clone)]
pub struct RequestLogRecord {
    pub request_id: Uuid,
    pub timestamp: DateTime<Utc>,
    pub method: String,
    pub path: String,
    pub status_code: u16,
    pub latency_ms: u64,
    pub org_id: Uuid,
    pub key_id: Option<Uuid>,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub tokens_in: Option<u64>,
    pub tokens_out: Option<u64>,
    pub cost: Option<f64>,
    pub cached: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct AuditEvent {
    pub event_id: Uuid,
    pub timestamp: DateTime<Utc>,
    pub org_id: Uuid,
    pub user_id: Uuid,
    pub action: String,       // "key.created", "provider.updated", "quota.changed"
    pub resource: String,     // "api_key:123e4567..."
    pub details: serde_json::Value,
    pub ip_address: Option<String>,
}
```

**Design Notes:**
- `RequestLogger` methods take `&self` (not `&mut self`) — implementations use interior mutability (e.g., `tracing::info!` is thread-safe).
- `log_request` is synchronous and non-blocking — it writes to a channel or directly to stdout. Never blocks the request thread.
- Metrics are fire-and-forget — no return value. If metrics emission fails, it is silently dropped (metrics are advisory, not critical).
- `AuditEvent` captures every mutating admin action for compliance and debugging.

---

## 9. Error Handling Strategy

### 9.1 Error Type Hierarchy

The system uses a layered error hierarchy. Each crate defines its own error type. The API layer maps all errors to HTTP responses.

```
GatewayError (gateway-common)
├── AuthError (gateway-auth)
│   ├── InvalidKey
│   ├── RevokedKey
│   ├── ExpiredKey
│   ├── InactiveOrg
│   └── InvalidSession
├── QuotaError (gateway-quota)
│   ├── Database
│   └── ConfigMissing
├── BillingError (gateway-quota)
│   └── Database
├── ProviderError (gateway-providers)
│   ├── HttpError { status, body }
│   ├── Authentication
│   ├── RateLimited { retry_after }
│   ├── Timeout
│   ├── Serialization
│   ├── ClientError
│   ├── InvalidResponse
│   └── CircuitOpen
├── CacheError (gateway-cache)
│   ├── Redis
│   ├── Serialization
│   └── PoolExhausted
├── RouterError (gateway-domain)
│   ├── ModelNotSupported
│   ├── AllProvidersUnhealthy
│   └── NoProvidersConfigured
├── ValidationError (gateway-common)
│   ├── JsonParse
│   ├── MissingField
│   ├── InvalidField
│   └── RequestTooLarge
└── InternalError (gateway-common)
    ├── DatabaseConnectionFailed
    ├── RedisConnectionFailed
    └── Unknown
```

### 9.2 Error Propagation

Errors propagate upward via the `?` operator and `Result` types. At each boundary, errors are converted using `From` implementations or `.map_err()`.

```rust
// Example: ProviderError → GatewayError
impl From<ProviderError> for GatewayError {
    fn from(err: ProviderError) -> Self {
        match err {
            ProviderError::Authentication(_) => GatewayError::Unauthorized,
            ProviderError::RateLimited { retry_after } => GatewayError::RateLimited(retry_after),
            ProviderError::Timeout(_) => GatewayError::GatewayTimeout,
            _ => GatewayError::BadGateway(err.to_string()),
        }
    }
}
```

**Propagation Rules:**
1. Internal crate errors (database connection lost, Redis timeout) are logged at `ERROR` level and converted to generic `500 Internal Server Error` responses. Internal details are never exposed to clients.
2. Provider errors are converted to appropriate HTTP status codes (502 for provider 5xx, 504 for timeout, 429 for rate limit).
3. Auth errors always return `401 Unauthorized` (no distinction between "key doesn't exist" and "key is wrong" — prevents enumeration attacks).
4. Quota exceeded returns `403 Forbidden` or `429` depending on config.
5. Validation errors return `400 Bad Request` with a structured error body.

### 9.3 Error Response Format

All API errors follow a consistent JSON structure (OpenAI-compatible):

```json
{
  "error": {
    "type": "quota_exceeded",
    "message": "Organization budget exhausted. Remaining: $0.00 / $100.00",
    "param": null,
    "code": "budget_exceeded"
  }
}
```

**Error Types:**

| Type | HTTP Status | When |
|------|-------------|------|
| `invalid_request` | 400 | JSON parse error, missing field, invalid parameter |
| `authentication_error` | 401 | Invalid or missing API key |
| `permission_denied` | 403 | Insufficient permissions, org inactive, budget exceeded |
| `not_found` | 404 | Model not found, resource not found |
| `rate_limit_exceeded` | 429 | Per-key or per-org rate limit hit |
| `quota_exceeded` | 429 | Budget or token quota exhausted |
| `server_error` | 500 | Unexpected internal error |
| `bad_gateway` | 502 | Provider returned 5xx |
| `service_unavailable` | 503 | No healthy provider for requested model |
| `gateway_timeout` | 504 | Provider request timed out |

### 9.4 Retry Logic and Idempotency

**Provider Retry Rules:**

| Provider Error | Retry? | Max Retries | Backoff |
|----------------|--------|-------------|---------|
| 500 Internal Server Error | Yes | 2 | 1s, 2s exponential |
| 502 Bad Gateway | Yes | 2 | 1s, 2s exponential |
| 503 Service Unavailable | Yes | 3 | 1s, 2s, 4s exponential |
| 429 Rate Limited | Yes | 3 | Respect `Retry-After` header, else 2s, 4s, 8s |
| 408 Request Timeout | Yes | 2 | 1s, 2s exponential |
| Timeout (gateway-side) | Yes | 2 | 1s, 2s exponential |
| 401 Unauthorized | No | 0 | N/A (config error) |
| 400 Bad Request | No | 0 | N/A (client error) |
| 403 Forbidden | No | 0 | N/A (provider-side permission) |
| Circuit breaker open | No | 0 | Use fallback provider |

**Idempotency:**
- Chat completion requests are **NOT idempotent** — each call generates a different response.
- The gateway does NOT implement idempotency keys for chat completions.
- For idempotent operations (admin API: key creation, config updates), the client can provide an `Idempotency-Key` header. The gateway stores the key → response mapping in Redis for 24 hours.
- Embedding requests are functionally idempotent (same input → same output) and are served from cache when possible.

**Retry Implementation:**

```rust
async fn call_with_retry<P: Provider>(
    provider: &P,
    request: ChatCompletionRequest,
) -> Result<ChatCompletionResponse, ProviderError> {
    let mut last_error = None;
    for attempt in 0..MAX_RETRIES {
        match provider.chat_completion(request.clone()).await {
            Ok(response) => return Ok(response),
            Err(e) if e.is_retryable() => {
                let backoff = exponential_backoff(attempt);
                tracing::warn!(error = %e, attempt, "Provider call failed, retrying");
                tokio::time::sleep(backoff).await;
                last_error = Some(e);
            }
            Err(e) => return Err(e), // non-retryable
        }
    }
    Err(last_error.unwrap_or_else(|| ProviderError::ClientError("Max retries exceeded".into())))
}
```

---

## 10. Configuration Strategy

### 10.1 What Is Configurable

| Category | Setting | Default | Source |
|----------|---------|---------|--------|
| **Server** | bind address | `0.0.0.0:3000` | env |
| | worker threads | `4` | env |
| | request timeout | `60s` | env |
| | max body size | `10MB` | env |
| | graceful shutdown timeout | `30s` | env |
| **Database** | PostgreSQL URL | required | env |
| | max connections | `20` | env |
| | connection timeout | `5s` | env |
| **Redis** | Redis URL | required | env |
| | connection pool size | `10` | env |
| **Providers** | provider definitions | required | file |
| | model pricing | required | file |
| **Cache** | exact cache TTL | `1 hour` | file |
| | exact cache enabled | `true` | file |
| | semantic cache enabled | `false` | file |
| | semantic cache similarity threshold | `0.95` | file |
| **Quota** | default rate limit tier | `Standard` | file |
| | budget check enabled | `true` | file |
| **Auth** | session TTL | `24 hours` | env |
| | API key prefix | `sk-gw-` | file |
| **Observability** | log format | `json` | env |
| | log level | `info` | env |
| | metrics enabled | `true` | env |

### 10.2 Configuration Sources

**Environment Variables (for secrets and deployment values):**

```bash
# Required
DATABASE_URL=postgres://gateway:password@localhost:5432/gateway
REDIS_URL=redis://localhost:6379

# Server
GATEWAY_BIND=0.0.0.0:3000
GATEWAY_WORKERS=4
GATEWAY_REQUEST_TIMEOUT=60
GATEWAY_MAX_BODY_SIZE=10485760

# Auth
GATEWAY_SESSION_SECRET=changeme-this-is-a-secret-key-min-32-chars
GATEWAY_SESSION_TTL=86400

# Observability
RUST_LOG=info
GATEWAY_LOG_FORMAT=json   # "json" or "pretty"
GATEWAY_METRICS_ENABLED=true
```

**TOML Configuration File (for structural config):**

```toml
# config.toml

[cache]
exact_enabled = true
exact_ttl_seconds = 3600
semantic_enabled = false
semantic_similarity_threshold = 0.95

[quota]
default_rate_limit_tier = "Standard"
budget_check_enabled = true

[providers.openai]
enabled = true
base_url = "https://api.openai.com/v1"
api_key_env = "OPENAI_API_KEY"  # reads from this env var
models = [
    { id = "gpt-4o", input_price_per_1k = 0.005, output_price_per_1k = 0.015 },
    { id = "gpt-4o-mini", input_price_per_1k = 0.00015, output_price_per_1k = 0.0006 },
]

[providers.anthropic]
enabled = true
base_url = "https://api.anthropic.com/v1"
api_key_env = "ANTHROPIC_API_KEY"
models = [
    { id = "claude-3-5-sonnet-20241022", input_price_per_1k = 0.003, output_price_per_1k = 0.015 },
]

[providers.ollama]
enabled = true
base_url = "http://localhost:11434"
api_key_env = ""  # no API key for local Ollama
models = [
    { id = "llama3.1", input_price_per_1k = 0.0, output_price_per_1k = 0.0 },
    { id = "mistral", input_price_per_1k = 0.0, output_price_per_1k = 0.0 },
]

[routing]
default_strategy = "Priority"  # "Priority", "Latency", "Cost", "RoundRobin"

[routing.models]
"gpt-4o" = { providers = ["openai"], strategy = "Priority" }
"claude-3-5-sonnet" = { providers = ["anthropic"], strategy = "Priority" }
```

### 10.3 Configuration Precedence

```
1. Environment variables (highest precedence — override everything)
2. TOML configuration file
3. Hardcoded defaults (lowest precedence)
```

Rules:
- Env vars use `GATEWAY_` prefix (e.g., `GATEWAY_BIND`, `GATEWAY_DATABASE_URL`)
- Env vars override TOML values for the same setting
- Provider API keys are NEVER in the TOML file — always in env vars referenced by `api_key_env`
- If a required setting is missing, the application fails to start with a clear error message

### 10.4 Validation

Configuration validation runs at startup, before any network connections are opened:

```rust
impl AppConfig {
    pub fn validate(&self) -> Result<(), Vec<ValidationError>> {
        let mut errors = Vec::new();

        // Server
        if self.server.workers == 0 {
            errors.push(ValidationError::new("server.workers", "must be > 0"));
        }

        // Database URL format
        if !self.database.url.starts_with("postgres://") {
            errors.push(ValidationError::new("database.url", "must be a valid PostgreSQL URL"));
        }

        // At least one provider must be enabled
        if self.providers.iter().all(|p| !p.enabled) {
            errors.push(ValidationError::new("providers", "at least one provider must be enabled"));
        }

        // Every enabled provider must have a non-empty base_url
        for provider in &self.providers {
            if provider.enabled && provider.base_url.is_empty() {
                errors.push(ValidationError::new(
                    &format!("providers.{}.base_url", provider.name),
                    "must not be empty when provider is enabled",
                ));
            }
        }

        // Every model must have a price (even if 0.0)
        for provider in &self.providers {
            for model in &provider.models {
                if model.input_price_per_1k < 0.0 || model.output_price_per_1k < 0.0 {
                    errors.push(ValidationError::new(
                        &format!("providers.{}.models.{}", provider.name, model.id),
                        "prices must be non-negative",
                    ));
                }
            }
        }

        // Cache TTL must be > 0
        if self.cache.exact_ttl_seconds == 0 {
            errors.push(ValidationError::new("cache.exact_ttl_seconds", "must be > 0"));
        }

        // Semantic threshold must be in (0, 1]
        if self.cache.semantic_similarity_threshold <= 0.0 
            || self.cache.semantic_similarity_threshold > 1.0 {
            errors.push(ValidationError::new(
                "cache.semantic_similarity_threshold",
                "must be in range (0.0, 1.0]",
            ));
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}
```

**Validation failure behavior:** Print all validation errors to stderr and exit with code `1`. Do not start the server with invalid config.

### 10.5 Secrets Management

| Secret | Storage | Rotation |
|--------|---------|----------|
| PostgreSQL password | `DATABASE_URL` env var | Manual: update env, restart container |
| Redis password | `REDIS_URL` env var | Manual: update env, restart container |
| Provider API keys | Individual env vars (`OPENAI_API_KEY`, etc.) | Manual: update env, restart; or via admin API (stored in DB encrypted) |
| Session signing key | `GATEWAY_SESSION_SECRET` env var | Manual: invalidates all sessions on change |
| Gateway API keys (issued to consumers) | PostgreSQL (Argon2 hashed) | Via admin dashboard: revoke old, create new |

**Provider API Key Storage in Database:**

When configured via the admin dashboard (not env vars), provider API keys are stored in PostgreSQL:
- Encryption at rest: AES-256-GCM with a key derived from `GATEWAY_MASTER_KEY` env var
- The master key is NOT stored in the database — it is a deployment secret
- Keys are encrypted before INSERT and decrypted after SELECT
- The encryption key is loaded once at startup and held in memory

**Security Requirements:**
- `GATEWAY_SESSION_SECRET` must be >= 32 bytes of random data
- `GATEWAY_MASTER_KEY` must be >= 32 bytes of random data
- Argon2id parameters for API key hashing: memory=64MB, iterations=3, parallelism=4
- All secrets are env vars (12-factor app compliance)
- No secrets in logs, error messages, or API responses

---

## Appendix A: Database Schema

```sql
-- Organizations (tenant isolation)
CREATE TABLE organizations (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name TEXT NOT NULL,
    slug TEXT UNIQUE NOT NULL,
    plan TEXT NOT NULL DEFAULT 'free', -- free, standard, premium
    budget_limit_usd DECIMAL(12,4),
    budget_alert_threshold DECIMAL(5,2), -- percentage, e.g., 0.80 for 80%
    is_active BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Users (dashboard login)
CREATE TABLE users (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    email TEXT NOT NULL,
    password_hash TEXT NOT NULL, -- Argon2id
    is_admin BOOLEAN NOT NULL DEFAULT false,
    is_active BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(org_id, email)
);

-- API Keys (issued to LLM API consumers)
CREATE TABLE api_keys (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    key_hash TEXT UNIQUE NOT NULL, -- Argon2id hash of the full key
    permissions TEXT[] NOT NULL DEFAULT '{chat:write,embeddings:read,models:read}',
    rate_limit_tier TEXT NOT NULL DEFAULT 'Standard',
    expires_at TIMESTAMPTZ,
    is_active BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_used_at TIMESTAMPTZ
);

-- Provider Configurations
CREATE TABLE provider_configs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    provider_name TEXT NOT NULL, -- "openai", "anthropic", etc.
    is_enabled BOOLEAN NOT NULL DEFAULT true,
    base_url TEXT,
    api_key_encrypted TEXT, -- AES-256-GCM encrypted
    config_json JSONB, -- provider-specific settings
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(org_id, provider_name)
);

-- Models (available models with pricing)
CREATE TABLE models (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id UUID REFERENCES organizations(id) ON DELETE CASCADE, -- null = system default
    provider_name TEXT NOT NULL,
    model_id TEXT NOT NULL, -- gateway-facing name, e.g., "gpt-4o"
    provider_model_id TEXT NOT NULL, -- provider's internal name
    input_price_per_1k DECIMAL(12,8) NOT NULL,
    output_price_per_1k DECIMAL(12,8) NOT NULL,
    is_enabled BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(org_id, model_id)
);

-- Quotas
CREATE TABLE quotas (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    scope TEXT NOT NULL, -- "organization", "user", "api_key"
    scope_id UUID, -- null for org-level
    requests_per_minute INT,
    tokens_per_day BIGINT,
    budget_per_month_usd DECIMAL(12,4),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Usage Records (append-only)
CREATE TABLE usage_records (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    request_id UUID NOT NULL,
    org_id UUID NOT NULL REFERENCES organizations(id),
    key_id UUID REFERENCES api_keys(id),
    user_id UUID REFERENCES users(id),
    provider TEXT NOT NULL,
    model TEXT NOT NULL,
    tokens_in BIGINT NOT NULL DEFAULT 0,
    tokens_out BIGINT NOT NULL DEFAULT 0,
    latency_ms BIGINT NOT NULL,
    cost_usd DECIMAL(12,8) NOT NULL DEFAULT 0,
    cached BOOLEAN NOT NULL DEFAULT false,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Indexes for common queries
CREATE INDEX idx_usage_org_created ON usage_records(org_id, created_at);
CREATE INDEX idx_usage_key_created ON usage_records(key_id, created_at);
CREATE INDEX idx_usage_provider ON usage_records(provider, created_at);
CREATE INDEX idx_api_keys_hash ON api_keys(key_hash);
CREATE INDEX idx_models_org ON models(org_id, model_id);

-- Audit Log
CREATE TABLE audit_log (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    timestamp TIMESTAMPTZ NOT NULL DEFAULT now(),
    org_id UUID NOT NULL,
    user_id UUID,
    action TEXT NOT NULL,
    resource TEXT NOT NULL,
    details JSONB,
    ip_address INET
);

CREATE INDEX idx_audit_org ON audit_log(org_id, timestamp);
```

## Appendix B: Deployment Architecture

```
┌──────────────────────────────────────────────────────────────┐
│                       VPS (Single Node)                       │
│                                                               │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────────┐    │
│  │   Nginx      │  │   Gateway    │  │    PostgreSQL    │    │
│  │   :443       │──│   :3000      │──│     :5432        │    │
│  │              │  │              │  │                  │    │
│  │ TLS terminate│  │ Axum +       │  │ Persistent data  │    │
│  │ Static files │  │ Business     │  │ Migrations on    │    │
│  │ (admin UI)   │  │ Logic        │  │ startup          │    │
│  └──────────────┘  └──────┬───────┘  └──────────────────┘    │
│                            │                                  │
│                            ▼                                  │
│                     ┌──────────────┐                          │
│                     │    Redis     │                          │
│                     │    :6379     │                          │
│                     │              │                          │
│                     │ Cache, rate  │                          │
│                     │ limiting     │                          │
│                     └──────────────┘                          │
│                                                               │
└──────────────────────────────────────────────────────────────┘
                              │
                              ▼
                     ┌──────────────────┐
                     │ External LLM APIs│
                     │ OpenAI, Anthropic│
                     │ Gemini, Ollama   │
                     └──────────────────┘
```

**Docker Compose:**

```yaml
# docker-compose.yml
version: "3.8"
services:
  gateway:
    build: .
    ports:
      - "3000:3000"
    environment:
      DATABASE_URL: postgres://gateway:${DB_PASSWORD}@postgres:5432/gateway
      REDIS_URL: redis://redis:6379
      GATEWAY_SESSION_SECRET: ${SESSION_SECRET}
    depends_on:
      postgres:
        condition: service_healthy
      redis:
        condition: service_started

  postgres:
    image: postgres:16-alpine
    volumes:
      - postgres_data:/var/lib/postgresql/data
    environment:
      POSTGRES_USER: gateway
      POSTGRES_PASSWORD: ${DB_PASSWORD}
      POSTGRES_DB: gateway
    healthcheck:
      test: ["CMD-SHELL", "pg_isready -U gateway"]
      interval: 5s
      timeout: 5s
      retries: 5

  redis:
    image: redis:7-alpine
    volumes:
      - redis_data:/data
    command: redis-server --appendonly yes --maxmemory 256mb --maxmemory-policy allkeys-lru

  nginx:
    image: nginx:alpine
    ports:
      - "80:80"
      - "443:443"
    volumes:
      - ./nginx.conf:/etc/nginx/nginx.conf
      - ./web/dist:/usr/share/nginx/html/admin
    depends_on:
      - gateway

volumes:
  postgres_data:
  redis_data:
```

**Operational Runbook (one person):**

| Task | Command | Time |
|------|---------|------|
| Deploy | `docker compose up -d` | 2 min |
| View logs | `docker compose logs -f gateway` | 10 sec |
| View DB | `docker compose exec postgres psql -U gateway -d gateway` | 10 sec |
| View Redis | `docker compose exec redis redis-cli` | 10 sec |
| Rotate session secret | 1. `export GATEWAY_SESSION_SECRET=$(openssl rand -hex 32)` 2. `docker compose up -d` | 2 min |
| Backup DB | `docker compose exec postgres pg_dump -U gateway gateway > backup.sql` | 5 min |
| Restore DB | `docker compose exec -T postgres psql -U gateway < backup.sql` | 10 min |
| Scale up | Increase `GATEWAY_WORKERS`, increase VPS CPU/RAM, `docker compose up -d` | 5 min |
| Add provider | Edit `config.toml`, `docker compose restart gateway` | 2 min |

---

## Appendix C: Metrics and Alerting

### Metrics Exposed

| Metric | Type | Labels | Description |
|--------|------|--------|-------------|
| `gateway_requests_total` | Counter | `method`, `path`, `status` | Total HTTP requests |
| `gateway_request_duration_ms` | Histogram | `method`, `path` | Request latency distribution |
| `gateway_provider_requests_total` | Counter | `provider`, `model`, `status` | Requests per provider |
| `gateway_provider_latency_ms` | Histogram | `provider`, `model` | Provider latency |
| `gateway_cache_hits_total` | Counter | `cache_type` | Cache hit count |
| `gateway_cache_misses_total` | Counter | `cache_type` | Cache miss count |
| `gateway_tokens_total` | Counter | `provider`, `model`, `direction` | Tokens consumed |
| `gateway_cost_usd_total` | Counter | `provider`, `model`, `org_id` | Cost incurred |
| `gateway_quota_checks_total` | Counter | `result` | Quota check outcomes |
| `gateway_active_connections` | Gauge | — | Current active HTTP connections |
| `gateway_db_pool_size` | Gauge | — | DB connection pool utilization |

### Health Endpoints

| Endpoint | Purpose | Response |
|----------|---------|----------|
| `GET /health` | Liveness probe | `{"status":"ok"}` (200) |
| `GET /ready` | Readiness probe | `{"status":"ready","checks":{"database":true,"redis":true}}` (200/503) |
| `GET /metrics` | Prometheus metrics | Text format (200) |

### Recommended Alerts (External Monitoring)

Configure external monitoring (UptimeRobot, Pingdom, or Grafana Cloud) to check:

| Alert | Condition | Severity |
|-------|-----------|----------|
| Gateway down | `/health` returns non-200 for 2 min | Critical |
| High error rate | `gateway_requests_total{status=~"5.."}` / total > 0.05 for 5 min | Warning |
| High latency | `gateway_request_duration_ms` p99 > 30000 for 5 min | Warning |
| Provider failure | `gateway_provider_requests_total{status!="200"}` spike | Warning |
| Budget approaching | Org usage > 80% of budget | Info |
| DB connection pool | `gateway_db_pool_size` > 80% capacity | Warning |

---

*End of Architecture Document*
