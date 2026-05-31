# ADR-004: Rate Limiting

## Status
Accepted

## Context
The AI Gateway mediates access to expensive LLM APIs. Without rate limiting, a single misconfigured client, runaway script, or malicious actor can:
- Exhaust an organization's monthly budget in minutes
- Trigger provider rate limits that affect all gateway users
- Generate thousands of dollars in unexpected API charges
- Overwhelm the gateway with requests, causing denial of service

Key forces:
- Rate limiting must be fast (<5ms per check); it runs on every request
- Different scopes need different limits: global (gateway protection), organization (budget protection), API key (granular control), provider (upstream protection)
- LLM requests are expensive in tokens, not just in count; token-per-minute limits are as important as request-per-second limits
- Bursts are normal (user opens a chat UI, sends 5 messages quickly); the limiter must allow short bursts while enforcing long-term averages
- The gateway targets a single-node deployment; distributed consensus for rate limiting is unnecessary

## Decision
We will implement a **Redis-backed hybrid rate limiting system** using a **token bucket** for burst-tolerant request limiting and a **sliding window log** for precise token-per-minute accounting.

**Algorithm choice:**
- **Token bucket** (Redis hash + Lua script): Primary algorithm for request-per-second limits. Allows bursts up to bucket capacity while maintaining average rate. Tokens refill continuously based on elapsed time.
- **Sliding window log** (Redis sorted set + Lua script): Secondary algorithm for token-per-minute limits. Stores timestamps of each request; evicts entries outside the window; counts remaining. Precise but slightly more memory-intensive.

**Why not fixed window:**
A fixed window allows a thundering herd at the window boundary. A client can send 2x the limit by sending N requests at 00:00:00 and another N requests at 00:00:01. The sliding window and token bucket eliminate this edge case.

**Rate limit layers (checked in order, short-circuit on rejection):**

| Layer | Key Pattern | Purpose | Default |
|-------|-------------|---------|---------|
| 1. Global | `ratelimit:global:req` | Protect the gateway instance | 2000 req/s |
| 2. Organization | `ratelimit:org:{id}:req` | Per-customer budget protection | Configurable |
| 3. API Key | `ratelimit:key:{id}:req` | Granular per-key control | Inherits from org |
| 4. API Key (tokens) | `ratelimit:key:{id}:tok` | Token-per-minute limit | Inherits from org |
| 5. Provider | `ratelimit:prov:{name}:req` | Prevent hitting upstream limits | Provider-specific |
| 6. IP Address | `ratelimit:ip:{ip}:req` | DDoS / abuse protection | 100 req/s |

**Lua script for atomic token bucket operation:**
```lua
local key = KEYS[1]
local rate = tonumber(ARGV[1])        -- tokens per second
local burst = tonumber(ARGV[2])       -- bucket capacity
local now = tonumber(ARGV[3])         -- current time in ms
local cost = tonumber(ARGV[4])        -- tokens this request costs

local bucket = redis.call('HMGET', key, 'tokens', 'last_update')
local tokens = tonumber(bucket[1]) or burst
local last_update = tonumber(bucket[2]) or now

local elapsed = (now - last_update) / 1000.0
local new_tokens = math.min(burst, tokens + elapsed * rate)

if new_tokens >= cost then
    new_tokens = new_tokens - cost
    redis.call('HMSET', key, 'tokens', new_tokens, 'last_update', now)
    redis.call('EXPIRE', key, 3600)
    return {1, math.floor(new_tokens)}
else
    redis.call('HSET', key, 'last_update', now)
    redis.call('EXPIRE', key, 3600)
    return {0, math.floor(new_tokens)}
end
```

Lua scripts execute atomically on the Redis server, preventing race conditions between read and update operations.

**Default rate limit tiers:**

| Tier | Req/s | Burst | Tok/min | Cost/day | Concurrent |
|------|-------|-------|---------|----------|------------|
| Free / Development | 10 | 20 | 100K | $10 | 5 |
| Small Business | 100 | 200 | 1M | $100 | 20 |
| Business | 500 | 1000 | 5M | $500 | 100 |
| Enterprise | Custom | Custom | Custom | Custom | Custom |

**In-process rate limit cache:**
To avoid a Redis round-trip on every request, a local token bucket mirror is maintained in L1 cache for each active key, synced to Redis every 10 seconds. This reduces the rate limit check from ~2ms (Redis RTT) to ~200ns (in-process lookup), with acceptable drift of up to 10 seconds of slightly exceeded limits.

**Rate limit response format (RFC 6585 compliant):**
```
HTTP/1.1 429 Too Many Requests
Retry-After: 2
RateLimit-Limit: 100
RateLimit-Remaining: 0
RateLimit-Reset: 1704153600
X-RateLimit-Layer: organization
```

## Alternatives Considered

### Alternative 1: Fixed Window Counter
- **Description:** Divide time into fixed windows (e.g., 1-second buckets). Count requests per window; reject when count exceeds limit.
- **Why rejected:** Allows 2x the intended rate at window boundaries (the "thundering herd" problem: N requests at the end of one window + N requests at the start of the next = 2N in a 1-second span). Unacceptable for cost protection where a burst can exhaust budgets.

### Alternative 2: Pure In-Process Rate Limiting
- **Description:** Track rate limits entirely in the gateway process memory using HashMaps.
- **Why rejected:** In-process state is lost on restart, allowing unlimited requests immediately after a crash/redeploy. No visibility into rate limit state from other processes (e.g., admin dashboard cannot display current usage). Cannot support horizontal scaling if needed in the future.

### Alternative 3: PostgreSQL-Backed Rate Limiting
- **Description:** Store rate limit counters in PostgreSQL with `UPDATE ... RETURNING`.
- **Why rejected:** PostgreSQL write throughput is the system's bottleneck. Adding a write on every request (not just every LLM call) would significantly increase database load. PostgreSQL row-level locking for concurrent updates introduces contention and latency spikes under burst traffic.

### Alternative 4: Third-Rate Limiting Service (e.g., Envoy, Kong)
- **Description:** Use an external rate limiting service or API gateway instead of implementing it in the gateway.
- **Why rejected:** Adds an additional infrastructure component to deploy and operate. Violates the "single binary" and "operable by one person" principles. The gateway already has Redis for caching; using Redis for rate limiting is a natural extension with no new dependencies.

## Tradeoffs

### What We Gain
- **Smooth traffic shaping:** Token bucket allows legitimate bursts while preventing sustained overuse.
- **Precise token accounting:** Sliding window log provides accurate token-per-minute enforcement for cost-sensitive workloads.
- **Multi-layer protection:** Six independent layers protect against different failure modes (DDoS, runaway scripts, provider limits).
- **Atomic operations:** Lua scripts ensure no race-condition overruns under concurrent load.
- **Low latency:** In-process cache reduces rate limit check to ~200ns for active keys.

### What We Give Up
- **Redis dependency:** Rate limiting requires Redis; if Redis is unavailable, rate limiting fails open or blocks requests (configurable, default: fail closed).
- **Memory overhead:** Sliding window sorted sets consume O(requests_in_window) memory per key. At 1000 req/s with a 1-minute window, each key uses ~60,000 entries.
- **10-second drift:** The in-process cache sync window means a client could exceed their limit by up to 10 seconds worth of requests before Redis enforces the global state.
- **Complexity:** Two algorithms (token bucket + sliding window) across six layers with Lua scripting is significantly more complex than a simple counter.

## Consequences
- Every request is checked against up to 6 rate limit layers in order; the first rejection short-circuits the rest.
- Rate limit checks happen after authentication but before quota checks and cache lookups.
- Token bucket keys have a 1-hour TTL; sliding window keys expire at window end.
- The `RateLimit-*` headers are included on every response to enable client-side backoff.
- In-process rate limit state is synced to Redis every 10 seconds; on gateway restart, state is reconstructed from Redis within one sync cycle.
- Provider-level rate limits prevent the gateway from being rate-limited by upstream providers, which would cascade errors to all users.
- Emergency bypass: super-admin keys can include `X-Emergency-Bypass` to bypass rate limits in production incidents (logged and alerted).

## Related Decisions
- **ADR-002 (Cache Strategy):** In-process rate limit state is stored in the L1 cache alongside other hot data.
- **ADR-003 (Authentication):** API keys carry a `rate_limit_tier` that determines which tier configuration applies.
- **ADR-005 (Tenant Model):** Organization-level rate limits enforce per-tenant resource boundaries.

## Notes
- Rate limiting is **fail-closed**: if Redis is unreachable, requests are rejected with `503 Service Unavailable` rather than allowing unlimited access. This is configurable for private deployments.
- The sliding window log uses Redis sorted sets with millisecond timestamps as scores; `ZREMRANGEBYSCORE` evicts old entries efficiently.
- Token bucket refill uses wall-clock time (from the gateway), not Redis time, to avoid clock skew issues between gateway and Redis running on the same host.
- Future work: Adaptive rate limiting based on provider response headers (OpenAI returns `x-ratelimit-*` headers that could dynamically adjust limits).
