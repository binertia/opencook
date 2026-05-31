# ADR-006: Observability

## Status
Accepted

## Context
Operating an AI Gateway requires understanding request flows, provider health, cost accumulation, and system performance. The target deployment is a single VPS managed by a small team (<5 engineers, no dedicated SRE or DevOps). Observability tools must be self-contained, require no external SaaS dependencies, and provide actionable insights without expert configuration.

Key forces:
- No budget or expertise for external observability SaaS (Datadog, Honeycomb, Grafana Cloud)
- Single-node deployment simplifies observability (no distributed tracing needed)
- Cost visibility is a core product feature (customers need to see their LLM spend)
- Alerting must work without PagerDuty or similar services (email/webhooks suffice)
- The system must be debuggable at 2 AM by a solo developer with standard Unix tools
- Rust has excellent structured logging support via the `tracing` ecosystem

## Decision
We will build observability into the gateway itself rather than depending on external tools. The `gateway-observability` crate provides structured logging, metrics collection, and dashboard data endpoints.

**Structured Logging:**
- `tracing` + `tracing-subscriber` for structured JSON logs in production, pretty logs in development
- Log correlation via `request_id` propagated through the entire request lifecycle
- Three log categories: request logs (every HTTP request), error logs (failures with context), audit logs (admin actions, config changes)
- All logs go to stdout; Docker captures them via `docker logs`

**Metrics:**
- Request-level: latency histograms (P50, P95, P99), throughput counters, error rates by status code
- Provider-level: latency by provider, error rate by provider, cost by provider, request count by provider
- Cache-level: hit rate (L1, L2 exact, L2 semantic), miss rate, eviction count
- Tenant-level: usage by organization, cost by organization, quota utilization
- Metrics are exposed via a Prometheus-compatible `/metrics` endpoint and stored in Redis for dashboard retrieval

**Dashboard Data:**
- The admin dashboard queries `/api/admin/organizations/:id/usage` for usage analytics
- Pre-aggregated metrics refreshed every 5 minutes by background workers
- Real-time request stream via WebSocket for the "Request Inspector" feature

**Why no distributed tracing for MVP:**
Distributed tracing (OpenTelemetry, Jaeger, Zipkin) solves the problem of understanding requests across multiple services. The AI Gateway is a single binary; a request flows through internal function calls, not network boundaries. A `request_id` in logs provides sufficient correlation for the single-node deployment. OpenTelemetry-compatible trace context is propagated in headers for future compatibility, but no spans are collected or exported.

**Alerting:**
- Budget threshold alerts (80%, 90%, 100% of monthly budget) emitted as dashboard notifications + optional webhook
- Provider health change alerts (circuit breaker OPEN -> HALF_OPEN, etc.)
- System resource alerts (Redis memory >80%, PostgreSQL connections >80%)
- Alerts are surfaced in the admin dashboard; webhooks can be configured per-organization

## Alternatives Considered

### Alternative 1: External Observability SaaS (Datadog / Honeycomb / Grafana Cloud)
- **Description:** Send logs, metrics, and traces to a hosted observability platform.
- **Why rejected:** Adds operational dependency and ongoing cost (Datadog charges per host + per GB of logs). Requires network egress from the VPS to the SaaS provider. Violates the "self-contained deployment" principle. Many target customers (self-hosted deployments) may not have accounts with these services. A solo developer managing a $50/month VPS cannot justify $200+/month for observability tooling.

### Alternative 2: Self-Hosted Grafana + Prometheus + Loki Stack
- **Description:** Deploy Grafana, Prometheus, and Loki as additional Docker containers alongside the gateway.
- **Why rejected:** Adds 3+ additional containers to manage, configure, and upgrade. Increases the VPS memory footprint by ~1GB. More operational complexity than the value it provides for a single-node system. The built-in dashboard provides sufficient visualization without the full Grafana stack.

### Alternative 3: Distributed Tracing (OpenTelemetry + Jaeger)
- **Description:** Instrument every function call with OpenTelemetry spans and export to Jaeger.
- **Why rejected:** The gateway is a single binary; there are no cross-service calls to trace. Internal function call chains are visible via structured logs with `request_id`. Jaeger would add another container to deploy and maintain. The latency overhead of span collection is non-trivial. OpenTelemetry trace context propagation is implemented for future compatibility, but span collection is deferred.

### Alternative 4: Plain Text Logging Only
- **Description:** Use simple `println!` or `log` crate with unstructured text output.
- **Why rejected:** Unstructured logs cannot be efficiently queried, aggregated, or used for dashboard metrics. The `tracing` crate provides structured JSON output with minimal overhead and is the Rust standard. Parsing unstructured logs with `grep`/`awk` at 2 AM is error-prone and slow.

## Tradeoffs

### What We Gain
- **Zero external dependencies:** Observability works in air-gapped deployments with no internet access.
- **Cost visibility as first-class feature:** Per-request cost tracking is built into the core pipeline, not bolted on via log analysis.
- **Operational simplicity:** `docker logs` for tailing, `curl /metrics` for scraping, dashboard for visualization. One person can understand and operate it all.
- **Request correlation:** `request_id` propagated from edge to response enables end-to-end request tracing via logs.
- **Future-proof:** OpenTelemetry-compatible trace context headers mean distributed tracing can be added later without client changes.

### What We Give Up
- **Advanced analytics:** No complex log querying (no LogQL, no SQL-on-logs). Log analysis is limited to `grep`, `jq`, and basic dashboard filters.
- **Distributed trace visualization:** No flame graphs or service dependency maps (irrelevant for single-node but limiting if we later scale).
- **Long-term metric retention:** Metrics stored in Redis have limited retention (configurable, default 30 days). No time-series database for historical analysis.
- **Sophisticated alerting:** No PagerDuty integration, no alert routing, no on-call scheduling. Just dashboard notifications and simple webhooks.

## Consequences
- The `gateway-observability` crate is a self-contained library with no external service dependencies.
- Request log records include: `request_id`, `timestamp`, `method`, `path`, `status_code`, `latency_ms`, `org_id`, `key_id`, `provider`, `model`, `tokens_in`, `tokens_out`, `cost`, `cached`, `error`.
- Metrics are aggregated in-process and flushed to Redis every 60 seconds. Dashboard queries read from Redis for near-real-time data.
- The `/metrics` endpoint exposes Prometheus-formatted counters and histograms for external scraping (if a customer later adds Prometheus).
- Background workers refresh materialized views every 5 minutes for dashboard analytics.
- Cost per request is computed as: `(tokens_in * input_price + tokens_out * output_price) / 1000` and recorded in the `usage_records` table (append-only, no updates).
- Alert thresholds are evaluated every 60 seconds by a background worker; triggered alerts are stored in PostgreSQL and surfaced via dashboard notifications.
- Log archival: logs older than 7 days are compressed; older than 30 days are deleted (configurable). No cold storage.

## Related Decisions
- **ADR-005 (Tenant Model):** Per-organization usage metrics require tenant-scoped observability data.
- **ADR-004 (Rate Limiting):** Rate limit metrics (throttled requests, tier utilization) are part of the observability data model.

## Notes
- The `tracing` crate supports structured key-value fields that map directly to JSON log entries. Example: `info!(request_id = %req_id, latency_ms = 42, "request completed")`.
- Log levels are configurable per module via `RUST_LOG` environment variable: `RUST_LOG=gateway_api=info,gateway_providers=debug`.
- The request inspector in the dashboard uses `request_id` to show the full lifecycle of a single request across all components.
- Future work: Optional OpenTelemetry span export for customers who want to integrate with their existing observability stack. This would be an opt-in configuration flag.
- Future work: Time-series database integration (InfluxDB, TimescaleDB) for long-term metric retention and advanced querying.
