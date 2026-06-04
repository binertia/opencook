# Quick Start Guide

> **Goal:** Get the AI Gateway running locally and send your first chat completion request in under 5 minutes.

---

## Prerequisites

| Tool | Version | Purpose |
|------|---------|---------|
| [Docker](https://docs.docker.com/get-docker/) | 24+ | Container runtime |
| [Docker Compose](https://docs.docker.com/compose/install/) | 2.20+ | Multi-service orchestration |
| `curl` | Any | HTTP client for testing |

---

## Step 1: Clone and Start

```bash
# Clone the repository
git clone https://github.com/ai-gateway/ai-gateway.git
cd ai-gateway

# Start all services (PostgreSQL, Redis, backend, frontend)
docker compose -f docker-compose.dev.yml up -d
```

This starts four containers:

| Service | Port | Description |
|---------|------|-------------|
| `postgres` | `5432` | PostgreSQL 16 with health checks |
| `redis` | `6379` | Redis 7.2 with persistence |
| `backend` | `8080` | Rust Axum API server (auto-reload) |
| `frontend` | `5173` | React dev server (Vite HMR) |

Wait for services to be healthy:

```bash
# Watch logs until you see "Server running on 0.0.0.0:8080"
docker compose -f docker-compose.dev.yml logs -f backend
```

Health check endpoints (no auth required):

```bash
curl http://localhost:8080/health   # {"status":"healthy"}
curl http://localhost:8080/ready    # {"status":"ready"}
```

---

## Step 2: Create Your First API Key

In **TEAM mode** (default), all AI API requests require an API key. The auth middleware is currently a stub that validates format only — no DB lookup — and uses the default organization. Create a valid-format key:

```bash
# Generate a valid API key (format: sk_gw_ + 32 base58 chars + 6-char checksum)
# Or use this example key for local testing:
API_KEY="sk_gw_1234567890123456789012345678901234567890123456789012345678abcd"
```

> **Note:** In `SOLO` mode (`gateway-solo`), no API key is required. See [DEPLOYMENT.md](DEPLOYMENT.md) for SOLO mode setup.

---

## Step 3: Send Your First Chat Completion

```bash
curl -X POST http://localhost:8080/v1/chat/completions \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $API_KEY" \
  -d '{
    "model": "gpt-4o-mini",
    "messages": [
      {"role": "system", "content": "You are a helpful assistant."},
      {"role": "user", "content": "Hello, AI Gateway!"}
    ],
    "temperature": 0.7,
    "max_tokens": 100
  }'
```

If no provider is configured, the gateway returns a **mock response** with the `X-Gateway-Mock-Response: true` header.

Expected response:

```json
{
  "id": "chatcmpl-mock-...",
  "object": "chat.completion",
  "created": 1717000000,
  "model": "gpt-4o-mini",
  "choices": [{
    "index": 0,
    "message": {
      "role": "assistant",
      "content": "Hello! I'm running in mock mode. Configure a provider to get real responses."
    },
    "finish_reason": "stop"
  }],
  "usage": {
    "prompt_tokens": 10,
    "completion_tokens": 20,
    "total_tokens": 30
  }
}
```

---

## Step 4: Open the Admin Dashboard

```bash
open http://localhost:5173   # macOS
# OR
xdg-open http://localhost:5173  # Linux
```

The dashboard provides:

- **Overview** — KPIs, recent requests, time range selector
- **Providers** — Add/edit provider configs (OpenAI, Anthropic, Gemini, Ollama)
- **Organization Settings** — Routing strategy, quota profiles
- **User Management** — Invite members, manage roles

> **Note:** The dashboard is served from the frontend dev server on `:5173`. In production, the backend serves the built SPA at `/admin/*`.

---

## Step 5: Configure a Real Provider

### OpenAI Example

1. Open the dashboard → **Providers** → **Add Provider**
2. Select **OpenAI** as the provider type
3. Enter your OpenAI API key
4. Set the base URL to `https://api.openai.com/v1`
5. Click **Test Connection**
6. Save

Now re-run the curl request — you'll get real responses from OpenAI.

---

## Quick Reference

### Docker Compose Commands

```bash
# Start everything
docker compose -f docker-compose.dev.yml up -d

# View backend logs
docker compose -f docker-compose.dev.yml logs -f backend

# Restart backend
docker compose -f docker-compose.dev.yml restart backend

# Stop everything
docker compose -f docker-compose.dev.yml down

# Stop and remove volumes (resets DB data)
docker compose -f docker-compose.dev.yml down -v
```

### Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `DATABASE_URL` | Required | `postgres://user:pass@host:5432/db` |
| `REDIS_URL` | Required | `redis://host:6379` |
| `RUST_LOG` | `info` | Log level (`debug`, `info`, `warn`, `error`) |
| `RUST_BACKTRACE` | `0` | Stack traces on panic |
| `APP_ENV` | `production` | Set to `development` to skip static file serving |

### Next Steps

- **[DEPLOYMENT.md](DEPLOYMENT.md)** — Deploy to production
- **[API.md](API.md)** — Full API reference
- **[API_SPEC.md](API_SPEC.md)** — Complete OpenAI-compatible spec
- **[TROUBLESHOOTING.md](TROUBLESHOOTING.md)** — If something goes wrong
