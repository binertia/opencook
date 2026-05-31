# Cache Architecture Specification — AI Gateway

**Version:** 1.0.0  
**Status:** IMPLEMENTATION-READY  
**Goal:** Reduce AI provider costs by 30-70% through intelligent multi-layer caching.  
**Target Deploy:** Single VPS with Redis (L2); L1 is in-process on the gateway node.  
**Non-Goal:** CDN edge caching (L3) — AI responses are dynamic, not cache-friendly at the edge.

---

## Table of Contents

1. [Caching Strategy Overview](#1-caching-strategy-overview)
2. [Response Cache (Primary)](#2-response-cache-primary--biggest-cost-saver)
3. [Configuration Cache](#3-configuration-cache)
4. [Rate Limit Cache](#4-rate-limit-cache)
5. [Session Cache](#5-session-cache)
6. [Cache Invalidation](#6-cache-invalidation)
7. [Cache Metrics](#7-cache-metrics)
8. [Security](#8-security)
9. [Performance Targets](#9-performance-targets)
10. [Semantic Cache Deep Dive](#10-semantic-cache-deep-dive)

---

## 1. Caching Strategy Overview

### 1.1 Architecture Diagram (Logical)

```
┌──────────────────────────────────────────────────────────────┐
│                        CLIENT REQUEST                        │
└──────────────────────────┬───────────────────────────────────┘
                           │
                    ┌──────▼──────┐
                    │   L1 Cache  │  ← moka::sync::Cache (in-process)
                    │  (hot data) │     sub-microsecond lookup
                    │   TTL: 60s  │     max_capacity: 10,000 entries
                    └──────┬──────┘
                           │ MISS
                    ┌──────▼──────┐
                    │   L2 Cache  │  ← Redis (single VPS instance)
                    │  (shared)   │     <5ms lookup over localhost
                    │   TTL: var  │     cross-instance future-proof
                    └──────┬──────┘
                           │ MISS
                    ┌──────▼──────┐
                    │  LLM PROVIDER│  ← OpenAI / Anthropic / etc.
                    │  ($$$ COST)  │
                    └─────────────┘
                           │
                    ┌──────▼──────┐
                    │  WRITE-BACK  │  ← Populate L1 + L2 on response
                    │  (async)     │
                    └─────────────┘
```

### 1.2 Layer Definitions

| Layer | Technology | Scope | Latency Target | Capacity | Eviction |
|-------|-----------|-------|---------------|----------|----------|
| **L1** | `moka` crate (`sync::Cache`) | In-process, single node | < 0.1ms | 10,000 entries (configurable) | LRU + TTL |
| **L2** | Redis (`redis-rs` with `tokio`) | Shared, cross-instance | < 5ms | VPS memory bounded (configurable) | TTL + LRU (allkeys-lru) |
| **L3** | *None* | N/A | N/A | N/A | N/A |

### 1.3 Why No L3 (CDN)

- AI responses are **non-deterministic** and **user-specific** (even with temperature=0, system prompts vary by tenant).
- Cache hit rate at CDN edge would be < 2% due to request diversity.
- CDN cache invalidation is coarse (path-based); LLM cache invalidation is fine-grained (semantic).
- Cost of CDN egress often exceeds savings from cache hits.

### 1.4 Cache Read Path (Pseudo-code)

```rust
async fn get_cached_response(req: &LLMRequest) -> Option<CachedResponse> {
    // Step 1: Check L1 (exact match only — no embeddings in L1)
    let exact_key = compute_exact_key(req);
    if let Some(hit) = L1.get(&exact_key).await {
        record_metric("l1.hit", &req.model);
        return Some(hit);
    }

    // Step 2: Check L2 exact match
    let l2_key = format!("llm:exact:{}", exact_key);
    if let Ok(Some(hit)) = redis.get::<_, Option<String>>(&l2_key).await {
        let response: CachedResponse = serde_json::from_str(&hit).unwrap();
        // Promote to L1
        L1.insert(exact_key, response.clone()).await;
        record_metric("l2.hit.exact", &req.model);
        return Some(response);
    }

    // Step 3: Check L2 semantic match (only for chat completions)
    if req.cache_config.semantic_enabled && req.is_chat_completion() {
        if let Some(semantic_hit) = semantic_search_l2(req).await {
            record_metric("l2.hit.semantic", &req.model);
            // Optionally promote to L1 (disabled by default — semantic matches
            // are lower confidence, don't pollute L1)
            return Some(semantic_hit);
        }
    }

    record_metric("cache.miss", &req.model);
    None
}
```

### 1.5 Cache Write Path (Pseudo-code)

```rust
async fn cache_response(req: &LLMRequest, resp: &LLMResponse) {
    if !is_cacheable(req, resp) {
        return;
    }

    let exact_key = compute_exact_key(req);
    let ttl = compute_ttl(req);
    let cached = CachedResponse {
        response: resp.clone(),
        cached_at: Utc::now(),
        cache_source: CacheSource::Exact,
    };

    // Write L2 first (source of truth)
    let l2_key = format!("llm:exact:{}", exact_key);
    let value = serde_json::to_string(&cached).unwrap();
    redis.set_ex(&l2_key, value, ttl.as_secs()).await.ok();

    // Write L1 (fire-and-forget, ignore errors)
    L1.insert(exact_key, cached).await;

    // If semantic caching enabled, store embedding → response mapping
    if req.cache_config.semantic_enabled {
        store_semantic_mapping(req, resp, ttl).await;
    }
}
```

### 1.6 Decision: Crate Choices

| Component | Choice | Rationale | Rejected Alternatives |
|-----------|--------|-----------|----------------------|
| L1 Cache | `moka` v0.12 | Built-in TTL, LRU eviction, async API, excellent hit rate | `dashmap` (no TTL — need external expiry management); `cached` (less flexible) |
| L2 Cache | `redis` v0.24 + `redis::aio::MultiplexedConnection` | Mature, `redis-rs` supports async pipelines for batch ops | `valkey` client (not Rust-native yet); custom TCP (reinventing) |
| Embeddings | `fastembed-rs` v3 | Local ONNX, no network call, permissive license, 384-dim output | `ort` (lower-level, more code); remote embedding API (adds latency, cost) |
| Serialization | `serde_json` + `rkyv` (L1 only) | JSON for Redis (human-debuggable); rkyv for L1 (zero-copy deserialization) | `bincode` (less portable); `msgpack` (L1 doesn't need compression) |

---

## 2. Response Cache (Primary — Biggest Cost Saver)

### 2.1 What Gets Cached

#### 2.1.1 Cacheable Requests

| Criterion | Rule | Rationale |
|-----------|------|-----------|
| **Method** | `POST /v1/chat/completions` or `POST /v1/completions` | These are the cost-heavy endpoints |
| **Temperature** | `temperature == 0` (or absent, defaulting to 0) | Temperature > 0 introduces non-determinism; caching would return stale deterministic answers |
| **Streaming** | `stream == false` OR `stream == true` with cacheable flag | Streaming responses are chunked; cache the full aggregated response and stream it back on hit |
| **Dynamic Params** | No `tools`/`functions` with dynamic schemas | Tool definitions change → response changes |
| **n (completions)** | `n == 1` (or cache each variant separately) | Multiple completions have different content; caching one would poison |
| **Max Tokens** | Any value allowed | Part of cache key |
| **System Prompt** | Static system prompts OK; dynamic (with timestamps, user IDs) are excluded | Dynamic system prompts never match |

#### 2.1.2 NOT Cached (Explicit Deny List)

| Condition | Action | Rationale |
|-----------|--------|-----------|
| `X-Cache-No-Store: true` header | Skip all caching | Client override for sensitive data |
| `temperature > 0` (configurable threshold, default 0.1) | Skip cache | Non-deterministic output |
| `stream == true` (unless `X-Cache-Stream: true`) | Skip cache | Default: don't cache streaming |
| Request body contains PII patterns (SSN, credit card, email regex) | Skip cache | Security: avoid caching sensitive data (see Section 8) |
| Request contains dynamic context (e.g., `{current_time}`, `{user_id}` in system prompt) | Skip cache | Never hit; wastes compute on key generation |
| Response contains error (4xx, 5xx from provider) | Skip cache | Don't cache failures |
| Response was rate-limited (429) | Skip cache | Transient; retry will succeed |

#### 2.1.3 Streaming Caching (Optional)

When `stream=true` AND `X-Cache-Stream: true`:

```rust
// On cache MISS: aggregate streaming chunks into a full response,
// cache the aggregated response, then stream it back.
// On cache HIT: retrieve full response from cache, re-chunk it,
// and stream chunks with Server-Sent Events format.
```

**Rationale:** Streaming responses have identical content to non-streaming; the difference is only in transport format. Caching the semantic content and re-streaming it provides identical UX.

**Trade-off:** Slightly higher memory (store full response) for cacheability. Disabled by default — opt-in via header.

### 2.2 Cache Key Design

#### 2.2.1 Exact Match Key

```rust
/// Computes a deterministic hash for exact-match cache lookups.
/// All fields that affect LLM output are included.
fn compute_exact_key(req: &LLMRequest) -> String {
    let mut hasher = Sha256::new();

    // 1. Model identifier (exact string, case-sensitive)
    hasher.update(req.model.as_bytes());
    hasher.update(b"|");

    // 2. Serialized messages (JSON array, with key sorting for determinism)
    let messages_json = canonical_json(&req.messages);
    hasher.update(messages_json.as_bytes());
    hasher.update(b"|");

    // 3. Temperature (string representation, default "0")
    hasher.update(req.temperature.to_string().as_bytes());
    hasher.update(b"|");

    // 4. Max tokens
    hasher.update(req.max_tokens.to_string().as_bytes());
    hasher.update(b"|");

    // 5. Top-p
    hasher.update(req.top_p.to_string().as_bytes());
    hasher.update(b"|");

    // 6. Presence penalty
    hasher.update(req.presence_penalty.to_string().as_bytes());
    hasher.update(b"|");

    // 7. Frequency penalty
    hasher.update(req.frequency_penalty.to_string().as_bytes());
    hasher.update(b"|");

    // 8. Stop sequences (sorted for determinism)
    let mut stop = req.stop_sequences.clone();
    stop.sort();
    hasher.update(canonical_json(&stop).as_bytes());
    hasher.update(b"|");

    // 9. Response format (JSON mode, etc.)
    hasher.update(canonical_json(&req.response_format).as_bytes());
    hasher.update(b"|");

    // 10. Tools / functions (sorted by name for determinism)
    let mut tools = req.tools.clone();
    tools.sort_by(|a, b| a.name.cmp(&b.name));
    hasher.update(canonical_json(&tools).as_bytes());
    hasher.update(b"|");

    // 11. Seed (OpenAI's deterministic seed feature)
    if let Some(seed) = req.seed {
        hasher.update(seed.to_string().as_bytes());
    }
    hasher.update(b"|");

    // 12. Logit bias (sorted by token ID)
    let mut logit_bias: Vec<_> = req.logit_bias.iter().collect();
    logit_bias.sort_by_key(|(k, _)| *k);
    hasher.update(canonical_json(&logit_bias).as_bytes());

    format!("{:x}", hasher.finalize())
}
```

**Key Properties:**
- **Deterministic:** Same request → same key, always.
- **Opaque:** SHA-256 hex string, no information leakage from key content.
- **Complete:** Every parameter that affects LLM output is included.
- **Sorted:** JSON fields and arrays are canonicalized to prevent ordering differences.

#### 2.2.2 Redis Key Namespacing

```
llm:exact:<tenant_id>:<model_slug>:<sha256_hash>   →  JSON response
llm:semantic:<tenant_id>:<model_slug>:<embedding_id> →  JSON response
llm:emb:<tenant_id>:<embedding_hash>               →  embedding vector (for search)
llm:meta:<tenant_id>:<model_slug>:stats            →  hit/miss counters
config:provider:<provider_id>                       →  provider config JSON
config:routing:<route_id>                           →  routing rules JSON
quota:<tenant_id>:<resource>                        →  quota remaining
ratelimit:<tenant_id>:<provider>:<window>           →  sliding window counter
session:<session_token>                             →  session data
```

**Rationale for namespacing:**
- `llm:` prefix separates from config/rate limit/session keys.
- `exact:` vs `semantic:` separates lookup mechanisms.
- `<tenant_id>` enables multi-tenant isolation (critical for security).
- `<model_slug>` enables per-model TTL and eviction policies.
- Colon (`:`) separator follows Redis convention; enables `KEYS llm:exact:*` pattern scanning.

#### 2.2.3 Tenant Isolation in Keys

```rust
fn redis_key_prefix(tenant_id: &str, model: &str) -> String {
    // Sanitize: only allow alphanumeric, dash, underscore
    let safe_tenant = sanitize(tenant_id);
    let safe_model = model.replace(":", "_"); // colon is Redis separator
    format!("llm:exact:{}:{}", safe_tenant, safe_model)
}

fn full_redis_key(tenant_id: &str, model: &str, hash: &str) -> String {
    format!("{}:{}", redis_key_prefix(tenant_id, model), hash)
}
```

**Security:** Tenant ID is ALWAYS part of the key. Cross-tenant cache poisoning is structurally impossible — even with a hash collision, the prefix mismatch prevents serving data to the wrong tenant.

### 2.3 TTL Strategy

#### 2.3.1 Default TTL by Model/Provider

| Model / Provider | Default TTL | Rationale |
|-----------------|-------------|-----------|
| OpenAI GPT-4o | 1 hour (3600s) | Balanced: content changes infrequently but not never |
| OpenAI GPT-4o-mini | 2 hours (7200s) | Lower cost = longer cache acceptable |
| Anthropic Claude 3.5 Sonnet | 1 hour (3600s) | Similar to GPT-4o |
| Anthropic Claude 3 Haiku | 3 hours (10800s) | Fast/cheap model = longer cache |
| Custom / fine-tuned models | 30 minutes (1800s) | Custom models may change more frequently |
| Embeddings (`text-embedding-*`) | 24 hours (86400s) | Embeddings of same text are truly immutable |
| Image generations | 24 hours (86400s) | Same prompt → same image at temperature=0 |

#### 2.3.2 Custom TTL Per Request

```rust
// Request-level TTL override
#[derive(Debug, Clone)]
struct CacheConfig {
    /// Enable/disable all caching for this request
    enabled: bool,
    /// Override default TTL (None = use model default)
    ttl_override: Option<Duration>,
    /// Enable semantic caching (default: true for chat completions)
    semantic_enabled: bool,
    /// Minimum similarity threshold for semantic match (default: 0.92)
    semantic_threshold: f32,
    /// Force cache refresh (bypass cache, write new entry)
    force_refresh: bool,
    /// Don't cache this response (one-way flag)
    no_store: bool,
}
```

**Header API for Clients:**

| Header | Values | Effect |
|--------|--------|--------|
| `X-Cache-TTL` | Integer (seconds) | Override TTL for this request's cache entry |
| `X-Cache-No-Store` | `true` / `false` | If `true`, don't write to cache |
| `X-Cache-Refresh` | `true` / `false` | If `true`, bypass cache read, force write |
| `X-Semantic-Cache` | `true` / `false` | Enable/disable semantic caching |
| `X-Semantic-Threshold` | Float (0.0-1.0) | Override similarity threshold |

#### 2.3.3 TTL Implementation in Redis

```rust
// Write with TTL
redis.set_ex(&key, value, ttl_seconds).await?;

// Write with TTL AND optional longer "soft TTL" for background refresh
// Soft TTL: still serve stale data while refreshing in background
let soft_ttl = ttl_seconds * 2; // Serve stale for 2x TTL
let hard_ttl = ttl_seconds * 3; // Evict at 3x TTL

// Use Redis key per entry for hard TTL
// Use separate "soft expiry" metadata in JSON value for conditional serving
```

### 2.4 Semantic Caching for LLM (High-Level)

Semantic caching is the primary differentiator. It increases hit rates by 3-10x over exact-match caching alone.

#### 2.4.1 Core Concept

Two prompts with **different wording** but **identical intent** should return the **same cached response**.

Examples of semantically equivalent prompts:
- `"Explain quantum computing in simple terms"`
- `"Can you give me a beginner-friendly explanation of quantum computing?"`
- `"What is quantum computing? Keep it simple."`

#### 2.4.2 How It Works

```
┌─────────────────┐     ┌──────────────────┐     ┌─────────────────────┐
│  User Prompt    │────▶│  Embedding Model │────▶│  384-dim vector     │
│  (text)         │     │  (local ONNX)    │     │  (f32 array)        │
└─────────────────┘     └──────────────────┘     └──────────┬──────────┘
                                                            │
                    ┌───────────────────────────────────────┘
                    │  ANN Search (Approximate Nearest Neighbor)
                    ▼
┌─────────────────────────────────────────────────────────────────────┐
│  Redis Vector Search (Redis Search / RediSearch)                     │
│  OR                                                                  │
│  Brute-force cosine similarity over stored embeddings (simpler)      │
│                                                                     │
│  Query: "Find embeddings with cosine similarity > 0.92 to target"  │
│  Result: 0 or more matching prompts with their cached responses      │
└─────────────────────────────────────────────────────────────────────┘
```

#### 2.4.3 Similarity Threshold

| Threshold | Behavior | Recommended Use |
|-----------|----------|----------------|
| 0.95 | Very strict; near-exact semantic match | High-stakes applications (medical, legal) |
| **0.92** | **Balanced; captures rephrasings, excludes meaning drift** | **Default for most use cases** |
| 0.88 | Permissive; captures broader semantic similarity | Cost-sensitive, low-stakes applications |
| 0.85 | Loose; may return off-topic responses | Not recommended for production |

**Tuning recommendation:** Start at 0.92, monitor `semantic_false_positive_rate` metric, adjust ±0.03 based on observed quality.

#### 2.4.4 Expected Hit Rate Improvement

| Cache Type | Typical Hit Rate | Primary Benefit |
|-----------|-----------------|----------------|
| Exact match only | 5-15% | Simple, zero false positives |
| **Exact + Semantic** | **25-50%** | **3-10x improvement; catches rephrasings** |
| Exact + Semantic (mature, high-volume) | 40-60% | Large corpus of cached responses improves coverage |

**Rationale:** In production AI applications, users rephrase the same question frequently. Exact match fails on these; semantic match succeeds.

---

## 3. Configuration Cache

### 3.1 What Is Cached

| Config Type | Source of Truth | Cache Location | Default TTL | Invalidation |
|-------------|----------------|----------------|-------------|--------------|
| **Provider configs** | PostgreSQL (admin DB) | L1 + L2 | 5 minutes (300s) | Webhook + manual |
| **Routing rules** | PostgreSQL | L1 + L2 | 1 minute (60s) | Webhook + manual |
| **Model definitions** | PostgreSQL + provider APIs | L1 + L2 | 10 minutes (600s) | Time-based only |
| **Quota settings** | PostgreSQL | L1 + L2 | 30 seconds (30s) | Event-driven |
| **API keys** | HashiCorp Vault / env | L1 only | 1 hour (3600s) | On rotation event |

### 3.2 Provider Config Cache

```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
struct ProviderConfig {
    provider_id: String,
    base_url: String,
    api_key_ref: String,        // Reference to secret, NOT the secret itself
    default_model: String,
    rate_limit: RateLimitConfig,
    timeout_ms: u64,
    retry_policy: RetryPolicy,
    // Cached at: timestamp for conditional re-fetch
    cached_at: DateTime<Utc>,
}

// L1: moka cache — key = provider_id
// L2: Redis — key = config:provider:<provider_id>
```

**Access Pattern:**
```rust
async fn get_provider_config(provider_id: &str) -> Result<ProviderConfig> {
    // L1 check
    if let Some(config) = L1_CONFIG.get(provider_id) {
        return Ok(config);
    }

    // L2 check
    let redis_key = format!("config:provider:{}", provider_id);
    if let Ok(Some(json)) = redis.get::<_, Option<String>>(&redis_key).await {
        let config: ProviderConfig = serde_json::from_str(&json)?;
        L1_CONFIG.insert(provider_id.to_string(), config.clone()).await;
        return Ok(config);
    }

    // DB fetch
    let config = db.fetch_provider_config(provider_id).await?;

    // Populate caches
    let json = serde_json::to_string(&config)?;
    redis.set_ex(&redis_key, json, 300).await?;
    L1_CONFIG.insert(provider_id.to_string(), config.clone()).await;

    Ok(config)
}
```

**Rationale for 5-minute TTL:** Provider configs change rarely (maybe weekly), but when they do (API key rotation, endpoint change), 5 minutes is an acceptable propagation delay. Event-driven invalidation (Section 6) reduces this to near-zero.

### 3.3 Routing Rules Cache

```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
struct RoutingRule {
    rule_id: String,
    priority: i32,              // Higher = evaluated first
    matcher: RouteMatcher,      // Model pattern, tenant, headers, etc.
    target_provider: String,
    target_model: String,
    fallback_chain: Vec<String>, // Ordered fallback providers
    cache_config: CacheOverride, // Per-route cache settings
}

// L1: moka cache — key = "routing:all" (full table)
// L2: Redis — key = config:routing:all
```

**Rationale for 1-minute TTL:** Routing rules may change during failover events (shift traffic from failing provider). 1-minute max staleness is acceptable; webhook invalidation handles urgent changes.

### 3.4 Quota Settings Cache

```rust
// Quota is checked on every request — must be fast
// L1: moka — key = quota:<tenant_id>:<resource>
// L2: Redis — key = quota:<tenant_id>:<resource>

// Quota values are small (integer counters), so L1 cache is very effective.
// 30-second TTL with "read-through" to DB on miss.
```

---

## 4. Rate Limit Cache

### 4.1 Architecture

Rate limiting is latency-critical (checked on every request). Redis-backed counters provide cross-instance consistency for future multi-node deployments.

```
┌──────────────┐     ┌──────────────────┐     ┌─────────────────────┐
│   Request    │────▶│  L1 Check (moka) │────▶│  L2 Check (Redis)   │
│   Incoming   │     │  (quota cache)   │     │  (sliding window)   │
└──────────────┘     └──────────────────┘     └─────────────────────┘
                                                        │
                                               ┌────────▼────────┐
                                               │  Decision:      │
                                               │  Allow / Deny   │
                                               │  (429 response) │
                                               └─────────────────┘
```

### 4.2 Sliding Window Implementation

**Algorithm:** Sliding window log (per-client, per-provider, per-time-window).

```rust
/// Sliding window rate limiter using Redis sorted sets.
/// Key: ratelimit:<tenant_id>:<provider>:<window_name>
/// Value: Sorted set of request timestamps (score = timestamp_ms)
struct SlidingWindowRateLimiter {
    redis: MultiplexedConnection,
}

impl SlidingWindowRateLimiter {
    async fn is_allowed(
        &mut self,
        tenant_id: &str,
        provider: &str,
        window_name: &str,     // e.g., "rpm" (requests per minute)
        limit: u64,            // max requests in window
        window_size_ms: u64,   // e.g., 60_000 for 1 minute
    ) -> Result<bool> {
        let key = format!("ratelimit:{}:{}:{}", tenant_id, provider, window_name);
        let now = unix_timestamp_ms();
        let window_start = now - window_size_ms;

        let mut conn = self.redis.clone();

        // Lua script for atomic check-and-record
        // Removes old entries outside the window, counts current, adds new
        let script = r#"
            local key = KEYS[1]
            local now = tonumber(ARGV[1])
            local window_start = tonumber(ARGV[2])
            local limit = tonumber(ARGV[3])

            -- Remove entries outside the sliding window
            redis.call('ZREMRANGEBYSCORE', key, 0, window_start)

            -- Count entries within the window
            local current = redis.call('ZCARD', key)

            if current >= limit then
                return 0  -- Denied
            end

            -- Add current request
            redis.call('ZADD', key, now, now)

            -- Set expiry on the key (cleanup)
            redis.call('EXPIRE', key, math.ceil((now - window_start) / 1000) + 1)

            return 1  -- Allowed
        "#;

        let result: i64 = redis::Script::new(script)
            .key(&key)
            .arg(now)
            .arg(window_start)
            .arg(limit)
            .invoke_async(&mut conn)
            .await?;

        Ok(result == 1)
    }
}
```

**Rationale for Lua script:** Rate limit check must be atomic (read + write). Without atomicity, race conditions allow burst-through. Redis Lua scripts execute atomically on the server.

**Alternative:** Redis Cell module (`CL.THROTTLE`) — simpler but requires module installation. Lua script is vanilla Redis, more portable.

### 4.3 Burst Bucket (Token Bucket)

```rust
/// Token bucket for burst tolerance.
/// Key: burst:<tenant_id>:<provider>
/// Allows short bursts while maintaining average rate.
impl SlidingWindowRateLimiter {
    async fn token_bucket_consume(
        &mut self,
        tenant_id: &str,
        provider: &str,
        capacity: u64,       // bucket size (max burst)
        refill_rate: f64,    // tokens per second
        requested: u64,      // tokens needed (usually 1)
    ) -> Result<bool> {
        let key = format!("burst:{}:{}", tenant_id, provider);

        let lua = r#"
            local key = KEYS[1]
            local capacity = tonumber(ARGV[1])
            local refill_rate = tonumber(ARGV[2])
            local requested = tonumber(ARGV[3])
            local now = tonumber(ARGV[4])

            local bucket = redis.call('HMGET', key, 'tokens', 'last_refill')
            local tokens = tonumber(bucket[1]) or capacity
            local last_refill = tonumber(bucket[2]) or now

            -- Refill tokens based on elapsed time
            local elapsed = (now - last_refill) / 1000.0
            tokens = math.min(capacity, tokens + elapsed * refill_rate)

            if tokens < requested then
                -- Not enough tokens, save state (don't consume)
                redis.call('HMSET', key, 'tokens', tokens, 'last_refill', now)
                redis.call('EXPIRE', key, 3600)
                return 0
            end

            -- Consume tokens
            tokens = tokens - requested
            redis.call('HMSET', key, 'tokens', tokens, 'last_refill', now)
            redis.call('EXPIRE', key, 3600)
            return 1
        "#;

        let result: i64 = redis::Script::new(lua)
            .key(&key)
            .arg(capacity)
            .arg(refill_rate)
            .arg(requested)
            .arg(unix_timestamp_ms())
            .invoke_async(&mut self.redis)
            .await?;

        Ok(result == 1)
    }
}
```

### 4.4 Rate Limit Headers (Return to Client)

On every response, include these headers:

```
X-RateLimit-Limit: 100
X-RateLimit-Remaining: 87
X-RateLimit-Reset: 1704067200
X-RateLimit-Window: 60
```

**Rationale:** Clients can implement client-side backoff, reducing load on the gateway.

---

## 5. Session Cache

### 5.1 Scope

| Data Type | Key Pattern | TTL | Size |
|-----------|-------------|-----|------|
| Dashboard session | `session:<token>` | 24 hours | < 2KB |
| OAuth state (login flow) | `oauth:state:<state>` | 10 minutes | < 256B |
| Temporary auth token | `authtemp:<token>` | 5 minutes | < 512B |
| CSRF token | `csrf:<token>` | 1 hour | < 128B |
| Admin MFA challenge | `mfa:<challenge_id>` | 5 minutes | < 256B |

### 5.2 Session Data Structure

```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
struct SessionData {
    user_id: String,
    tenant_id: String,           // Organization ID
    role: UserRole,              // Admin, Viewer, API
    permissions: Vec<String>,    // Granular permission flags
    created_at: DateTime<Utc>,
    last_activity: DateTime<Utc>,
    ip_address: String,          // For session binding (optional)
    user_agent_hash: String,     // Detect session theft
}

// L2 only (Redis) — sessions must survive gateway restarts
// No L1: session lookups are infrequent (only on dashboard requests, not LLM requests)
// and data must be consistent across potential future gateway instances.
```

### 5.3 Session Lookup Flow

```rust
async fn get_session(token: &str) -> Result<Option<SessionData>> {
    let key = format!("session:{}", token);

    // Check Redis
    let result: Option<String> = redis.get(&key).await?;

    if let Some(json) = result {
        let session: SessionData = serde_json::from_str(&json)?;

        // Check expiry (manual since we track last_activity)
        if session.last_activity + Duration::hours(24) < Utc::now() {
            redis.del(&key).await?;
            return Ok(None);
        }

        // Update last_activity (fire-and-forget)
        let mut updated = session.clone();
        updated.last_activity = Utc::now();
        let json = serde_json::to_string(&updated)?;
        redis.set_ex(&key, json, 86400).await.ok();

        return Ok(Some(session));
    }

    Ok(None)
}
```

### 5.4 Decision: No L1 for Sessions

| Factor | Analysis |
|--------|----------|
| Access frequency | Low (dashboard only, not per-LLM-request) |
| Consistency requirement | High (logout must invalidate immediately) |
| Data size | Small (< 2KB) — Redis handles this fine |
| Survival across restarts | Required (users shouldn't be logged out on deploy) |
| **Verdict** | **L2 (Redis) only, no L1** |

---

## 6. Cache Invalidation

### 6.1 Invalidation Strategies Overview

```
┌─────────────────────────────────────────────────────────────────┐
│                    INVALIDATION TRIGGERS                        │
├─────────────────┬─────────────────┬─────────────────────────────┤
│   MANUAL        │   TIME-BASED    │   EVENT-DRIVEN              │
│                 │                 │                             │
│ • Admin API     │ • TTL expiry    │ • Provider config change    │
│ • Pattern purge │ • Soft TTL      │ • Model update              │
│ • Force refresh │   (stale serve) │ • Quota change              │
│                 │                 │ • API key rotation          │
│                 │                 │ • Deployment event          │
└─────────────────┴─────────────────┴─────────────────────────────┘
```

### 6.2 Manual Invalidation API

```rust
// Admin API endpoints (protected by admin auth)

// DELETE /admin/cache/exact/:tenant_id/:model/:hash
// → Delete a specific exact-match entry
async fn invalidate_exact(
    Path((tenant_id, model, hash)): Path<(String, String, String)>,
) -> Result<StatusCode> {
    let key = format!("llm:{}:{}:{}", "exact", tenant_id, model);
    let full_key = format!("{}:{}", key, hash);

    redis.del(&full_key).await?;
    L1.invalidate(&hash).await;

    event_bus.publish(CacheInvalidationEvent {
        key_pattern: full_key,
        source: InvalidationSource::Manual,
        timestamp: Utc::now(),
    }).await;

    Ok(StatusCode::NO_CONTENT)
}

// DELETE /admin/cache/pattern
// Body: { "pattern": "llm:exact:tenant-123:*", "scope": "l1+l2" }
// → Delete all keys matching pattern
async fn invalidate_pattern(
    Json(body): Json<PatternInvalidateRequest>,
) -> Result<Json<InvalidateResponse>> {
    // Use Redis SCAN + DEL (not KEYS — KEYS blocks on large datasets)
    let mut cursor = 0u64;
    let mut deleted = 0u64;

    loop {
        let (next_cursor, keys): (u64, Vec<String>) = redis::cmd("SCAN")
            .arg(cursor)
            .arg("MATCH")
            .arg(&body.pattern)
            .arg("COUNT")
            .arg(100)
            .query_async(&mut redis)
            .await?;

        if !keys.is_empty() {
            let count: i64 = redis.del(&keys).await?;
            deleted += count as u64;
        }

        cursor = next_cursor;
        if cursor == 0 {
            break;
        }
    }

    // Clear L1 entries that match pattern (iterate L1 keys)
    L1.invalidate_entries_if(|key, _| {
        // key in L1 is just the hash; we'd need to rebuild prefix
        // Simpler: clear entire L1 on pattern invalidation
        true // clear all L1 to be safe
    }).await;

    Ok(Json(InvalidateResponse { deleted }))
}

// POST /admin/cache/refresh
// Body: { "tenant_id": "*", "model": "gpt-4o" }
// → Set soft-expired flag; next requests will re-fetch from provider
async fn soft_invalidate(
    Json(body): Json<SoftInvalidateRequest>,
) -> Result<Json<InvalidateResponse>> {
    // Implementation: set a "generation" counter per tenant:model
    // Cache entries include the generation number; mismatch = treat as miss
    let gen_key = format!("cache:gen:{}:{}", body.tenant_id, body.model);
    let new_gen: u64 = redis.incr(&gen_key, 1).await?;

    // Also set TTL on gen key so it doesn't grow forever
    redis.expire(&gen_key, 86400 * 30).await?; // 30 days

    // Clear L1 entirely (conservative but correct)
    L1.invalidate_all();

    Ok(Json(InvalidateResponse { deleted: new_gen }))
}
```

### 6.3 Time-Based Expiry

**Two-tier expiry system:**

```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
struct CachedResponse {
    response: LLMResponse,
    cached_at: DateTime<Utc>,
    cache_source: CacheSource,
    generation: u64,            // Monotonic counter for soft invalidation
    soft_ttl_seconds: u64,      // Serve stale until this
    hard_ttl_seconds: u64,      // Redis EXPIRE — key deleted after this
}

// Hard TTL: Redis key-level EXPIRE
//   → Key is deleted by Redis automatically
//   → Zero memory leak guarantee

// Soft TTL: Application-level check on read
//   → If soft_ttl expired but hard_ttl not: serve stale + trigger background refresh
//   → "Stale-while-revalidate" pattern — no user-visible latency

async fn read_with_soft_ttl(key: &str) -> Option<CachedResponse> {
    let json: Option<String> = redis.get(key).await.ok()?;
    let cached: CachedResponse = serde_json::from_str(&json).ok()?;

    let age = Utc::now() - cached.cached_at;

    if age > Duration::seconds(cached.hard_ttl_seconds as i64) {
        // Hard expired — key should have been deleted by Redis, but double-check
        return None;
    }

    if age > Duration::seconds(cached.soft_ttl_seconds as i64) {
        // Soft expired — serve stale, trigger refresh
        tokio::spawn(async move {
            trigger_background_refresh(key).await;
        });
        return Some(cached); // stale but valid
    }

    Some(cached) // fresh
}
```

### 6.4 Event-Driven Invalidation

```rust
// Event bus (Redis pub/sub or in-process broadcast for single node)
#[derive(Clone, Debug, Serialize, Deserialize)]
enum CacheInvalidationEvent {
    ProviderConfigChanged { provider_id: String },
    RoutingRulesChanged,
    QuotaChanged { tenant_id: String, resource: String },
    ApiKeyRotated { provider_id: String },
    ModelUpdated { model: String },
    TenantPurged { tenant_id: String },
}

// Subscriber (runs in background task)
async fn invalidation_listener(mut pubsub: redis::aio::PubSub) {
    let mut stream = pubsub.on_message();
    while let Some(msg) = stream.next().await {
        let event: CacheInvalidationEvent = match serde_json::from_slice(&msg.get_payload_bytes()) {
            Ok(e) => e,
            Err(_) => continue,
        };

        match event {
            ProviderConfigChanged { provider_id } => {
                L1_CONFIG.invalidate(&provider_id).await;
                redis.del(format!("config:provider:{}", provider_id)).await.ok();
            }
            RoutingRulesChanged => {
                L1_CONFIG.invalidate_all();
                redis.del("config:routing:all").await.ok();
            }
            QuotaChanged { tenant_id, resource } => {
                let key = format!("quota:{}:{}", tenant_id, resource);
                L1_CONFIG.invalidate(&key).await;
                redis.del(&key).await.ok();
            }
            TenantPurged { tenant_id } => {
                // Delete ALL cache entries for tenant
                let pattern = format!("llm:*:{}:*", tenant_id);
                invalidate_pattern(&pattern).await.ok();
                L1.invalidate_all();
            }
            // ... etc
        }
    }
}

// Publisher (called by admin API / background workers)
async fn publish_invalidation(event: CacheInvalidationEvent) {
    let json = serde_json::to_string(&event).unwrap();
    redis.publish("cache:invalidations", json).await.ok();
}
```

### 6.5 Pattern-Based Purging Summary

| Pattern | Use Case | Performance | Risk |
|---------|----------|-------------|------|
| Full key | Single entry invalidation | O(1) | None |
| `SCAN` + `DEL` | Tenant/model purge | O(N) where N = matching keys | Blocks if N is huge; use `UNLINK` instead of `DEL` for async deletion |
| Generation counter | Bulk soft invalidation | O(1) | Lazy — stale data served until accessed |
| `FLUSHDB` | Nuclear option (emergency) | O(1) | Destroys ALL data; only for total cache reset |

**Recommendation:** Use generation counters for bulk invalidation (O(1), non-blocking). Use `SCAN` + `UNLINK` for precise purging when immediate removal is required.

---

## 7. Cache Metrics

### 7.1 Metrics Schema

All metrics are emitted as Prometheus-compatible counters/gauges, plus stored in Redis for dashboard retrieval.

```rust
// Prometheus metrics (using `metrics` crate with Prometheus exporter)
use metrics::{counter, gauge, histogram};

lazy_static! {
    // --- Hit/Miss Counters ---
    static ref CACHE_HIT_L1: Counter = counter!("cache_hit_total", "layer" => "l1");
    static ref CACHE_HIT_L2_EXACT: Counter = counter!("cache_hit_total", "layer" => "l2_exact");
    static ref CACHE_HIT_L2_SEMANTIC: Counter = counter!("cache_hit_total", "layer" => "l2_semantic");
    static ref CACHE_MISS: Counter = counter!("cache_miss_total");

    // --- Per-Model Breakdown ---
    static ref CACHE_HIT_BY_MODEL: CounterVec = CounterVec::new(
        opts!("cache_hit_by_model_total", "Cache hits per model"),
        &["model", "layer", "type"]
    ).unwrap();

    // --- Cost Savings ---
    static ref COST_SAVED_CENTS: Counter = counter!("cost_saved_cents_total");
    static ref COST_SAVED_BY_MODEL: CounterVec = CounterVec::new(
        opts!("cost_saved_cents_by_model", "Cost savings per model"),
        &["model", "provider"]
    ).unwrap();

    // --- Cache Size ---
    static ref L1_ENTRY_COUNT: Gauge = gauge!("cache_l1_entries");
    static ref L1_MEMORY_BYTES: Gauge = gauge!("cache_l1_memory_bytes");
    static ref L2_KEY_COUNT: Gauge = gauge!("cache_l2_keys");

    // --- Performance ---
    static ref LOOKUP_DURATION: Histogram = histogram!(
        "cache_lookup_duration_seconds",
        vec![0.0001, 0.0005, 0.001, 0.005, 0.01, 0.05, 0.1]
    );
    static ref WRITE_DURATION: Histogram = histogram!(
        "cache_write_duration_seconds",
        vec![0.0001, 0.0005, 0.001, 0.005, 0.01, 0.05, 0.1]
    );
}
```

### 7.2 Cost Savings Calculation

```rust
/// Per-model pricing table (updated periodically from provider docs)
static MODEL_PRICING: phf::Map<&'static str, TokenPricing> = phf_map! {
    "gpt-4o" => TokenPricing { input_per_1k: 0.00250, output_per_1k: 0.01000 },
    "gpt-4o-mini" => TokenPricing { input_per_1k: 0.00015, output_per_1k: 0.00060 },
    "claude-3-5-sonnet" => TokenPricing { input_per_1k: 0.00300, output_per_1k: 0.01500 },
    "claude-3-haiku" => TokenPricing { input_per_1k: 0.00025, output_per_1k: 0.00125 },
    // ... etc
};

fn record_cost_savings(response: &LLMResponse, model: &str) {
    let pricing = MODEL_PRICING.get(model);
    if pricing.is_none() { return; }
    let pricing = pricing.unwrap();

    // Calculate cost that would have been incurred
    let input_tokens = response.usage.prompt_tokens as f64;
    let output_tokens = response.usage.completion_tokens as f64;

    let input_cost = (input_tokens / 1000.0) * pricing.input_per_1k;
    let output_cost = (output_tokens / 1000.0) * pricing.output_per_1k;
    let total_cost_dollars = input_cost + output_cost;
    let total_cost_cents = total_cost_dollars * 100.0;

    // Record metric
    COST_SAVED_CENTS.increment_by(total_cost_cents as u64);
    COST_SAVED_BY_MODEL
        .with_label_values(&[model, provider])
        .increment_by(total_cost_cents as u64);

    // Also store in Redis for dashboard aggregation
    let daily_key = format!(
        "metrics:cost_saved:{}",
        Utc::now().format("%Y-%m-%d")
    );
    redis.hincr(daily_key, model, total_cost_cents).await.ok();
    redis.expire(daily_key, 86400 * 90).await.ok(); // Keep 90 days
}
```

### 7.3 Metrics Dashboard (Redis-Backed Aggregation)

```rust
// Periodic aggregator (runs every 60 seconds)
async fn aggregate_metrics() {
    let now = Utc::now();

    // --- Hit Rate Calculation ---
    let hits_l1: u64 = redis.hget("metrics:hits:l1", "count").await.unwrap_or(0);
    let hits_l2_exact: u64 = redis.hget("metrics:hits:l2_exact", "count").await.unwrap_or(0);
    let hits_l2_semantic: u64 = redis.hget("metrics:hits:l2_semantic", "count").await.unwrap_or(0);
    let misses: u64 = redis.hget("metrics:misses", "count").await.unwrap_or(0);

    let total = hits_l1 + hits_l2_exact + hits_l2_semantic + misses;
    let hit_rate = if total > 0 {
        (hits_l1 + hits_l2_exact + hits_l2_semantic) as f64 / total as f64
    } else { 0.0 };

    let semantic_hit_rate = if total > 0 {
        hits_l2_semantic as f64 / total as f64
    } else { 0.0 };

    // Store for dashboard
    let daily_key = format!("metrics:daily:{}", now.format("%Y-%m-%d"));
    redis.hset(&daily_key, "hit_rate", hit_rate.to_string()).await.ok();
    redis.hset(&daily_key, "semantic_hit_rate", semantic_hit_rate.to_string()).await.ok();
    redis.expire(&daily_key, 86400 * 365).await.ok();

    // --- Per-Model Breakdown ---
    for model in get_cached_models().await {
        let model_hits: u64 = redis.hget(&format!("metrics:hits:model:{}", model), "count")
            .await.unwrap_or(0);
        let model_misses: u64 = redis.hget(&format!("metrics:misses:model:{}", model), "count")
            .await.unwrap_or(0);
        let model_total = model_hits + model_misses;
        let model_hit_rate = if model_total > 0 {
            model_hits as f64 / model_total as f64
        } else { 0.0 };

        redis.hset(&daily_key, &format!("hit_rate:{}", model), model_hit_rate.to_string())
            .await.ok();
    }
}
```

### 7.4 Key Performance Indicators (KPIs)

| KPI | Target | Measurement |
|-----|--------|-------------|
| **Overall cache hit rate** | > 30% (month 1), > 45% (month 3) | (hits_l1 + hits_l2) / total_requests |
| **Semantic hit rate** | > 15% of total requests | hits_l2_semantic / total_requests |
| **Cost reduction** | 30-70% of provider spend | sum(cost_saved) / sum(actual_provider_cost) |
| **L1 lookup p99** | < 0.1ms | histogram percentile |
| **L2 lookup p99** | < 5ms | histogram percentile |
| **Cache write p99** | < 5ms | histogram percentile |
| **False positive rate (semantic)** | < 2% | Manual audit sample |
| **Memory overhead** | < 10% of cached response size | (cache_metadata_bytes / response_bytes) |

---

## 8. Security

### 8.1 Cache Poisoning Prevention

#### 8.1.1 Tenant Isolation

**Mechanism:** Every cache key includes a `tenant_id` prefix. This is non-negotiable.

```rust
// CORRECT: tenant in key
let key = format!("llm:exact:{}:{}:{}", tenant_id, model, hash);

// WRONG: shared key space (vulnerable to cross-tenant poisoning)
let key = format!("llm:exact:{}:{}", model, hash);
```

**Validation:** The gateway MUST authenticate the request and extract `tenant_id` BEFORE any cache lookup. The `tenant_id` used in cache keys MUST come from the authenticated identity, NOT from user-provided headers or request body.

```rust
async fn handle_request(req: Request) -> Response {
    // Step 1: Authenticate → extract tenant_id from auth token
    let identity = authenticate(&req).await?;
    let tenant_id = identity.tenant_id; // TRUSTED

    // Step 2: Cache lookup uses TRUSTED tenant_id
    if let Some(cached) = get_cached_response(&req, &tenant_id).await {
        return cached.into_response();
    }

    // ... proxy to provider
}
```

#### 8.1.2 Key Entropy

Cache keys are SHA-256 hashes. This prevents:

1. **Information leakage:** An attacker who gains Redis access cannot read prompts from keys (keys are opaque hashes).
2. **Key enumeration:** 2^256 key space makes brute-force enumeration infeasible.
3. **Length-based inference:** SHA-256 produces fixed-length output regardless of input size.

**Alternative considered:** Use base64-encoded raw request as key. **Rejected:** Leaks prompt content in Redis key names; violates confidentiality.

#### 8.1.3 Input Sanitization

```rust
fn is_safe_to_cache(req: &LLMRequest) -> bool {
    // Check for cache-busting patterns in messages
    let json = serde_json::to_string(&req.messages).unwrap();

    // Reject if messages contain timestamps or random values
    // These would never cache-hit and waste storage
    let cache_busting_patterns = [
        regex!(r"\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}"), // ISO timestamps
        regex!(r"\b[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}\b"), // UUIDs
        regex!(r"\b\d{13,}\b"), // Millisecond timestamps
    ];

    for pattern in &cache_busting_patterns {
        if pattern.is_match(&json) {
            return false; // Dynamic content — don't cache
        }
    }

    true
}
```

### 8.2 Sensitive Data Handling

#### 8.2.1 PII Detection (Basic)

```rust
fn contains_pii(text: &str) -> bool {
    lazy_static! {
        static ref PII_PATTERNS: Vec<Regex> = vec![
            // US Social Security Numbers
            regex!(r"\b\d{3}-\d{2}-\d{4}\b"),
            // Credit card numbers (basic Luhn-checkable patterns)
            regex!(r"\b(?:4\d{3}|5[1-5]\d{2}|3[47]\d{2})[\s-]?\d{4}[\s-]?\d{4}[\s-]?\d{4}\b"),
            // Email addresses
            regex!(r"\b[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Z|a-z]{2,}\b"),
            // Phone numbers (US format)
            regex!(r"\b\(?\d{3}\)?[\s.-]?\d{3}[\s.-]?\d{4}\b"),
            // API keys / secrets patterns
            regex!(r"\b(sk-[a-zA-Z0-9]{48})\b"), // OpenAI-style keys
            regex!(r"\b(ghp_[a-zA-Z0-9]{36})\b"), // GitHub PAT
        ];
    }

    PII_PATTERNS.iter().any(|re| re.is_match(text))
}

// In cache write path:
if contains_pii(&request_body_text) {
    // Don't cache requests containing PII
    // Log at debug level (not info — avoid logging PII)
    log::debug!("PII detected, skipping cache");
    return CacheDecision::Skip;
}
```

**Rationale:** PII in cache = compliance risk (GDPR, HIPAA). Basic regex detection catches common patterns without adding latency (regex is fast). Advanced PII detection (ML-based) can be added later as an optional middleware.

**Trade-off:** Some legitimate requests may be incorrectly flagged. Hit `false_positive_skip` metric and tune regexes.

#### 8.2.2 Opt-Out Header

```
X-Cache-No-Store: true
```

When present, the gateway skips ALL caching (read and write) for this request. Response is never stored; cache is never checked.

**Use case:** Clients processing sensitive data can ensure no data persistence.

### 8.3 Cache Key Security Summary

| Threat | Mitigation | Verification |
|--------|-----------| ------------|
| Cross-tenant poisoning | `tenant_id` in every key prefix | Code review: grep for key construction without tenant |
| Key enumeration | SHA-256 hash keys (2^256 space) | Static analysis: ensure no raw request data in keys |
| Information leakage via keys | Opaque hash keys | Redis `MONITOR` audit: keys should be hex strings only |
| PII in cache | Regex-based PII skip + opt-out header | Unit tests with known PII patterns |
| Cache timing attacks | Fixed-time cache lookup (always check L1, then L2) | No early returns on miss |
| Replay attacks | Cache entries tied to tenant + model + full request hash | Authenticated tenant_id in key |

---

## 9. Performance Targets

### 9.1 Target Matrix

| Operation | Target (p50) | Target (p99) | Measurement Method |
|-----------|-------------|--------------|-------------------|
| L1 cache read | < 0.05ms | < 0.1ms | `metrics` histogram, in-app |
| L2 cache read | < 2ms | < 5ms | `metrics` histogram, Redis `SLOWLOG` |
| L1 cache write | < 0.1ms | < 0.5ms | `metrics` histogram |
| L2 cache write | < 2ms | < 5ms | `metrics` histogram |
| Semantic search (embedding + ANN) | < 20ms | < 50ms | End-to-end timer |
| Embedding computation (local) | < 10ms | < 30ms | Timer around ONNX inference |
| Cache decision (all checks) | < 1ms | < 2ms | Total time in cache middleware |
| Memory overhead (L1) | < 5% of data | < 10% of data | `moka` weight-based sizing |
| Memory overhead (L2) | < 15% of data | < 25% of data | Redis `INFO memory` |

### 9.2 Load Testing Targets

| Scenario | Target |
|----------|--------|
| Single VPS (4 vCPU, 8GB RAM) | 1000 req/s sustained with 30% cache hit rate |
| L1 hit under load | No degradation at 10,000 req/s (moka is lock-free) |
| L2 (Redis) under load | < 5ms p99 at 5000 req/s (Redis is single-threaded, pipelining helps) |
| Embedding throughput | 500 prompts/second (batch size 32, ONNX runtime) |
| Max concurrent connections | 10,000 (Tokio async) |

### 9.3 Sizing Guidelines

| Resource | Sizing Formula | Example (1000 req/s, 30% hit) |
|----------|---------------|-------------------------------|
| L1 entries | ~1-2x unique request rate per minute | 10,000 entries |
| L1 memory | avg_response_size * entry_count * 1.5 | ~150 MB (10KB avg response) |
| L2 memory (exact) | daily_unique_requests * avg_response_size * 2 | ~2-5 GB |
| L2 memory (semantic vectors) | unique_prompts * 384 dims * 4 bytes * 2 | ~300 MB (100K unique prompts) |
| Redis total | L2 exact + L2 semantic + config + rate limit + session | ~4-8 GB |
| VPS RAM | Redis + Gateway + OS overhead | 8-16 GB total |

### 9.4 Performance Optimization Strategies

```rust
// 1. L1: Use rkyv for zero-copy deserialization in hot path
//    moka supports custom Weigher — size L1 by actual memory, not entry count

let cache = Cache::builder()
    .max_capacity(10_000)           // Max 10K entries
    .weigher(|_key, value: &CachedResponse| -> u32 {
        // Weight by estimated memory size in KB (capped at u32::MAX)
        (std::mem::size_of_val(value) / 1024).min(u32::MAX as usize) as u32
    })
    .time_to_live(Duration::from_secs(60))
    .time_to_idle(Duration::from_secs(30))
    .build();

// 2. L2: Pipeline Redis operations
//    Use redis::pipe() for batch gets/sets

let mut pipe = redis::pipe();
for key in &batch_keys {
    pipe.get(key);
}
let results: Vec<Option<String>> = pipe.query_async(&mut redis).await?;

// 3. Embeddings: Batch inference
//    Collect multiple semantic cache candidates, batch the embedding computation

let batch_size = 32; // Tune based on memory and latency requirements
for chunk in requests.chunks(batch_size) {
    let embeddings = embedding_model.embed(chunk, BatchOptions::default()).await?;
    // Process batch...
}

// 4. Hot path: Minimize allocations
//    Use stack-allocated arrays where possible
//    Reuse buffers for embedding computation

// 5. Async: Use tokio::spawn for cache writes (fire-and-forget)
//    Don't block response delivery on cache population

tokio::spawn(async move {
    cache_write_l2(key, value, ttl).await.ok(); // ignore errors
});
```

---

## 10. Semantic Cache Deep Dive

### 10.1 Embedding Model Selection

#### 10.1.1 Chosen Model: `sentence-transformers/all-MiniLM-L6-v2` (via `fastembed-rs`)

| Attribute | Value |
|-----------|-------|
| **Model** | `all-MiniLM-L6-v2` |
| **Dimensions** | 384 |
| **Parameters** | 22M |
| **Max sequence length** | 256 tokens |
| **ONNX runtime** | `ort` v2 (Rust bindings) |
| **Quantization** | INT8 (via ONNX Dynamic Quantization) |
| **Embedding time** | ~5ms per prompt (single), ~2ms per prompt (batch 32) |
| **Memory footprint** | ~30 MB model + 5 MB runtime |
| **License** | Apache-2.0 |

#### 10.1.2 Why This Model

| Criterion | `all-MiniLM-L6-v2` | Alternative: `BAAI/bge-small-en-v1.5` | Alternative: `OpenAI text-embedding-3-small` (API) |
|-----------|---------------------|----------------------------------------|---------------------------------------------------|
| **Speed** | Excellent (22M params) | Good (33M params) | N/A (network latency: 100-300ms) |
| **Quality** | Good for semantic similarity | Slightly better (fine-tuned for retrieval) | Best quality |
| **Cost** | Free (local compute) | Free (local compute) | $0.02 / 1M tokens (adds cost, defeats purpose) |
| **Local execution** | Yes (ONNX) | Yes (ONNX) | No (API call) |
| **Offline capable** | Yes | Yes | No |
| **License** | Apache-2.0 | MIT | Proprietary |

**Decision:** `all-MiniLM-L6-v2` provides the best speed/cost trade-off. The embedding quality is sufficient for LLM semantic caching (we need "similar meaning", not "perfect retrieval"). Upgrading to `bge-small-en-v1.5` is a drop-in replacement if quality needs improvement.

#### 10.1.3 Model Loading

```rust
use fastembed::{TextEmbedding, EmbeddingModel, InitOptions};

lazy_static! {
    static ref EMBEDDING_MODEL: TextEmbedding = {
        TextEmbedding::try_new(InitOptions {
            model_name: EmbeddingModel::AllMiniLML6V2,
            show_download_progress: true,
            ..Default::default()
        }).expect("Failed to load embedding model")
    };
}

/// Compute embedding for a single prompt.
/// Returns a normalized 384-dimensional f32 vector.
fn embed_prompt(prompt: &str) -> Vec<f32> {
    let embeddings = EMBEDDING_MODEL.embed(vec![prompt], None)
        .expect("Embedding inference failed");
    embeddings.into_iter().next().unwrap()
}
```

**Note:** Model is loaded ONCE at startup (~30MB RAM). First inference "warms up" the ONNX session. Warm-up should be done during gateway initialization (embed a dummy prompt).

### 10.2 Similarity Search

#### 10.2.1 Cosine Similarity

```rust
/// Compute cosine similarity between two normalized embedding vectors.
/// Vectors are assumed to be L2-normalized (fastembed outputs normalized vectors).
/// Range: [-1.0, 1.0] where 1.0 = identical direction.
fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    assert_eq!(a.len(), b.len(), "Embedding dimension mismatch");
    // Since vectors are already normalized, dot product = cosine similarity
    a.iter().zip(b.iter()).map(|(x, y)| x * y).sum()
}

/// Check if two prompts are semantically similar enough to share a cache entry.
fn is_semantic_match(query_embedding: &[f32], cached_embedding: &[f32], threshold: f32) -> bool {
    cosine_similarity(query_embedding, cached_embedding) >= threshold
}
```

#### 10.2.2 ANN Search Strategy: Redis-Backed Brute Force (Phase 1)

For initial deployment (single VPS, < 100K cached prompts), brute-force cosine similarity over stored embeddings is sufficient and simple.

```rust
/// Semantic search implementation — Phase 1 (brute force)
/// Stores embeddings alongside cached responses in Redis.
/// On lookup: fetch candidate embeddings, compute cosine similarity.
struct SemanticCacheL2 {
    redis: MultiplexedConnection,
    threshold: f32,
    max_candidates: usize, // Limit scan to prevent latency spikes
}

impl SemanticCacheL2 {
    async fn search(&mut self, tenant_id: &str, model: &str, query_embedding: &[f32])
        -> Option<CachedResponse>
    {
        // Key pattern for semantic entries of this tenant:model
        let pattern = format!("llm:semantic:{}:{}:*", tenant_id, model);

        // Use Redis HSCAN on a hash that stores embedding_id → (embedding, response_key)
        // Alternative: store in a single sorted set per tenant:model
        let hash_key = format!("llm:emb:index:{}:{}", tenant_id, model);

        // HGETALL to get all embeddings (acceptable for < 100K entries)
        let entries: HashMap<String, String> = redis.hgetall(&hash_key).await.ok()?;

        let mut best_match: Option<(f32, String)> = None;

        for (emb_id, emb_json) in entries {
            let cached_emb: Vec<f32> = match serde_json::from_str(&emb_json) {
                Ok(v) => v,
                Err(_) => continue,
            };

            let similarity = cosine_similarity(query_embedding, &cached_emb);

            if similarity >= self.threshold {
                if best_match.as_ref().map_or(true, |(best_sim, _)| similarity > *best_sim) {
                    best_match = Some((similarity, emb_id));
                }
            }
        }

        if let Some((sim, emb_id)) = best_match {
            // Fetch the actual cached response
            let response_key = format!("llm:semantic:{}:{}:{}", tenant_id, model, emb_id);
            let json: String = redis.get(&response_key).await.ok()?;
            let mut response: CachedResponse = serde_json::from_str(&json).ok()?;
            response.cache_source = CacheSource::Semantic { similarity: sim };
            return Some(response);
        }

        None
    }

    async fn store(&mut self, tenant_id: &str, model: &str,
                   embedding: &[f32], response: &CachedResponse, ttl: Duration) -> Result<()> {
        // Generate embedding ID from embedding content (deterministic)
        let emb_id = format!("{:x}", Sha256::digest(
            embedding.iter().map(|f| f.to_le_bytes()).flatten().collect::<Vec<_>>()
        ));

        // Store embedding in index
        let hash_key = format!("llm:emb:index:{}:{}", tenant_id, model);
        let emb_json = serde_json::to_string(embedding)?;
        redis.hset(&hash_key, &emb_id, emb_json).await?;
        redis.expire(&hash_key, ttl.as_secs() as usize * 3).await?;

        // Store actual response
        let response_key = format!("llm:semantic:{}:{}:{}", tenant_id, model, emb_id);
        let response_json = serde_json::to_string(response)?;
        redis.set_ex(&response_key, response_json, ttl.as_secs() as usize).await?;

        Ok(())
    }
}
```

**Phase 1 Complexity:**
- **Storage:** O(N) where N = cached prompts per tenant:model
- **Lookup:** O(N * D) where D = 384 dimensions; N < 100K → < 10ms
- **Write:** O(1) — single HSET + SETEX

#### 10.2.3 ANN Search Strategy: HNSW Index (Phase 2)

When cached prompts exceed 100K or lookup latency exceeds 20ms, upgrade to HNSW (Hierarchical Navigable Small World) approximate nearest neighbor.

```rust
/// Phase 2: Use `hnsw_rs` for in-memory ANN index
/// Index is rebuilt periodically from Redis-stored embeddings
/// Query path: Compute embedding → HNSW search (sub-ms) → Redis fetch response

use hnsw_rs::Hnsw;

struct SemanticCachePhase2 {
    redis: MultiplexedConnection,
    // In-memory HNSW index: embedding vector → embedding_id
    // Rebuilt every N minutes or on invalidation events
    hnsw: Arc<RwLock<Hnsw<f32, DistCosine>>>,
    // Mapping: embedding_id → Redis response key
    id_to_key: Arc<RwLock<HashMap<String, String>>>,
    threshold: f32,
}

impl SemanticCachePhase2 {
    async fn search(&self, query_embedding: &[f32]) -> Option<CachedResponse> {
        let hnsw = self.hnsw.read().await;
        let id_map = self.id_to_key.read().await;

        // Search for nearest neighbors
        let neighbors = hnsw.search(query_embedding, 5, 64); // ef=64, k=5

        for (embedding_id, distance) in neighbors {
            // HNSW returns L2 distance; convert to cosine similarity
            // For normalized vectors: cos_sim = 1 - (L2_distance^2 / 2)
            let l2_dist = distance.sqrt();
            let cos_sim = 1.0 - (l2_dist * l2_dist / 2.0);

            if cos_sim >= self.threshold {
                if let Some(response_key) = id_map.get(&embedding_id) {
                    // Fetch from Redis
                    if let Ok(Some(json)) = self.redis.get::<_, Option<String>>(response_key).await {
                        if let Ok(mut response) = serde_json::from_str::<CachedResponse>(&json) {
                            response.cache_source = CacheSource::Semantic { similarity: cos_sim };
                            return Some(response);
                        }
                    }
                }
            }
        }

        None
    }

    /// Rebuild HNSW index from Redis — called periodically or on significant invalidation
    async fn rebuild_index(&self) {
        // Fetch all embeddings from Redis
        let pattern = "llm:emb:index:*:*";
        // ... HSCAN all tenant:model hashes
        // ... Insert into HNSW
        // ... Update id_to_key mapping
    }
}
```

**Phase 2 Complexity:**
- **Build:** O(N log N) — amortized (rebuilt infrequently)
- **Lookup:** O(log N) — sub-millisecond ANN search
- **Memory:** ~2x raw embedding size (HNSW graph overhead)
- **Trade-off:** Slightly lower recall (99% vs 100%) for 100x speedup

**Decision:** Implement Phase 1 first. Phase 2 is a drop-in upgrade when metrics show N > 100K or lookup > 20ms p99.

#### 10.2.4 Alternative: Redis Vector Library (Phase 3)

Redis 7.2+ with RediSearch 2.8+ supports vector similarity search:

```sql
-- Create vector index
FT.CREATE llm_embeddings ON HASH PREFIX 1 llm:emb: SCHEMA emb VECTOR FLAT 6 DIM 384 DISTANCE_METRIC COSINE TYPE FLOAT32

-- Search
FT.SEARCH llm_embeddings "*=>[KNN 5 @emb $vec]" PARAMS 2 vec <binary_embedding> DIALECT 2
```

**Phase 3 Criteria:**
- RediSearch module available on VPS
- Need sub-5ms semantic search at > 500K prompts
- Willing to add module dependency

### 10.3 Threshold Tuning

#### 10.3.1 Default Threshold: 0.92

This threshold captures rephrasings while excluding meaning drift.

#### 10.3.2 Dynamic Threshold (Per-Tenant/Per-Model)

```rust
#[derive(Clone, Debug)]
struct SemanticThresholdConfig {
    /// Global default
    default: f32,
    /// Per-tenant overrides
    per_tenant: HashMap<String, f32>,
    /// Per-model overrides
    per_model: HashMap<String, f32>,
    /// Per-tenant:model overrides (highest priority)
    per_tenant_model: HashMap<(String, String), f32>,
}

impl SemanticThresholdConfig {
    fn get(&self, tenant_id: &str, model: &str) -> f32 {
        self.per_tenant_model
            .get(&(tenant_id.to_string(), model.to_string()))
            .copied()
            .or_else(|| self.per_model.get(model).copied())
            .or_else(|| self.per_tenant.get(tenant_id).copied())
            .unwrap_or(self.default)
    }
}
```

#### 10.3.3 Threshold Calibration Process

```
Step 1: Deploy with threshold 0.92
Step 2: Collect for 1 week:
  - semantic_hit_rate (metric)
  - semantic_false_positive_samples (manual audit 100 random semantic hits)
Step 3: If false_positive_rate > 2%: increase threshold by 0.02
Step 4: If semantic_hit_rate < 10% AND false_positive_rate < 1%: decrease threshold by 0.02
Step 5: Repeat until convergence
```

### 10.4 Fallback Chain

```rust
async fn get_response_with_fallback(req: &LLMRequest) -> Option<CachedResponse> {
    // Priority 1: L1 exact match (fastest, highest confidence)
    if let Some(hit) = check_l1_exact(req).await {
        return Some(hit.with_source(CacheSource::L1Exact));
    }

    // Priority 2: L2 exact match
    if let Some(hit) = check_l2_exact(req).await {
        // Promote to L1
        l1_insert(req, &hit).await;
        return Some(hit.with_source(CacheSource::L2Exact));
    }

    // Priority 3: L2 semantic match (slower, lower confidence)
    if req.cache_config.semantic_enabled {
        let threshold = req.cache_config.semantic_threshold;
        if let Some(hit) = check_l2_semantic(req, threshold).await {
            // DO NOT promote semantic hits to L1 (lower confidence)
            return Some(hit.with_source(CacheSource::L2Semantic));
        }
    }

    // All cache layers missed — fall through to LLM provider
    None
}
```

### 10.5 Storage of Vectors

#### 10.5.1 Comparison: Redis vs In-Memory vs Dedicated Vector DB

| Approach | Latency | Capacity | Persistence | Cost | Complexity |
|----------|---------|----------|-------------|------|------------|
| **Redis hash + brute force** | 5-20ms | ~100K | Yes (Redis persistence) | $0 (included) | Low |
| **In-memory (HNSW)** | 0.1-1ms | ~1M | No (rebuild on restart) | $0 | Medium |
| **Redis + RediSearch vector** | 2-5ms | ~1M | Yes | $0 (module) | Medium |
| **Dedicated (Qdrant/Milvus)** | 1-2ms | Unlimited | Yes | +$50-200/mo | High |

**Decision:**
- **Phase 1 (now):** Redis hash + brute force — simplest, sufficient for launch
- **Phase 2 (growth):** In-memory HNSW with Redis persistence — best latency
- **Phase 3 (scale):** Redis RediSearch vector — if available and needed
- **Phase 4 (enterprise):** Dedicated vector DB — only if > 10M cached prompts

#### 10.5.2 Vector Storage Format

```rust
/// Embedding stored as JSON array of f32 values.
/// 384 dims * 4 bytes = 1536 bytes per embedding.
/// With JSON overhead: ~3000 bytes per embedding in Redis.
/// 100K embeddings: ~300 MB in Redis.

/// Optimization: Store as binary blob for 2x space reduction
/// Use MessagePack or raw bytes instead of JSON.

// JSON format (human-readable, debuggable):
// "llm:emb:index:tenant-1:gpt-4o" → hash:
//   "emb_abc123" → "[0.023, -0.156, 0.891, ... (384 values)]"

// Binary format (space-efficient):
//   "emb_abc123" → raw bytes: [0x00, 0x00, 0x00, 0x00, ...] (1536 bytes)

// Phase 1: JSON (debuggable)
// Phase 2: MessagePack binary (efficient)
```

### 10.6 Semantic Cache Update Strategy

#### 10.6.1 Write-Through (Default)

On cache miss → fetch from provider → store in BOTH exact and semantic caches simultaneously.

```rust
async fn handle_cache_miss(req: &LLMRequest) -> Result<LLMResponse> {
    // Fetch from LLM provider
    let response = provider.complete(req).await?;

    // Write to exact cache (always)
    cache_exact(req, &response).await;

    // Write to semantic cache (if enabled)
    if req.cache_config.semantic_enabled {
        cache_semantic(req, &response).await;
    }

    Ok(response)
}
```

#### 10.6.2 Semantic Deduplication

Before storing a new semantic entry, check if a semantically similar entry already exists:

```rust
async fn cache_semantic(req: &LLMRequest, response: &LLMResponse) {
    let embedding = embed_prompt(&req.prompt_text());

    // Check if an existing semantic entry is "close enough"
    // Use a tighter threshold for dedup (e.g., 0.97) than for retrieval (0.92)
    let dedup_threshold = 0.97;

    if let Some(existing) = semantic_search_l2(req, dedup_threshold).await {
        // Don't store duplicate — the existing entry covers this prompt
        log::debug!("Semantic dedup: skipping storage (similarity {:.3})", existing.similarity);
        return;
    }

    // Store new semantic entry
    semantic_cache_l2.store(tenant_id, model, &embedding, response, ttl).await;
}
```

**Rationale:** Prevents unbounded growth of the semantic cache. Two prompts with 0.95 similarity will return the same answer; storing both is wasteful.

#### 10.6.3 Eviction Policy

When the semantic cache grows beyond configured limits:

```rust
enum SemanticEvictionPolicy {
    /// Remove oldest entries first (by cached_at timestamp)
    OldestFirst,
    /// Remove least frequently accessed entries
    /// (track access count in Redis hash field)
    LeastFrequentlyUsed,
    /// Remove entries with the lowest similarity scores
    /// (these are "marginal" matches, less valuable)
    LowestSimilarity,
    /// Combined: score = age * (1 / access_count)
    /// Evict lowest score
    CombinedScore,
}

// Default: CombinedScore — balances freshness and utility
```

### 10.7 Expected Performance Impact

| Metric | Exact-Only Cache | With Semantic Cache | Improvement |
|--------|-----------------|---------------------|-------------|
| Hit rate (new deployment) | 5-10% | 20-35% | **3-5x** |
| Hit rate (mature, high volume) | 15-25% | 40-60% | **2-3x** |
| Additional latency on miss | 0ms | ~10ms (embedding) | Minimal |
| Additional latency on semantic hit | 0ms | ~15ms (embedding + search) | vs 500-3000ms provider call |
| Storage overhead per entry | 0 | +3KB (embedding) | Acceptable |
| False positive rate | 0% | < 2% (tunable) | Acceptable |

---

## Appendix A: Data Structures Reference

### A.1 CachedResponse

```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
struct CachedResponse {
    /// The actual LLM response (choices, usage, etc.)
    response: LLMResponse,
    /// When this entry was cached
    cached_at: DateTime<Utc>,
    /// How this entry was cached / retrieved
    cache_source: CacheSource,
    /// Generation counter for soft invalidation
    generation: u64,
    /// Embedding vector (stored for semantic entries, None for exact)
    #[serde(skip_serializing_if = "Option::is_none")]
    embedding: Option<Vec<f32>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
enum CacheSource {
    L1Exact,
    L2Exact,
    Semantic { similarity: f32 },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct LLMResponse {
    id: String,
    object: String,
    created: i64,
    model: String,
    choices: Vec<Choice>,
    usage: Usage,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Choice {
    index: i32,
    message: Message,
    finish_reason: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Usage {
    prompt_tokens: i32,
    completion_tokens: i32,
    total_tokens: i32,
}
```

### A.2 CacheConfig (Per-Request)

```rust
#[derive(Clone, Debug)]
struct CacheConfig {
    enabled: bool,
    ttl_override: Option<Duration>,
    semantic_enabled: bool,
    semantic_threshold: f32,
    force_refresh: bool,
    no_store: bool,
    streaming_cache: bool,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            ttl_override: None,
            semantic_enabled: true,
            semantic_threshold: 0.92,
            force_refresh: false,
            no_store: false,
            streaming_cache: false,
        }
    }
}
```

### A.3 Cache Decision Flow

```
REQUEST ARRIVES
│
├─► Auth → extract tenant_id (TRUSTED)
│
├─► Parse request → extract model, messages, parameters
│
├─► is_cacheable()? (Section 2.1)
│   ├─ temperature > threshold? ──► BYPASS CACHE ──► PROXY TO PROVIDER
│   ├─ streaming without flag? ──► BYPASS CACHE
│   ├─ PII detected? ────────────► BYPASS CACHE
│   ├─ no_store header? ─────────► BYPASS CACHE
│   └─ dynamic content? ─────────► BYPASS CACHE
│
├─► force_refresh? ──► SKIP READ, WRITE ON RESPONSE
│
├─► COMPUTE EXACT KEY (SHA-256)
│
├─► L1 LOOKUP (exact key)
│   ├─ HIT ──► RETURN CACHED + record metrics
│   └─ MISS ──► L2 LOOKUP
│       ├─ EXACT HIT ──► PROMOTE TO L1 + RETURN + record metrics
│       └─ EXACT MISS ──► SEMANTIC LOOKUP (if enabled)
│           ├─ SEMANTIC HIT ──► RETURN (no L1 promote) + record metrics
│           └─ SEMANTIC MISS ──► PROXY TO PROVIDER
│               │
│               ▼
│           RESPONSE RECEIVED
│               │
│               ├─ is_cacheable response? (no errors)
│               │   ├─ YES ──► WRITE L2 EXACT + WRITE L2 SEMANTIC + WRITE L1
│               │   └─ NO  ──► SKIP (don't cache errors)
│               │
│               └─ RETURN RESPONSE TO CLIENT
```

---

## Appendix B: Configuration Reference

### B.1 Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `REDIS_URL` | `redis://127.0.0.1:6379` | Redis connection URL |
| `CACHE_L1_MAX_CAPACITY` | `10000` | Max L1 cache entries |
| `CACHE_L1_TTL_SECONDS` | `60` | L1 entry TTL |
| `CACHE_L2_DEFAULT_TTL_SECONDS` | `3600` | Default L2 TTL |
| `CACHE_SEMANTIC_ENABLED` | `true` | Enable semantic caching |
| `CACHE_SEMANTIC_THRESHOLD` | `0.92` | Default similarity threshold |
| `CACHE_SEMANTIC_MAX_CANDIDATES` | `100000` | Max semantic entries per tenant:model |
| `CACHE_STREAMING_ENABLED` | `false` | Cache streaming responses |
| `CACHE_PII_CHECK_ENABLED` | `true` | Enable PPI detection skip |
| `EMBEDDING_MODEL` | `all-MiniLM-L6-v2` | Embedding model name |
| `EMBEDDING_BATCH_SIZE` | `32` | Batch size for embedding inference |
| `METRICS_ENABLED` | `true` | Emit cache metrics |
| `CACHE_SOFT_TTL_MULTIPLIER` | `2` | Soft TTL = default TTL * multiplier |
| `CACHE_HARD_TTL_MULTIPLIER` | `3` | Hard TTL = default TTL * multiplier |

### B.2 Per-Model TTL Overrides (Config File)

```yaml
# cache-config.yaml
cache:
  default_ttl_seconds: 3600
  model_overrides:
    gpt-4o:
      ttl_seconds: 3600
      semantic_threshold: 0.92
    gpt-4o-mini:
      ttl_seconds: 7200
      semantic_threshold: 0.90
    claude-3-5-sonnet:
      ttl_seconds: 3600
      semantic_threshold: 0.92
    claude-3-haiku:
      ttl_seconds: 10800
      semantic_threshold: 0.88
    text-embedding-3-small:
      ttl_seconds: 86400
      semantic_enabled: false  # Embeddings don't need semantic cache
```

---

## Appendix C: Implementation Checklist

### Phase 1 — MVP (Week 1-2)

- [ ] L1 cache (moka) with exact-match keys
- [ ] L2 cache (Redis) with exact-match keys
- [ ] Cache write on LLM response
- [ ] Cache read in request path
- [ ] TTL support (default + per-request override)
- [ ] Cache skip for non-cacheable requests (temperature > 0, streaming, errors)
- [ ] Basic metrics (hit/miss counters)
- [ ] Tenant isolation in keys

### Phase 2 — Semantic Cache (Week 3-4)

- [ ] Embedding model integration (fastembed-rs)
- [ ] Semantic cache storage in Redis
- [ ] Cosine similarity search (brute force)
- [ ] Configurable similarity threshold
- [ ] Semantic deduplication on write
- [ ] Semantic hit metrics
- [ ] Threshold calibration process

### Phase 3 — Advanced Features (Week 5-6)

- [ ] Configuration cache (provider, routing, quota)
- [ ] Rate limit cache (sliding window + token bucket)
- [ ] Session cache
- [ ] Manual invalidation API
- [ ] Event-driven invalidation (pub/sub)
- [ ] Soft TTL / stale-while-revalidate
- [ ] Pattern-based purging
- [ ] PII detection skip
- [ ] Cost savings calculation

### Phase 4 — Optimization (Week 7-8)

- [ ] HNSW index for semantic search (Phase 2)
- [ ] rkyv zero-copy deserialization for L1
- [ ] Redis pipelining for batch operations
- [ ] Embedding batch inference optimization
- [ ] Memory usage optimization (binary vector storage)
- [ ] Load testing and performance tuning
- [ ] Dashboard for cache metrics visualization

---

## Appendix D: Decision Log

| # | Decision | Chosen | Rejected | Rationale |
|---|----------|--------|----------|-----------|
| 1 | L1 cache crate | `moka` | `dashmap`, `cached` | Built-in TTL, async API, LRU eviction |
| 2 | L2 cache | Redis (single VPS) | Valkey, Memcached | Mature Rust client, pub/sub, persistence |
| 3 | Embedding model | `all-MiniLM-L6-v2` | `bge-small`, OpenAI API | Best speed/cost, local, Apache-2 |
| 4 | Embedding runtime | `fastembed-rs` (ONNX) | `ort` raw, remote API | Higher-level, simpler integration |
| 5 | Similarity search (P1) | Brute force over Redis | HNSW, RediSearch | Simplest, sufficient for < 100K |
| 6 | Key hashing | SHA-256 | Blake3, raw content | SHA-256 is standard, well-known, sufficient speed |
| 7 | Serialization (L2) | JSON | MessagePack, bincode | Human-debuggable, language-agnostic |
| 8 | Serialization (L1) | `rkyv` (planned) | JSON | Zero-copy deserialization for hot path |
| 9 | Rate limit algorithm | Sliding window (Lua) | Token bucket only, fixed window | Sliding window is fair; Lua for atomicity |
| 10 | Session storage | L2 only | L1 + L2 | Consistency requirement; low access frequency |
| 11 | CDN (L3) | None | CloudFront, Cloudflare | AI responses are dynamic; CDN hit rate < 2% |
| 12 | Vector storage (P1) | Redis hash | Separate vector DB | Single infrastructure component, simpler ops |

---

*End of Cache Architecture Specification v1.0.0*
