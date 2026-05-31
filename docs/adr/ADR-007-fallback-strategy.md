# ADR-007: Fallback Strategy

## Status
Accepted

## Context
LLM providers experience outages, rate limits, and degraded performance. When a primary provider fails, the gateway must transparently route requests to a fallback provider to maintain service availability. Customers depend on the gateway for production workloads; provider downtime must not cause customer downtime.

Key forces:
- Provider failures are common: OpenAI, Anthropic, and Gemini each experience incidents several times per year
- Failover must be automatic and fast (seconds, not minutes)
- Different providers have different model capabilities; not all models have direct equivalents
- Customers should control their fallback preferences (cost vs. latency vs. capability)
- Circuit breakers prevent cascading failures from repeated attempts to a failing provider
- Health checks must distinguish between transient errors (retryable) and persistent failures (circuit open)

## Decision
We will implement a **provider failover system** with circuit breaker pattern, automatic health checks, and customer-configurable fallback chains.

**Circuit Breaker Pattern:**
Three states: `CLOSED` (normal) → `OPEN` (failing) → `HALF_OPEN` (testing recovery).

```rust
struct CircuitBreakerConfig {
    failure_threshold: u32,      // 5 consecutive failures → OPEN
    success_threshold: u32,      // 2 consecutive successes → CLOSE
    timeout_secs: u64,           // 30 seconds in OPEN before HALF_OPEN
    half_open_max_requests: u32, // 3 test requests in HALF_OPEN
    error_types: Vec<ErrorType>, // timeouts, 5xx, connection refused
}
```

Counted as failures: timeouts, 5xx responses, connection refused.
NOT counted: 4xx client errors, 429 rate-limited (handled by separate retry logic).

**Provider Selection and Fallback:**
1. Router selects the primary provider based on the requested model and routing strategy (`fixed`, `priority`, `latency`, `cost`).
2. A fallback chain is constructed: `[primary, secondary, tertiary]`.
3. If the primary fails (circuit OPEN or request error), the gateway retries up to 2 times with exponential backoff (1s, 2s).
4. If retries are exhausted, the gateway attempts the next provider in the fallback chain.
5. If no provider in the chain is healthy, return `503 Service Unavailable` with `X-Unavailable-Reason: no_healthy_provider`.

**Health Checks:**
- Every 30 seconds: background worker sends a lightweight health check (`GET /models` or equivalent) to each configured provider.
- Health status is stored in Redis (`health:{provider}`) with a 30-second TTL.
- Circuit breaker state is stored in L1 cache (in-process) for fast access and in Redis for persistence across restarts.
- Health status includes: latency P95, error rate over 5-minute window, consecutive successes/failures, and circuit state.

**Customer Control:**
- Fallback chains are configurable per-organization via the admin dashboard.
- Routing strategies: `fixed` (always use configured provider), `priority` (first healthy in chain), `latency` (lowest recent latency), `cost` (cheapest available).
- Customers can disable fallback entirely (fail fast if primary provider is down).
- Model mapping: when falling back to a different provider, the gateway maps the requested model to the closest equivalent on the fallback provider (e.g., `gpt-4o` → `claude-3-5-sonnet`).

**Retry Logic:**
- Retryable errors: 5xx, timeout, connection refused.
- Non-retryable errors: 4xx (client error).
- Rate-limited (429): retry once after the `Retry-After` header duration, then failover.
- Max 2 retries per provider with exponential backoff (1s, 2s).

## Alternatives Considered

### Alternative 1: No Fallback (Fail Fast)
- **Description:** If the primary provider fails, immediately return an error to the client.
- **Why rejected:** Provider outages are common; failing fast would make the gateway's availability dependent on a single third-party service. This would result in unacceptable downtime for production workloads. The gateway's value proposition includes reliable access to AI models.

### Alternative 2: Client-Side Fallback
- **Description:** Return an error to the client and let the client decide whether to try another provider.
- **Why rejected:** Shifts complexity to every client application. Most clients are not equipped to manage multiple provider integrations, track health, or implement circuit breakers. The gateway exists precisely to abstract this complexity.

### Alternative 3: Random Provider Selection
- **Description:** Randomly select a provider for each request without health awareness.
- **Why rejected:** Random selection would send requests to failing providers, causing unnecessary errors and latency. No circuit breaker means repeated attempts to a downed provider, degrading user experience and potentially hitting rate limits on healthy providers.

### Alternative 4: Always-Parallel Requests
- **Description:** Send the request to all providers simultaneously and return the fastest response.
- **Why rejected:** Multiplies API costs by the number of providers (3x-5x cost increase). Wasteful for the vast majority of requests where the primary provider is healthy. Acceptable only for critical real-time applications where latency matters more than cost.

## Tradeoffs

### What We Gain
- **High availability:** Provider outages are transparent to customers; service continues via fallback providers.
- **Automatic recovery:** Circuit breakers and health checks automatically detect and recover from provider failures without human intervention.
- **Customer control:** Configurable fallback chains and routing strategies let customers optimize for their priorities (cost, latency, capability).
- **Cascading failure prevention:** Circuit breakers stop sending traffic to failing providers, preventing overload and giving them time to recover.
- **Transparent operation:** Fallback events are logged and surfaced in the dashboard; customers can see when and why fallbacks occurred.

### What We Give Up
- **Response quality variance:** Fallback models may have different capabilities than the primary. A request expecting GPT-4o-level reasoning may get Claude 3 Haiku if the chain is cost-optimized.
- **Increased latency on fallback:** Failover adds retry delays (up to 3 seconds) plus the fallback provider's response time. Worst-case latency can be 2x normal.
- **Higher costs on fallback:** Fallback providers may have different pricing; cost-optimized routing may select a more expensive provider during an outage.
- **Complexity:** Circuit breakers, health checks, fallback chains, and model mapping significantly increase the routing logic complexity.

## Consequences
- The `gateway-core::Router` constructs a fallback chain for every request based on routing config and provider health.
- Circuit breaker state transitions (CLOSED → OPEN, OPEN → HALF_OPEN, HALF_OPEN → CLOSED) are logged as events and surfaced in the admin dashboard.
- Health check failures trigger circuit breaker state changes; successful health checks in HALF_OPEN state restore the circuit to CLOSED.
- If all providers in a fallback chain are unhealthy, the gateway returns `503 Service Unavailable` with a descriptive `X-Unavailable-Reason` header.
- Streaming requests are more complex to failover: if the primary fails mid-stream, the connection to the client is already established and the stream cannot be transparently resumed on a fallback provider. For streaming, failover only applies to the initial connection attempt.
- Request cancellation (client disconnect) propagates to the provider: if a client disconnects mid-request, the provider call is aborted regardless of fallback state.
- Provider health is checked every 30 seconds; during the 30-second window between a provider failing and the health check detecting it, up to `failure_threshold` (default 5) requests may fail before the circuit opens.

## Related Decisions
- **ADR-001 (Provider Abstraction):** Fallback depends on the unified `Provider` trait; all providers are interchangeable at the trait level.
- **ADR-002 (Cache Strategy):** Cache lookup happens before provider selection; cache hits bypass the fallback chain entirely.
- **ADR-008 (Ollama Support):** Ollama is typically the last resort in a fallback chain due to its local resource constraints and different latency characteristics.

## Notes
- Model mapping between providers is a configuration table (`model_mappings`) that maps gateway model names to provider-specific model IDs. This table is cached in L1 and refreshed every 60 seconds.
- Fallback events include: primary provider, fallback provider, reason (circuit_open, timeout, 5xx, rate_limited), retry count, and additional latency introduced.
- The default fallback chain is: `[openai, anthropic, gemini]` for general chat; `[ollama]` is never included in the default chain and must be explicitly configured by the customer.
- Future work: Intelligent fallback that considers request complexity (token count, tool usage) when selecting a fallback model.
- Future work: Fallback cost caps — maximum additional cost willing to pay for fallback responses.
