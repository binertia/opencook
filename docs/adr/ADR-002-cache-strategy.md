# ADR-002: Cache Strategy

## Status
Accepted

## Decision
We will implement a **multi-level cache** with two tiers: an **in-process L1 cache** (using the `moka` crate) and a **shared L2 cache** (using Redis). Both exact-match and semantic caching are supported, with semantic caching being the primary differentiator for cost reduction. The cache system is designed to reduce AI provider costs by 30-70%.

## Context
LLM API calls are expensive and slow. A single GPT-4o request can cost $0.01-$0.10 and take 2-10 seconds. Many AI workloads repeat identical or semantically similar prompts:
- Customer support bots answer the same FAQ questions repeatedly
- Code generation tools see recurring patterns
- RAG applications query similar documents
- Content summarization pipelines process similar text

Without caching, every request incurs full provider cost and latency. A cache miss costs $0.05; a cache hit costs near-zero and returns in milliseconds.

Key forces:
- AI Gateway targets a single VPS deployment; cache must fit in limited RAM
- The primary value proposition is cost reduction on LLM API calls
- Exact-match caching alone yields only 5-15% hit rates for natural language (users rephrase the same question)
- Semantic caching (matching meaning, not text) increases hit rates to 25-50%
- Cache must respect tenant isolation (no cross-tenant data leakage)
- Response freshness matters: cached responses should not outlive model updates

## Alternatives Considered

### Alternative 1: Single-Layer Cache (Redis Only)
- **Description:** Use only Redis for all caching, skipping the in-process L1 layer entirely.
- **Why rejected:** Every cache lookup would require a network round-trip to Redis (~0.5-2ms). While acceptable for response caching, configuration and provider metadata lookups happen on every request; a 1ms Redis round-trip on every request at 1000 req/s wastes 1 second of latency budget per 1000 requests. The in-process L1 reduces this to sub-microsecond for hot data.

### Alternative 2: File-System Cache
- **Description:** Store cached responses on disk using SQLite or flat files.
- **Why rejected:** Disk I/O is orders of magnitude slower than memory (10ms vs. 1 microsecond). LLM responses are frequently accessed; disk latency would negate the benefit. Also introduces file-system management complexity on a VPS.

### Alternative 3: CDN Edge Caching (Cloudflare, etc.)
- **Description:** Cache responses at CDN edge nodes close to users.
- **Why rejected:** AI responses are non-deterministic (even with temperature=0, system prompts vary by tenant) and user-specific. CDN hit rates would be <2% due to request diversity. CDN cache invalidation is path-based and coarse; LLM caching needs fine-grained semantic invalidation. CDN egress costs often exceed LLM API savings.

### Alternative 4: Semantic Cache Only (No Exact Match)
- **Description:** Skip exact-match caching and rely solely on semantic similarity.
- **Why rejected:** Exact-match caching is essentially free (SHA-256 hash lookup, no embedding computation) and catches 5-15% of traffic. Semantic caching requires embedding generation (~20ms CPU, ~80MB RAM). Running both layers provides combined hit rates of 25-50% vs. 20-35% for semantic alone.

## Tradeoffs

### What We Gain
- **Dramatic cost reduction:** 25-50% cache hit rate directly translates to 25-50% reduction in LLM API spend.
- **Sub-millisecond cache hits:** L1 hits are <1 microsecond; L2 hits are <5ms over localhost Redis.
- **Semantic moat:** Semantic caching is a key differentiator from simple API proxies; competitors without it offer inferior cost savings.
- **Tenant isolation:** Cache keys always include tenant ID, making cross-tenant leakage structurally impossible.
- **Configurable TTL:** Per-model TTL defaults (embedding models: 24h; chat models: 1h) balance freshness with hit rate.

### What We Give Up
- **Memory consumption:** Response cache dominates RAM. At 1000 req/s with 1h TTL, Redis can grow to ~14GB (mitigated by `allkeys-lru` eviction and `maxmemory` limits).
- **Stale responses:** Cached responses may reflect an older model version. Mitigated by TTL and manual invalidation API.
- **Embedding compute cost:** Semantic caching requires local ONNX runtime (~80MB RAM, ~20ms CPU per uncached request).
- **Complexity:** Two cache layers, two lookup mechanisms (hash + vector similarity), invalidation logic, and streaming cache support increase system complexity.

## Consequences
- L1 in-process cache uses `moka` with TTL + LRU eviction, max 10,000 entries / 50MB. It caches provider configs, org settings, and exact-match responses.
- L2 Redis cache uses key patterns: `llm:exact:{tenant}:{model}:{hash}` and `llm:semantic:{tenant}:{model}:{embedding_id}`. TTL defaults to 1 hour for chat, 24 hours for embeddings.
- Exact-match cache keys are SHA-256 hashes of normalized request JSON (sorted keys, normalized whitespace, deterministic ordering).
- Semantic cache uses a local ONNX embedding model (`all-MiniLM-L6-v2`, 384-dim vectors) with cosine similarity threshold of 0.92 (configurable per-request via `X-Semantic-Threshold`).
- Streaming responses are cached as aggregated complete responses; on cache hit they are re-chunked and streamed back with synthetic inter-chunk delays.
- Cache write-back is async and non-blocking: responses are cached after being sent to the client. Cache write failures are logged, not fatal.
- Manual invalidation is available via admin API: per-key deletion, pattern-based purge (SCAN + UNLINK), and soft invalidation via generation counters.
- No automatic cache warming: AI requests are too variable; warming would be mostly ineffective.

## Related Decisions
- **ADR-005 (Tenant Model):** Cache keys include tenant ID for isolation; tenant purge invalidates all associated cache entries.
- **ADR-007 (Fallback Strategy):** Provider failover does not bypass cache; cache is checked before provider selection.
- **ADR-008 (Ollama Support):** Ollama responses are NOT cached; local inference is "free" and caching adds no value while consuming RAM.

## Notes
- **Performance target:** <25ms total gateway overhead excluding provider latency; cache lookups must be <5ms (L2) or <0.1ms (L1).
- **Semantic cache hit rate expectations by workload:**
  - Customer support FAQ: 30-40% combined
  - Code generation (repetitive): 35-45% combined
  - RAG (unique queries): 7-12% combined
  - Chat (conversational): 4-6% combined
- **Security:** Requests containing PII patterns (SSN, credit card, email) bypass the cache entirely. The `X-Cache-No-Store: true` header forces a cache bypass.
- **Future work:** Redis Vector Search (RediSearch) for larger-scale semantic search; cross-tenant cache sharing for common prompts (with privacy guarantees).
