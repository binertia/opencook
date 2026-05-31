# Scalability Architecture — Single-Node AI Gateway

**Document Version:** 1.0
**Scope:** Single VPS deployment (monolith, non-distributed)
**Target Latency Budget:** <5ms gateway overhead (excluding provider latency)
**Tech Stack:** Rust (Axum), PostgreSQL 16, Redis 7, Docker Compose

---

## Table of Contents

1. [Scaling Philosophy](#1-scaling-philosophy)
2. [Capacity Planning](#2-capacity-planning)
3. [Caching Strategy](#3-caching-strategy)
4. [Connection Management](#4-connection-management)
5. [Async Processing](#5-async-processing)
6. [Rate Limiting Architecture](#6-rate-limiting-architecture)
7. [Request Coalescing](#7-request-coalescing)
8. [Database Scalability](#8-database-scalability)
9. [Bottleneck Analysis](#9-bottleneck-analysis)
10. [Scaling Triggers](#10-scaling-triggers)

---

## 1. Scaling Philosophy

### Principle: Vertical-First, Single-Node-by-Design

This gateway is architected for a single VPS not as a limitation, but as an intentional constraint that eliminates an entire class of distributed systems failures. The product's core differentiator — deploy in <10 minutes on a $20/month VPS — is only possible because the system does not require horizontal scaling for its target workload.

### Scaling Ladder

| Stage | Trigger | Action | VPS Spec | Cost (est.) |
|-------|---------|--------|----------|-------------|
| 0 | Prototype / testing | Shared CPU, minimal resources | 1 vCPU, 2 GB RAM | $6-12/mo |
| 1 | Production, <10 req/s | Dedicated resources, SSD | 2 vCPU, 4 GB RAM | $24-48/mo |
| 2 | 10-100 req/s sustained | More cores for concurrency | 4 vCPU, 8 GB RAM | $48-96/mo |
| 3 | 100-500 req/s | Memory for caching, DB tuning | 8 vCPU, 16 GB RAM | $96-192/mo |
| 4 | 500-1000 req/s | Near single-node limit | 16 vCPU, 32 GB RAM | $192-384/mo |
| 5 | >1000 req/s sustained | Architecture re-evaluation required | N/A (see Section 10) | >$384/mo |

### Why We Avoid Horizontal Scaling for MVP

1. **Operational simplicity**: Single-node deployment is the #1 competitive differentiator. No consensus protocols, no network partitions, no service discovery.
2. **SME workload reality**: 90% of target customers process <100 req/s. The highest-traffic SME in our segment is unlikely to exceed 500 req/s sustained.
3. **Rust efficiency**: Reference benchmarks (Helicone's Rust gateway) achieve 8ms P50 latency with 64MB memory footprint. Our architecture targets similar efficiency.
4. **Failure mode simplicity**: Single-node failures are total (detectable immediately) versus partial failures in distributed systems (subtle, hard to debug).
5. **Engineering bandwidth**: Solo founder + <5 engineers. Building distributed consensus, cluster membership, and split-brain handling is a multi-quarter effort.

### When We Would Re-Architecture

Re-architecture to multi-node becomes necessary ONLY when ALL of the following are true:
- Sustained throughput >1000 req/s for 30 consecutive days
- Single VPS vertical scaling exceeds $500/month (cost efficiency degrades)
- At least one of: CPU >90% for >12 hours/day, memory >90% for >12 hours/day, or network I/O saturates the VPS NIC
- Customer count exceeds 100 paying SaaS customers (validates revenue to fund infrastructure work)

**Until then, vertical scaling + aggressive caching + request coalescing is sufficient.**

### Theoretical Single-Node Ceiling

Given our tech stack (Rust async, single binary), the theoretical maximum on a 16 vCPU/32 GB VPS:
- **HTTP requests:** ~5000-8000 req/s (I/O bound on network, not CPU)
- **LLM proxy requests:** ~2000-3000 req/s (connection pool to providers is the bottleneck)
- **Practical sustained:** ~1000-1500 req/s (accounting for PostgreSQL write throughput, Redis round-trips, and memory pressure)

The real limit is not the gateway itself but PostgreSQL write throughput for request logging and Redis memory for response caching.

---

## 2. Capacity Planning

### 2.1 Request Handling Architecture

#### Concurrency Model

The gateway uses a multi-threaded async runtime (Tokio) with the following configuration:

```
Worker threads:     equal to vCPU count (auto-detected)
Blocking threads:   512 (for sync DB operations)
Max connections:    10,000 (per-instance HTTP listener)
Request timeout:    120s (configurable per provider)
```

#### Connection Pool Sizing

**Formula:** `pool_size = (vCPU * 2) + 1` for CPU-bound work; `pool_size = vCPU * 4` for I/O-bound work. AI gateway is I/O-bound (waiting on provider responses).

| Target | Description | Pool Size | Rationale |
|--------|-------------|-----------|-----------|
| Provider HTTP | Outbound to OpenAI, Anthropic, etc. | 100-500 | High: LLM responses take 5-30s, so many concurrent connections needed |
| PostgreSQL | Application DB | 10-40 | Low: most DB work is short queries; writes are batched |
| Redis | Cache + rate limiting | 20-100 | Medium: very fast operations, multiplexing via pipelining |

**Per-Scale Configuration:**

| Scale | vCPU | Provider Pool | PostgreSQL Pool | Redis Pool | Max Concurrent |
|-------|------|---------------|-----------------|------------|----------------|
| 10 req/s | 2 | 50 | 10 | 20 | 100 |
| 100 req/s | 4 | 100 | 20 | 40 | 500 |
| 1000 req/s | 16 | 500 | 40 | 100 | 2000 |

#### Timeout Strategy

| Operation | Timeout | Retry | Rationale |
|-----------|---------|-------|-----------|
| Provider request (chat) | 120s | 1 retry, different provider | LLM responses are inherently slow |
| Provider request (embeddings) | 30s | 2 retries | Embeddings are faster, should not hang |
| Cache read (Redis) | 50ms | 1 retry | Cache must be fast or it's not a cache |
| Cache read (in-process) | 1ms | 0 | In-process cache is local memory; if it fails, something is wrong |
| DB query (hot path) | 100ms | 0 (fail fast) | Hot path queries must be sub-100ms |
| DB query (background) | 5s | 1 retry | Background tasks can tolerate slower queries |
| Rate limit check | 20ms | 0 | Rate limiting must not add latency |

### 2.2 Resource Usage Projections

**Methodology:**
- CPU: 1 request = ~0.5ms CPU time (routing + serialization + cache check + DB log write). Rust async runtime overhead is minimal. At 1000 req/s, CPU is ~50% utilized on 8 vCPU.
- RAM: In-process cache (provider configs, rate limit state) + connection buffers + request/response bodies in flight. Estimate: 50MB base + 1MB per 10 concurrent requests (buffering) + cache size.
- DB connections: Each connection ~5MB RAM. Pool size directly determines connection count.
- Redis memory: Response cache dominates. Average LLM response ~2KB (compressed). At 1000 req/s with 1-hour TTL = ~7.2M responses = ~14GB max. Pruned by TTL and LRU.
- Network: Each proxied request = request body + response body. Average ~5KB per direction. At 1000 req/s = ~10MB/s = 80 Mbps (well within 1Gbps VPS).
- PostgreSQL load: Write-heavy (every request is logged). ~1000 writes/sec at peak. PostgreSQL on SSD handles 3000-5000 simple writes/sec.

| Metric | 10 req/s | 100 req/s | 1000 req/s |
|--------|----------|-----------|------------|
| **CPU** | 2% (0.04 cores) | 15% (0.6 cores) | 80% (12.8 cores on 16-vCPU) |
| **RAM** | 128 MB | 512 MB | 8 GB (mostly response cache) |
| **DB connections** | 5 active | 15 active | 35 active |
| **Redis memory** | 10 MB | 500 MB | 8 GB (TTL-limited response cache) |
| **Network** | 0.5 Mbps | 5 Mbps | 80 Mbps |
| **PostgreSQL IOPS** | 20 write IOPS | 200 write IOPS | 2000 write IOPS |
| **PostgreSQL load** | 1% CPU | 5% CPU | 40% CPU |

**Notes:**
- CPU at 1000 req/s assumes aggressive caching (20% hit rate) reduces actual provider calls to 800/sec.
- Redis memory at 1000 req/s assumes 1-hour TTL on cached responses; actual will be lower due to LRU eviction at memory limit.
- PostgreSQL load assumes batched writes (see Section 5) — without batching, write IOPS would be 5x higher.
- These projections assume an 8 vCPU / 16 GB RAM VPS at 100 req/s and a 16 vCPU / 32 GB VPS at 1000 req/s.

### 2.3 Cost Projections Per Scale Tier

| Component | 10 req/s | 100 req/s | 1000 req/s |
|-----------|----------|-----------|------------|
| VPS (Hetzner / DigitalOcean) | $24/mo | $48/mo | $192/mo |
| PostgreSQL (same VPS) | included | included | included |
| Redis (same VPS) | included | included | included |
| Backup storage | $5/mo | $10/mo | $30/mo |
| **Total infrastructure** | **$29/mo** | **$58/mo** | **$222/mo** |

---

## 3. Caching Strategy

### 3.1 Multi-Level Cache Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│                         REQUEST FLOW                                │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│   Request ──► L1 (In-Process) ──► L2 (Redis) ──► L3 (Provider)   │
│                  DashMap/Moka          Response cache               │
│                                                                     │
│   L1: ~1-5 microsecond lookup                                       │
│   L2: ~0.5-2 millisecond lookup                                     │
│   L3: ~50-5000 millisecond (provider RTT)                           │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

#### L1: In-Process Cache (dashmap + moka)

**Purpose:** Hot data that is read on every request and changes rarely.

**Contents:**
- Provider configurations (endpoint URLs, API keys, models, weights)
- Organization settings (rate limits, budget caps, enabled providers)
- Token bucket state for rate limiting (synced to Redis periodically)
- Compiled regex patterns and routing rules

**Specifications:**
```
Library:            moka (sync_cache) for TTL + LRU eviction
Max entries:        10,000
Max memory:         50 MB
TTL:                60 seconds for provider configs
                    300 seconds for organization settings
                    10 seconds for rate limit state (sync window)
Eviction policy:    LRU when memory limit reached
Hit rate target:    >95%
```

**Why moka over dashmap alone:** moka provides built-in TTL, size-based eviction, and the `sync_cache` variant supports entry-level expiration. dashmap alone would require manual eviction logic.

**Rationale for in-process:** Provider config lookup happens on every request. At 1000 req/s, 60-second TTL means we make 60,000 requests before one Redis round-trip to refresh. In-process eliminates network latency entirely for this hot path.

#### L2: Redis Cache

**Purpose:** Response caching (exact and semantic), cross-instance sharing (if multi-instance ever needed), and distributed rate limit state.

**Contents:**
- Exact-match response cache (prompt hash -> response)
- Semantic cache embeddings (prompt embedding -> response)
- Provider health status (circuit breaker state)
- Aggregated cost counters (for budget tracking)

**Specifications:**
```
Connection model:   Multiplexed (pipelining for batch ops)
Max memory:         75% of available RAM (configurable)
Eviction policy:    allkeys-lru (Redis handles eviction)
Persistence:        disabled for cache data (RDB only for non-cache)
Key prefix:         "agw:{version}:{tenant}:{type}"
```

**Cache Segments in Redis:**

| Segment | Key Pattern | TTL | Memory Model |
|---------|-------------|-----|-------------|
| Exact response | `agw:v1:{org}:ex:{hash}` | 1 hour default | Full response body (compressed) |
| Semantic vector | `agw:v1:{org}:sem:{embedding_hash}` | 1 hour default | Embedding + response reference |
| Provider health | `agw:v1:health:{provider}` | 30 seconds | Circuit state (OPEN/HALF_OPEN/CLOSED) |
| Cost counter | `agw:v1:{org}:cost:{period}` | 24 hours | Running cost counter (atomic incr) |

#### L3: Provider-Level Cache Hints

Some providers (e.g., Cloudflare AI Gateway, certain OpenAI endpoints) support cache control headers. We pass through:
- `Cache-Control: no-cache` when user requests fresh response
- `If-None-Match: {etag}` when we have a cached version (provider may return 304)
- Accept provider-side caching as a transparent optimization (no special handling needed)

### 3.2 Cache Key Strategy

#### Exact-Match Cache Keys

```rust
fn exact_cache_key(req: &ChatRequest, org_id: &str) -> String {
    // Key components in order of significance:
    // 1. Organization ID (cache isolation)
    // 2. Provider + model (different providers = different responses)
    // 3. Temperature, max_tokens, top_p (generation parameters)
    // 4. Messages content (SHA-256 hash of normalized JSON)
    // 5. System prompt (if separate from messages)
    
    let params_hash = sha256(&json!({
        "temperature": req.temperature,
        "max_tokens": req.max_tokens,
        "top_p": req.top_p,
        "presence_penalty": req.presence_penalty,
        "frequency_penalty": req.frequency_penalty,
    }));
    
    let messages_hash = sha256(&normalize_messages(&req.messages));
    
    format!("agw:v1:{}:ex:{}:{}", org_id, params_hash, messages_hash)
}
```

**Normalization rules:**
- Messages sorted by role order (system, user, assistant, tool)
- Whitespace normalized (trim, collapse multiple spaces)
- JSON keys sorted alphabetically before hashing
- Tool call IDs stripped (not semantically relevant for caching)

#### Semantic Cache Keys

Semantic caching uses vector similarity, not exact matching:

```rust
fn semantic_cache_lookup(
    embedding: &[f32],           // Generated by embedding model (local or API)
    threshold: f32,              // Cosine similarity threshold (default 0.95)
    org_id: &str,
) -> Option<CachedResponse> {
    // 1. Compute embedding for current request
    // 2. Query Redis vector index (RedisJSON + RediSearch, or HNSW if available)
    // 3. Find nearest neighbor above threshold
    // 4. Return cached response if found
}
```

**Embedding generation:**
- Use a lightweight local model (e.g., `sentence-transformers/all-MiniLM-L6-v2` via ONNX Runtime, ~80MB RAM) for sub-100ms embedding generation
- Fallback: skip semantic cache if embedding model is unavailable (graceful degradation)
- Cache embeddings themselves in-process for identical prompts (common in batch workloads)

**Semantic cache key in Redis:**
```
agw:v1:{org}:sem:{l2_hash_of_embedding}
Value: { "embedding": [f32; 384], "response_hash": "...", "timestamp": unix_ms }
```

The L2 hash of the embedding enables approximate nearest-neighbor lookup. Full embedding stored for precise similarity recalculation (the L2 hash may have collisions).

### 3.3 TTL Strategy Per Cache Type

| Cache Type | Default TTL | Max TTL | Configurable | Eviction Trigger |
|------------|-------------|---------|-------------|-----------------|
| Provider configs | 60s | 300s | Per org | Config update webhook/API call |
| Organization settings | 300s | 3600s | No | Settings update |
| Rate limit state | 10s | 60s | No | Sliding window expiration |
| Exact response cache | 3600s | 86400s | Per endpoint | TTL expiry, LRU, manual purge |
| Semantic cache entry | 3600s | 86400s | Per endpoint | TTL expiry, LRU |
| Provider health | 30s | 60s | No | Successful health check |
| Cost counters | 86400s | 86400s | No | Period rollover |

**Rationale for response cache TTL of 1 hour:**
- LLM responses to identical prompts do not change significantly within 1 hour (models are static versions)
- Most "cacheable" traffic (repeated prompts, template-based applications) has high temporal locality
- 1 hour balances cache hit rate against stale response risk
- User can override per-request via `X-Cache-TTL` header (range: 0 to max_ttl)

### 3.4 Eviction Policy

**In-process (moka):**
- Size-based eviction: max 50MB or 10,000 entries, whichever comes first
- TTL-based expiration: entries auto-expire per their TTL
- No manual eviction needed (auto-handled by moka)

**Redis:**
- `allkeys-lru` eviction: when maxmemory reached, least recently used keys evicted
- Segmented by key prefix: all cache keys share prefix `agw:*`, allowing `SCAN + DEL` for manual purge
- Memory split: 70% exact cache, 20% semantic cache, 10% metadata (health, counters)

### 3.5 Cache Warming / Preloading

**No automatic cache warming.** Rationale:
- AI requests are highly variable (unique prompts); warming is mostly ineffective
- The cost of cache misses is a provider API call, not a DB query (acceptable)
- Cache warming would require predicting future requests, which is not feasible for LLM workloads

**Exception — Provider config warming:**
- On startup, preload all provider configs and organization settings into L1 cache
- Reduces cold-start latency from ~50ms (Redis fetch) to ~1ms (in-process hit)
- Triggered by: application startup, provider config change webhook

### 3.6 Cache Invalidation Triggers

| Trigger | Action | Latency |
|---------|--------|---------|
| Provider config updated | Purge L1 entry for that provider; L2 purged on TTL | <10ms |
| Organization settings updated | Purge L1 entry for that org; L2 purged on TTL | <10ms |
| Manual cache purge API (`POST /admin/cache/purge`) | Delete all keys matching prefix | <100ms |
| Per-organization purge (`POST /admin/cache/purge/{org}`) | Delete keys with org prefix | <50ms |
| TTL expiration | Automatic, no action needed | N/A |
| Model version change (detected via provider API) | Invalidate all semantic + exact cache for that model | <5s |

**Cache invalidation strategy:** TTL-first, manual purge for emergencies. No pub/sub invalidation (single-node, so L1 invalidation is a simple in-process operation).

### 3.7 Semantic Caching for LLM

**Problem:** Exact-match caching has near-zero hit rate for natural language prompts because users rephrase the same question differently. Semantic caching matches meaning, not text.

**Implementation:**

1. **Embedding generation:** For each incoming request, compute a 384-dimensional embedding of the concatenated message content. Use ONNX Runtime with `all-MiniLM-L6-v2` (~80MB RAM, ~20ms CPU per request on modern hardware).

2. **Similarity search:** Query Redis for embeddings with cosine similarity > 0.95. Redis 7.2+ with RediSearch supports vector similarity search (KNN). For single-node deployment without RediSearch, use in-process HNSW index (hnsw crate, ~50MB for 100K vectors).

3. **Cache hit:** If similarity > threshold, return the cached response. Update access time for LRU tracking.

4. **Cache miss:** Forward to provider. Store response with embedding for future lookups.

**Hit Rate Expectations by Workload Type:**

| Workload Type | Exact Hit Rate | Semantic Hit Rate | Combined |
|---------------|---------------|-------------------|----------|
| Customer support FAQ bot | 5% | 25-35% | 30-40% |
| Code generation (repetitive patterns) | 15% | 20-30% | 35-45% |
| RAG applications (unique queries) | 2% | 5-10% | 7-12% |
| Content summarization | 3% | 15-20% | 18-23% |
| Chat (conversational, unique) | 1% | 3-5% | 4-6% |

**Overall target: 20% combined hit rate** (validated by Helicone's 95% cost reduction claim at high semantic cache efficiency, though 20% is realistic for mixed workloads).

### 3.8 Per-Provider Caching Considerations

| Provider | Cache Benefit | Streaming Cache | Notes |
|----------|--------------|-----------------|-------|
| OpenAI | High | Partial (store final) | Most traffic, highest ROI on cache |
| Anthropic | High | Partial (store final) | Similar to OpenAI |
| Gemini | Medium | Partial | Less traffic typically |
| Ollama (local) | **None** | N/A | Local inference is "free"; caching adds no value |
| Azure OpenAI | High | Partial | Enterprise deployments, repeated queries |
| Cohere | Low | Partial | Less common for chat use cases |
| Mistral | Medium | Partial | Similar to OpenAI |

**Streaming response caching:**
- Streamed responses are cached as complete responses after the stream finishes
- The cached entry is a JSON array of chunks, reconstructed on cache hit as a synthetic stream
- Cache key computed from the original request (before streaming begins)
- Partial streaming responses (client disconnects mid-stream) are NOT cached (incomplete)
- On cache hit of a streaming request: emit chunks from cache with artificial 10ms inter-chunk delay to simulate streaming

---

## 4. Connection Management

### 4.1 HTTP Connection Pooling to Providers

**Library:** `reqwest` with `hyper` connection pooling (default in Rust ecosystem)

**Pool Configuration:**
```rust
let client = reqwest::Client::builder()
    .pool_max_idle_per_host(20)           // Keep 20 idle connections per provider host
    .pool_idle_timeout(Duration::from_secs(90))  // Keep idle conns alive 90s
    .timeout(Duration::from_secs(120))    // Per-request timeout
    .connect_timeout(Duration::from_secs(10))    // TCP connect timeout
    .tcp_keepalive(Duration::from_secs(60))      // TCP keepalive probes
    .http2_adaptive_window(true)          // Enable HTTP/2 flow control adaptation
    .build()?;
```

**Per-Provider Connection Limits:**

| Provider | Max Concurrent | Pool Size | Rationale |
|----------|---------------|-----------|-----------|
| OpenAI | 100 | 50 | High traffic, reliable |
| Anthropic | 80 | 40 | Slightly lower concurrency cap |
| Gemini | 60 | 30 | Lower volume |
| Ollama (local) | 20 | 10 | Local resource constraints |
| Azure OpenAI | 100 | 50 | Enterprise, high throughput |
| Fallback pool | 50 | 25 | Shared across secondary providers |

**Rationale for these limits:** Each concurrent LLM request holds a connection open for 5-30 seconds (time to first token + generation). At 100 req/s to OpenAI with 15s average response time, we need 1500 concurrent connections. The pool limit of 100 concurrent means we can have 100 requests in flight to OpenAI simultaneously; excess requests wait in a queue with timeout.

### 4.2 Keep-Alive Configuration

| Setting | Value | Rationale |
|---------|-------|-----------|
| TCP keepalive interval | 60s | Detect dead connections without waiting for timeout |
| Idle connection timeout | 90s | Balance between connection reuse and resource waste |
| HTTP/2 ping interval | 30s | Keep HTTP/2 connections alive (multiplexed) |
| Connection retry on failure | 1 immediate retry | Transient network blip protection |

### 4.3 Circuit Breaker Pattern

**States:** `CLOSED` (normal) → `OPEN` (failing) → `HALF_OPEN` (testing recovery)

**Configuration per provider:**
```rust
struct CircuitBreakerConfig {
    failure_threshold: u32,      // 5 consecutive failures
    success_threshold: u32,      // 2 consecutive successes to close
    timeout_secs: u64,           // 30 seconds in OPEN before HALF_OPEN
    half_open_max_requests: u32, // 3 test requests in HALF_OPEN
    error_types: Vec<ErrorType>, // Which errors count as failures
}
```

**Default per-provider circuit breaker:**
- Failure threshold: 5 consecutive failures
- Recovery timeout: 30 seconds
- Half-open test requests: 3 successful requests required to close
- Counted errors: timeouts, 5xx responses, connection refused
- NOT counted: 4xx (client error), 429 (rate limited by provider — separate handling)

**Circuit breaker state storage:**
- L1 (in-process): Current state per provider (CLOSED/OPEN/HALF_OPEN)
- L2 (Redis): State persistence across restarts, state visible to other instances (if ever)
- State transitions logged as events for observability

**Action when circuit OPEN:**
1. Stop sending requests to that provider
2. Attempt fallback to next provider in routing chain
3. If no fallback available, return 503 with `Retry-After: {circuit_timeout}` header
4. Emit alert to admin dashboard

### 4.4 Backpressure

**When incoming requests exceed capacity, apply backpressure in this order:**

1. **Request queue:** Max 1000 queued requests (configurable). When full, return `503 Service Unavailable` with `Retry-After` header.
2. **Per-provider concurrency limit:** When a provider's concurrent connection limit is reached, queue requests with 30s timeout. Queue full → try fallback provider.
3. **Adaptive timeout:** When system load >80%, reduce request timeout by 25% to fail faster and free resources.
4. **Graceful degradation:** When memory >90%, disable semantic cache (frees ~80MB), disable streaming response buffering.

**Backpressure signals:**
```rust
enum BackpressureLevel {
    Normal,      // All systems nominal
    Elevated,    // Latency p99 > 2x baseline; reduce timeouts slightly
    High,        // Queue depth > 50%; enable circuit breakers aggressively
    Critical,    // Memory >90% or CPU >95%; disable non-essential features
}
```

### 4.5 Request Timeouts Per Provider

| Provider | Connect Timeout | Request Timeout | First-Byte Timeout |
|----------|----------------|----------------|-------------------|
| OpenAI | 10s | 120s | 30s |
| Anthropic | 10s | 120s | 30s |
| Gemini | 10s | 120s | 30s |
| Ollama (local) | 2s | 300s | 60s |
| Azure OpenAI | 10s | 120s | 30s |
| Cohere | 10s | 60s | 15s |
| Mistral | 10s | 120s | 30s |

**First-byte timeout** is the maximum time to wait for the first chunk of a streaming response. If exceeded, the request is treated as a failure and may trigger circuit breaker.

---

## 5. Async Processing

### 5.1 Synchronous vs Asynchronous Work

**Synchronous (in request path — must complete before response):**

| Operation | Latency Budget | Why Sync |
|-----------|---------------|----------|
| Request validation | <1ms | Must reject invalid requests immediately |
| Authentication / API key lookup | <5ms | User must be identified before routing |
| Rate limit check | <5ms | Must enforce limits before processing |
| Provider selection / routing | <1ms | Determines where to send the request |
| L1 cache check | <1ms | Must check cache before provider call |
| L2 (Redis) cache check | <5ms | Cache hit = skip provider entirely |
| Request transformation | <2ms | May modify request before forwarding |
| Provider HTTP request | 5-30s | The actual LLM call (async I/O, not blocking) |
| Response transformation | <2ms | May modify response before returning |
| Cost tracking (sync portion) | <2ms | Increment counters in-process |
| Response streaming | Real-time | Pass-through to client |

**Total sync overhead budget: <25ms** (excluding provider latency)

**Asynchronous (background tasks — do not block response):**

| Operation | Trigger | Why Async |
|-----------|---------|-----------|
| Request/response logging to PostgreSQL | After response completes | Logging must not add latency to response |
| Cost aggregation and budget checking | Periodic (every 10s) + on response | Budget enforcement can be eventual (10s grace acceptable) |
| Cache warming/preloading | On startup, on config change | Startup must not block requests |
| Provider health checks | Every 30 seconds | Background monitoring |
| Log archival / rotation | Daily | Cleanup must not affect requests |
| Analytics aggregation | Every 5 minutes | Dashboard data can be stale by 5 min |
| Cache eviction cleanup | Periodic | Automated, no urgency |
| Alert generation (budget, rate limit) | On threshold crossing | Alerts are best-effort |

### 5.2 Task Queue Design (No Kafka)

**Design:** PostgreSQL-backed job queue with in-process workers. No external message broker.

**Why PostgreSQL for the queue:**
1. Already required for application data — no additional infrastructure
2. ACID guarantees (jobs are not lost on crash)
3. `SKIP LOCKED` enables concurrent job consumption without conflicts
4. Observability: jobs are just table rows, queryable with SQL

**Queue table schema:**
```sql
CREATE TABLE job_queue (
    id              BIGSERIAL PRIMARY KEY,
    job_type        VARCHAR(50) NOT NULL,      -- 'log_request', 'aggregate_cost', etc.
    payload         JSONB NOT NULL,
    status          VARCHAR(20) NOT NULL DEFAULT 'pending',
        -- pending, running, completed, failed, dead_letter
    priority        INT NOT NULL DEFAULT 100,   -- Lower = higher priority
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    scheduled_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    started_at      TIMESTAMPTZ,
    completed_at    TIMESTAMPTZ,
    retry_count     INT NOT NULL DEFAULT 0,
    max_retries     INT NOT NULL DEFAULT 3,
    error_message   TEXT,
    worker_id       VARCHAR(50)                 -- Which worker claimed this job
);

CREATE INDEX idx_job_queue_pending 
    ON job_queue (priority, scheduled_at, id) 
    WHERE status = 'pending';

CREATE INDEX idx_job_queue_worker 
    ON job_queue (worker_id, status) 
    WHERE status = 'running';
```

**Job consumer pattern:**
```sql
-- Claim next job (runs in transaction)
WITH next_job AS (
    SELECT id 
    FROM job_queue
    WHERE status = 'pending' 
      AND scheduled_at <= NOW()
    ORDER BY priority ASC, scheduled_at ASC, id ASC
    LIMIT 1
    FOR UPDATE SKIP LOCKED
)
UPDATE job_queue
SET status = 'running', 
    started_at = NOW(), 
    worker_id = $1
FROM next_job
WHERE job_queue.id = next_job.id
RETURNING job_queue.*;
```

**Why `SKIP LOCKED`:** Enables multiple worker threads to claim jobs concurrently without blocking each other. Standard `FOR UPDATE` would cause serialization.

### 5.3 Background Workers

Worker pool: 4-16 threads (scaled with vCPU count), each running a job consumer loop.

| Worker Type | Count | Job Types | Priority |
|-------------|-------|-----------|----------|
| Request logger | 2 | `log_request`, `log_response` | 10 (high) |
| Cost aggregator | 1 | `aggregate_cost`, `check_budget` | 20 |
| Health checker | 1 | `provider_health_check` | 50 |
| Maintenance | 1 | `cache_cleanup`, `log_archive`, `alert_dispatch` | 100 (low) |

**Worker scaling formula:**
```
logger_workers = max(2, vCPU / 2)
maintenance_workers = 1  # always 1, low priority
total_workers = logger_workers + 2  # cost + health + maintenance
```

#### Request Logger Worker

**Purpose:** Persist request/response metadata to PostgreSQL.

**Batching strategy:**
- Collect 100 log entries or wait 5 seconds, whichever comes first
- Insert as single `COPY` or multi-row `INSERT` (10x faster than individual inserts)
- On failure: retry 3x with exponential backoff; after 3 failures, move to dead letter queue

**Log entry payload:**
```json
{
    "request_id": "uuid",
    "timestamp": "2025-01-01T00:00:00Z",
    "organization_id": "org_xxx",
    "api_key_id": "key_xxx",
    "provider": "openai",
    "model": "gpt-4o",
    "tokens_input": 150,
    "tokens_output": 80,
    "cost_usd": 0.0045,
    "latency_ms": 2500,
    "status": "success",
    "cache_hit": false,
    "routing_rule": "cost_optimize"
}
```

#### Cost Aggregation Worker

**Purpose:** Aggregate costs for budget enforcement and reporting.

**Strategy:**
- Every 10 seconds: flush in-process cost counters to PostgreSQL
- Every 60 seconds: recalculate per-organization, per-key, and per-project cost totals
- On budget threshold crossing (80%, 90%, 100%): emit alert, update rate limiter state
- Budget enforcement is eventual: a 10-second overspend window is acceptable for SME use cases

#### Provider Health Check Worker

**Purpose:** Monitor provider availability and update circuit breaker state.

**Strategy:**
- Every 30 seconds: send lightweight health check to each configured provider
- Health check: `GET /models` or equivalent lightweight endpoint
- Track success rate over 5-minute window (60 samples)
- Update circuit breaker state in L1 cache + Redis
- On state change (CLOSED → OPEN, OPEN → HALF_OPEN): emit event to event log

**Health check criteria:**
```rust
struct HealthStatus {
    provider: String,
    last_check: Timestamp,
    consecutive_successes: u32,
    consecutive_failures: u32,
    latency_ms_95th: f64,      // 95th percentile latency over 5-min window
    error_rate: f64,            // % of failed requests over 5-min window
    circuit_state: CircuitState,
}
```

#### Maintenance Worker

**Purpose:** Cleanup, archival, and periodic tasks.

**Schedule:**
| Task | Frequency | Description |
|------|-----------|-------------|
| Cache cleanup | Every 5 minutes | Remove expired entries from in-process cache |
| Log archival | Daily at 02:00 | Compress logs older than 7 days, move to cold storage (S3-compatible if configured, else local filesystem) |
| Old log deletion | Daily at 03:00 | Delete logs older than retention period (default 30 days, configurable) |
| Analytics refresh | Every 5 minutes | Update materialized views for dashboard |
| Stale job cleanup | Every 15 minutes | Mark jobs running >10 minutes as failed (orphaned) |
| Redis memory check | Every 5 minutes | Alert if Redis memory usage >80% |

### 5.4 Why Not In-Process Channels?

In-process channels (Tokio mpsc) were evaluated and rejected as the primary queue because:
1. **Durability:** Channel messages are lost on process crash; PostgreSQL persists jobs
2. **Observability:** Cannot query channel state; PostgreSQL jobs are inspectable
3. **Backpressure handling:** Channel boundedness causes either blocking or dropping; PostgreSQL queue naturally handles backpressure via row count
4. **Multiple consumers:** Channels require complex fan-out for multiple worker types; PostgreSQL queue supports multiple independent consumers naturally

**In-process channels ARE used for:** Hot-path communication between request handlers and background workers for non-critical signals (e.g., "flush counters now"), where durability is not required.

---

## 6. Rate Limiting Architecture

### 6.1 Algorithm Selection

**Primary:** Token Bucket (Redis `INCR` with expiration)
**Secondary:** Sliding Window Log (for precise enforcement of per-second limits)

**Why not Fixed Window:** Burst at window boundary allows 2x the limit (thundering herd at :00 seconds).

**Why not pure Sliding Window Counter:** Still allows some burst; token bucket provides smoother rate limiting.

**Hybrid approach:**
- Token bucket for request-per-second limits (smooth, allows burst)
- Sliding window for token-per-minute limits (accurate cost accounting)

### 6.2 Token Bucket Implementation (Redis)

```rust
// Lua script for atomic token bucket operation
const RATE_LIMIT_SCRIPT: &str = r#"
local key = KEYS[1]
local rate = tonumber(ARGV[1])        -- tokens per second
local burst = tonumber(ARGV[2])       -- bucket capacity
local now = tonumber(ARGV[3])         -- current time in ms
local cost = tonumber(ARGV[4])        -- tokens this request costs (usually 1)

local bucket = redis.call('HMGET', key, 'tokens', 'last_update')
local tokens = tonumber(bucket[1]) or burst
local last_update = tonumber(bucket[2]) or now

-- Add tokens based on time elapsed
local elapsed = (now - last_update) / 1000.0
local new_tokens = math.min(burst, tokens + elapsed * rate)

if new_tokens >= cost then
    new_tokens = new_tokens - cost
    redis.call('HMSET', key, 'tokens', new_tokens, 'last_update', now)
    redis.call('EXPIRE', key, 3600)  -- 1 hour TTL
    return {1, math.floor(new_tokens)}  -- allowed, remaining
else
    redis.call('HSET', key, 'last_update', now)
    redis.call('EXPIRE', key, 3600)
    return {0, math.floor(new_tokens)}  -- denied, remaining
end
"#;
```

**Why Lua script:** Redis `EVAL` executes atomically. Without atomicity, race conditions between `GET` and `SET` would allow limit overruns under concurrent load.

### 6.3 Rate Limit Layers

Rate limiting is applied at multiple levels, checked in order:

| Layer | Key | Default Limit | Scope |
|-------|-----|---------------|-------|
| 1. Global (instance) | `global:req` | 2000 req/s | Protects the gateway instance |
| 2. Organization | `org:{id}:req` | Configurable per org | Per-customer limit |
| 3. API Key | `key:{id}:req` | Inherits from org | Per-key limit |
| 4. API Key (token) | `key:{id}:tok` | Inherits from org | Per-key token limit (tokens/min) |
| 5. Provider | `prov:{name}:req` | Provider's own limit | Prevents hitting provider rate limits |
| 6. IP address | `ip:{ip}:req` | 100 req/s | DDoS / abuse protection |

**Check order and short-circuit:** If any layer rejects, the request is immediately rejected. Layers are checked from most specific (API key) to most general (global) to provide the most informative error message.

**Rate limit response (when exceeded):**
```json
{
    "error": {
        "type": "rate_limit_exceeded",
        "layer": "organization",
        "limit": 100,
        "window": "second",
        "retry_after": 1.5
    }
}
```

Headers (RFC 6585 compliant):
```
RateLimit-Limit: 100
RateLimit-Remaining: 0
RateLimit-Reset: 1704153600
Retry-After: 2
```

### 6.4 Rate Limit Configuration

```rust
struct RateLimitConfig {
    requests_per_second: Option<f64>,     // Token bucket rate
    requests_burst: Option<u32>,          // Token bucket capacity
    tokens_per_minute: Option<u64>,       // Sliding window token limit
    tokens_per_day: Option<u64>,          // Daily token budget
    cost_per_day: Option<f64>,            // Daily USD cost budget
    concurrent_requests: Option<u32>,     // Max in-flight per key
}
```

**Default tiers:**

| Tier | Req/s | Burst | Tok/min | Cost/day | Concurrent |
|------|-------|-------|---------|----------|------------|
| Free / Development | 10 | 20 | 100K | $10 | 5 |
| Small Business | 100 | 200 | 1M | $100 | 20 |
| Business | 500 | 1000 | 5M | $500 | 100 |
| Enterprise | Custom | Custom | Custom | Custom | Custom |

### 6.5 Burst Handling

Token bucket naturally allows bursts up to the bucket capacity. For additional burst tolerance:

1. **Steady-state bonus:** If a client stays under 50% of their limit for 60 seconds, grant a "burst credit" of 2x their normal burst capacity (one-time).
2. **Priority queuing:** Requests with `X-Priority: high` header (authenticated admin users) use a separate token bucket with 10% of total capacity reserved.
3. **Emergency bypass:** When all rate limits are hit, the gateway can still process requests with `X-Emergency-Bypass` header from a super-admin key (logged and alerted, but not blocked).

### 6.6 In-Process Rate Limit Cache

To avoid a Redis round-trip on every request:

1. Maintain a local token bucket mirror in L1 cache for each active key
2. Sync to Redis every 10 seconds (acceptable drift: 10 seconds of slightly exceeded limits)
3. On startup or cache miss, fetch current bucket state from Redis
4. This reduces rate limit check from ~2ms (Redis RTT) to ~200ns (in-process HashMap lookup)

---

## 7. Request Coalescing

### 7.1 When to Coalesce

Request coalescing (also called "request deduplication" or "singleflight") is applied when:

1. **Multiple identical requests arrive simultaneously** (within a time window)
2. **The request is cacheable** (not marked `X-No-Cache`)
3. **The request is not a streaming request** (streaming requests are too latency-sensitive to wait)

**Identical request definition:**
- Same cache key (same organization, provider, model, parameters, normalized messages)
- Same streaming vs non-streaming mode

### 7.2 Implementation

```rust
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock, oneshot};

/// In-flight request tracker
type InFlightMap = Arc<RwLock<HashMap<String, Vec<oneshot::Sender<Response>>>>>;

async fn handle_with_coalescing(
    cache_key: String,
    in_flight: InFlightMap,
    execute: impl FnOnce() -> Future<Output = Response>,
) -> Response {
    // 1. Check if identical request is already in flight
    {
        let read_guard = in_flight.read().await;
        if let Some(waiters) = read_guard.get(&cache_key) {
            // Someone else is already fetching this; join the wait queue
            let (tx, rx) = oneshot::channel();
            drop(read_guard);
            // Need write lock to add our channel — re-acquire
            let mut write_guard = in_flight.write().await;
            write_guard.entry(cache_key.clone()).or_default().push(tx);
            drop(write_guard);
            return rx.await.expect("coalescing sender dropped");
        }
    }
    
    // 2. We are the first; register ourselves as the fetcher
    {
        let mut write_guard = in_flight.write().await;
        write_guard.insert(cache_key.clone(), Vec::new());
    }
    
    // 3. Execute the request (this is the slow part)
    let response = execute().await;
    
    // 4. Notify all waiters
    {
        let mut write_guard = in_flight.write().await;
        if let Some(waiters) = write_guard.remove(&cache_key) {
            for tx in waiters {
                let _ = tx.send(response.clone());
            }
        }
    }
    
    response
}
```

### 7.3 Coalescing Window

| Scenario | Window | Rationale |
|----------|--------|-----------|
| Exact same request | Duration of provider request (5-30s) | Natural; ends when response arrives |
| Cached response | 0ms (immediate) | Cache hit means no coalescing needed |
| Failed request | 5s backoff | Prevent thundering herd on failing provider |

### 7.4 Expected Impact

| Traffic Pattern | % Coalesced | Latency Impact |
|-----------------|-------------|----------------|
| Batch workloads (same prompt, N users) | 10-30% | Coalesced requests wait ~50% less (avg) |
| Retry storms (client retry on timeout) | 50-80% | Eliminates duplicate provider calls |
| Cache stampede (cache expiry) | 90%+ | Only 1 request hits the provider |
| Normal chat traffic | 1-3% | Minimal impact |

**Coalescing is most valuable during cache stampedes** — when a popular cache entry expires and hundreds of clients request it simultaneously. Without coalescing, all requests hit the provider simultaneously (thundering herd). With coalescing, only one request hits the provider; the rest wait and receive the same response.

### 7.5 Memory Safety

The coalescing map uses:
- `RwLock` for read-heavy access pattern (most requests check, few register)
- Automatic cleanup when response arrives (entry removed from map)
- TTL of 60 seconds on entries (prevents leak if response never arrives due to panic)
- Bounded by max concurrent requests (10,000 entries max)

---

## 8. Database Scalability

### 8.1 Query Patterns

**Hot path queries** (executed on every request, must be <10ms):
```sql
-- 1. API key lookup (by key hash)
SELECT k.id, k.organization_id, k.rate_limit_config, o.settings
FROM api_keys k
JOIN organizations o ON k.organization_id = o.id
WHERE k.key_hash = $1 AND k.is_active = true;

-- 2. Provider config lookup (by org + provider name)
SELECT config FROM provider_configs
WHERE organization_id = $1 AND provider = $2 AND is_active = true;

-- 3. Routing rules (by org)
SELECT rules FROM routing_configs
WHERE organization_id = $1 AND is_active = true
ORDER BY priority DESC;
```

**Warm path queries** (executed periodically, <100ms acceptable):
```sql
-- 4. Cost aggregation (by org, by day)
SELECT organization_id, SUM(cost_usd), SUM(tokens_input), SUM(tokens_output)
FROM request_logs
WHERE created_at >= $1 AND created_at < $2
GROUP BY organization_id;

-- 5. Dashboard analytics (materialized view refresh)
REFRESH MATERIALIZED VIEW CONCURRENTLY daily_usage_stats;
```

**Cold path queries** (background, <5s acceptable):
```sql
-- 6. Log archival (old data export)
SELECT * FROM request_logs WHERE created_at < $1 LIMIT 10000;

-- 7. Admin reporting
SELECT provider, model, COUNT(*), AVG(latency_ms), SUM(cost_usd)
FROM request_logs
WHERE created_at >= $1
GROUP BY provider, model;
```

### 8.2 Index Strategy

**Indexes for hot path (sub-10ms lookups):**
```sql
-- API key lookup (THE most critical index)
CREATE UNIQUE INDEX idx_api_keys_key_hash ON api_keys(key_hash) WHERE is_active = true;

-- Provider config lookup
CREATE INDEX idx_provider_configs_org_provider ON provider_configs(organization_id, provider) WHERE is_active = true;

-- Routing rules lookup
CREATE INDEX idx_routing_configs_org_priority ON routing_configs(organization_id, priority DESC) WHERE is_active = true;

-- Request logs (for user-facing recent history)
CREATE INDEX idx_request_logs_org_created ON request_logs(organization_id, created_at DESC);
CREATE INDEX idx_request_logs_key_created ON request_logs(api_key_id, created_at DESC);
```

**Indexes for warm path (aggregation, reporting):**
```sql
-- Cost aggregation (partial index for current period)
CREATE INDEX idx_request_logs_cost_agg ON request_logs(organization_id, created_at) 
    INCLUDE (cost_usd, tokens_input, tokens_output)
    WHERE created_at > NOW() - INTERVAL '7 days';

-- Provider performance monitoring
CREATE INDEX idx_request_logs_provider_perf ON request_logs(provider, created_at) 
    INCLUDE (latency_ms, status)
    WHERE created_at > NOW() - INTERVAL '24 hours';
```

**No indexes needed (or harmful):**
- Full-text search on request content (not supported in MVP; use log export if needed)
- Generic `created_at` index without partitioning (table too large, index too slow)

### 8.3 Connection Pool Sizing Formula

**PostgreSQL connection pool sizing (using deadpool-postgres):**

```
optimal_pool_size = (vCPU_count * 2) + effective_spindle_count

For VPS (SSD, no spindle limitation):
optimal_pool_size = vCPU_count * 2

With headroom for background workers:
application_pool_size = (vCPU_count * 2) + 4
```

| VPS vCPU | Pool Size | Max Connections (PostgreSQL) |
|----------|-----------|------------------------------|
| 2 | 8 | 20 |
| 4 | 12 | 30 |
| 8 | 20 | 50 |
| 16 | 36 | 80 |

**Why not more connections?** PostgreSQL uses one process per connection. Each connection consumes ~5MB RAM + CPU for context switching. Too many connections cause thrashing (connection contention) rather than improved throughput. With pgBouncer (if ever needed), the formula changes; but single-node deployment does not use pgBouncer.

**PostgreSQL `max_connections` setting:**
```ini
max_connections = 100          # Enough for app pool + background workers + admin
shared_buffers = 25% of RAM    # e.g., 4GB on 16GB VPS
effective_cache_size = 75% of RAM
work_mem = 16MB                # Per-query sort/hash memory
maintenance_work_mem = 256MB   # For VACUUM, CREATE INDEX
wal_buffers = 64MB
random_page_cost = 1.1         # SSD tuning (lower = SSD preferred over seq scan)
effective_io_concurrency = 200  # SSD tuning
```

### 8.4 Read Replica Consideration

**Not needed for single-node deployment.** The PostgreSQL instance runs on the same VPS as the application. Network latency between app and DB is ~0.1ms (localhost/Unix socket).

**If ever needed (Stage 5 scaling):**
- PostgreSQL streaming replication to a second VPS
- Read replicas handle: analytics queries, log exports, dashboard data
- Primary handles: hot path lookups, writes
- Switchover: promote replica to primary on failure (manual or with repmgr)

**Trigger for read replica:** Analytics queries (cold path) consistently take >5 seconds AND interfere with hot path query latency. Expected at ~500+ req/s sustained with heavy dashboard usage.

### 8.5 Partitioning Strategy

**Partitioned table:** `request_logs`

**Partitioning method:** Range partitioning on `created_at` (monthly partitions)

```sql
CREATE TABLE request_logs (
    id              BIGSERIAL,
    request_id      UUID NOT NULL,
    organization_id VARCHAR(50) NOT NULL,
    api_key_id      VARCHAR(50) NOT NULL,
    provider        VARCHAR(50) NOT NULL,
    model           VARCHAR(100) NOT NULL,
    tokens_input    INT NOT NULL DEFAULT 0,
    tokens_output   INT NOT NULL DEFAULT 0,
    cost_usd        NUMERIC(12,8) NOT NULL DEFAULT 0,
    latency_ms      INT NOT NULL,
    status          VARCHAR(20) NOT NULL,
    cache_hit       BOOLEAN NOT NULL DEFAULT false,
    routing_rule    VARCHAR(50),
    created_at      TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (id, created_at)
) PARTITION BY RANGE (created_at);

-- Create monthly partitions
CREATE TABLE request_logs_2025_01 PARTITION OF request_logs
    FOR VALUES FROM ('2025-01-01') TO ('2025-02-01');
CREATE TABLE request_logs_2025_02 PARTITION OF request_logs
    FOR VALUES FROM ('2025-02-01') TO ('2025-03-01');
-- Auto-create future partitions via cron/extension
```

**Why partition:**
- Query performance: Analytics queries for "last 7 days" only scan 1-2 partitions, not the full table
- Maintenance: `VACUUM` and `REINDEX` run per-partition, not on the full table
- Archival: Drop old partitions (instant) instead of `DELETE` (slow, causes bloat)
- At 1000 req/s, 1 month = 2.6 billion rows; partitioning keeps each partition manageable

### 8.6 Archiving Strategy

**Retention policy:**

| Data Type | Hot Storage (PostgreSQL) | Warm Storage (Compressed) | Cold Storage (Archive) |
|-----------|-------------------------|---------------------------|----------------------|
| Request logs (full) | 7 days | 30 days | Configurable (default 90 days) |
| Request logs (aggregated) | 90 days | 1 year | Indefinite |
| Analytics materialized views | Always (refreshed) | N/A | N/A |

**Archival process (daily at 02:00):**
1. Export logs older than 7 days to Parquet format (compressed, columnar)
2. Store in local filesystem (`/var/lib/ai-gateway/archives/`) or S3-compatible bucket (if configured)
3. `DETACH PARTITION` for partitions older than retention period
4. `DROP TABLE` for partitions older than cold storage period
5. Update aggregated stats table (per-org daily summaries) before dropping

**Archive format:**
```
/var/lib/ai-gateway/archives/
  request_logs/
    2025/
      01/
        request_logs_2025_01_01.parquet (daily, ~50MB compressed per day at 1000 req/s)
```

**Recovery:** Archived logs can be re-imported via admin API for forensic analysis. Dashboard queries for old data show aggregated summaries only (not individual requests).

---

## 9. Bottleneck Analysis

### Top 5 Bottlenecks (in order of likelihood)

#### 1. PostgreSQL Write Throughput (Request Logging)

**Why #1:** Every request generates a log row. At 1000 req/s, that's 86.4 million rows/day. Even batched, this is the highest sustained write load in the system.

**Symptoms:**
- `INSERT` latency increases from <5ms to >50ms
- PostgreSQL CPU spikes during batch insert operations
- WAL (Write-Ahead Log) disk I/O saturates
- Autovacuum cannot keep up; table bloat increases

**Mitigation:**
- **Batch inserts:** Collect 100-1000 log entries, insert as single `COPY` operation (10x throughput improvement)
- **Async logging:** Request response is NOT blocked on log persistence (see Section 5)
- **Partitioning:** Monthly partitions prevent any single table from growing too large
- **Unlogged table for buffer:** Use PostgreSQL `UNLOGGED` table for raw log buffer, then migrate to permanent table (trades crash durability for write speed — acceptable since logs are best-effort)
- **Write tuning:** `synchronous_commit = off` for the log database connection (ack WAL flush asynchronously — at most 1 second of lost logs on crash)

**Monitoring:**
- `pg_stat_database.tup_inserted` rate
- `pg_stat_activity` wait events (`WALWrite`, `WALSync`)
- Disk I/O utilization (`iostat`)
- Autovacuum lag (`pg_stat_user_tables.n_dead_tup`)

#### 2. Redis Memory Exhaustion (Response Cache)

**Why #2:** Cached LLM responses accumulate rapidly. At 1000 req/s with 2KB average response size and 1-hour TTL, peak memory usage is ~7.2GB. A 16GB VPS has ~12GB available after OS + PostgreSQL + app.

**Symptoms:**
- Redis `used_memory` approaches `maxmemory`
- Eviction rate spikes (LRU kicking out recently used entries)
- Cache hit rate drops suddenly
- `INFO stats` shows high `evicted_keys` count

**Mitigation:**
- **Response compression:** Compress cached responses with zstd (typically 3-5x reduction for JSON text)
- **Tiered TTL:** High-frequency responses get longer TTL; one-off queries get shorter TTL (adaptive based on access patterns)
- **Memory cap with graceful degradation:** When Redis memory >80%, reduce max TTL by 50%; when >90%, disable semantic cache
- **Redis `maxmemory-policy allkeys-lru`:** Automatic eviction of least-used entries
- **Monitor hit rate:** If hit rate drops below 15% at steady state, cache is too small or TTL too short

**Monitoring:**
- `redis-cli INFO memory` — `used_memory`, `used_memory_rss`
- `redis-cli INFO stats` — `evicted_keys` rate
- Cache hit rate (calculated by application: hits / total_requests)
- Application-level `cache_size_bytes` gauge

#### 3. Provider Connection Saturation

**Why #3:** LLM requests are long-lived (5-30 seconds per request). At 100 req/s with 15s average response time, we need 1500 concurrent connections. Each provider has its own connection limit.

**Symptoms:**
- Requests queue in the provider connection pool (wait time >1s)
- Timeouts increase despite provider being healthy
- Gateway latency p99 spikes while p50 remains normal (queueing tail latency)
- Provider returns 429 (Too Many Requests) — their rate limit, not ours

**Mitigation:**
- **Connection pool sizing:** Dynamic pool size based on measured provider latency (if avg latency increases, increase pool size)
- **Request coalescing:** Identical simultaneous requests share one provider connection (Section 7)
- **Circuit breaker:** Open circuit when provider returns 429 or connection timeouts spike
- **Provider rotation:** Spread load across multiple API keys for the same provider
- **HTTP/2 multiplexing:** Where providers support HTTP/2, reuse a single TCP connection for multiple concurrent streams

**Monitoring:**
- Pool wait time histogram (custom metric)
- Active connections per provider gauge
- Provider 429 rate
- Queue depth per provider

#### 4. In-Process Memory Growth

**Why #4:** Tokio runtime, connection buffers, in-process cache, request coalescing state, and concurrent request bodies all consume memory. At high concurrency (1000+ concurrent requests), memory usage can grow unbounded.

**Symptoms:**
- RSS memory grows steadily over time (memory leak pattern)
- OOM killer terminates the gateway process
- Latency increases as system swaps (if swap enabled — it should not be)
- `dmesg` shows OOM events

**Mitigation:**
- **Request body size limits:** 10MB max request body, 50MB max response body (enforced at the HTTP layer)
- **Streaming responses:** For responses >1MB, stream without buffering the full body in memory
- **Bounded channels:** All async channels have bounded capacity; backpressure propagates naturally
- **jemalloc:** Use jemalloc as the global allocator (better fragmentation handling than glibc malloc)
- **Memory cap:** Hard limit at 80% of VPS RAM; when exceeded, disable non-essential features (semantic cache, detailed logging)

**Monitoring:**
- RSS memory (Prometheus `process_resident_memory_bytes`)
- Heap allocation rate (if jemalloc profiling enabled)
- Number of concurrent requests gauge
- Request/response body size histogram

#### 5. CPU Saturation During Peak Load

**Why #5:** Rust is efficient, but at 1000+ req/s, serialization/deserialization (JSON), embedding generation (semantic cache), and request routing logic consume significant CPU.

**Symptoms:**
- CPU utilization >90% sustained
- Tokio task scheduling latency increases (tasks waiting for CPU)
- Gateway overhead increases from <5ms to >20ms
- Health check failures (health endpoint itself starved for CPU)

**Mitigation:**
- **JSON parser optimization:** Use `simd-json` crate (SIMD-accelerated JSON parsing) instead of standard `serde_json`
- **Skip semantic cache under load:** When CPU >80%, disable local embedding generation (saves ~20ms CPU per request)
- **Connection batching:** Batch independent Redis operations (pipelining)
- **Reduce worker threads:** Counter-intuitively, reducing Tokio worker threads to vCPU count (not vCPU * 2) reduces context switching at high load
- **CPU profiling:** Use `perf` + flamegraph to identify hot functions; optimize the top 3

**Monitoring:**
- `process_cpu_seconds_total` (Prometheus)
- Tokio runtime metrics (task scheduling time, if `tokio-metrics` enabled)
- Per-endpoint CPU time (if profiling enabled)
- Gateway overhead latency (total latency - provider latency)

---

## 10. Scaling Triggers

### 10.1 Metrics That Trigger Re-evaluation

| Metric | Yellow Threshold (Plan) | Red Threshold (Act) | Action |
|--------|------------------------|---------------------|--------|
| **Request rate** | >500 req/s sustained | >1000 req/s sustained | Yellow: Upgrade VPS. Red: Re-architecture evaluation |
| **CPU utilization** | >70% for 1 hour | >90% for 4 hours | Yellow: Optimize code. Red: Upgrade VPS vCPUs |
| **Memory utilization** | >70% for 1 hour | >90% for 30 min | Yellow: Reduce cache size. Red: Upgrade VPS RAM |
| **PostgreSQL write IOPS** | >1000 sustained | >3000 sustained | Yellow: Increase batch size. Red: Add dedicated DB VPS |
| **Redis memory usage** | >70% of maxmemory | >90% of maxmemory | Yellow: Reduce TTL. Red: Upgrade VPS RAM or add Redis VPS |
| **Disk usage** | >70% | >90% | Yellow: Archive old logs. Red: Expand disk |
| **Provider connection wait** | p99 >500ms | p99 >2s | Yellow: Increase pool size. Red: Add provider API keys |
| **Cache hit rate** | <15% | <10% | Yellow: Review cache TTL. Red: Investigate workload pattern |
| **Gateway overhead p99** | >10ms | >25ms | Yellow: Profile and optimize. Red: Upgrade VPS or reduce features |
| **Error rate** | >1% | >5% | Yellow: Alert on-call. Red: Emergency circuit break all providers |

### 10.2 What Changes at Each Trigger Point

#### Stage 1: Vertical Scaling (Immediate, No Code Changes)

**Trigger:** Any yellow threshold
**Actions:**
- Resize VPS: double vCPU or RAM (most cloud providers allow resize with <1 min downtime)
- Tune PostgreSQL: increase `shared_buffers`, `work_mem` proportionally
- Tune Redis: increase `maxmemory`
- Tune connection pools: increase pool sizes in config
- Enable more background workers

**Cost:** Linear with VPS size. $48 → $96 → $192/month.

#### Stage 2: Configuration Optimization (Same VPS, Code/Config Changes)

**Trigger:** Yellow persists after vertical scaling OR multiple yellow metrics
**Actions:**
- Enable response compression in cache (3-5x memory reduction)
- Reduce cache TTLs (trade hit rate for memory)
- Increase batch insert sizes for logging (2x write throughput)
- Disable semantic cache (saves 80MB RAM + 20ms CPU per request)
- Add provider connection pool multiplexing (HTTP/2)
- Enable read replica for analytics queries (if using separate VPS for DB)

**Cost:** $0 (configuration changes only)

#### Stage 3: Architecture Change (Split Services)

**Trigger:** Red threshold on PostgreSQL write IOPS OR Redis memory
**Actions:**
- **Split DB:** Move PostgreSQL to dedicated VPS (same datacenter, private network)
  - App VPS: 8 vCPU, 8 GB (gateway + Redis)
  - DB VPS: 4 vCPU, 16 GB (PostgreSQL with large `shared_buffers`)
  - Network latency: ~0.5ms (same datacenter private network)
- **Split Redis:** Move Redis to dedicated VPS OR use managed Redis (Upstash, etc.)
- **Add read replica:** PostgreSQL streaming replica for analytics queries

**Cost:** 2x VPS (~$300-400/month total). Still no Kubernetes.

#### Stage 4: Multi-Instance Gateway (Horizontal, Minimal)

**Trigger:** Red on CPU or memory despite split DB; >1000 req/s sustained
**Actions:**
- Run 2 gateway instances behind a load balancer (e.g., Caddy or Traefik on the VPS)
- Shared state via Redis (circuit breakers, rate limits, cache)
- PostgreSQL remains shared (connection pool split between instances)
- Session affinity NOT required (stateless gateway design)

**Architecture:**
```
Load Balancer (Caddy/Traefik) ──► Gateway Instance 1 ──┐
                              ──► Gateway Instance 2 ──┼──► PostgreSQL (dedicated)
                                                        ├──► Redis (dedicated)
```

**Cost:** 3 VPS (~$500-600/month). This is the first true architecture change.

#### Stage 5: Full Re-architecture (Only If Revenue Justifies)

**Trigger:** >2000 req/s sustained for 30 days AND >100 paying customers
**Actions:**
- Evaluate managed services: Cloudflare AI Gateway for edge caching, Upstash for Redis, managed PostgreSQL
- Consider LiteLLM or Portkey as the routing layer (we become the control plane, not the data plane)
- OR: Build minimal horizontal scaling with consistent hashing for cache sharding

**This stage requires a dedicated infrastructure quarter and is NOT planned for the first 12 months.**

### 10.3 Cost Implications at Each Stage

| Stage | Infrastructure Cost | Engineering Effort | Monthly Cost (est.) |
|-------|--------------------|--------------------|---------------------|
| 0: Prototype | None (shared/dev) | None | $6-12 |
| 1: Small production | Small VPS | None | $24-48 |
| 2: Growing (vertical) | Medium VPS | None | $48-96 |
| 3: Large (vertical max) | Large VPS | None | $96-192 |
| 4: Split services | 2 VPS | 1-2 days | $192-384 |
| 5: Multi-instance | 3+ VPS, load balancer | 1-2 weeks | $384-600 |
| 6: Managed services | Cloudflare + managed DB | 2-4 weeks | $500-1000+ |

**Key decision:** The product is profitable on a $48/month VPS for customers doing <100 req/s. At $96/month (8 vCPU), it handles 500 req/s. The cost efficiency is maintained until Stage 5. Re-architecture to distributed systems is deferred until revenue from those high-traffic customers funds the engineering work.

---

## Appendix: Performance Checklist

Before each deployment, verify:

- [ ] Connection pool sizes match VPS vCPU count
- [ ] PostgreSQL `max_connections` >= pool size + 10
- [ ] Redis `maxmemory` set to 70% of available RAM
- [ ] Redis `maxmemory-policy` set to `allkeys-lru`
- [ ] Circuit breaker thresholds configured for all providers
- [ ] Rate limit Lua script loaded in Redis (pre-load on startup)
- [ ] Request body size limit enforced at reverse proxy / Axum layer
- [ ] Log batching enabled (batch size >= 100)
- [ ] In-process cache TTLs set (provider config: 60s, org settings: 300s)
- [ ] Semantic cache embedding model loaded (or graceful fallback configured)
- [ ] Backpressure thresholds configured (normal/elevated/high/critical)
- [ ] Archive job scheduled (daily at 02:00)
- [ ] Monitoring metrics exposed on `/metrics` (Prometheus format)

---

*End of Document*
