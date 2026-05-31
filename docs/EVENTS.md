# Event System Specification — AI Gateway

**Version**: 1.0  
**Status**: Implementation-Ready  
**Scope**: Analytics, cost tracking, webhook delivery, audit logging  
**Constraint**: No external message broker (Kafka/RabbitMQ). PostgreSQL or in-process only.

---

## Table of Contents

1. [Event System Architecture](#1-event-system-architecture)
2. [Event Schema](#2-event-schema)
3. [Event Production](#3-event-production)
4. [Event Consumption](#4-event-consumption)
5. [Webhook System](#5-webhook-system)
6. [Audit Events](#6-audit-events)
7. [Implementation Reference](#7-implementation-reference)

---

## 1. Event System Architecture

### 1.1 Design Decision: Hybrid (PostgreSQL + In-Process)

**Chosen Architecture**: Hybrid — PostgreSQL as durable event log + in-process async channels for immediate distribution.

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                           AI Gateway Process                                │
│                                                                             │
│  ┌──────────┐   ┌──────────────┐   ┌─────────────────────────────────────┐ │
│  │  HTTP    │   │   Event      │   │         tokio::sync::broadcast      │ │
│  │ Handler  │──▶│  Producer    │──▶│         (in-process bus)            │ │
│  └──────────┘   └──────────────┘   └─────────────────────────────────────┘ │
│                                             │                               │
│                              ┌──────────────┼──────────────┐               │
│                              ▼              ▼              ▼               │
│  ┌─────────────────────────────────────────────────────────────────────┐  │
│  │                    PostgreSQL `events` table                         │  │
│  │              (durable, append-only, partitioned)                    │  │
│  └─────────────────────────────────────────────────────────────────────┘  │
│           ▲                    ▲                    ▲                     │
│           │                    │                    │                     │
│  ┌────────┴────────┐  ┌───────┴───────┐  ┌────────┴────────┐            │
│  │  Cost Consumer  │  │ Webhook Poll  │  │  Audit Consumer  │            │
│  │  (in-process)   │  │  (in-process)  │  │  (in-process)   │            │
│  └─────────────────┘  └───────────────┘  └─────────────────┘            │
│                                                                             │
│  ┌─────────────────────────────────────────────────────────────────────┐  │
│  │  Durable Consumers (poll PG): Webhook Dispatcher, Analytics, etc.   │  │
│  └─────────────────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 1.2 Rationale

| Criterion | PostgreSQL Only | In-Process Only | Hybrid (Chosen) |
|---|---|---|---|
| **Durability** | Strong (ACID) | None on crash | Strong |
| **Startup latency** | Consumer polls | Immediate delivery | Immediate for hot path |
| **Backpressure** | Natural (poll interval) | Requires bounded channels | Bounded channels + PG spillover |
| **Operational cost** | Zero extra infra | Zero extra infra | Zero extra infra |
| **Consumer recovery** | Replay from table | Lost on restart | Replay from table |
| **Complexity** | Low | Low | Medium |

**Justification for hybrid**:
- In-process `tokio::sync::broadcast` delivers events immediately to consumers running inside the gateway process (cost aggregator, cache invalidator, audit logger).
- PostgreSQL `events` table provides durability for crash recovery and supports out-of-process consumers (webhook dispatcher) that can be restarted without data loss.
- On crash: events are in PostgreSQL, consumers resume from last processed offset.
- No message broker means zero additional infrastructure on a single VPS.

### 1.3 Component Roles

| Component | Role | Technology |
|---|---|---|
| Event Producer | Creates event envelope, writes to PG, broadcasts on bus | Rust (tokio) |
| Broadcast Bus | In-process event distribution | `tokio::sync::broadcast` (capacity: 10,000) |
| Event Store | Durable append-only log | PostgreSQL table `events` |
| Durable Consumer | Polls PG for missed events | Rust (tokio::time::interval) |

### 1.4 Alternatives Considered

**A. PostgreSQL LISTEN/NOTIFY**
- Rejected: Payload limited to 8KB, no delivery guarantees if consumer is disconnected, adds NOTIFY overhead on every event.

**B. SQLite WAL mode**
- Rejected: Single-writer limitation, poor concurrent write performance under gateway load.

**C. Redis Streams**
- Rejected: Requires running Redis instance, violates "no external message broker" constraint.

---

## 2. Event Schema

### 2.1 Standard Event Envelope

Every event uses this envelope. Field semantics are strict.

```json
{
  "event_id": "550e8400-e29b-41d4-a716-446655440000",
  "event_type": "request.completed",
  "timestamp": "2025-01-01T00:00:00.123456Z",
  "organization_id": "660e8400-e29b-41d4-a716-446655440001",
  "api_key_id": "770e8400-e29b-41d4-a716-446655440002",
  "payload": { ... }
}
```

**Envelope Fields**:

| Field | Type | Required | Description |
|---|---|---|---|
| `event_id` | UUID v4 | Yes | Unique identifier for the event. Used for deduplication. |
| `event_type` | String | Yes | Dot-notation event classifier. Immutable once defined. |
| `timestamp` | RFC 3339 | Yes | UTC time when the event occurred (not when it was written). |
| `organization_id` | UUID | Yes | Tenant scope. All queries are filtered by this. |
| `api_key_id` | UUID | Conditional | API key that triggered the event. Null for non-request events. |
| `payload` | JSONB | Yes | Event-specific data. Schema defined per event_type. |

### 2.2 Event Type Registry

#### request.started

Emitted when a request passes validation and is forwarded to the upstream provider.

```json
{
  "event_id": "...",
  "event_type": "request.started",
  "timestamp": "2025-01-01T00:00:00Z",
  "organization_id": "...",
  "api_key_id": "...",
  "payload": {
    "request_id": "550e8400-e29b-41d4-a716-446655440000",
    "provider": "openai",
    "model": "gpt-4o",
    "route": "chat.completions",
    "input_tokens_estimate": 150,
    "metadata": {
      "source_ip": "192.168.1.1",
      "user_agent": "my-app/1.0"
    }
  }
}
```

| Payload Field | Type | Description |
|---|---|---|
| `request_id` | UUID | Correlates with `request.completed` / `request.failed`. |
| `provider` | String | Target provider slug (e.g., `openai`, `anthropic`). |
| `model` | String | Model identifier passed to the provider. |
| `route` | String | Internal route identifier. |
| `input_tokens_estimate` | Integer | Pre-request token estimate (if available). |
| `metadata` | Object | Arbitrary request metadata (IP, user-agent, headers). |

---

#### request.completed

Emitted when a provider returns a successful response.

```json
{
  "event_id": "...",
  "event_type": "request.completed",
  "timestamp": "2025-01-01T00:00:01.500Z",
  "organization_id": "...",
  "api_key_id": "...",
  "payload": {
    "request_id": "550e8400-e29b-41d4-a716-446655440000",
    "provider": "openai",
    "model": "gpt-4o",
    "route": "chat.completions",
    "latency_ms": 1450,
    "input_tokens": 150,
    "output_tokens": 320,
    "total_tokens": 470,
    "cost_usd": 0.00825,
    "cache_hit": false,
    "fallback_used": false,
    "status_code": 200,
    "metadata": {
      "provider_region": "us-east-1",
      "response_headers": {}
    }
  }
}
```

| Payload Field | Type | Description |
|---|---|---|
| `request_id` | UUID | Correlates with `request.started`. |
| `latency_ms` | Integer | Total time from gateway receive to response send. |
| `input_tokens` | Integer | Actual input tokens consumed. |
| `output_tokens` | Integer | Actual output tokens consumed. |
| `total_tokens` | Integer | Sum of input + output. |
| `cost_usd` | Decimal | Calculated cost in USD (from provider pricing matrix). |
| `cache_hit` | Boolean | Whether response was served from cache. |
| `fallback_used` | Boolean | Whether a fallback provider was used. |
| `status_code` | Integer | HTTP status code from provider. |

---

#### request.failed

Emitted when a request fails definitively (after retries, or non-retryable error).

```json
{
  "event_id": "...",
  "event_type": "request.failed",
  "timestamp": "2025-01-01T00:00:02Z",
  "organization_id": "...",
  "api_key_id": "...",
  "payload": {
    "request_id": "550e8400-e29b-41d4-a716-446655440000",
    "provider": "openai",
    "model": "gpt-4o",
    "route": "chat.completions",
    "latency_ms": 500,
    "error_category": "provider_timeout",
    "error_message": "Request timed out after 30s",
    "retry_count": 3,
    "status_code": 504,
    "fallback_attempted": true,
    "fallback_succeeded": false
  }
}
```

| Payload Field | Type | Description |
|---|---|---|
| `error_category` | Enum | `provider_timeout`, `provider_error`, `rate_limited`, `invalid_request`, `gateway_error`, `quota_exceeded`. |
| `error_message` | String | Human-readable error (do not include secrets). |
| `retry_count` | Integer | Number of retry attempts made. |
| `status_code` | Integer | HTTP status code returned to client. |
| `fallback_attempted` | Boolean | Whether fallback was tried. |
| `fallback_succeeded` | Boolean | Whether fallback succeeded (if attempted). |

---

#### cache.hit

Emitted when a request is served from cache without calling the provider.

```json
{
  "event_id": "...",
  "event_type": "cache.hit",
  "timestamp": "2025-01-01T00:00:00Z",
  "organization_id": "...",
  "api_key_id": "...",
  "payload": {
    "request_id": "...",
    "cache_key_hash": "sha256:abc123...",
    "cache_tier": "memory",
    "entry_age_ms": 45000,
    "saved_latency_ms": 1200,
    "saved_cost_usd": 0.005
  }
}
```

| Payload Field | Type | Description |
|---|---|---|
| `cache_key_hash` | String | SHA-256 hash of the cache key (not the raw key). |
| `cache_tier` | Enum | `memory`, `redis`, `disk` (where the hit occurred). |
| `entry_age_ms` | Integer | Age of the cached entry in milliseconds. |
| `saved_latency_ms` | Integer | Estimated latency saved by cache hit. |
| `saved_cost_usd` | Decimal | Cost saved by not calling provider. |

---

#### cache.miss

Emitted when a cache lookup fails and the request proceeds to the provider.

```json
{
  "event_id": "...",
  "event_type": "cache.miss",
  "timestamp": "2025-01-01T00:00:00Z",
  "organization_id": "...",
  "api_key_id": "...",
  "payload": {
    "request_id": "...",
    "cache_key_hash": "sha256:abc123...",
    "cache_tier_checked": ["memory", "redis"]
  }
}
```

---

#### provider.error

Emitted when a provider returns a non-2xx response that triggers fallback or retry logic.

```json
{
  "event_id": "...",
  "event_type": "provider.error",
  "timestamp": "2025-01-01T00:00:01Z",
  "organization_id": "...",
  "api_key_id": "...",
  "payload": {
    "request_id": "...",
    "provider": "openai",
    "model": "gpt-4o",
    "error_category": "rate_limited",
    "provider_status_code": 429,
    "provider_error_code": "insufficient_quota",
    "provider_error_message": "You exceeded your current quota",
    "will_retry": true,
    "will_fallback": false,
    "latency_ms": 250
  }
}
```

---

#### provider.fallback_activated

Emitted when the primary provider fails and a fallback provider is invoked.

```json
{
  "event_id": "...",
  "event_type": "provider.fallback_activated",
  "timestamp": "2025-01-01T00:00:01.200Z",
  "organization_id": "...",
  "api_key_id": "...",
  "payload": {
    "request_id": "...",
    "primary_provider": "openai",
    "fallback_provider": "anthropic",
    "primary_error": "rate_limited",
    "activation_latency_ms": 1200
  }
}
```

---

#### quota.threshold_reached

Emitted when an organization's usage crosses a configurable threshold (e.g., 80% of limit).

```json
{
  "event_id": "...",
  "event_type": "quota.threshold_reached",
  "timestamp": "2025-01-01T00:00:00Z",
  "organization_id": "...",
  "api_key_id": null,
  "payload": {
    "quota_type": "monthly_spend",
    "threshold_percent": 80,
    "current_value_usd": 800.00,
    "limit_value_usd": 1000.00,
    "period_start": "2025-01-01T00:00:00Z",
    "period_end": "2025-01-31T23:59:59Z"
  }
}
```

---

#### quota.exceeded

Emitted when a request is blocked because the organization exceeded its quota.

```json
{
  "event_id": "...",
  "event_type": "quota.exceeded",
  "timestamp": "2025-01-01T00:00:00Z",
  "organization_id": "...",
  "api_key_id": "...",
  "payload": {
    "quota_type": "monthly_spend",
    "current_value_usd": 1001.50,
    "limit_value_usd": 1000.00,
    "blocked_request_id": "...",
    "blocked_model": "gpt-4o",
    "estimated_cost_blocked": 0.02
  }
}
```

---

#### key.created

Emitted when a new API key is generated.

```json
{
  "event_id": "...",
  "event_type": "key.created",
  "timestamp": "2025-01-01T00:00:00Z",
  "organization_id": "...",
  "api_key_id": "...",
  "payload": {
    "created_by_user_id": "...",
    "key_name": "Production Key",
    "key_prefix": "ag_abc1",
    "scopes": ["chat:write", "models:read"],
    "rate_limit_rpm": 60,
    "expires_at": "2025-12-31T23:59:59Z"
  }
}
```

**Security note**: The full key is never included in events. Only a 6-character prefix is logged.

---

#### key.revoked

Emitted when an API key is revoked or expires.

```json
{
  "event_id": "...",
  "event_type": "key.revoked",
  "timestamp": "2025-01-01T00:00:00Z",
  "organization_id": "...",
  "api_key_id": "...",
  "payload": {
    "revoked_by_user_id": "...",
    "revocation_reason": "manual",
    "key_prefix": "ag_abc1",
    "previous_scopes": ["chat:write", "models:read"]
  }
}
```

---

#### user.login

Emitted on successful authentication.

```json
{
  "event_id": "...",
  "event_type": "user.login",
  "timestamp": "2025-01-01T00:00:00Z",
  "organization_id": "...",
  "api_key_id": null,
  "payload": {
    "user_id": "...",
    "method": "password",
    "ip_address": "192.168.1.1",
    "user_agent": "Mozilla/5.0 ...",
    "session_id": "...",
    "mfa_used": true
  }
}
```

---

#### user.logout

Emitted on session termination.

```json
{
  "event_id": "...",
  "event_type": "user.logout",
  "timestamp": "2025-01-01T00:00:00Z",
  "organization_id": "...",
  "api_key_id": null,
  "payload": {
    "user_id": "...",
    "session_id": "...",
    "reason": "manual",
    "ip_address": "192.168.1.1"
  }
}
```

---

#### config.changed

Emitted when gateway configuration is modified.

```json
{
  "event_id": "...",
  "event_type": "config.changed",
  "timestamp": "2025-01-01T00:00:00Z",
  "organization_id": "...",
  "api_key_id": null,
  "payload": {
    "changed_by_user_id": "...",
    "config_namespace": "provider_routing",
    "config_key": "openai.timeout_ms",
    "old_value": 30000,
    "new_value": 45000,
    "change_type": "update"
  }
}
```

| Payload Field | Type | Description |
|---|---|---|
| `config_namespace` | String | Logical grouping (`provider_routing`, `caching`, `quotas`, `auth`). |
| `change_type` | Enum | `create`, `update`, `delete`. |
| `old_value` | JSON | Previous value (redacted if sensitive). |
| `new_value` | JSON | New value (redacted if sensitive). |

---

#### webhook.delivered

Emitted when a webhook is successfully delivered.

```json
{
  "event_id": "...",
  "event_type": "webhook.delivered",
  "timestamp": "2025-01-01T00:00:00Z",
  "organization_id": "...",
  "api_key_id": null,
  "payload": {
    "webhook_id": "...",
    "endpoint_url": "https://example.com/webhook",
    "event_type_delivered": "request.completed",
    "event_id_delivered": "...",
    "http_status": 200,
    "latency_ms": 45,
    "attempt_number": 1
  }
}
```

---

#### webhook.failed

Emitted when a webhook delivery exhausts all retries.

```json
{
  "event_id": "...",
  "event_type": "webhook.failed",
  "timestamp": "2025-01-01T00:00:00Z",
  "organization_id": "...",
  "api_key_id": null,
  "payload": {
    "webhook_id": "...",
    "endpoint_url": "https://example.com/webhook",
    "event_type_delivered": "request.completed",
    "event_id_delivered": "...",
    "final_error": "Connection refused",
    "total_attempts": 5,
    "last_http_status": null,
    "moved_to_dead_letter": true
  }
}
```

### 2.3 Event Type Summary Table

| Event Type | Category | org_id | api_key_id | Durable | Broadcast |
|---|---|---|---|---|---|
| `request.started` | request lifecycle | required | required | Yes | Yes |
| `request.completed` | request lifecycle | required | required | Yes | Yes |
| `request.failed` | request lifecycle | required | required | Yes | Yes |
| `cache.hit` | cache | required | required | Yes | Yes |
| `cache.miss` | cache | required | required | Yes | No |
| `provider.error` | provider | required | required | Yes | Yes |
| `provider.fallback_activated` | provider | required | required | Yes | Yes |
| `quota.threshold_reached` | billing | required | null | Yes | Yes |
| `quota.exceeded` | billing | required | required | Yes | Yes |
| `key.created` | key management | required | required | Yes | Yes |
| `key.revoked` | key management | required | required | Yes | Yes |
| `user.login` | auth | required | null | Yes | No |
| `user.logout` | auth | required | null | Yes | No |
| `config.changed` | config | required | null | Yes | Yes |
| `webhook.delivered` | webhook | required | null | Yes | No |
| `webhook.failed` | webhook | required | null | Yes | Yes |



---

## 3. Event Production

### 3.1 Production Locations in Request Lifecycle

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                        REQUEST LIFECYCLE EVENT HOOKS                        │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│   [1] POST /v1/chat/completions                                             │
│        │                                                                    │
│        ▼                                                                    │
│   [2] Auth & Rate Limit Check                                               │
│        │                                                                    │
│        ├──► cache.lookup ──► [3a] cache.hit ───────────────────────────►    │
│        │                           (serve cached, emit cache.hit)           │
│        │                                                                    │
│        └──► cache.miss ──► [3b] emit request.started ──────────────────►    │
│                                │                                            │
│                                ▼                                            │
│                        [4] Forward to Provider                              │
│                                │                                            │
│                    ┌───────────┴────────────┐                              │
│                    ▼                        ▼                               │
│              [5a] 2xx response         [5b] Provider error                 │
│                    │                        │                               │
│                    ▼                        ▼                               │
│         emit request.completed       emit provider.error                   │
│         (update cache)               (decide: retry? fallback?)            │
│                                              │                              │
│                    ┌─────────────────────────┘                              │
│                    ▼                                                        │
│         [5c] Fallback activated ──► emit provider.fallback_activated        │
│                    │                                                        │
│                    ▼                                                        │
│         [5d] Retry exhausted ──► emit request.failed                        │
│                    │                                                        │
│                    ▼                                                        │
│         [6] Quota check ──► emit quota.threshold_reached / exceeded         │
│                                                                             │
│   [7] Response returned to client (request latency ends)                    │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 3.2 Event Production Points

| Step | Event(s) Produced | Location in Code | Trigger |
|---|---|---|---|
| 1 | — | API handler | Request received |
| 2 | — | Auth middleware | Authentication success |
| 3a | `cache.hit` | Cache middleware | Hash match in cache store |
| 3b | `request.started` | Request router | Cache miss, forwarding begins |
| 5a | `request.completed` | Provider client | 2xx response body received |
| 5b | `provider.error` | Provider client | Non-2xx from provider |
| 5c | `provider.fallback_activated` | Fallback router | Primary fails, backup invoked |
| 5d | `request.failed` | Error handler | All retries/fallbacks exhausted |
| 6 | `quota.threshold_reached`, `quota.exceeded` | Quota enforcer | Usage crosses threshold |

### 3.3 Synchronous vs Asynchronous Production

**Rule**: Event production is **asynchronous with respect to the HTTP response**. The client response is never blocked waiting for event production.

```
┌──────────────┐     ┌──────────────────┐     ┌──────────────────┐
│   Request    │────▶│  Gateway Handler  │────▶│ Client Response  │
│   Arrives    │     │  (processes req)  │     │   (returned)     │
└──────────────┘     └──────────────────┘     └──────────────────┘
                               │
                               │ (spawn fire-and-forget task)
                               ▼
                    ┌──────────────────────┐
                    │   Event Producer     │
                    │  (async, no await)   │
                    └──────────────────────┘
```

**Implementation**:
```rust
// Pseudocode — event production is fire-and-forget
async fn handle_request(req: Request) -> Response {
    let result = process_request(&req).await;
    
    // Spawn event production as detached task
    // NEVER await this — must not block response
    tokio::spawn(async move {
        let event = build_event(&req, &result);
        if let Err(e) = producer.send(event).await {
            // Log only — event loss is acceptable vs. request failure
            tracing::warn!("event_production_failed: {}", e);
        }
    });
    
    result.into_response()
}
```

### 3.4 Error Handling

**Constraint**: Event production failure must NOT fail the request.

| Failure Mode | Handling | Rationale |
|---|---|---|
| Broadcast bus full (lagging consumer) | Drop event, increment counter `events_dropped_total` | Backpressure on consumers, not producers |
| PostgreSQL insert fails | Retry once (50ms backoff), then drop | Single VPS — PG failure is catastrophic anyway |
| Serialization failure | Log error, drop event | Bug — should never happen |
| Spawn failure (runtime resource) | Log error, continue | System under extreme load |

**Failure Metric**: `gateway_events_dropped_total{reason="bus_full|db_error|spawn"}`

### 3.5 Event Production Order

```
1. Build event envelope (UUID, timestamp, payload)
2. INSERT INTO events table (async, fire-and-forget)
3. Broadcast on tokio::sync::broadcast (async, fire-and-forget)
4. Return from handler (never blocked)
```

---

## 4. Event Consumption

### 4.1 Consumer Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         CONSUMER REGISTRY                                   │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  Consumer Name         │ Events Consumed           │ Mode │ Guarantee      │
│  ──────────────────────┼───────────────────────────┼──────┼─────────────── │
│  Cost Aggregator       │ request.completed,       │ Bus  │ At-least-once  │
│                        │ request.failed,          │      │                │
│                        │ cache.hit                │      │                │
│  ──────────────────────┼───────────────────────────┼──────┼─────────────── │
│  Webhook Dispatcher    │ All (filtered by          │ Poll │ At-least-once  │
│                        │ subscription)             │      │                │
│  ──────────────────────┼───────────────────────────┼──────┼─────────────── │
│  Audit Logger          │ All events                │ Bus  │ At-least-once  │
│  ──────────────────────┼───────────────────────────┼──────┼─────────────── │
│  Analytics Updater     │ request.*, cache.*,       │ Bus  │ At-least-once  │
│                        │ provider.*, quota.*       │      │                │
│  ──────────────────────┼───────────────────────────┼──────┼─────────────── │
│  Cache Invalidator     │ key.revoked,              │ Bus  │ At-least-once  │
│                        │ config.changed            │      │                │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 4.2 Cost Aggregator

| Property | Value |
|---|---|
| **Consumes** | `request.completed`, `request.failed`, `cache.hit` |
| **Mode** | Broadcast bus subscriber (in-process) |
| **Guarantee** | At-least-once (events stored in PG for replay if needed) |
| **Purpose** | Update real-time spend counters, trigger quota checks |

**Processing Logic**:
```
On request.completed:
  org_spend_monthly += event.payload.cost_usd
  total_tokens += event.payload.total_tokens
  UPDATE usage_stats SET spend = spend + $1 WHERE org_id = $2

On request.failed:
  failed_requests_counter += 1
  (no cost added — only successful requests are billed)

On cache.hit:
  cache_hits_counter += 1
  cache_savings_usd += event.payload.saved_cost_usd
```

**Error Handling**:
- DB update failure: Retry 3 times with 100ms exponential backoff
- After retries: Log error, continue consuming (do not crash consumer)
- Missing `request.started` event: OK — cost aggregator only needs completion events

**Retry Policy**: 3 retries, 100ms base, exponential backoff (100ms, 200ms, 400ms).

---

### 4.3 Webhook Dispatcher

| Property | Value |
|---|---|
| **Consumes** | All events (filtered per-subscription) |
| **Mode** | Polls PostgreSQL `events` table + broadcast listener |
| **Guarantee** | At-least-once delivery to subscriber endpoints |
| **Purpose** | Send HTTP POSTs to registered webhook endpoints |

**Dual-Mode Operation**:
1. **Fast path**: Subscribes to broadcast bus for low-latency delivery.
2. **Recovery path**: Polls `events` table every 5 seconds for events missed during restart.

**Polling Query**:
```sql
SELECT * FROM events
WHERE event_id > $last_processed_event_id
  AND event_type = ANY($subscribed_types)
  AND organization_id = $webhook_org_id
ORDER BY event_id
LIMIT 100;
```

**Error Handling**:
- HTTP delivery failure: Retry with exponential backoff (see Section 5)
- Max retries exceeded: Move to dead letter table
- Consumer crash: Resume from `last_processed_event_id` stored in `webhook_consumers` table

---

### 4.4 Audit Logger

| Property | Value |
|---|---|
| **Consumes** | All events |
| **Mode** | Broadcast bus subscriber (in-process) |
| **Guarantee** | At-least-once (PG is source of truth) |
| **Purpose** | Write structured audit records for compliance |

**Processing Logic**:
- Every event is inserted into `audit_log` table with full payload
- `audit_log` is append-only, never updated, never deleted (retention: 1 year)
- Events flagged as `audit` (see Section 6) get an `audit_class` tag

**Error Handling**:
- Insert failure: Retry 3 times, then alert (audit loss is critical)
- Backpressure: If insert lags >1000 events, pause and log warning

**Retry Policy**: 5 retries, 200ms base, exponential backoff (200ms, 400ms, 800ms, 1600ms, 3200ms).

---

### 4.5 Analytics Updater

| Property | Value |
|---|---|
| **Consumes** | `request.started`, `request.completed`, `request.failed`, `cache.hit`, `cache.miss`, `provider.error`, `provider.fallback_activated`, `quota.threshold_reached`, `quota.exceeded` |
| **Mode** | Broadcast bus subscriber (in-process) |
| **Guarantee** | At-least-once |
| **Purpose** | Maintain time-series aggregates for dashboard |

**Processing Logic**:
- Maintains in-memory circular buffers (last 1 hour) for latency percentiles
- Flushes to `analytics_timeseries` table every 60 seconds (batch INSERT)
- Aggregates: request count, latency p50/p95/p99, error rate, token usage, cost

**Batch Flush**:
```sql
INSERT INTO analytics_timeseries (bucket, org_id, requests, latency_ms_p50, ...)
VALUES ...
ON CONFLICT (bucket, org_id) DO UPDATE SET ...
```

**Error Handling**:
- Flush failure: Retry once in 10 seconds, buffer is circular so old data rotates out
- Consumer crash: Dashboard shows stale data until restart (acceptable — analytics, not billing)

**Retry Policy**: 1 retry after 10s (loss acceptable for analytics).

---

### 4.6 Cache Invalidator

| Property | Value |
|---|---|
| **Consumes** | `key.revoked`, `config.changed` (when namespace = `caching`) |
| **Mode** | Broadcast bus subscriber (in-process) |
| **Guarantee** | At-least-once |
| **Purpose** | Invalidate cache entries when keys or cache config change |

**Processing Logic**:
- On `key.revoked`: Invalidate all cache entries associated with the revoked key
- On `config.changed` with `caching` namespace: Flush relevant cache tier if config affects cache keys

**Error Handling**:
- Invalidation failure: Log error, do not retry (cache entries have TTL as safety net)
- Missing invalidation: Data served from cache may be stale until TTL expires (acceptable)

**Retry Policy**: None (TTL provides eventual consistency).

### 4.7 Consumer Offset Management

**For broadcast bus consumers**: No offset tracking — in-process, loss on restart acceptable.

**For polling consumers** (Webhook Dispatcher):

```sql
-- Table: webhook_consumer_offsets
CREATE TABLE webhook_consumer_offsets (
    consumer_id    UUID PRIMARY KEY REFERENCES webhooks(id),
    last_event_id  UUID NOT NULL REFERENCES events(event_id),
    last_event_ts  TIMESTAMPTZ NOT NULL,
    updated_at     TIMESTAMPTZ DEFAULT NOW()
);
```

**Offset commit**: After successful event processing, update `last_event_id` atomically.
**Offset recovery**: On startup, read `last_event_id` and poll from that point.

### 4.8 Consumer Health Metrics

| Metric | Type | Description |
|---|---|---|
| `consumer_events_processed_total` | Counter | Events processed per consumer |
| `consumer_processing_duration_ms` | Histogram | Processing time per event type |
| `consumer_lag_events` | Gauge | Number of unprocessed events (for polling consumers) |
| `consumer_errors_total` | Counter | Processing errors per consumer |
| `consumer_drops_total` | Counter | Events dropped due to backpressure |



---

## 5. Webhook System

### 5.1 Registration

#### 5.1.1 Create Webhook Endpoint

**Endpoint**: `POST /v1/webhooks`

**Request Body**:
```json
{
  "endpoint_url": "https://example.com/webhooks/ai-gateway",
  "event_types": ["request.completed", "request.failed", "quota.exceeded"],
  "secret": null,
  "description": "Production webhook for request events",
  "active": true,
  "retry_policy": {
    "max_retries": 5,
    "initial_interval_ms": 1000,
    "max_interval_ms": 360000,
    "exponential_base": 2.0
  }
}
```

**Validation Rules**:

| Field | Rule | Error if violated |
|---|---|---|
| `endpoint_url` | HTTPS only, max 2048 chars, valid URL format | `invalid_url` |
| `endpoint_url` | Must resolve to public IP (no localhost, 10.x, 192.168.x, 127.x, 169.254.x) | `private_ip_blocked` |
| `event_types` | Non-empty array, values from allowed set | `invalid_event_types` |
| `event_types` | Max 50 event types per webhook | `too_many_event_types` |
| `secret` | If null, auto-generated 32-byte random string | — |
| `retry_policy.max_retries` | Integer, 0-10 | `invalid_retry_count` |
| `retry_policy.initial_interval_ms` | Integer, 1000-30000 | `invalid_interval` |

**URL Validation Implementation**:
```rust
fn validate_webhook_url(url: &str) -> Result<(), WebhookError> {
    let parsed = Url::parse(url)?;
    
    // Require HTTPS
    if parsed.scheme() != "https" {
        return Err(WebhookError::NotHttps);
    }
    
    // Block private IP ranges
    let host = parsed.host_str().ok_or(WebhookError::MissingHost)?;
    if let Ok(ip) = host.parse::<IpAddr>() {
        if is_private_ip(ip) {
            return Err(WebhookError::PrivateIpBlocked);
        }
    }
    // If hostname (not IP), DNS resolution happens at delivery time
    // DNS rebinding protection: resolve once and cache IP for 5 minutes
    
    Ok(())
}
```

**Response** (201 Created):
```json
{
  "id": "880e8400-e29b-41d4-a716-446655440000",
  "endpoint_url": "https://example.com/webhooks/ai-gateway",
  "event_types": ["request.completed", "request.failed", "quota.exceeded"],
  "secret": "whsec_auto_generated_secret_shown_once",
  "secret_prefix": "whsec_abc1",
  "active": true,
  "created_at": "2025-01-01T00:00:00Z",
  "retry_policy": {
    "max_retries": 5,
    "initial_interval_ms": 1000,
    "max_interval_ms": 360000,
    "exponential_base": 2.0
  }
}
```

**Secret Behavior**: If `secret` is null in the request, the gateway generates a cryptographically random 32-byte secret. The **full secret is returned only in the create response**. On subsequent reads, only a 6-character prefix is shown (`secret_prefix`).

#### 5.1.2 List / Update / Delete Webhooks

Standard REST operations on `/v1/webhooks/{id}`. Updating `event_types` takes effect immediately — no restart required.

**Event Type Subscription Allowed Values**:
```
request.started, request.completed, request.failed,
cache.hit, cache.miss,
provider.error, provider.fallback_activated,
quota.threshold_reached, quota.exceeded,
key.created, key.revoked,
user.login, user.logout,
config.changed
```

### 5.2 Delivery

#### 5.2.1 HTTP POST Delivery

**Method**: `POST`
**Content-Type**: `application/json`
**Timeout**: 30 seconds connect, 30 seconds read (60s total max)

**Headers**:

| Header | Value | Description |
|---|---|---|
| `User-Agent` | `AI-Gateway-Webhook/1.0` | Identifies the sender |
| `Content-Type` | `application/json` | Payload format |
| `X-Webhook-ID` | UUID of the webhook registration | For receiver tracking |
| `X-Event-ID` | UUID of the event being delivered | For deduplication |
| `X-Event-Type` | Event type string | Quick filtering without parsing body |
| `X-Delivery-Timestamp` | RFC 3339 timestamp | When delivery was attempted |
| `X-Attempt-Number` | Integer (1-based) | Current retry attempt |
| `X-Signature-256` | `t=<timestamp>,v1=<hmac_hex>` | HMAC-SHA256 signature |

#### 5.2.2 Payload Format

```json
{
  "event_id": "550e8400-e29b-41d4-a716-446655440000",
  "event_type": "request.completed",
  "timestamp": "2025-01-01T00:00:00Z",
  "data": {
    "request_id": "...",
    "provider": "openai",
    "model": "gpt-4o",
    "latency_ms": 1450,
    "input_tokens": 150,
    "output_tokens": 320,
    "total_tokens": 470,
    "cost_usd": 0.00825
  }
}
```

Note: The `data` field contains the event payload (same structure as Section 2). The envelope wraps it with delivery metadata.

#### 5.2.3 Signature Verification

**Algorithm**: HMAC-SHA256
**Secret**: The webhook's configured secret (32 bytes, base64 or hex encoded)

**Signature Construction**:
```
signed_payload = <timestamp_as_string> + "." + <json_payload_as_string>
signature = hex(HMAC_SHA256(secret, signed_payload))
```

**Header Value**:
```
X-Signature-256: t=1704067200,v1=5ff9e2f8b3c4d1a6e7f8g9h0i1j2k3l4m5n6o7p8q9r0s1t2u3v4w5x6y7z8
```

**Verification (receiver-side pseudocode)**:
```python
import hmac, hashlib, time

def verify_signature(payload_body, secret, signature_header):
    # Parse header: t=<timestamp>,v1=<signature>
    parts = dict(p.split('=') for p in signature_header.split(','))
    timestamp = parts['t']
    expected_sig = parts['v1']
    
    # Reconstruct signed payload
    signed_payload = f"{timestamp}.{payload_body}"
    
    # Compute HMAC
    computed_sig = hmac.new(
        secret.encode(),
        signed_payload.encode(),
        hashlib.sha256
    ).hexdigest()
    
    # Constant-time compare
    if not hmac.compare_digest(expected_sig, computed_sig):
        raise ValueError("Invalid signature")
    
    # Timestamp tolerance: 5 minutes
    if abs(int(timestamp) - int(time.time())) > 300:
        raise ValueError("Timestamp too old")
```

### 5.3 Retry Schedule

#### 5.3.1 Exponential Backoff

Default retry policy (configurable per webhook):

| Attempt | Delay (formula) | Delay (seconds) |
|---|---|---|
| 1 | `initial_interval` | 1s |
| 2 | `initial * 2^1` | 2s |
| 3 | `initial * 2^2` | 4s |
| 4 | `initial * 2^3` | 8s |
| 5 | `initial * 2^4` | 16s |
| 6 | `min(initial * 2^5, max_interval)` | 32s |
| ... | ... | ... |
| N | `min(initial * 2^(N-1), max_interval)` | capped at 360s |

**Retryable Conditions**:
- HTTP status: 408, 429, 500, 502, 503, 504
- Network errors: timeout, connection refused, DNS failure, TLS error
- Empty response body

**Non-Retryable Conditions** (immediate dead-letter):
- HTTP status: 400, 401, 403, 404, 410
- Response body containing `{"ai_gateway_disable": true}` (receiver opt-out)
- Redirects (3xx) — follow up to 3 redirects, then dead-letter

#### 5.3.2 Retry Implementation

```rust
// Pseudocode
async fn deliver_with_retry(webhook: &Webhook, event: &Event) -> DeliveryResult {
    let mut attempt = 1;
    let mut delay_ms = webhook.retry_policy.initial_interval_ms;
    
    loop {
        match deliver(webhook, event, attempt).await {
            Ok(response) if response.status().is_success() => {
                record_delivery_success(webhook.id, event.event_id, attempt);
                return DeliveryResult::Success;
            }
            Ok(response) if is_non_retryable(response.status()) => {
                record_delivery_failed(webhook.id, event.event_id, "non_retryable");
                return move_to_dead_letter(webhook.id, event).await;
            }
            Err(e) | Ok(response) => {
                if attempt >= webhook.retry_policy.max_retries {
                    return move_to_dead_letter(webhook.id, event).await;
                }
                sleep(Duration::from_millis(delay_ms)).await;
                delay_ms = min(
                    delay_ms * webhook.retry_policy.exponential_base as u64,
                    webhook.retry_policy.max_interval_ms
                );
                attempt += 1;
            }
        }
    }
}
```

#### 5.3.3 Delivery Status Tracking

```sql
-- Table: webhook_delivery_log
CREATE TABLE webhook_delivery_log (
    id              BIGSERIAL PRIMARY KEY,
    webhook_id      UUID NOT NULL REFERENCES webhooks(id),
    event_id        UUID NOT NULL REFERENCES events(event_id),
    attempt_number  INT NOT NULL DEFAULT 1,
    http_status     INT,
    response_body   TEXT,  -- truncated to 1KB
    latency_ms      INT,
    error_message   TEXT,
    delivered_at    TIMESTAMPTZ,
    created_at      TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX idx_delivery_log_webhook ON webhook_delivery_log(webhook_id, created_at);
CREATE INDEX idx_delivery_log_event ON webhook_delivery_log(event_id);
```

**Retention**: Delivery logs kept for 30 days. Dead letter events kept for 7 days.

### 5.4 Dead Letter Handling

**Table**: `webhook_dead_letter`

```sql
CREATE TABLE webhook_dead_letter (
    id              BIGSERIAL PRIMARY KEY,
    webhook_id      UUID NOT NULL REFERENCES webhooks(id),
    event_id        UUID NOT NULL,
    event_type      VARCHAR(128) NOT NULL,
    payload         JSONB NOT NULL,
    final_error     TEXT,
    total_attempts  INT NOT NULL DEFAULT 0,
    created_at      TIMESTAMPTZ DEFAULT NOW(),
    expires_at      TIMESTAMPTZ DEFAULT NOW() + INTERVAL '7 days'
);
```

**Dead Letter Actions**:
1. **Auto-expire**: Entries deleted after 7 days (via cron or PostgreSQL `pg_cron`)
2. **Manual replay**: `POST /v1/webhooks/{id}/dead-letter/{dead_letter_id}/replay`
3. **Bulk replay**: `POST /v1/webhooks/{id}/dead-letter/replay-all`
4. **Webhook disable**: After 100 consecutive failures, webhook auto-deactivated

### 5.5 Webhook Security

**Additional security measures**:

| Measure | Implementation |
|---|---|
| IP Allowlist | Optional `allowed_ips` CIDR list per webhook |
| Signature verification | HMAC-SHA256 on all payloads |
| Timestamp validation | Reject deliveries >5 minutes old |
| Secret rotation | `POST /v1/webhooks/{id}/rotate-secret` — generates new secret, old valid for 24h |
| TLS enforcement | Only TLS 1.2+, certificate validation enforced |
| Request timeout | 30s connect + 30s read max |
| Body size limit | Max 1MB request body |
| Rate limit | Max 100 deliveries/second per webhook endpoint |

---

## 6. Audit Events

### 6.1 Audit Event Classification

All events are written to `events` table. Events flagged for audit have `is_audit = true` and are additionally written to the `audit_log` table with enhanced retention.

### 6.2 Audit-Logged Events

| Event Type | Audit Class | Retention | Rationale |
|---|---|---|---|
| `user.login` | authentication | 2 years | Track who accessed the system |
| `user.logout` | authentication | 2 years | Track session termination |
| `key.created` | key_management | 7 years | Compliance — API key provenance |
| `key.revoked` | key_management | 7 years | Compliance — key lifecycle |
| `config.changed` | configuration | 3 years | Track all configuration mutations |
| `quota.exceeded` | access_control | 1 year | Deny decisions for disputes |
| `webhook.created` | access_control | 1 year | Track external integration changes |
| `webhook.deleted` | access_control | 1 year | Track external integration changes |
| `organization.created` | access_control | 7 years | Tenant lifecycle |
| `organization.deleted` | access_control | 7 years | Tenant lifecycle |

**Note**: `request.*` events are NOT audit-logged by default (they are analytics/cost events). They can be enabled per-organization for compliance requirements.

### 6.3 Audit Log Schema

```sql
CREATE TABLE audit_log (
    id              BIGSERIAL PRIMARY KEY,
    event_id        UUID NOT NULL,
    event_type      VARCHAR(128) NOT NULL,
    timestamp       TIMESTAMPTZ NOT NULL,
    organization_id UUID NOT NULL,
    api_key_id      UUID,
    audit_class     VARCHAR(64) NOT NULL,
    actor_user_id   UUID,           -- who performed the action (if applicable)
    actor_ip        INET,           -- IP address of actor
    resource_type   VARCHAR(64),    -- what was affected (key, config, user)
    resource_id     UUID,           -- ID of affected resource
    action          VARCHAR(32),    -- created, updated, deleted, accessed
    payload         JSONB NOT NULL,
    integrity_hash  VARCHAR(64),    -- SHA-256 hash for tamper detection
    created_at      TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX idx_audit_log_org ON audit_log(organization_id, timestamp DESC);
CREATE INDEX idx_audit_log_class ON audit_log(audit_class, timestamp DESC);
CREATE INDEX idx_audit_log_event ON audit_log(event_id);
CREATE INDEX idx_audit_log_actor ON audit_log(actor_user_id, timestamp DESC);
```

### 6.4 Integrity Verification

Each audit log entry includes a hash chain for tamper detection:

```
integrity_hash = SHA-256(
    event_id + event_type + timestamp + organization_id + 
    payload_json + previous_integrity_hash
)
```

The `previous_integrity_hash` is the `integrity_hash` of the most recent audit log entry for the same organization. This creates a per-organization hash chain.

**Verification endpoint**: `GET /v1/audit/verify?from=&to=` — returns chain integrity status.

### 6.5 Audit Log Query API

**Endpoint**: `GET /v1/audit/log`

**Query Parameters**:
| Parameter | Type | Description |
|---|---|---|
| `from` | RFC 3339 | Start time (required) |
| `to` | RFC 3339 | End time (required, max 30-day range) |
| `audit_class` | String | Filter by class |
| `event_type` | String | Filter by event type |
| `actor_user_id` | UUID | Filter by who performed the action |
| `limit` | Integer | Max 1000 per page |
| `offset` | Integer | Pagination offset |

**Response**:
```json
{
  "entries": [
    {
      "event_id": "...",
      "event_type": "key.created",
      "timestamp": "2025-01-01T00:00:00Z",
      "audit_class": "key_management",
      "actor_user_id": "...",
      "actor_ip": "192.168.1.1",
      "resource_type": "api_key",
      "resource_id": "...",
      "action": "created",
      "payload": { ... },
      "integrity_hash": "sha256:abc123..."
    }
  ],
  "total": 150,
  "integrity_verified": true
}
```

### 6.6 Retention and Compliance

| Audit Class | Retention Period | Purge Schedule | Access Level |
|---|---|---|---|
| authentication | 2 years | Monthly | org_admin, gateway_admin |
| key_management | 7 years | Yearly | gateway_admin only |
| configuration | 3 years | Quarterly | org_admin, gateway_admin |
| access_control | 1 year | Monthly | org_admin, gateway_admin |

**Purge implementation**: Soft-delete (set `purged_at`) then hard-delete after 30 days. Purge operations are themselves audit-logged as `config.changed` events.



---

## 7. Implementation Reference

### 7.1 Database Schema

#### 7.1.1 Core Events Table

```sql
-- ============================================================
-- EVENT STORE
-- Append-only, partitioned by month for query efficiency.
-- ============================================================

CREATE TABLE events (
    event_id        UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    event_type      VARCHAR(128) NOT NULL,
    timestamp       TIMESTAMPTZ NOT NULL,
    organization_id UUID NOT NULL,
    api_key_id      UUID,
    payload         JSONB NOT NULL,
    is_audit        BOOLEAN NOT NULL DEFAULT false,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    broadcasted     BOOLEAN NOT NULL DEFAULT false,
    
    -- Constraints
    CONSTRAINT valid_event_type CHECK (event_type IN (
        'request.started', 'request.completed', 'request.failed',
        'cache.hit', 'cache.miss',
        'provider.error', 'provider.fallback_activated',
        'quota.threshold_reached', 'quota.exceeded',
        'key.created', 'key.revoked',
        'user.login', 'user.logout',
        'config.changed',
        'webhook.delivered', 'webhook.failed'
    ))
);

-- Indexes for consumer polling patterns
CREATE INDEX idx_events_org_time ON events(organization_id, timestamp DESC);
CREATE INDEX idx_events_type_time ON events(event_type, timestamp DESC);
CREATE INDEX idx_events_created ON events(created_at DESC);
CREATE INDEX idx_events_id_created ON events(event_id, created_at);

-- Partial index for events that need broadcasting (not yet broadcasted)
CREATE INDEX idx_events_unbroadcasted ON events(created_at) 
    WHERE broadcasted = false;

-- GIN index for flexible payload queries (analytics, debugging)
CREATE INDEX idx_events_payload ON events USING GIN (payload jsonb_path_ops);

-- Optional: Partition by month for large deployments
-- CREATE TABLE events_2025_01 PARTITION OF events
--     FOR VALUES FROM ('2025-01-01') TO ('2025-02-01');
```

**Table Rationale**:
- `broadcasted` flag: Used by the startup recovery consumer to find events produced while the gateway was down.
- `is_audit` flag: Allows filtering audit events without scanning payloads.
- No `processed_by` array: Consumers track their own offsets externally (avoids row bloat from UPDATE).
- UUID PK: Enables distributed ID generation without sequence contention.

#### 7.1.2 Webhook Registry Table

```sql
-- ============================================================
-- WEBHOOK REGISTRY
-- Stores configured webhook endpoints and their settings.
-- ============================================================

CREATE TABLE webhooks (
    id                      UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    organization_id         UUID NOT NULL,
    endpoint_url            TEXT NOT NULL,
    event_types             VARCHAR(128)[] NOT NULL,
    secret_hash             VARCHAR(128) NOT NULL,  -- bcrypt hashed
    secret_prefix           VARCHAR(16) NOT NULL,   -- first 6 chars of secret
    description             TEXT,
    active                  BOOLEAN NOT NULL DEFAULT true,
    consecutive_failures    INT NOT NULL DEFAULT 0,
    max_retries             INT NOT NULL DEFAULT 5,
    retry_initial_ms        INT NOT NULL DEFAULT 1000,
    retry_max_ms            INT NOT NULL DEFAULT 360000,
    retry_exponential_base  DECIMAL(3,1) NOT NULL DEFAULT 2.0,
    allowed_ips             INET[],  -- NULL = any IP
    created_at              TIMESTAMPTZ DEFAULT NOW(),
    updated_at              TIMESTAMPTZ DEFAULT NOW(),
    deactivated_at          TIMESTAMPTZ,
    deactivated_reason      VARCHAR(64)
);

CREATE INDEX idx_webhooks_org ON webhooks(organization_id, active);
CREATE INDEX idx_webhooks_types ON webhooks USING GIN (event_types);
```

#### 7.1.3 Webhook Consumer Offsets

```sql
-- ============================================================
-- CONSUMER OFFSETS
-- Tracks per-webhook last processed event for recovery.
-- ============================================================

CREATE TABLE webhook_consumer_offsets (
    webhook_id      UUID PRIMARY KEY REFERENCES webhooks(id),
    last_event_id   UUID NOT NULL,
    last_event_ts   TIMESTAMPTZ NOT NULL,
    updated_at      TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX idx_offsets_event ON webhook_consumer_offsets(last_event_id);
```

#### 7.1.4 Audit Log Table

```sql
-- See Section 6.3 for full schema.
-- Additional: trigger to auto-populate is_audit flag

CREATE OR REPLACE FUNCTION set_audit_flag()
RETURNS TRIGGER AS $$
BEGIN
    NEW.is_audit := NEW.event_type IN (
        'user.login', 'user.logout',
        'key.created', 'key.revoked',
        'config.changed',
        'quota.exceeded',
        'webhook.delivered', 'webhook.failed'
    );
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trg_events_set_audit
    BEFORE INSERT ON events
    FOR EACH ROW
    EXECUTE FUNCTION set_audit_flag();
```

#### 7.1.5 Usage Statistics Table

```sql
-- ============================================================
-- USAGE STATISTICS (updated by Cost Aggregator consumer)
-- ============================================================

CREATE TABLE usage_stats (
    organization_id     UUID PRIMARY KEY,
    period_start        TIMESTAMPTZ NOT NULL,
    period_end          TIMESTAMPTZ NOT NULL,
    
    -- Request counts
    requests_total      BIGINT NOT NULL DEFAULT 0,
    requests_failed     BIGINT NOT NULL DEFAULT 0,
    requests_cached     BIGINT NOT NULL DEFAULT 0,
    
    -- Token usage
    input_tokens_total  BIGINT NOT NULL DEFAULT 0,
    output_tokens_total BIGINT NOT NULL DEFAULT 0,
    
    -- Cost
    spend_usd           DECIMAL(18,8) NOT NULL DEFAULT 0,
    cache_savings_usd   DECIMAL(18,8) NOT NULL DEFAULT 0,
    
    -- Latency
    latency_ms_total    BIGINT NOT NULL DEFAULT 0,
    latency_ms_count    BIGINT NOT NULL DEFAULT 0,
    
    -- Quota tracking
    quota_limit_usd     DECIMAL(18,8),
    quota_threshold_pct INT,
    threshold_alert_sent BOOLEAN DEFAULT false,
    
    updated_at          TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX idx_usage_stats_period ON usage_stats(period_start, period_end);
```

#### 7.1.6 Analytics Timeseries Table

```sql
-- ============================================================
-- ANALYTICS TIMESERIES (updated by Analytics Updater consumer)
-- 5-minute buckets for dashboard data
-- ============================================================

CREATE TABLE analytics_timeseries (
    bucket              TIMESTAMPTZ NOT NULL,
    organization_id     UUID NOT NULL,
    
    requests            BIGINT NOT NULL DEFAULT 0,
    requests_failed     BIGINT NOT NULL DEFAULT 0,
    requests_cached     BIGINT NOT NULL DEFAULT 0,
    
    latency_ms_p50      INT,
    latency_ms_p95      INT,
    latency_ms_p99      INT,
    latency_ms_sum      BIGINT NOT NULL DEFAULT 0,
    latency_ms_count    BIGINT NOT NULL DEFAULT 0,
    
    input_tokens        BIGINT NOT NULL DEFAULT 0,
    output_tokens       BIGINT NOT NULL DEFAULT 0,
    spend_usd           DECIMAL(18,8) NOT NULL DEFAULT 0,
    
    error_count         BIGINT NOT NULL DEFAULT 0,
    fallback_count      BIGINT NOT NULL DEFAULT 0,
    
    PRIMARY KEY (bucket, organization_id)
);

-- Auto-cleanup old analytics (retain 90 days)
CREATE INDEX idx_analytics_timeseries_old ON analytics_timeseries(bucket)
    WHERE bucket < NOW() - INTERVAL '90 days';
```

### 7.2 Consumer Polling Mechanism

#### 7.2.1 Webhook Dispatcher Polling Loop

```rust
/// Polling consumer that reads from PostgreSQL events table.
/// Runs as a background tokio task.
async fn webhook_dispatcher_poll_loop(
    db_pool: PgPool,
    event_bus: broadcast::Sender<Event>,
    mut shutdown: tokio::sync::watch::Receiver<bool>,
) {
    let mut interval = tokio::time::interval(Duration::from_secs(5));
    interval.set_missed_tick_behavior(MissedTickBehavior::Skip);
    
    // Also subscribe to broadcast for low-latency delivery
    let mut bus_receiver = event_bus.subscribe();
    
    loop {
        tokio::select! {
            // Tick: poll database for recovery
            _ = interval.tick() => {
                for webhook in fetch_active_webhooks(&db_pool).await.unwrap_or_default() {
                    if let Err(e) = poll_and_dispatch(&db_pool, &webhook).await {
                        tracing::error!("webhook_poll_error: webhook={}, err={}", webhook.id, e);
                    }
                }
            }
            
            // Fast path: receive from broadcast bus
            Ok(event) = bus_receiver.recv() => {
                dispatch_via_bus(&db_pool, event).await;
            }
            
            // Shutdown signal
            _ = shutdown.changed() => {
                if *shutdown.borrow() {
                    tracing::info!("webhook_dispatcher_shutdown");
                    break;
                }
            }
        }
    }
}

async fn poll_and_dispatch(db_pool: &PgPool, webhook: &Webhook) -> Result<()> {
    // Get last processed position
    let offset: Option<ConsumerOffset> = sqlx::query_as(
        "SELECT * FROM webhook_consumer_offsets WHERE webhook_id = $1"
    )
    .bind(webhook.id)
    .fetch_optional(db_pool)
    .await?;
    
    let last_event_id = offset.map(|o| o.last_event_id);
    
    // Fetch unprocessed events matching subscription
    let events: Vec<Event> = sqlx::query_as(
        r#"
        SELECT * FROM events
        WHERE event_type = ANY($1)
          AND organization_id = $2
          AND ($3::uuid IS NULL OR event_id > $3)
        ORDER BY event_id
        LIMIT 100
        "#
    )
    .bind(&webhook.event_types)
    .bind(webhook.organization_id)
    .bind(last_event_id)
    .fetch_all(db_pool)
    .await?;
    
    for event in events {
        if let Err(e) = deliver_webhook(webhook, &event).await {
            tracing::error!("webhook_delivery_failed: event={}, err={}", event.event_id, e);
        }
        
        // Commit offset after each event (idempotent — safe to reprocess)
        sqlx::query(
            r#"
            INSERT INTO webhook_consumer_offsets (webhook_id, last_event_id, last_event_ts)
            VALUES ($1, $2, $3)
            ON CONFLICT (webhook_id) DO UPDATE
            SET last_event_id = $2, last_event_ts = $3, updated_at = NOW()
            "#
        )
        .bind(webhook.id)
        .bind(event.event_id)
        .bind(event.timestamp)
        .execute(db_pool)
        .await?;
    }
    
    Ok(())
}
```

#### 7.2.2 Cost Aggregator (Broadcast Consumer)

```rust
/// In-process broadcast consumer. No polling — receives events via channel.
async fn cost_aggregator_consumer(
    mut receiver: broadcast::Receiver<Event>,
    db_pool: PgPool,
    mut shutdown: tokio::sync::watch::Receiver<bool>,
) {
    // Batch buffer for write efficiency
    let mut pending_updates: HashMap<Uuid, UsageUpdate> = HashMap::new();
    let mut flush_interval = tokio::time::interval(Duration::from_secs(5));
    
    loop {
        tokio::select! {
            Ok(event) = receiver.recv() => {
                match event.event_type.as_str() {
                    "request.completed" => {
                        if let Ok(payload) = serde_json::from_value::<RequestCompletedPayload>(event.payload.clone()) {
                            let update = pending_updates
                                .entry(event.organization_id)
                                .or_default();
                            update.requests_total += 1;
                            update.input_tokens += payload.input_tokens as i64;
                            update.output_tokens += payload.output_tokens as i64;
                            update.spend_usd += payload.cost_usd;
                            update.latency_ms_total += payload.latency_ms as i64;
                            update.latency_ms_count += 1;
                        }
                    }
                    "request.failed" => {
                        let update = pending_updates
                            .entry(event.organization_id)
                            .or_default();
                        update.requests_failed += 1;
                        update.requests_total += 1;
                    }
                    "cache.hit" => {
                        if let Ok(payload) = serde_json::from_value::<CacheHitPayload>(event.payload.clone()) {
                            let update = pending_updates
                                .entry(event.organization_id)
                                .or_default();
                            update.requests_cached += 1;
                            update.cache_savings_usd += payload.saved_cost_usd;
                        }
                    }
                    _ => {}
                }
            }
            
            // Periodic flush to database
            _ = flush_interval.tick() => {
                if !pending_updates.is_empty() {
                    if let Err(e) = flush_usage_updates(&db_pool, &pending_updates).await {
                        tracing::error!("usage_flush_failed: {}", e);
                        // Retain pending_updates for next flush attempt
                        continue;
                    }
                    pending_updates.clear();
                }
            }
            
            _ = shutdown.changed() => {
                if *shutdown.borrow() {
                    // Final flush before exit
                    let _ = flush_usage_updates(&db_pool, &pending_updates).await;
                    break;
                }
            }
        }
    }
}
```

### 7.3 In-Process Channel Design

#### 7.3.1 Broadcast Channel Configuration

```rust
use tokio::sync::broadcast;

/// Capacity: 10,000 events. Trade-off: memory (~5-10MB) vs. tolerance for slow consumers.
const BROADCAST_CAPACITY: usize = 10_000;

/// Creates the event bus. Held by the main gateway and cloned for consumers.
pub fn create_event_bus() -> broadcast::Sender<Event> {
    broadcast::channel(BROADCAST_CAPACITY).0
}
```

**Why `broadcast` over `mpsc`**:
- `broadcast` allows multiple consumers (cost, audit, analytics, cache).
- `mpsc` is single-consumer — would need a fan-out layer.
- `broadcast::Receiver::recv()` is lagging — slow consumers miss events when buffer is full.

#### 7.3.2 Channel Lag Behavior

```
Producer ──► [broadcast channel: capacity 10,000] ──► Consumer A (fast)
                                │
                                └──► Consumer B (slow — writing to disk)
```

When the channel is full:
- New events **overwrite oldest** (circular buffer).
- Slow consumer's `recv()` returns `Lagged(n)` — indicating `n` events were missed.

**Handling Lag**:
```rust
match receiver.recv().await {
    Ok(event) => process(event).await,
    Err(broadcast::error::RecvError::Lagged(n)) => {
        // Consumer is too slow. Options:
        // 1. Log warning, continue (acceptable for analytics)
        // 2. Switch to DB polling mode for recovery
        // 3. Alert operator
        tracing::warn!("consumer_lagged: missed {} events", n);
        
        // For critical consumers (audit), switch to polling
        if consumer.is_critical() {
            consumer.switch_to_polling_mode().await;
        }
    }
    Err(broadcast::error::RecvError::Closed) => break,
}
```

#### 7.3.3 Channel Wiring

```rust
pub struct EventSystem {
    /// The broadcast sender — cloned for each producer call
    bus: broadcast::Sender<Event>,
    /// Database pool for durable storage
    db: PgPool,
    /// Background task handles
    handles: Vec<JoinHandle<()>>,
}

impl EventSystem {
    pub fn new(db: PgPool) -> Self {
        let (bus, _) = broadcast::channel(BROADCAST_CAPACITY);
        Self { bus, db, handles: vec![] }
    }
    
    pub fn spawn_consumers(&mut self, shutdown: watch::Receiver<bool>) {
        // Cost Aggregator
        let rx = self.bus.subscribe();
        self.handles.push(tokio::spawn(cost_aggregator_consumer(
            rx, self.db.clone(), shutdown.clone()
        )));
        
        // Audit Logger
        let rx = self.bus.subscribe();
        self.handles.push(tokio::spawn(audit_logger_consumer(
            rx, self.db.clone(), shutdown.clone()
        )));
        
        // Analytics Updater
        let rx = self.bus.subscribe();
        self.handles.push(tokio::spawn(analytics_consumer(
            rx, self.db.clone(), shutdown.clone()
        )));
        
        // Cache Invalidator
        let rx = self.bus.subscribe();
        self.handles.push(tokio::spawn(cache_invalidator_consumer(
            rx, self.db.clone(), shutdown.clone()
        )));
        
        // Webhook Dispatcher (dual-mode: bus + poll)
        let rx = self.bus.subscribe();
        self.handles.push(tokio::spawn(webhook_dispatcher_poll_loop(
            self.db.clone(), self.bus.clone(), shutdown.clone()
        )));
    }
    
    pub async fn produce(&self, event: Event) {
        // 1. Write to database (fire-and-forget)
        let db = self.db.clone();
        let event_clone = event.clone();
        tokio::spawn(async move {
            if let Err(e) = insert_event(&db, &event_clone).await {
                tracing::warn!("event_db_insert_failed: {}", e);
            }
        });
        
        // 2. Broadcast (fire-and-forget)
        // send() only fails when all receivers are dropped (no consumers)
        let _ = self.bus.send(event);
    }
}
```

### 7.4 Error Handling and Retries

#### 7.4.1 Event Insert Retry

```rust
/// Insert event with single retry on transient DB errors.
async fn insert_event(db: &PgPool, event: &Event) -> Result<(), sqlx::Error> {
    match try_insert(db, event).await {
        Ok(_) => Ok(()),
        Err(e) if is_transient(&e) => {
            tokio::time::sleep(Duration::from_millis(50)).await;
            try_insert(db, event).await
        }
        Err(e) => {
            tracing::error!("event_insert_failed_permanently: event_id={}, err={}", event.event_id, e);
            Err(e)
        }
    }
}

async fn try_insert(db: &PgPool, event: &Event) -> Result<PgQueryResult, sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO events (event_id, event_type, timestamp, organization_id, 
                           api_key_id, payload, is_audit)
        VALUES ($1, $2, $3, $4, $5, $6, $7)
        "#
    )
    .bind(event.event_id)
    .bind(&event.event_type)
    .bind(event.timestamp)
    .bind(event.organization_id)
    .bind(event.api_key_id)
    .bind(&event.payload)
    .bind(event.is_audit)
    .execute(db)
    .await
}

fn is_transient(err: &sqlx::Error) -> bool {
    matches!(err, sqlx::Error::PoolTimedOut | sqlx::Error::PoolClosed)
}
```

#### 7.4.2 Consumer Error Classification

| Error Type | Consumer Action | Alert? |
|---|---|---|
| Transient DB error | Retry with backoff | No |
| Permanent DB error | Log, skip event, continue | Yes (after 10 in 1 min) |
| Deserialization error | Log, skip event, continue | Yes (indicates schema bug) |
| Timeout in external call | Retry with backoff | No |
| Lag detected (>1000 events) | Switch to poll mode, alert | Yes |

### 7.5 Monitoring Event Lag

#### 7.5.1 Lag Metrics

For polling consumers, lag is the count of unprocessed events:

```sql
-- Lag for a specific webhook consumer
SELECT COUNT(*) as lag_events
FROM events e
WHERE e.event_type = ANY(ARRAY['request.completed', 'request.failed'])
  AND e.organization_id = 'org-uuid-here'
  AND e.event_id > (
      SELECT last_event_id 
      FROM webhook_consumer_offsets 
      WHERE webhook_id = 'webhook-uuid-here'
  );
```

#### 7.5.2 Prometheus Metrics

```rust
use metrics::{counter, gauge, histogram};

// Event production
counter!("gateway_events_produced_total", "event_type" => event_type);
counter!("gateway_events_dropped_total", "reason" => "bus_full");

// Event consumption
counter!("gateway_events_consumed_total", "consumer" => "cost", "event_type" => "request.completed");
histogram!("gateway_event_processing_duration_ms", 45.0, "consumer" => "cost");
gauge!("gateway_consumer_lag_events", 150.0, "consumer" => "webhook_dispatch");

// Webhook delivery
counter!("gateway_webhook_delivered_total", "webhook_id" => "...", "status" => "200");
counter!("gateway_webhook_failed_total", "webhook_id" => "...", "reason" => "timeout");
gauge!("gateway_webhook_consecutive_failures", 5.0, "webhook_id" => "...");

// Audit
counter!("gateway_audit_events_logged_total");
gauge!("gateway_audit_log_size_bytes", 1_000_000.0);
```

#### 7.5.3 Health Check Endpoint

```
GET /v1/system/health

Response (200 OK):
{
  "status": "healthy",
  "components": {
    "event_bus": {
      "status": "ok",
      "receiver_count": 5,
      "capacity": 10000,
      "queued": 23
    },
    "event_db": {
      "status": "ok",
      "last_insert_ms_ago": 45,
      "total_events": 1523401
    },
    "consumers": [
      {"name": "cost_aggregator", "status": "ok", "lag_events": 0},
      {"name": "audit_logger", "status": "ok", "lag_events": 0},
      {"name": "webhook_dispatcher", "status": "ok", "lag_events": 12},
      {"name": "analytics_updater", "status": "ok", "lag_events": 0},
      {"name": "cache_invalidator", "status": "ok", "lag_events": 0}
    ],
    "webhooks": {
      "active": 15,
      "disabled": 2,
      "dead_letter_count": 8
    }
  }
}
```

#### 7.5.4 Event Lag Alerting Rules

| Condition | Severity | Action |
|---|---|---|
| `gateway_consumer_lag_events > 100` | warning | Log, alert PagerDuty if sustained 5 min |
| `gateway_consumer_lag_events > 1000` | critical | Alert immediately, consumer may need restart |
| `gateway_webhook_consecutive_failures > 10` | warning | Auto-disable webhook after 100 |
| `gateway_events_dropped_total[1m] > 0` | warning | Log, investigate consumer speed |
| `gateway_audit_events_logged_total` not incrementing | critical | Audit pipeline stalled |

### 7.6 Startup and Shutdown

#### 7.6.1 Startup Sequence

```
1. Connect to PostgreSQL
2. Create event bus (broadcast channel)
3. Spawn consumers:
   a. Cost Aggregator
   b. Audit Logger  
   c. Analytics Updater
   d. Cache Invalidator
   e. Webhook Dispatcher (enters dual-mode, polls for recovery)
4. Mark gateway as ready
5. Webhook dispatcher polls DB for events missed during downtime
6. Normal operation begins
```

**Recovery on startup**: Webhook dispatcher polls `events` table from `last_event_id` in `webhook_consumer_offsets`. Broadcast consumers start fresh (no recovery — acceptable for in-process analytics).

#### 7.6.2 Graceful Shutdown

```
1. Stop accepting new HTTP requests
2. Wait for in-flight requests to complete (max 30s)
3. Signal shutdown to event system (watch channel)
4. Wait up to 10s for:
   a. Pending events to be inserted into DB
   b. Consumers to flush pending batches
   c. Webhook in-flight deliveries to complete
5. Force-close broadcast channel
6. Exit
```

### 7.7 Configuration

| Parameter | Default | Description |
|---|---|---|
| `events.broadcast_capacity` | 10000 | Broadcast channel buffer size |
| `events.db_insert_timeout_ms` | 5000 | Timeout for event DB insert |
| `events.poll_interval_sec` | 5 | Polling interval for webhook consumer |
| `events.poll_batch_size` | 100 | Max events per poll query |
| `events.cost_flush_interval_sec` | 5 | Cost aggregator batch flush interval |
| `events.analytics_flush_interval_sec` | 60 | Analytics batch flush interval |
| `events.audit_insert_timeout_ms` | 10000 | Timeout for audit log insert |
| `webhook.max_concurrent_deliveries` | 50 | Max parallel webhook HTTP requests |
| `webhook.delivery_timeout_sec` | 30 | HTTP request timeout |
| `webhook.max_redirects` | 3 | Max HTTP redirects to follow |
| `webhook.auto_disable_threshold` | 100 | Consecutive failures before auto-disable |

---

## Appendix A: Decision Log

| Decision | Choice | Rationale |
|---|---|---|
| Message broker | None — hybrid PG + broadcast | Zero extra infrastructure on single VPS |
| Broadcast capacity | 10,000 events | ~5-10MB memory, tolerates 10s consumer stall at 1K events/sec |
| Event ordering | Event ID (UUID v4) lexicographic | Not strictly time-ordered — consumers use `created_at` for time queries |
| Exactly-once delivery | Not guaranteed | At-least-once via idempotent consumers. Exactly-once requires distributed transactions — too complex for single VPS. |
| Event serialization | JSONB in PostgreSQL | Human-readable, schema-flexible, PostgreSQL-native querying |
| Webhook signature | HMAC-SHA256 with timestamp | Industry standard (Stripe-compatible pattern) |
| Audit retention | 1-7 years by class | Meets SOC 2 and GDPR requirements |
| Audit integrity | Hash chain per org | Tamper detection without blockchain complexity |
| Consumer offset storage | PostgreSQL table | Survives consumer restarts, simple to query |
| Failure isolation | Event loss acceptable | Request success is priority. Event delivery is best-effort with retry. |

## Appendix B: Alternatives and Future Evolution

| Current | Future (if scale requires) | Migration Path |
|---|---|---|
| PostgreSQL event log | ClickHouse/TimescaleDB for analytics | Dual-write, then switch consumers |
| In-process broadcast | Redis Pub/Sub | Replace channel with Redis client |
| Single VPS | Multi-instance | Add Redis Streams for cross-instance broadcast |
| PG polling | Outbox pattern with CDC | Add Debezium or `pg_logical` |
| Hash chain audit | Merkle tree or blockchain anchoring | Periodic hash publication |

