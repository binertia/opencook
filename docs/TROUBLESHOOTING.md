# Troubleshooting Guide

> **Common issues and their solutions.** If your problem isn't here, check logs with `RUST_LOG=debug` and the [GitHub Issues](https://github.com/ai-gateway/ai-gateway/issues).

---

## Table of Contents

1. [Database Connection Issues](#1-database-connection-issues)
2. [Provider Configuration Errors](#2-provider-configuration-errors)
3. [Cache Problems](#3-cache-problems)
4. [Performance Tuning](#4-performance-tuning)
5. [Auth & API Key Issues](#5-auth--api-key-issues)
6. [Deployment Issues](#6-deployment-issues)
7. [Dashboard Issues](#7-dashboard-issues)

---

## 1. Database Connection Issues

### "Connection refused" on startup

**Symptom:**

```
Error: pool timed out while waiting for an open connection
```

**Causes & Fixes:**

| Cause | Fix |
|-------|-----|
| PostgreSQL not running | `docker compose -f docker-compose.dev.yml up -d postgres` |
| Wrong `DATABASE_URL` | Verify host, port, user, password, and database name |
| Firewall blocking port 5432 | Check `ufw` / `iptables` rules |
| Connection pool exhausted | Increase pool size or reduce connection lifetime |

**Verify connectivity:**

```bash
# Test PostgreSQL connection
psql $DATABASE_URL -c "SELECT 1;"

# From inside the container
docker compose -f docker-compose.dev.yml exec postgres pg_isready -U gateway
```

### Migration failures

**Symptom:**

```
Error: migration 0004_create_api_keys was previously applied but has been modified
```

**Fix:**

```bash
# In development: reset the database
docker compose -f docker-compose.dev.yml down -v
docker compose -f docker-compose.dev.yml up -d postgres

# In production: create a new migration instead of editing old ones
sqlx migrate add fix_something
```

### SQLite "database is locked"

**Symptom (SOLO mode):**

```
database is locked
```

**Fix:** SQLite handles one writer at a time. If running multiple `gateway-solo` instances, switch to PostgreSQL (TEAM mode) or ensure only one process accesses the file.

---

## 2. Provider Configuration Errors

### "Mock response" instead of real provider output

**Symptom:** Response has `X-Gateway-Mock-Response: true` header.

**Causes & Fixes:**

| Cause | Fix |
|-------|-----|
| No provider configured | Add a provider via the admin dashboard or API |
| Provider disabled | Enable the provider in settings |
| API key empty in config | Enter a valid provider API key |

**Add provider via API:**

```bash
curl -X POST http://localhost:8080/api/v1/organizations/{org_id}/providers \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $API_KEY" \
  -d '{
    "name": "OpenAI Production",
    "provider_type": "openai",
    "base_url": "https://api.openai.com/v1",
    "api_key": "sk-openai-...",
    "models": ["gpt-4o-mini", "gpt-4o"],
    "enabled": true
  }'
```

### "Provider error" (502 Bad Gateway)

**Symptom:**

```json
{"error": {"code": "provider_error", "message": "OpenAI returned 429", "status": 502}}
```

**Causes & Fixes:**

| Cause | Fix |
|-------|-----|
| Provider rate limit | Enable fallback routing; check provider dashboard |
| Invalid API key | Re-enter the provider API key; verify it works directly |
| Network timeout | Increase timeout or check firewall rules |
| Model not available on provider | Verify the model ID matches the provider's naming |

**Test provider directly:**

```bash
curl https://api.openai.com/v1/models \
  -H "Authorization: Bearer $OPENAI_API_KEY"
```

### Ollama connection refused

**Symptom:** Ollama provider shows "unhealthy" in dashboard.

**Fix:**

```bash
# Ensure Ollama is running and accessible
curl http://localhost:11434/api/tags

# If running in Docker, use host.docker.internal
curl http://host.docker.internal:11434/api/tags

# Update provider base_url to match
docker compose exec gateway curl http://host.docker.internal:11434/api/tags
```

---

## 3. Cache Problems

### Cache not reducing costs

**Symptom:** `gateway_cache_hits_total` metric stays at 0.

**Causes & Fixes:**

| Cause | Fix |
|-------|-----|
| Redis not connected | Check `REDIS_URL` and Redis logs |
| Cache disabled | No explicit disable flag; ensure Redis is healthy |
| Requests are unique | Cache keys include model, messages, temperature — small changes miss |
| TTL expired | Default TTL is 60s (L1) / configurable (L2) |

**Check cache status:**

```bash
# Redis ping
docker compose -f docker-compose.dev.yml exec redis redis-cli ping

# Check cache keys
docker compose -f docker-compose.dev.yml exec redis redis-cli keys "gateway:cache:*"
```

### Redis memory issues

**Symptom:** Redis evicting keys prematurely.

**Fix:**

```bash
# Check memory usage
docker compose exec redis redis-cli info memory

# Increase maxmemory in redis.conf or docker-compose
command: redis-server --appendonly yes --maxmemory 512mb --maxmemory-policy allkeys-lru
```

---

## 4. Performance Tuning

### High latency on first request

**Symptom:** First request after startup is slow (>2s).

**Cause:** Cold start — provider config loaded from DB, connection pools warming up.

**Fix:** Send a health check or warm-up request on deployment:

```bash
# Kubernetes post-start hook
lifecycle:
  postStart:
    exec:
      command: ["curl", "-f", "http://localhost:8080/health"]
```

### Request queue backlog

**Symptom:** `429 rate_limit_exceeded` or slow responses under load.

**Tuning checklist:**

| Layer | Tuning |
|-------|--------|
| Connection pool | Increase `max_connections` in DB config |
| Redis | Ensure Redis has dedicated CPU; use connection manager |
| Rate limits | Adjust per-org limits if too aggressive |
| Provider timeout | Increase if provider is consistently slow |
| Horizontal scaling | Run multiple gateway instances behind a load balancer |

### Memory usage growing

**Symptom:** Gateway container RSS increases over time.

**Causes:**

- L1 cache (`moka`) growing — default max 10,000 entries
- Request/response bodies not being freed
- Memory leak in streaming handlers

**Fix:**

```bash
# Check L1 cache size (from /metrics)
curl -s http://localhost:8080/metrics | grep gateway_cache

# Restart container if memory is critical
docker compose restart gateway

# Reduce L1 cache capacity in code if needed
```

---

## 5. Auth & API Key Issues

### "Unauthorized" (401)

**Symptom:**

```json
{"error": {"code": "unauthorized", "message": "Invalid API key", "status": 401}}
```

**Causes & Fixes:**

| Cause | Fix |
|-------|-----|
| Missing `Authorization` header | Add `-H "Authorization: Bearer sk_gw_..."` |
| Wrong key format | Key must start with `sk_gw_` and be 44 chars |
| Key expired or revoked | Generate a new key in the dashboard |
| Clock skew (JWT) | Ensure server time is synced (`ntp`) |

**Verify key format:**

```bash
KEY="sk_gw_1234567890123456789012345678901234567890123456789012345678abcd"
[[ ${#KEY} -eq 44 && "$KEY" == sk_gw_* ]] && echo "Valid format" || echo "Invalid format"
```

### RBAC access denied

**Symptom:** `403` on admin endpoints despite valid session.

**Fix:** Check the user's role in the organization. Roles: `owner`, `admin`, `member`, `viewer`. Some endpoints require `owner` or `admin`.

---

## 6. Deployment Issues

### Frontend assets 404 in production

**Symptom:** Dashboard loads but shows blank page; console shows 404 for JS/CSS.

**Cause:** Vite `base` path doesn't match the served path.

**Fix:**

```bash
# Ensure frontend is built with correct base
cd frontend && pnpm build  # uses base: '/admin/' from vite.config.ts

# Backend serves at /admin/*
# BrowserRouter basename is /admin
```

### CORS errors from frontend

**Symptom:** Browser blocks requests with CORS policy errors.

**Fix:** The backend CORS layer allows `Any` origin in dev. In production, restrict to your domain:

```rust
// In router.rs — replace Any with specific origin
.allow_origin("https://gateway.example.com".parse::<HeaderValue>().unwrap())
```

### Port already in use

**Symptom:** `AddrInUse` on startup.

**Fix:**

```bash
# Find and kill process on port 8080
lsof -ti:8080 | xargs kill -9

# Or use a different port
PORT=9090 cargo run --bin gateway-api
```

---

## 7. Dashboard Issues

### Dashboard shows "Loading" forever

**Symptom:** Spinners never resolve.

**Causes & Fixes:**

| Cause | Fix |
|-------|-----|
| Backend not running | Check `docker compose ps` |
| CORS blocked | See [CORS errors](#cors-errors-from-frontend) above |
| API returns 500 | Check backend logs for panic/error |
| Network disconnect | Refresh page; check browser dev tools Network tab |

### Dark mode not persisting

**Fix:** Theme preference is stored in `localStorage`. Clear site data if stuck:

```javascript
localStorage.removeItem('theme');
location.reload();
```

---

## Getting Help

1. **Enable debug logging:** `RUST_LOG=debug` then reproduce the issue
2. **Check logs:** `docker compose logs -f backend`
3. **Verify health:** `curl http://localhost:8080/health` and `/ready`
4. **Review metrics:** `curl http://localhost:8080/metrics`
5. **File an issue:** Include logs, config (redact secrets), and reproduction steps
