# AI Gateway Observability Specification

> Version: 1.0.0
> Scope: Single-VPS deployment
> Status: Implementation-ready

---

## Table of Contents

1. [Observability Strategy](#1-observability-strategy)
2. [Metrics](#2-metrics)
3. [Log Strategy](#3-log-strategy)
4. [Dashboard Design](#4-dashboard-design)
5. [Alerting Rules](#5-alerting-rules)
6. [Health Check Endpoints](#6-health-check-endpoints)
7. [Cost Visibility](#7-cost-visibility-critical-feature)
8. [Implementation Stack](#8-implementation-stack)
9. [Appendix A: Metric Reference](#appendix-a-metric-reference)
10. [Appendix B: Alert Rule Reference](#appendix-b-alert-rule-reference)
11. [Appendix C: Cost Calculation Formulas](#appendix-c-cost-calculation-formulas)

---

## 1. Observability Strategy

### 1.1 Philosophy

Minimal infrastructure, maximum insight. Every component must justify its operational cost. On a single VPS, resource overhead from observability must stay below 5% of total capacity.

**Principles:**
- **Pull over push** — Prometheus pull model is simpler to operate than push-based alternatives
- **Structured logs** — Machine-parseable from day one; no regex parsing in production
- **Actionable alerts** — Every alert must have a defined response action
- **Cost-aware by design** — Cost tracking is not an afterthought; it is core to the product

### 1.2 Three Pillars (MVP)

| Pillar | Tool Choice | Purpose | Footprint |
|--------|-------------|---------|-----------|
| **Metrics** | Prometheus (embedded client) + optional Prometheus server | Quantitative data, trends, SLI/SLO | ~10MB RAM, minimal CPU |
| **Logs** | Structured JSON to stdout/stderr + file rotation | Event records, debugging, audit trail | Disk only, rotated |
| **Alerts** | Webhook-based evaluator | Anomaly detection, notification | In-process, zero external deps |

> **Why no distributed tracing for MVP?**
> - Single VPS = single process (no cross-service calls)
> - `trace_id` in logs provides sufficient request correlation
> - Jaeger/Tempo would add ~100MB RAM + storage overhead
> - **Revisit when:** gateway splits into multiple services or latency debugging exceeds log correlation capability

### 1.3 Tool Choices with Rationale

#### Metrics Collection: `prometheus` crate (Rust client)
- **Chosen because:** De facto standard, pull-based (resilient to short outages), rich ecosystem, histogram support for latency percentiles
- **Alternative:** `statsd` + Telegraf — rejected because push model loses data during outages, harder to aggregate percentiles
- **Alternative:** CloudWatch/Datadog — rejected due to vendor lock-in and cost; $50+/month at scale

#### Log Output: `tracing` crate with JSON subscriber
- **Chosen because:** Structured by default, async-friendly (no blocking), widely adopted in Rust ecosystem
- **Alternative:** `slog` — rejected because `tracing` has better async/await support and OpenTelemetry compatibility
- **Alternative:** `log` + `env_logger` — rejected because unstructured text logs require parsing

#### Dashboard: Built-in React dashboard (primary) + optional Grafana
- **Chosen because:** Grafana adds ~150MB RAM (Grafana + PostgreSQL); built-in dashboard shows data directly from API with zero extra infrastructure
- **Grafana:** Available as optional add-on for users who already run it; can scrape the `/metrics` endpoint
- **Alternative:** Prometheus + Alertmanager only — rejected because visual dashboards are essential for cost visibility (success metric #8)

#### Alert Delivery: Webhook dispatcher
- **Chosen because:** Zero additional infrastructure; integrates with user's existing Slack/Discord/PagerDuty
- **Alternative:** Prometheus Alertmanager — rejected because it requires separate process and configuration; webhook dispatcher is 200 lines of code

### 1.4 Data Retention Strategy

| Data Type | Retention | Storage | Rationale |
|-----------|-----------|---------|-----------|
| Raw metrics | 15 days | In-memory time series | Single-VPS default; Prometheus compression makes this ~100MB |
| Aggregated metrics | 1 year | Disk (compressed) | Long-term trend analysis |
| Logs | 7 days hot, 30 days compressed | Rotated files | GDPR-friendly, sufficient for debugging |
| Cost records | 7 years | SQLite + optional export | Compliance, tax records |

---

## 2. Metrics

### 2.1 Metric Naming Convention

All metrics follow Prometheus naming: `gateway_<domain>_<name>_<unit>`

- Counter: `_total` suffix
- Gauge: no suffix (or `_current` for clarity)
- Histogram: `_duration_seconds` or `_size_bytes` with `_bucket`, `_sum`, `_count`

### 2.2 System Metrics (Infrastructure)

| # | Metric Name | Type | Labels | Collection | Description |
|---|-------------|------|--------|------------|-------------|
| S1 | `gateway_system_cpu_percent` | Gauge | `mode="user\|system\|iowait"` | `sysinfo` crate polled every 15s | CPU utilization percentage per mode |
| S2 | `gateway_system_memory_used_bytes` | Gauge | `type="used\|cached\|free"` | `sysinfo` crate polled every 15s | Memory utilization in bytes |
| S3 | `gateway_system_memory_percent` | Gauge | — | Derived from S2 | Memory utilization percentage |
| S4 | `gateway_system_disk_used_bytes` | Gauge | `mount="/"` | `fs2` crate polled every 60s | Disk usage in bytes |
| S5 | `gateway_system_disk_free_bytes` | Gauge | `mount="/"` | `fs2` crate polled every 60s | Disk free in bytes |
| S6 | `gateway_system_disk_read_bytes_total` | Counter | `device="sda"` | `sysinfo` disk I/O stats | Total bytes read from disk |
| S7 | `gateway_system_disk_write_bytes_total` | Counter | `device="sda"` | `sysinfo` disk I/O stats | Total bytes written to disk |
| S8 | `gateway_system_network_receive_bytes_total` | Counter | `interface="eth0"` | `sysinfo` network stats | Total network bytes received |
| S9 | `gateway_system_network_transmit_bytes_total` | Counter | `interface="eth0"` | `sysinfo` network stats | Total network bytes transmitted |
| S10 | `gateway_system_load_average` | Gauge | `period="1m\|5m\|15m"` | `sysinfo` load average | System load average |
| S11 | `gateway_container_restarts_total` | Counter | `container="gateway"` | Docker API or systemd | Number of container restarts |

**Collection Method:** System metrics are collected by a background tokio task using the `sysinfo` crate, polled every 15 seconds, and exposed via the `/metrics` endpoint.

**Rationale for 15s interval:** Balances granularity vs. overhead. 5s is too noisy for a VPS; 60s misses latency spikes.

### 2.3 Application Metrics (Custom)

#### 2.3.1 Request Metrics

| # | Metric Name | Type | Labels | Collection | Description |
|---|-------------|------|--------|------------|-------------|
| A1 | `gateway_request_total` | Counter | `method`, `path`, `status`, `provider`, `model`, `org_id` | Middleware after response | Total HTTP requests processed |
| A2 | `gateway_request_duration_seconds` | Histogram | `method`, `path`, `provider`, `model` | Middleware, exponential buckets [0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1, 2.5, 5, 10] | Request latency distribution |
| A3 | `gateway_request_errors_total` | Counter | `error_type`, `provider`, `model` | Error handler middleware | Total errors by classification |
| A4 | `gateway_active_connections` | Gauge | `provider` | Connection pool wrapper | Currently open connections to providers |
| A5 | `gateway_tokens_input_total` | Counter | `provider`, `model`, `org_id` | Token counter on request | Input tokens processed |
| A6 | `gateway_tokens_output_total` | Counter | `provider`, `model`, `org_id` | Token counter on response | Output tokens generated |

**Bucket Configuration Rationale:** Exponential buckets from 5ms to 10s cover the range from fast cache hits to slow provider timeouts. 11 buckets keep cardinality manageable.

#### 2.3.2 Cache Metrics

| # | Metric Name | Type | Labels | Collection | Description |
|---|-------------|------|--------|------------|-------------|
| C1 | `gateway_cache_hit_total` | Counter | `provider`, `model`, `cache_tier` | Cache middleware | Cache hits by tier |
| C2 | `gateway_cache_miss_total` | Counter | `provider`, `model`, `miss_reason` | Cache middleware | Cache misses with reason |
| C3 | `gateway_cache_hit_rate` | Gauge | `provider`, `model` | Derived from C1/C2 | Current hit rate (0.0-1.0) |
| C4 | `gateway_cache_size_bytes` | Gauge | `tier` | Cache store periodic report | Current cache memory usage |
| C5 | `gateway_cache_entries` | Gauge | `tier` | Cache store periodic report | Number of cached entries |
| C6 | `gateway_cache_eviction_total` | Counter | `tier`, `reason` | Cache store | Evicted entries by reason (ttl/lru/size) |
| C7 | `gateway_cache_savings_usd` | Counter | `provider`, `model` | Cache middleware | Estimated cost savings from cache hits |

**`cache_tier` label values:** `memory` (Redis/memory cache), `disk` (filesystem fallback)

**`miss_reason` label values:** `not_found`, `expired`, `bypass_method` (mutating requests), `bypass_header` (no-cache header)

#### 2.3.3 Provider Metrics

| # | Metric Name | Type | Labels | Collection | Description |
|---|-------------|------|--------|------------|-------------|
| P1 | `gateway_provider_request_total` | Counter | `provider`, `model`, `endpoint` | Provider client wrapper | Requests sent to each provider |
| P2 | `gateway_provider_latency_seconds` | Histogram | `provider`, `model` | Provider client wrapper, same buckets as A2 | Provider response latency |
| P3 | `gateway_provider_errors_total` | Counter | `provider`, `error_type` | Provider error classifier | Provider errors by type |
| P4 | `gateway_provider_fallback_total` | Counter | `from_provider`, `to_provider`, `reason` | Fallback handler | Fallback events triggered |
| P5 | `gateway_provider_status` | Gauge | `provider` | Health check task | Provider health status (0=unknown, 1=healthy, 2=degraded, 3=unhealthy) |
| P6 | `gateway_provider_rate_limit_hits_total` | Counter | `provider`, `model` | Rate limit response handler | Rate limit responses from providers |
| P7 | `gateway_provider_retry_total` | Counter | `provider`, `attempt` | Retry handler | Retry attempts by attempt number |

**`error_type` label values:** `timeout`, `connection_error`, `rate_limited`, `auth_error`, `server_error_5xx`, `invalid_response`, `cancelled`

**`status` gauge values:**
- `0` — Unknown (never checked)
- `1` — Healthy (last check successful, latency < threshold)
- `2` — Degraded (last check slow but successful)
- `3` — Unhealthy (last check failed)

#### 2.3.4 Cost Metrics

| # | Metric Name | Type | Labels | Collection | Description |
|---|-------------|------|--------|------------|-------------|
| B1 | `gateway_cost_per_request_usd` | Histogram | `provider`, `model`, `org_id` | Cost calculator on response | Cost distribution per request |
| B2 | `gateway_cost_total_usd` | Counter | `provider`, `model`, `org_id` | Cost accumulator | Cumulative cost |
| B3 | `gateway_cost_without_gateway_usd` | Counter | `provider`, `model` | What-If calculator | Cost if gateway optimizations did not exist |
| B4 | `gateway_cost_savings_usd` | Counter | `org_id`, `source` | Derived from B2-B3 | Savings delivered by gateway |
| B5 | `gateway_quota_used_percent` | Gauge | `org_id`, `quota_type` | Quota tracker | Quota utilization percentage |
| B6 | `gateway_budget_remaining_usd` | Gauge | `org_id` | Budget tracker | Remaining budget in USD |

**`source` label values:** `cache`, `model_downgrade`, `batching`, `rate_optimization`

#### 2.3.5 API Key & Quota Metrics

| # | Metric Name | Type | Labels | Collection | Description |
|---|-------------|------|--------|------------|-------------|
| K1 | `gateway_apikey_requests_total` | Counter | `key_id`, `org_id`, `key_name` | Auth middleware | Requests per API key |
| K2 | `gateway_apikey_cost_usd` | Counter | `key_id`, `org_id` | Cost accumulator | Cost per API key |
| K3 | `gateway_ratelimit_hit_total` | Counter | `org_id`, `key_id`, `limit_type` | Rate limiter middleware | Rate limit enforcement hits |
| K4 | `gateway_org_active` | Gauge | — | Org registry | Number of active organizations |

### 2.4 Business Metrics

| # | Metric Name | Type | Labels | Collection | Description |
|---|-------------|------|--------|------------|-------------|
| BM1 | `gateway_business_active_orgs` | Gauge | `tier` | Database query (daily) | Active organizations by tier |
| BM2 | `gateway_business_requests_per_org` | Counter | `org_id`, `tier` | Request middleware | Requests per organization |
| BM3 | `gateway_business_cost_per_org_usd` | Counter | `org_id`, `tier` | Cost aggregator | Cost per organization |
| BM4 | `gateway_business_savings_per_org_usd` | Counter | `org_id` | Savings calculator | Cost savings delivered per org |
| BM5 | `gateway_business_signups_total` | Counter | `source` | Registration handler | New organization signups |
| BM6 | `gateway_business_revenue_usd` | Counter | `source` (saas/direct) | Billing integration | Revenue from SaaS subscriptions |

---

## 3. Log Strategy

### 3.1 Log Levels

| Level | Use Case | Production Default | Volume |
|-------|----------|-------------------|--------|
| `ERROR` | Request failures, provider errors, auth failures, data corruption | Always on | Low |
| `WARN` | Rate limit hits, quota thresholds, cache misses above expected rate, degraded provider | Always on | Medium |
| `INFO` | Request summaries (one line per request), config changes, startup/shutdown | Always on | High |
| `DEBUG` | Detailed request/response bodies, provider payload inspection | Development only | Very High |
| `TRACE` | Internal flow: middleware chain, cache lookups, connection pool ops | Rarely used; per-module toggle | Extreme |

**Log level configuration:** Controlled via `RUST_LOG` environment variable:
```bash
RUST_LOG=gateway=info,gateway::provider=warn,gateway::cache=debug
```

### 3.2 Structured Log Format (JSON)

Every log line is a single JSON object. No multiline logs except for panic traces.

#### 3.2.1 Request Log (INFO — one per completed request)

```json
{
  "timestamp": "2024-01-15T09:23:47.123456Z",
  "level": "INFO",
  "target": "gateway::middleware::request",
  "fields": {
    "message": "request completed",
    "trace_id": "550e8400-e29b-41d4-a716-446655440000",
    "request_id": "req_abc123",
    "organization_id": "org_def456",
    "api_key_id": "key_ghi789",
    "api_key_name": "production-key",
    "provider": "openai",
    "model": "gpt-4",
    "endpoint": "/v1/chat/completions",
    "method": "POST",
    "status_code": 200,
    "latency_ms": 1234,
    "tokens_in": 150,
    "tokens_out": 75,
    "cost_usd": 0.004875,
    "cache_hit": false,
    "cache_tier": null,
    "user_agent": "MyApp/1.0",
    "remote_addr": "203.0.113.42"
  },
  "spans": [
    {
      "request_id": "req_abc123",
      "organization_id": "org_def456"
    }
  ]
}
```

#### 3.2.2 Error Log (ERROR)

```json
{
  "timestamp": "2024-01-15T09:24:12.789012Z",
  "level": "ERROR",
  "target": "gateway::provider::client",
  "fields": {
    "message": "provider request failed",
    "trace_id": "550e8400-e29b-41d4-a716-446655440001",
    "request_id": "req_abc124",
    "organization_id": "org_def456",
    "api_key_id": "key_ghi789",
    "provider": "anthropic",
    "model": "claude-3-opus",
    "error_type": "timeout",
    "error_message": "request timed out after 30s",
    "attempt": 1,
    "max_retries": 3,
    "latency_ms": 30000,
    "tokens_in": 200,
    "tokens_out": 0,
    "cost_usd": 0.0
  },
  "spans": [
    {
      "request_id": "req_abc124",
      "provider": "anthropic"
    }
  ]
}
```

#### 3.2.3 Cache Hit Log (INFO)

```json
{
  "timestamp": "2024-01-15T09:25:01.456789Z",
  "level": "INFO",
  "target": "gateway::cache::middleware",
  "fields": {
    "message": "cache hit served",
    "trace_id": "550e8400-e29b-41d4-a716-446655440002",
    "request_id": "req_abc125",
    "organization_id": "org_def456",
    "provider": "openai",
    "model": "gpt-3.5-turbo",
    "cache_tier": "memory",
    "latency_ms": 5,
    "cost_usd": 0.0,
    "saved_cost_usd": 0.0015
  },
  "spans": [
    {
      "request_id": "req_abc125"
    }
  ]
}
```

#### 3.2.4 Fallback Event Log (WARN)

```json
{
  "timestamp": "2024-01-15T09:26:33.111222Z",
  "level": "WARN",
  "target": "gateway::fallback",
  "fields": {
    "message": "provider fallback triggered",
    "trace_id": "550e8400-e29b-41d4-a716-446655440003",
    "request_id": "req_abc126",
    "organization_id": "org_def456",
    "from_provider": "openai",
    "to_provider": "anthropic",
    "from_model": "gpt-4",
    "to_model": "claude-3-sonnet",
    "reason": "timeout",
    "latency_ms": 450
  },
  "spans": [
    {
      "request_id": "req_abc126",
      "fallback": "openai->anthropic"
    }
  ]
}
```

#### 3.2.5 Cost Alert Log (WARN)

```json
{
  "timestamp": "2024-01-15T09:30:00.000000Z",
  "level": "WARN",
  "target": "gateway::billing::alerts",
  "fields": {
    "message": "organization approaching budget threshold",
    "organization_id": "org_def456",
    "budget_usd": 100.00,
    "spent_usd": 85.00,
    "percent_used": 85.0,
    "threshold_percent": 80.0,
    "alert_type": "budget_threshold"
  }
}
```

### 3.3 Log Storage and Rotation

#### Docker Configuration

```yaml
# docker-compose.yml (logging section)
services:
  gateway:
    logging:
      driver: "json-file"
      options:
        max-size: "100m"
        max-file: "5"
        labels: "service"
        env: "RUST_LOG"
    environment:
      - RUST_LOG=gateway=info
```

#### Retention Policy

| Environment | Retention | Max Size | Rotation |
|-------------|-----------|----------|----------|
| Production | 30 days | 500MB total | 100MB per file, 5 files |
| Staging | 7 days | 100MB total | 20MB per file, 5 files |
| Development | 1 day | 50MB total | 10MB per file, 5 files |

**Implementation:** Docker `json-file` log driver handles rotation. For systemd deployments, configure `journald` with `SystemMaxUse=500M`.

#### Log Sampling (Production)

At high request volumes (>1000 req/s), apply sampling to INFO logs:
- Log 100% of ERROR/WARN logs
- Log 100% of requests with `status_code >= 400`
- Log 10% of successful requests (random sampling)
- Always log cost-related fields even when sampling

This keeps log volume manageable while preserving error visibility and cost tracking.

---

## 4. Dashboard Design

### 4.1 Architecture

The built-in dashboard is a React single-page application served from the gateway's static file handler. It reads data from internal REST APIs that aggregate metrics from the in-memory store.

```
User -> Browser -> /dashboard/ (static HTML/JS)
                       |
                       v
              /api/v1/dashboard/* (JSON APIs)
                       |
                       v
              In-Memory Metrics Store
                       |
            +----------+----------+
            v                     v
    Time-Series Ring Buffer   SQLite (cost data)
```

### 4.2 API Endpoints for Dashboard

| Endpoint | Data | Cache TTL |
|----------|------|-----------|
| `GET /api/v1/dashboard/overview` | Summary cards, aggregated counts | 10s |
| `GET /api/v1/dashboard/usage?from=&to=` | Time series requests/cost | 30s |
| `GET /api/v1/dashboard/providers` | Provider status and latency | 5s |
| `GET /api/v1/dashboard/cache` | Cache hit rates, savings | 30s |
| `GET /api/v1/dashboard/orgs` | Organization usage list | 60s |
| `GET /api/v1/dashboard/keys?org=` | API key usage | 60s |
| `GET /api/v1/dashboard/realtime` | WebSocket or SSE for live data | Real-time |

### 4.3 Page 1: Overview

**Layout:** Top row of summary cards, middle row of trend charts, bottom row of status indicators.

```
+------------------------------------------------------------------+
|  AI Gateway Dashboard                              [Org Selector] |
+------------------------------------------------------------------+
|                                                                    |
|  [ Total Requests: 45.2K  ]  [ Total Cost: $127.50  ]           |
|  [ Cache Hit Rate: 34.2%  ]  [ Active Providers: 3/3  ]         |
|  [ Error Rate: 0.8%       ]  [ Avg Latency: 1.2s    ]          |
|                                                                    |
+------------------------------------------------------------------+
|                                                                    |
|  [ Requests Trend (24h)                        ]  [ Cost by      ]
|  [ Line chart: req/s over 24h, 1h intervals   ]  [ Provider     ]
|  [ Show: total, cache hit, cache miss         ]  [ Pie chart    ]
|                                                   [ of $127.50   ]
|                                                                    |
+------------------------------------------------------------------+
|                                                                    |
|  [ Provider Status      ]  [ Recent Alerts (last 5)             ]
|  [ OpenAI:    Healthy   ]  [ WARN: org_123 at 85% budget       ]
|  [ Anthropic: Healthy   ]  [ ERROR: provider timeout (09:24)    ]
|  [ Azure:     Degraded  ]  [ INFO: new org signup (08:15)       ]
|                                                                    |
+------------------------------------------------------------------+
```

**Data Sources:**
- Total requests: `sum(rate(gateway_request_total[1d]))`
- Total cost: `sum(gateway_cost_total_usd)` with delta from previous period
- Cache hit rate: `sum(rate(gateway_cache_hit_total[1h])) / sum(rate(gateway_cache_lookup_total[1h]))`
- Error rate: `sum(rate(gateway_request_errors_total[1h])) / sum(rate(gateway_request_total[1h]))`
- Provider status: `gateway_provider_status` gauge

**Comparison Logic:** Each summary card shows current period vs. previous period (e.g., today vs. yesterday) with up/down arrow and percentage.

### 4.4 Page 2: Usage Analytics

**Layout:** Time series charts on top, breakdown tables below.

```
+------------------------------------------------------------------+
|  Usage Analytics                                      [Export CSV] |
+------------------------------------------------------------------+
|                                                                    |
|  [ Requests Over Time (7d)                              ]         |
|  [ Line chart, x-axis: time, y-axis: requests            ]         |
|  [ Series: total, by provider (stacked), cache hits      ]         |
|  [ Granularity: 1h (day), 15m (today)                    ]         |
|                                                                    |
+------------------------------------------------------------------+
|                                                                    |
|  [ Cost by Provider (7d)        ]  [ Cost by Model (7d)          ] |
|  [ Horizontal bar chart         ]  [ Horizontal bar chart        ] |
|  [ OpenAI:   $85.20             ]  [ GPT-4:      $78.50          ] |
|  [ Anthropic: $32.30            ]  [ GPT-3.5:    $12.50          ] |
|  [ Azure:     $10.00            ]  [ Claude-3:   $32.30          ] |
|                                                                    |
+------------------------------------------------------------------+
|                                                                    |
|  [ Top API Keys (today)                              ]            |
|  | Rank | Key Name     | Org      | Requests | Cost  | Hit Rate | |
|  | 1    | prod-key-1   | Acme Inc | 12,450   | $45.20| 38%      | |
|  | 2    | staging-key  | Acme Inc | 3,200    | $12.10| 25%      | |
|  | 3    | dev-key      | Beta LLC | 890      | $3.50 | 15%      | |
|                                                                    |
+------------------------------------------------------------------+
|                                                                    |
|  [ Tokens Used (7d)                                     ]         |
|  [ Line chart: input tokens, output tokens over time    ]         |
|                                                                    |
+------------------------------------------------------------------+
```

**Interactivity:**
- Date range picker: Today, Yesterday, Last 7 Days, Last 30 Days, Custom
- Export CSV button: Downloads current view as CSV
- Drill-down: Click a provider bar to filter all charts by that provider
- Pagination: Top API keys table paginated at 25 rows

### 4.5 Page 3: Provider Health

```
+------------------------------------------------------------------+
|  Provider Health                                                   |
+------------------------------------------------------------------+
|                                                                    |
|  [ Provider Status Grid                               ]            |
|  +------------+----------+-----------+--------+---------+          |
|  | Provider   | Status   | Latency   | Errors | Load    |          |
|  +------------+----------+-----------+--------+---------+          |
|  | OpenAI     | Healthy  | 1.1s p95  | 0.2%   | 34 req/s|          |
|  | Anthropic  | Healthy  | 2.3s p95  | 0.5%   | 12 req/s|          |
|  | Azure      | Degraded | 5.8s p95  | 3.1%   | 8 req/s |          |
|  | Groq       | Healthy  | 0.4s p95  | 0.1%   | 5 req/s |          |
|  +------------+----------+-----------+--------+---------+          |
|                                                                    |
+------------------------------------------------------------------+
|                                                                    |
|  [ Latency Heatmap (24h)                              ]            |
|  [ x-axis: time (1h buckets)                          ]            |
|  [ y-axis: latency buckets (0-0.5s, 0.5-1s, ..., >5s)]            |
|  [ color intensity: request count in bucket           ]            |
|                                                                    |
+------------------------------------------------------------------+
|                                                                    |
|  [ Error Rate by Provider (24h)                       ]  [Fallback]
|  [ Line chart: error % per provider over time         ]  [Events  ]
|  [ Threshold line at 10%                             ]  |Count: 3|
|                                                           |Last:   |
|                                                           |5m ago  |
+------------------------------------------------------------------+
```

**Status Calculation:**
```rust
fn provider_status(latency_p95: f64, error_rate: f64, last_check: Instant) -> Status {
    if last_check.elapsed() > Duration::from_secs(120) {
        return Status::Unknown;
    }
    if error_rate > 0.10 || latency_p95 > 10.0 {
        return Status::Unhealthy;
    }
    if error_rate > 0.05 || latency_p95 > 5.0 {
        return Status::Degraded;
    }
    Status::Healthy
}
```

### 4.6 Page 4: Cache Performance

```
+------------------------------------------------------------------+
|  Cache Performance                                                 |
+------------------------------------------------------------------+
|                                                                    |
|  [ Hit Rate Over Time (7d)                            ]            |
|  [ Line chart: hit rate %, 1h granularity             ]            |
|  [ Target line: 40% (configurable)                    ]            |
|  [ Current: 34.2%                                     ]            |
|                                                                    |
+------------------------------------------------------------------+
|                                                                    |
|  [ Cost Savings from Cache (7d)                       ]            |
|  [ Bar chart: savings $ per day                       ]            |
|  [ Total saved: $342.50                               ]            |
|  [ Without gateway cost: $1,245.00                    ]            |
|  [ Effective savings: 27.5%                           ]            |
|                                                                    |
+------------------------------------------------------------------+
|                                                                    |
|  [ Top Cached Responses (today)                       ]            |
|  | Rank | Query Pattern      | Hits | Hit Rate | Saved $        |  |
|  | 1    | "summarize: *"     | 450  | 89%      | $22.50         |  |
|  | 2    | "translate: en-*"  | 320  | 78%      | $16.00         |  |
|  | 3    | "classify: *"      | 180  | 65%      | $9.00          |  |
|                                                                    |
+------------------------------------------------------------------+
|                                                                    |
|  [ Cache Size          ]  [ Evictions (24h)         ]             |
|  | Memory: 45MB / 128MB |  | TTL:     1,230          |             |
|  | Disk:   120MB / 512MB|  | LRU:       450          |             |
|  | Entries: 12,450      |  | Size:       89          |             |
|                                                                    |
+------------------------------------------------------------------+
```

### 4.7 Page 5: Real-Time (Optional)

```
+------------------------------------------------------------------+
|  Real-Time Monitor                                          [Live] |
+------------------------------------------------------------------+
|                                                                    |
|  [ Live Request Stream                               ]            |
|  | Time   | Org      | Provider | Model    | Lat | Status | Cost| |
|  | 09:24:01| Acme Inc | OpenAI   | GPT-4    |1.2s | 200   | $.01| |
|  | 09:24:02| Beta LLC | Anthropic| Claude-3 |2.1s | 200   | $.02| |
|  | 09:24:02| Acme Inc | [CACHE]  | GPT-3.5  | 5ms | 200   | $0  | |
|  | 09:24:03| Gamma Co | OpenAI   | GPT-4    |3.4s | 500   | $0  | |
|  [Auto-scrolls, max 100 visible, color-coded status]              |
|                                                                    |
+------------------------------------------------------------------+
|                                                                    |
|  [ Current Throughput: 45 req/s      ]  [ Active Conns: 12 ]     |
|  [ Avg Latency (last 1m): 1.2s       ]  [ Error Rate: 0.8%]     |
|                                                                    |
+------------------------------------------------------------------+
```

**Implementation:** Server-Sent Events (SSE) endpoint at `/api/v1/dashboard/realtime/stream`. Reconnects automatically. Max 10 concurrent real-time viewers to prevent resource exhaustion.

### 4.8 Grafana Dashboard (Optional)

If Grafana is deployed, a pre-built dashboard JSON is provided at `/dashboards/grafana-gateway.json`. Import via Grafana UI.

**Differences from built-in dashboard:**
- Built-in: Zero-config, works immediately, optimized for AI Gateway semantics
- Grafana: Better for users already running Grafana, more visualization options, easier sharing

---

## 5. Alerting Rules

### 5.1 Alert Engine Architecture

The alerting engine runs as a background task inside the gateway process. It evaluates rules against the in-memory metrics store on a configurable interval (default: 30s).

```
Metrics Store -> Rule Evaluator -> Alert State Manager -> Webhook Dispatcher
                                      |
                              +-------+-------+
                              v               v
                          Firing        Resolved
                          (notify)      (notify)
```

**Alert States:**
- `pending` — Condition met but not for long enough (`for` duration not reached)
- `firing` — Condition met for `for` duration; notification sent
- `resolved` — Condition no longer met; resolution notification sent

**Deduplication:** Alerts are deduplicated by `(rule_id, labels_hash)`. Multiple identical alerts within 5 minutes are suppressed.

### 5.2 Critical Alerts (Immediate Response Required)

| ID | Name | Condition | For | Severity | Notification |
|----|------|-----------|-----|----------|--------------|
| CRIT-01 | Provider Down | `gateway_provider_status == 3` (unhealthy) for any provider | 2m | critical | Webhook (PagerDuty/Slack) |
| CRIT-02 | High Error Rate | `rate(gateway_request_errors_total[5m]) / rate(gateway_request_total[5m]) > 0.10` | 5m | critical | Webhook (PagerDuty/Slack) |
| CRIT-03 | Disk Full | `gateway_system_disk_used_bytes / (gateway_system_disk_used_bytes + gateway_system_disk_free_bytes) > 0.85` | 1m | critical | Webhook (PagerDuty/Slack) |
| CRIT-04 | Memory Full | `gateway_system_memory_percent > 90` | 2m | critical | Webhook (PagerDuty/Slack) |
| CRIT-05 | Database Unavailable | Health check DB connection fails | 1m | critical | Webhook (PagerDuty/Slack) |
| CRIT-06 | Redis Unavailable | Health check Redis connection fails | 1m | critical | Webhook (PagerDuty/Slack) |
| CRIT-07 | All Providers Down | `count(gateway_provider_status == 1) == 0` | 1m | critical | Webhook (PagerDuty/Slack) |

**Response Playbooks:**
- **CRIT-01:** Check provider status page; verify API key validity; check network connectivity
- **CRIT-02:** Check recent error logs; identify failing provider/model; verify request payloads
- **CRIT-03/04:** SSH to VPS; check for log bloat; restart container; expand disk if needed
- **CRIT-05/06:** Check Docker logs for DB/Redis; restart dependent services
- **CRIT-07:** Check all API keys; verify network; ensure at least one provider configured

### 5.3 Warning Alerts (Investigate Soon)

| ID | Name | Condition | For | Severity | Notification |
|----|------|-----------|-----|----------|--------------|
| WARN-01 | High Latency | `histogram_quantile(0.95, gateway_request_duration_seconds) > 5` | 10m | warning | Webhook (Slack) |
| WARN-02 | Cache Hit Rate Drop | `gateway_cache_hit_rate < 0.20` and `rate(gateway_request_total[1h]) > 10` | 15m | warning | Webhook (Slack) |
| WARN-03 | Budget Threshold | `gateway_quota_used_percent > 80` | 0m (immediate) | warning | Webhook (Slack + email) |
| WARN-04 | Rate Limit Spike | `rate(gateway_ratelimit_hit_total[5m]) > 10` | 5m | warning | Webhook (Slack) |
| WARN-05 | Provider Degraded | `gateway_provider_status == 2` (degraded) | 5m | warning | Webhook (Slack) |
| WARN-06 | High Load Average | `gateway_system_load_average{period="5m"} > 4` | 10m | warning | Webhook (Slack) |
| WARN-07 | SSL Certificate Expiry | Certificate expiry < 7 days | 1d | warning | Webhook (Slack) |
| WARN-08 | Error Rate Elevated | `rate(gateway_request_errors_total[5m]) / rate(gateway_request_total[5m]) > 0.05` | 10m | warning | Webhook (Slack) |
| WARN-09 | Token Usage Spike | `rate(gateway_tokens_input_total[1h]) + rate(gateway_tokens_output_total[1h])` > 3x avg | 30m | warning | Webhook (Slack) |
| WARN-10 | Cost Spike | `gateway_cost_total_usd` increase > 3x average for same hour previous 7 days | 1h | warning | Webhook (Slack) |

### 5.4 Info Alerts (Awareness Only)

| ID | Name | Condition | For | Severity | Notification |
|----|------|-----------|-----|----------|--------------|
| INFO-01 | New Organization | New `org_id` seen in first request | 0m | info | Webhook (Slack) |
| INFO-02 | Provider Config Changed | Admin API updates provider configuration | 0m | info | Webhook (Slack) |
| INFO-03 | API Key Revoked | Key revocation API called | 0m | info | Webhook (Slack) |
| INFO-04 | Model Fallback Used | `gateway_provider_fallback_total` increments | 0m | info | Webhook (Slack) |
| INFO-05 | Daily Summary | Daily aggregated stats | 1d | info | Webhook (Slack) |
| INFO-06 | Cost Savings Milestone | `gateway_cost_savings_usd` crosses $100/$500/$1000 threshold | 0m | info | Webhook (Slack) |

### 5.5 Webhook Payload Format

```json
{
  "version": "1.0",
  "alert": {
    "id": "alert_abc123",
    "rule_id": "CRIT-02",
    "rule_name": "High Error Rate",
    "severity": "critical",
    "status": "firing",
    "fired_at": "2024-01-15T09:24:00Z",
    "resolved_at": null,
    "description": "Error rate is 15.2% over the last 5 minutes (threshold: 10%)",
    "runbook_url": "https://docs.gateway.run/runbooks/CRIT-02"
  },
  "labels": {
    "provider": "openai",
    "model": "gpt-4",
    "instance": "gateway-prod-01"
  },
  "values": {
    "current_value": 0.152,
    "threshold": 0.10,
    "unit": "ratio"
  },
  "context": {
    "trace_ids": ["abc123", "def456", "ghi789"],
    "recent_errors": 3,
    "dashboard_url": "https://gateway.run/dashboard/providers"
  }
}
```

### 5.6 Webhook Integrations

#### Slack
```json
{
  "text": ":rotating_light: *CRITICAL: High Error Rate*",
  "blocks": [
    {
      "type": "section",
      "text": {
        "type": "mrkdwn",
        "text": "*Error Rate: 15.2%* (threshold: 10%)\nProvider: openai | Model: gpt-4\nDuration: 5m"
      }
    }
  ]
}
```

#### Discord
Uses Discord webhook format with embeds.

#### PagerDuty
Uses PagerDuty Events API v2 format with `routing_key` and `event_action` (`trigger`/`resolve`).

### 5.7 Alert Configuration

Alerts are configured via `alerts.yaml`:

```yaml
# /etc/gateway/alerts.yaml
webhooks:
  - name: "slack-critical"
    url: "${SLACK_CRITICAL_WEBHOOK_URL}"
    severity_filter: ["critical"]
  - name: "slack-general"
    url: "${SLACK_GENERAL_WEBHOOK_URL}"
    severity_filter: ["warning", "info"]

rules:
  - id: CRIT-02
    enabled: true
    cooldown: "30m"  # Minimum time between re-notifications

  - id: WARN-03
    enabled: true
    threshold_override: 70  # Lower budget threshold

  - id: INFO-05
    enabled: false  # Disable daily summary
```

---

## 6. Health Check Endpoints

### 6.1 GET /health

**Purpose:** Liveness probe. Returns overall system health with component status.

**Response (200 OK when healthy, 503 when degraded/unhealthy):**

```json
{
  "status": "healthy",
  "version": "1.0.0",
  "timestamp": "2024-01-15T09:23:47Z",
  "uptime_seconds": 86400,
  "components": {
    "database": {
      "status": "healthy",
      "response_ms": 2,
      "last_check": "2024-01-15T09:23:45Z"
    },
    "cache": {
      "status": "healthy",
      "response_ms": 1,
      "last_check": "2024-01-15T09:23:45Z",
      "details": {
        "type": "redis",
        "connected": true
      }
    },
    "providers": [
      {
        "name": "openai",
        "status": "healthy",
        "response_ms": 450,
        "last_success": "2024-01-15T09:23:40Z",
        "last_error": null
      },
      {
        "name": "anthropic",
        "status": "healthy",
        "response_ms": 1200,
        "last_success": "2024-01-15T09:23:42Z",
        "last_error": null
      },
      {
        "name": "azure",
        "status": "degraded",
        "response_ms": 5800,
        "last_success": "2024-01-15T09:22:15Z",
        "last_error": "2024-01-15T09:20:10Z: timeout after 30s"
      }
    ]
  }
}
```

**Status Logic:**
- `status: "healthy"` — All critical components healthy, all providers healthy
- `status: "degraded"` — At least one provider degraded but at least one healthy; or a non-critical component slow
- `status: "unhealthy"` — All providers down, or database/cache unavailable, or error rate critical

**HTTP Status Codes:**
- `200` — Healthy or degraded (some providers may be down but gateway functional)
- `503` — Unhealthy (gateway cannot process requests)

### 6.2 GET /health/ready

**Purpose:** Readiness probe for load balancer or orchestrator (Docker Swarm, Kubernetes).

**Response (200 OK when ready, 503 when not):**

```json
{
  "ready": true,
  "timestamp": "2024-01-15T09:23:47Z",
  "checks": {
    "database": true,
    "cache": true,
    "configuration_loaded": true,
    "initial_providers_tested": true
  }
}
```

**Ready Conditions:**
- Database connection established and queryable
- Cache (Redis) connection established
- Configuration file loaded and valid
- At least one provider has been successfully health-checked

**Not Ready Conditions:**
- Database unavailable (migrations running or DB down)
- Cache unavailable
- Configuration invalid
- No providers configured

### 6.3 GET /metrics

**Purpose:** Prometheus-compatible metrics endpoint.

**Response Format:** Plain text, Prometheus exposition format

```
# HELP gateway_request_total Total HTTP requests processed
# TYPE gateway_request_total counter
gateway_request_total{method="POST",path="/v1/chat/completions",status="200",provider="openai",model="gpt-4",org_id="org_123"} 15420

# HELP gateway_request_duration_seconds Request latency distribution
# TYPE gateway_request_duration_seconds histogram
gateway_request_duration_seconds_bucket{method="POST",path="/v1/chat/completions",provider="openai",model="gpt-4",le="0.5"} 2450
gateway_request_duration_seconds_bucket{method="POST",path="/v1/chat/completions",provider="openai",model="gpt-4",le="1.0"} 8760
gateway_request_duration_seconds_bucket{method="POST",path="/v1/chat/completions",provider="openai",model="gpt-4",le="+Inf"} 15420
gateway_request_duration_seconds_sum{method="POST",path="/v1/chat/completions",provider="openai",model="gpt-4"} 18456.7
gateway_request_duration_seconds_count{method="POST",path="/v1/chat/completions",provider="openai",model="gpt-4"} 15420

# HELP gateway_system_cpu_percent CPU utilization percentage
# TYPE gateway_system_cpu_percent gauge
gateway_system_cpu_percent{mode="user"} 23.5
gateway_system_cpu_percent{mode="system"} 5.2
gateway_system_cpu_percent{mode="iowait"} 0.1
```

### 6.4 HEAD /health

Lightweight liveness check. Returns `200 OK` with empty body. Used by simple load balancers.

### 6.5 Health Check Configuration

```yaml
# Health check settings in gateway.yaml
health:
  check_interval_seconds: 30      # How often to check dependencies
  provider_timeout_seconds: 10    # Timeout for provider health checks
  max_provider_latency_ms: 5000   # Threshold for "degraded" status
  startup_grace_period_seconds: 60 # Don't mark unhealthy during startup
```

---

## 7. Cost Visibility (Critical Feature)

### 7.1 Cost Tracking Architecture

Cost tracking is a first-class citizen, not an afterthought. Every request flows through the cost calculator.

```
Request -> Provider Client -> Response
                  |
                  v
         +--------+--------+
         |                 |
         v                 v
    Token Counter    Cost Calculator
    (count in/out)   (lookup rate)
         |                 |
         v                 v
    Metrics Store    Cost Record (SQLite)
         |                 |
         +--------+--------+
                  |
                  v
           Real-time Dashboard
                  |
                  v
           Periodic Aggregation
```

### 7.2 Cost Per Request Calculation

#### 7.2.1 Token Counting

Tokens are counted using the same method as the billing provider:

| Provider | Token Counting Method | Notes |
|----------|----------------------|-------|
| OpenAI | `usage.prompt_tokens` / `usage.completion_tokens` from API response | Fallback: tiktoken estimation |
| Anthropic | `usage.input_tokens` / `usage.output_tokens` from API response | Fallback: approximate word count x 1.3 |
| Azure OpenAI | Same as OpenAI | Includes Azure-specific headers |
| Groq | `usage.prompt_tokens` / `usage.completion_tokens` from API response | Fast inference, same counting |
| Cohere | `usage.prompt_tokens` / `usage.completion_tokens` from API response | Billed per token |

**Fallback Strategy:** If provider response omits token counts:
1. Use tiktoken (for GPT models) or equivalent tokenizer
2. If tokenizer unavailable, estimate: `words * 1.3` for input, `words * 1.5` for output
3. Log a WARN when fallback estimation is used (so it can be addressed)

#### 7.2.2 Rate Lookup

Cost per request is calculated using a pricing table maintained in the database:

```sql
CREATE TABLE provider_pricing (
    provider TEXT NOT NULL,           -- "openai", "anthropic", etc.
    model TEXT NOT NULL,              -- "gpt-4", "claude-3-opus", etc.
    input_token_price REAL NOT NULL,  -- USD per 1M input tokens
    output_token_price REAL NOT NULL, -- USD per 1M output tokens
    effective_date TEXT NOT NULL,     -- ISO 8601 date
    PRIMARY KEY (provider, model, effective_date)
);
```

**Sample Pricing Data (updated periodically):**

| Provider | Model | Input ($/1M) | Output ($/1M) | Updated |
|----------|-------|-------------|--------------|---------|
| openai | gpt-4o | 5.00 | 15.00 | 2024-07-01 |
| openai | gpt-4o-mini | 0.15 | 0.60 | 2024-07-01 |
| openai | gpt-4 | 30.00 | 60.00 | 2024-01-01 |
| anthropic | claude-3-opus | 15.00 | 75.00 | 2024-04-01 |
| anthropic | claude-3-sonnet | 3.00 | 15.00 | 2024-04-01 |
| anthropic | claude-3-haiku | 0.25 | 1.25 | 2024-04-01 |

#### 7.2.3 Cost Calculation Formula

```rust
fn calculate_request_cost(
    provider: &str,
    model: &str,
    tokens_in: u64,
    tokens_out: u64,
) -> f64 {
    let pricing = PRICING_TABLE.get(provider, model);
    
    let input_cost = (tokens_in as f64 / 1_000_000.0) * pricing.input_token_price;
    let output_cost = (tokens_out as f64 / 1_000_000.0) * pricing.output_token_price;
    
    let total = input_cost + output_cost;
    
    // Round to 10 significant decimal places (USD precision)
    (total * 10_000_000_000.0).round() / 10_000_000_000.0
}
```

**Example:**
```
Provider: OpenAI, Model: GPT-4o
Input: 150 tokens, Output: 75 tokens
Input cost: (150 / 1,000,000) * $5.00 = $0.00075
Output cost: (75 / 1,000,000) * $15.00 = $0.001125
Total: $0.001875
```

### 7.3 Cost Aggregation

#### 7.3.1 Real-Time Aggregation

Every request immediately increments:
- In-memory counter: `gateway_cost_total_usd` (Prometheus counter)
- SQLite row: `request_costs` table with request-level granularity

```sql
CREATE TABLE request_costs (
    id INTEGER PRIMARY KEY,
    timestamp TEXT NOT NULL,           -- ISO 8601
    trace_id TEXT NOT NULL,
    organization_id TEXT NOT NULL,
    api_key_id TEXT NOT NULL,
    provider TEXT NOT NULL,
    model TEXT NOT NULL,
    tokens_in INTEGER NOT NULL,
    tokens_out INTEGER NOT NULL,
    cost_usd REAL NOT NULL,
    cache_hit BOOLEAN NOT NULL,
    saved_cost_usd REAL DEFAULT 0.0,   -- Cost avoided due to cache/model optimization
    
    INDEX idx_timestamp (timestamp),
    INDEX idx_organization (organization_id, timestamp),
    INDEX idx_provider (provider, model, timestamp)
);
```

#### 7.3.2 Periodic Aggregation

A background task aggregates costs hourly into summary tables:

```sql
CREATE TABLE cost_hourly (
    hour TEXT NOT NULL,                -- "2024-01-15 09:00:00"
    organization_id TEXT NOT NULL,
    provider TEXT NOT NULL,
    model TEXT NOT NULL,
    requests INTEGER NOT NULL,
    tokens_in_total INTEGER NOT NULL,
    tokens_out_total INTEGER NOT NULL,
    cost_usd REAL NOT NULL,
    saved_cost_usd REAL NOT NULL,
    PRIMARY KEY (hour, organization_id, provider, model)
);
```

**Aggregation Schedule:**
- Every hour: Aggregate previous hour into `cost_hourly`
- Every day: Aggregate into `cost_daily`
- Every week: Archive to compressed format

### 7.4 Cost Comparison (What-If Analysis)

The gateway calculates what the user would have paid without gateway optimizations:

| Optimization | Savings Calculation | Method |
|-------------|-------------------|--------|
| **Cache Hit** | `saved_cost_usd = cost_of_original_request` | Count tokens that would have been sent to provider |
| **Model Downgrade** (fallback to cheaper model) | `saved_cost_usd = expensive_model_cost - actual_model_cost` | Calculate cost at originally requested model's rate |
| **Batching** | `saved_cost_usd = sum(individual_costs) - batched_cost` | Compare unbatched vs. batched token pricing |
| **Rate Optimization** | `saved_cost_usd = highest_rate_cost - chosen_rate_cost` | Compare provider pricing tiers |

**What-If API:**
```http
GET /api/v1/cost/whatif?org_id=org_123&period=7d
```

```json
{
  "organization_id": "org_123",
  "period": "7d",
  "actual_cost_usd": 127.50,
  "without_gateway_cost_usd": 342.00,
  "total_savings_usd": 214.50,
  "savings_breakdown": {
    "cache": 156.00,
    "model_downgrade": 45.50,
    "batching": 8.00,
    "rate_optimization": 5.00
  },
  "savings_percentage": 62.7
}
```

### 7.5 Budget Alerts

Organizations can set monthly budgets:

```sql
CREATE TABLE org_budgets (
    organization_id TEXT PRIMARY KEY,
    monthly_budget_usd REAL NOT NULL,
    alert_threshold_1_percent INTEGER DEFAULT 80,
    alert_threshold_2_percent INTEGER DEFAULT 95,
    alert_threshold_3_percent INTEGER DEFAULT 100,
    webhook_url TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
```

**Alert Levels:**
| Level | Threshold | Action |
|-------|-----------|--------|
| 1 | 80% of budget | Warning notification to configured webhook |
| 2 | 95% of budget | Urgent notification; consider rate limiting |
| 3 | 100% of budget | Critical; optional hard stop (configurable) |

### 7.6 Cost Export

**CSV Export:**
```http
GET /api/v1/cost/export?org_id=org_123&from=2024-01-01&to=2024-01-31&format=csv
```

Response: `cost-export-org_123-20240101-20240131.csv`
```csv
timestamp,provider,model,requests,tokens_in,tokens_out,cost_usd,cache_hit,saved_cost_usd
2024-01-15T09:00:00Z,openai,gpt-4,120,15000,7500,0.900,false,0.000
2024-01-15T09:00:00Z,openai,gpt-4,45,0,0,0.000,true,0.270
```

**JSON Export:**
```http
GET /api/v1/cost/export?org_id=org_123&from=2024-01-01&to=2024-01-31&format=json
```

### 7.7 Cost Visibility Dashboard Integration

The Overview dashboard prominently displays:

```
+----------------------------------------------------------+
|  COST VISIBILITY                                         |
|                                                          |
|  This Month: $127.50    (vs $89.20 last month, +43%)    |
|  Saved: $214.50 (62.7%)                                  |
|  Budget: $500.00 (25.5% used)                            |
|                                                          |
|  [||||||||||||                                    ] 25%  |
|                                                          |
|  Without gateway: $342.00 | You paid: $127.50           |
|  [===========REAL COST=============][===SAVINGS===]      |
+----------------------------------------------------------+
```

---

## 8. Implementation Stack

### 8.1 Recommended Stack Summary

| Concern | Component | Crate / Tool | Footprint |
|---------|-----------|-------------|-----------|
| Metrics collection | Prometheus client | `prometheus` crate | ~2MB RAM |
| Metrics storage | In-memory ring buffer | Custom + `dashmap` | ~50MB RAM |
| Metrics exposition | HTTP endpoint | `axum` + `prometheus` encoder | In-process |
| Dashboard API | REST endpoints | `axum` | In-process |
| Dashboard UI | React SPA | `vite` + `recharts` | Static files |
| Log output | Structured JSON | `tracing` + `tracing-subscriber` | ~1MB RAM |
| Log rotation | Docker/systemd | `json-file` driver / journald | External |
| Alert engine | In-process evaluator | Custom (see below) | ~5MB RAM |
| Alert delivery | Webhook dispatcher | `reqwest` (async) | In-process |
| Cost database | SQLite | `sqlx` + `sqlite` | ~10MB RAM, disk |
| Health checks | Background tasks | `tokio::task` | In-process |

**Total observability overhead target: < 70MB RAM, < 2% CPU**

### 8.2 Crate Configuration

#### Metrics: `prometheus` crate

```toml
[dependencies]
prometheus = { version = "0.13", features = ["process"] }
lazy_static = "1.4"
```

```rust
use prometheus::{CounterVec, HistogramVec, GaugeVec, register_counter_vec, register_histogram_vec, register_gauge_vec, exponential_buckets};

lazy_static! {
    pub static ref REQUEST_TOTAL: CounterVec = register_counter_vec!(
        "gateway_request_total",
        "Total HTTP requests processed",
        &["method", "path", "status", "provider", "model", "org_id"]
    ).unwrap();

    pub static ref REQUEST_DURATION: HistogramVec = register_histogram_vec!(
        "gateway_request_duration_seconds",
        "Request latency distribution",
        &["method", "path", "provider", "model"],
        exponential_buckets(0.005, 2.0, 11).unwrap() // 5ms to ~5s, then +Inf
    ).unwrap();

    pub static ref PROVIDER_STATUS: GaugeVec = register_gauge_vec!(
        "gateway_provider_status",
        "Provider health status (0=unknown,1=healthy,2=degraded,3=unhealthy)",
        &["provider"]
    ).unwrap();
}
```

#### Logs: `tracing` crate with JSON

```toml
[dependencies]
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["json", "env-filter"] }
```

```rust
use tracing::{info, warn, error, instrument};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

pub fn init_logging() {
    tracing_subscriber::registry()
        .with(EnvFilter::from_default_env())
        .with(tracing_subscriber::fmt::layer().json())
        .init();
}

// Usage in request handler
#[instrument(skip(req), fields(trace_id = %Uuid::new_v4(), org_id = %req.org_id))]
async fn handle_request(req: Request) -> Result<Response> {
    info!(provider = %req.provider, model = %req.model, "request started");
    
    let start = Instant::now();
    let result = process_request(req).await;
    let latency_ms = start.elapsed().as_millis();
    
    match &result {
        Ok(resp) => {
            info!(
                status = resp.status.as_u16(),
                latency_ms = latency_ms,
                tokens_in = resp.tokens_in,
                tokens_out = resp.tokens_out,
                cost_usd = resp.cost_usd,
                cache_hit = resp.cache_hit,
                "request completed"
            );
        }
        Err(e) => {
            error!(
                error = %e,
                latency_ms = latency_ms,
                error_type = e.classify(),
                "request failed"
            );
        }
    }
    
    result
}
```

#### Alert Engine: Custom Implementation

```rust
use std::collections::HashMap;
use tokio::time::{interval, Duration};

pub struct AlertEngine {
    rules: Vec<AlertRule>,
    state: DashMap<String, AlertState>,
    webhook_tx: mpsc::Sender<WebhookPayload>,
}

impl AlertEngine {
    pub async fn run(mut self) {
        let mut ticker = interval(Duration::from_secs(30));
        
        loop {
            ticker.tick().await;
            
            for rule in &self.rules {
                if let Some(value) = rule.evaluate().await {
                    let state_key = format!("{}:{:?}", rule.id, rule.labels);
                    let current = self.state.get(&state_key);
                    
                    match (value, current.as_deref()) {
                        (true, None) => {
                            // First time firing - go to pending
                            self.state.insert(state_key.clone(), AlertState::Pending(Instant::now()));
                        }
                        (true, Some(AlertState::Pending(since))) => {
                            if since.elapsed() >= rule.for_duration {
                                // Transition to firing
                                self.state.insert(state_key.clone(), AlertState::Firing);
                                self.fire_alert(rule).await;
                            }
                        }
                        (false, Some(AlertState::Firing)) => {
                            // Transition to resolved
                            self.state.remove(&state_key);
                            self.resolve_alert(rule).await;
                        }
                        (false, _) => {
                            // Not firing, clean up any pending state
                            self.state.remove(&state_key);
                        }
                    }
                }
            }
        }
    }
}
```

### 8.3 Deployment Configuration

#### Docker Compose (Observability Services)

The primary stack runs entirely within the gateway container. Optional Grafana sidecar:

```yaml
# docker-compose.yml - OPTIONAL Grafana add-on
version: "3.8"

services:
  gateway:
    build: .
    ports:
      - "8080:8080"
    environment:
      - RUST_LOG=gateway=info
      - DATABASE_URL=sqlite:/data/gateway.db
      - REDIS_URL=redis://redis:6379
    volumes:
      - gateway-data:/data
    depends_on:
      - redis
    logging:
      driver: "json-file"
      options:
        max-size: "100m"
        max-file: "5"

  redis:
    image: redis:7-alpine
    volumes:
      - redis-data:/data
    # No persistence needed for cache, but helpful for warm restarts

  # OPTIONAL: Prometheus for long-term metrics storage
  prometheus:
    image: prom/prometheus:latest
    volumes:
      - ./prometheus.yml:/etc/prometheus/prometheus.yml
      - prometheus-data:/prometheus
    ports:
      - "9090:9090"
    profiles: ["monitoring"]  # Only started with --profile monitoring

  # OPTIONAL: Grafana for advanced dashboards
  grafana:
    image: grafana/grafana:latest
    volumes:
      - grafana-data:/var/lib/grafana
      - ./grafana-dashboards:/etc/grafana/provisioning/dashboards
    ports:
      - "3000:3000"
    profiles: ["monitoring"]

volumes:
  gateway-data:
  redis-data:
  prometheus-data:
  grafana-data:
```

#### Prometheus Configuration (Optional)

```yaml
# prometheus.yml
scrape_configs:
  - job_name: 'gateway'
    static_configs:
      - targets: ['gateway:8080']
    metrics_path: '/metrics'
    scrape_interval: 15s
```

### 8.4 Implementation Checklist

#### Phase 1: Foundation (Week 1)
- [ ] Install `prometheus`, `tracing` crates
- [ ] Implement `/metrics` endpoint
- [ ] Configure structured JSON logging
- [ ] Implement `GET /health` and `GET /health/ready`
- [ ] Add system metrics collection background task

#### Phase 2: Application Metrics (Week 2)
- [ ] Instrument request middleware (A1-A6)
- [ ] Instrument cache middleware (C1-C7)
- [ ] Instrument provider client (P1-P7)
- [ ] Implement cost calculator and per-request cost tracking (B1-B6)
- [ ] Create SQLite schema for cost persistence

#### Phase 3: Dashboard (Week 3)
- [ ] Build dashboard API endpoints
- [ ] Implement Overview page
- [ ] Implement Usage Analytics page
- [ ] Implement Provider Health page
- [ ] Implement Cache Performance page

#### Phase 4: Alerts (Week 4)
- [ ] Implement alert rule engine
- [ ] Configure critical alerts (CRIT-01 through CRIT-07)
- [ ] Configure warning alerts (WARN-01 through WARN-10)
- [ ] Implement webhook dispatcher (Slack/Discord/PagerDuty)
- [ ] Test alert firing and resolution

#### Phase 5: Polish (Week 5)
- [ ] Implement real-time page (optional)
- [ ] Add Grafana dashboard JSON export
- [ ] Performance test observability overhead (< 5% target)
- [ ] Document runbooks for each alert

---

## Appendix A: Metric Reference

### Complete Metric Registry

| Metric | Type | Labels | Help Text |
|--------|------|--------|-----------|
| `gateway_system_cpu_percent` | Gauge | `mode` | CPU utilization percentage |
| `gateway_system_memory_used_bytes` | Gauge | `type` | Memory usage in bytes |
| `gateway_system_memory_percent` | Gauge | — | Memory utilization percentage |
| `gateway_system_disk_used_bytes` | Gauge | `mount` | Disk usage in bytes |
| `gateway_system_disk_free_bytes` | Gauge | `mount` | Disk free in bytes |
| `gateway_system_disk_read_bytes_total` | Counter | `device` | Total bytes read from disk |
| `gateway_system_disk_write_bytes_total` | Counter | `device` | Total bytes written to disk |
| `gateway_system_network_receive_bytes_total` | Counter | `interface` | Total network bytes received |
| `gateway_system_network_transmit_bytes_total` | Counter | `interface` | Total network bytes transmitted |
| `gateway_system_load_average` | Gauge | `period` | System load average |
| `gateway_request_total` | Counter | `method`, `path`, `status`, `provider`, `model`, `org_id` | Total HTTP requests |
| `gateway_request_duration_seconds` | Histogram | `method`, `path`, `provider`, `model` | Request latency |
| `gateway_request_errors_total` | Counter | `error_type`, `provider`, `model` | Total errors |
| `gateway_active_connections` | Gauge | `provider` | Active provider connections |
| `gateway_tokens_input_total` | Counter | `provider`, `model`, `org_id` | Input tokens |
| `gateway_tokens_output_total` | Counter | `provider`, `model`, `org_id` | Output tokens |
| `gateway_cache_hit_total` | Counter | `provider`, `model`, `cache_tier` | Cache hits |
| `gateway_cache_miss_total` | Counter | `provider`, `model`, `miss_reason` | Cache misses |
| `gateway_cache_hit_rate` | Gauge | `provider`, `model` | Cache hit rate |
| `gateway_cache_size_bytes` | Gauge | `tier` | Cache size |
| `gateway_cache_entries` | Gauge | `tier` | Cache entry count |
| `gateway_cache_eviction_total` | Counter | `tier`, `reason` | Cache evictions |
| `gateway_cache_savings_usd` | Counter | `provider`, `model` | Cache cost savings |
| `gateway_provider_request_total` | Counter | `provider`, `model`, `endpoint` | Provider requests |
| `gateway_provider_latency_seconds` | Histogram | `provider`, `model` | Provider latency |
| `gateway_provider_errors_total` | Counter | `provider`, `error_type` | Provider errors |
| `gateway_provider_fallback_total` | Counter | `from_provider`, `to_provider`, `reason` | Fallback events |
| `gateway_provider_status` | Gauge | `provider` | Provider health status |
| `gateway_provider_rate_limit_hits_total` | Counter | `provider`, `model` | Provider rate limits |
| `gateway_provider_retry_total` | Counter | `provider`, `attempt` | Retry attempts |
| `gateway_cost_per_request_usd` | Histogram | `provider`, `model`, `org_id` | Cost per request |
| `gateway_cost_total_usd` | Counter | `provider`, `model`, `org_id` | Cumulative cost |
| `gateway_cost_without_gateway_usd` | Counter | `provider`, `model` | Cost without optimizations |
| `gateway_cost_savings_usd` | Counter | `org_id`, `source` | Savings delivered |
| `gateway_quota_used_percent` | Gauge | `org_id`, `quota_type` | Quota utilization |
| `gateway_budget_remaining_usd` | Gauge | `org_id` | Remaining budget |
| `gateway_apikey_requests_total` | Counter | `key_id`, `org_id`, `key_name` | Requests per API key |
| `gateway_apikey_cost_usd` | Counter | `key_id`, `org_id` | Cost per API key |
| `gateway_ratelimit_hit_total` | Counter | `org_id`, `key_id`, `limit_type` | Rate limit hits |
| `gateway_org_active` | Gauge | — | Active organizations |
| `gateway_business_active_orgs` | Gauge | `tier` | Active orgs by tier |
| `gateway_business_requests_per_org` | Counter | `org_id`, `tier` | Requests per org |
| `gateway_business_cost_per_org_usd` | Counter | `org_id`, `tier` | Cost per org |
| `gateway_business_savings_per_org_usd` | Counter | `org_id` | Savings per org |
| `gateway_business_signups_total` | Counter | `source` | New signups |
| `gateway_business_revenue_usd` | Counter | `source` | Revenue |

---

## Appendix B: Alert Rule Reference

### Critical Alerts

| ID | Name | PromQL / Condition | For | Action |
|----|------|-------------------|-----|--------|
| CRIT-01 | Provider Down | `gateway_provider_status{provider=~".+"} == 3` | 2m | PagerDuty + Slack #alerts-critical |
| CRIT-02 | High Error Rate | `rate(gateway_request_errors_total[5m]) / rate(gateway_request_total[5m]) > 0.10` | 5m | PagerDuty + Slack #alerts-critical |
| CRIT-03 | Disk Full | `(gateway_system_disk_used_bytes / (gateway_system_disk_used_bytes + gateway_system_disk_free_bytes)) > 0.85` | 1m | PagerDuty + Slack #alerts-critical |
| CRIT-04 | Memory Full | `gateway_system_memory_percent > 90` | 2m | PagerDuty + Slack #alerts-critical |
| CRIT-05 | DB Unavailable | health_check_db == 0 | 1m | PagerDuty + Slack #alerts-critical |
| CRIT-06 | Redis Unavailable | health_check_redis == 0 | 1m | PagerDuty + Slack #alerts-critical |
| CRIT-07 | All Providers Down | `count(gateway_provider_status == 1) == 0` | 1m | PagerDuty + Slack #alerts-critical |

### Warning Alerts

| ID | Name | Condition | For | Action |
|----|------|-----------|-----|--------|
| WARN-01 | High Latency | `histogram_quantile(0.95, gateway_request_duration_seconds) > 5` | 10m | Slack #alerts-warning |
| WARN-02 | Cache Hit Rate Drop | `gateway_cache_hit_rate < 0.20` and request rate > 10/hr | 15m | Slack #alerts-warning |
| WARN-03 | Budget Threshold | `gateway_quota_used_percent > 80` | 0m | Slack #billing + email |
| WARN-04 | Rate Limit Spike | `rate(gateway_ratelimit_hit_total[5m]) > 10` | 5m | Slack #alerts-warning |
| WARN-05 | Provider Degraded | `gateway_provider_status == 2` | 5m | Slack #alerts-warning |
| WARN-06 | High Load | `gateway_system_load_average{period="5m"} > 4` | 10m | Slack #alerts-warning |
| WARN-07 | SSL Expiry | cert_expiry_days < 7 | 1d | Slack #alerts-warning |
| WARN-08 | Error Rate Elevated | error rate > 5% | 10m | Slack #alerts-warning |
| WARN-09 | Token Spike | token usage > 3x average | 30m | Slack #alerts-warning |
| WARN-10 | Cost Spike | cost > 3x same-hour average | 1h | Slack #billing |

### Info Alerts

| ID | Name | Condition | For | Action |
|----|------|-----------|-----|--------|
| INFO-01 | New Org | First request from new org_id | 0m | Slack #activity |
| INFO-02 | Config Change | Provider config updated via API | 0m | Slack #activity |
| INFO-03 | Key Revoked | API key revocation event | 0m | Slack #security |
| INFO-04 | Fallback Used | `gateway_provider_fallback_total` increments | 0m | Slack #activity |
| INFO-05 | Daily Summary | Daily aggregate stats | 1d | Slack #activity |
| INFO-06 | Savings Milestone | Savings cross $100/$500/$1000 | 0m | Slack #activity |

---

## Appendix C: Cost Calculation Formulas

### C.1 Request Cost

```
request_cost_usd = (tokens_in / 1,000,000 * input_price_per_1m)
                 + (tokens_out / 1,000,000 * output_price_per_1m)
```

### C.2 Cache Savings

```
cache_savings_usd = sum(for each cache_hit:
    (cached_request.tokens_in / 1,000,000 * cached_request.input_price)
  + (cached_request.tokens_out / 1,000,000 * cached_request.output_price)
)
```

### C.3 Model Downgrade Savings

```
downgrade_savings_usd = sum(for each fallback:
    (tokens_in / 1,000,000 * (original_model_input_price - actual_model_input_price))
  + (tokens_out / 1,000,000 * (original_model_output_price - actual_model_output_price))
)
```

### C.4 Total Savings

```
total_savings_usd = cache_savings_usd
                  + downgrade_savings_usd
                  + batching_savings_usd
                  + rate_optimization_savings_usd

savings_percentage = total_savings_usd / (total_savings_usd + actual_cost_usd) * 100
```

### C.5 Cost Without Gateway

```
cost_without_gateway_usd = actual_cost_usd + total_savings_usd
```

### C.6 Quota Utilization

```
quota_used_percent = (current_period_cost_usd / monthly_budget_usd) * 100
```

### C.7 Cost Per Token (Efficiency Metric)

```
cost_per_1k_tokens_usd = (total_cost_usd / (total_tokens_in + total_tokens_out)) * 1000
```

---

## Document Information

| Property | Value |
|----------|-------|
| Version | 1.0.0 |
| Status | Implementation-ready |
| Author | Observability Architect |
| Last Updated | 2024-01-15 |
| Scope | Single-VPS AI Gateway MVP |
| Review Cycle | Monthly during active development |

---

*End of Observability Specification*
