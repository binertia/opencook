# AI Gateway Grafana Monitoring

Pre-built Grafana dashboard and alert rules for the AI Gateway Prometheus metrics.

## Files

| File | Purpose |
|------|---------|
| `dashboard-gateway.json` | Grafana 10+ dashboard with 6 rows of panels |
| `alerts.yml` | Prometheus AlertManager rules |

## Dashboard Rows

1. **Request Overview** — RPS, error rate, latency p50/p90/p99
2. **Cache Performance** — hit rate, hits by layer, estimated cost savings
3. **Provider Health** — health status, latency, error rate per provider
4. **Token Usage** — input/output tokens and tokens by model
5. **Quotas** — quota exceeded and rate-limited request rates
6. **System** — active connections, memory, CPU (requires `process_` metrics)

## Quick Start

### 1. Start Prometheus

Ensure your `prometheus.yml` scrapes the gateway `/metrics` endpoint:

```yaml
scrape_configs:
  - job_name: 'gateway'
    static_configs:
      - targets: ['localhost:8080']
    metrics_path: /metrics
    scrape_interval: 15s
```

### 2. Import the Dashboard

Option A — Grafana UI:
- Open **Dashboards → Import**
- Upload `dashboard-gateway.json`
- Select your Prometheus data source

Option B — Grafana provisioning:

```yaml
# /etc/grafana/provisioning/dashboards/dashboards.yml
apiVersion: 1
providers:
  - name: 'AI Gateway'
    orgId: 1
    folder: 'AI Gateway'
    type: file
    disableDeletion: false
    editable: true
    options:
      path: /var/lib/grafana/dashboards
```

Copy `dashboard-gateway.json` to the path referenced above and restart Grafana.

### 3. Apply Alert Rules

```bash
promtool check rules monitoring/grafana/alerts.yml
# Copy to your Prometheus rule_files directory or ConfigMap
```

Example Prometheus config:

```yaml
rule_files:
  - /etc/prometheus/alerts/gateway-alerts.yml

alerting:
  alertmanagers:
    - static_configs:
        - targets: ['localhost:9093']
```

## Metric Reference

| Metric | Type | Labels | Description |
|--------|------|--------|-------------|
| `gateway_request_total` | Counter | `model`, `provider`, `status` | Total requests |
| `gateway_request_duration_ms` | Histogram | `model`, `provider`, `status` | Request latency |
| `gateway_cache_hit_total` | Counter | `layer` (`l1`, `l2`, `semantic`) | Cache hits |
| `gateway_cache_miss_total` | Counter | — | Cache misses |
| `gateway_tokens_total` | Counter | `type` (`input`/`output`), `model` | Token usage |
| `gateway_cost_total` | Counter | `model`, `provider` | Cost in micro-USD |
| `gateway_quota_exceeded_total` | Counter | `metric`, `scope` | Quota exceeded events |
| `gateway_rate_limited_total` | Counter | `key_hash` | Rate-limited requests |
| `gateway_provider_health` | Gauge | `provider`, `org` | 1 = healthy, 0 = unhealthy |
| `gateway_active_connections` | Gauge | — | Active HTTP connections |

## Alerts

| Alert | Condition | Severity |
|-------|-----------|----------|
| `GatewayHighErrorRate` | Error rate > 5% for 5 min | warning |
| `GatewayHighP99Latency` | p99 latency > 10 s for 5 min | warning |
| `GatewayProviderUnhealthy` | Provider health == 0 for 5 min | critical |
| `GatewayRateLimitSpike` | Rate-limited req/s > 10 for 5 min | warning |
| `GatewayQuotaExceeded` | Any quota exceeded event | info |

## Troubleshooting

- **No data in panels**: Verify Prometheus is scraping `/metrics` and job label matches the dashboard variable.
- **Cache cost saved is 0**: The panel is an estimate based on average request cost × cache hits. It requires both `gateway_cache_hit_total` and `gateway_cost_total`.
- **Missing process metrics**: The System row uses `process_resident_memory_bytes` and `process_cpu_seconds_total`. If your deployment does not expose these, those panels show "No data".
