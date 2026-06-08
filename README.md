# OpenCook

![CI](https://github.com/ai-gateway/ai-gateway/actions/workflows/ci.yml/badge.svg)
![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue)
![Rust](https://img.shields.io/badge/rust-1.78%2B-orange?logo=rust)

> **One API. Every LLM. Self-hosted in 60 seconds.**

OpenCook is a lightweight, open-source AI Gateway built in Rust. Drop it in front of your LLM providers and get a single OpenAI-compatible endpoint with intelligent routing, semantic caching, budget caps, and a built-in admin dashboard.

**Why OpenCook?**
- **Save 30–70% on API bills** with smart routing + semantic caching
- **One API key to rule them all** — unify OpenAI, Anthropic, Gemini, Ollama, and 7 more
- **Hard budget caps that actually stop spending** — not just alerts
- **Deploys anywhere** — single binary (SQLite) or Docker Compose (PostgreSQL)

---

## Table of Contents

- [Quick Start](#quick-start)
- [How to Use](#how-to-use)
  - [SOLO Mode — Zero Config](#solo-mode--zero-config)
  - [TEAM Mode — Full Stack](#team-mode--full-stack)
  - [Configure Providers](#configure-providers)
  - [Make API Requests](#make-api-requests)
  - [Use the Dashboard](#use-the-dashboard)
- [Features](#features)
- [Supported Providers](#supported-providers)
- [Configuration Reference](#configuration-reference)
- [Development](#development)
- [Architecture](#architecture)
- [License](#license)

---

## Quick Start

**The fastest way to run OpenCook (SOLO mode):**

```bash
# 1. Clone
git clone https://github.com/ai-gateway/ai-gateway.git
cd ai-gateway

# 2. Build (Rust 1.78+ required)
cargo build --release --bin gateway-solo

# 3. Run — zero configuration, SQLite auto-created
./target/release/gateway-solo serve
```

**That's it.** The gateway is running on `http://localhost:8080`.

Test it:
```bash
curl -X POST http://localhost:8080/v1/chat/completions \
  -H "Content-Type: application/json" \
  -d '{"model":"gpt-4o","messages":[{"role":"user","content":"Hello!"}]}'
```

---

## How to Use

### SOLO Mode — Zero Config

SOLO mode is designed for personal use, local development, and single-tenant deployments. No database setup. No auth. No Docker.

```bash
# Build
cargo build --release --bin gateway-solo

# Optional: add provider API keys so you get real responses instead of mocks
export OPENAI_API_KEY="sk-..."
export ANTHROPIC_API_KEY="sk-ant-..."

# Run
./target/release/gateway-solo serve
```

**What you get:**
| Endpoint | URL |
|----------|-----|
| OpenAI-compatible API | `http://localhost:8080/v1/chat/completions` |
| Models list | `http://localhost:8080/v1/models` |
| Health check | `http://localhost:8080/health` |
| Metrics (Prometheus) | `http://localhost:8080/metrics` |
| Admin dashboard | `http://localhost:8080/admin` |

**SQLite database** is auto-created at `./data/gateway.db`.

**Routing profiles** let you optimize for different goals:
```bash
# Privacy-first: routes to local Ollama whenever possible
./target/release/gateway-solo serve --profile privacy-first

# Frugal: always picks the cheapest model
./target/release/gateway-solo serve --profile frugal

# Speed: lowest-latency provider
./target/release/gateway-solo serve --profile speed
```

---

### TEAM Mode — Full Stack

TEAM mode adds multi-tenant auth, PostgreSQL persistence, Redis caching, and the React admin dashboard. Designed for teams and production.

#### Prerequisites

- Docker & Docker Compose
- OpenSSL (to generate JWT keys)

#### Step 1: Generate secrets

```bash
# Master key for encrypting provider configs (32 bytes = 64 hex chars)
export GATEWAY_MASTER_KEY=$(openssl rand -hex 32)

# JWT signing keys (RS256)
openssl genrsa -out /tmp/jwt-private.pem 2048
openssl rsa -in /tmp/jwt-private.pem -pubout -out /tmp/jwt-public.pem
export GATEWAY_JWT_PRIVATE_KEY=$(cat /tmp/jwt-private.pem)
export GATEWAY_JWT_PUBLIC_KEY=$(cat /tmp/jwt-public.pem)
```

#### Step 2: Start services

```bash
# Copy the example env and edit if needed
cp .env.example .env

# Start PostgreSQL, Redis, backend, and frontend
docker compose -f docker-compose.dev.yml up -d

# Watch logs until "gateway-api listening on http://0.0.0.0:8080"
docker compose -f docker-compose.dev.yml logs -f backend
```

| Service | Port | Description |
|---------|------|-------------|
| Gateway API | `8080` | Rust Axum API server |
| Dashboard (dev) | `5173` | React Vite dev server |
| PostgreSQL | `5432` | pgvector-enabled PostgreSQL 16 |
| Redis | `6379` | Cache + rate limiting |

#### Step 3: Create your first admin user

The database starts empty. Create an organization and admin user via the registration endpoint:

```bash
curl -X POST http://localhost:8080/auth/register \
  -H "Content-Type: application/json" \
  -d '{
    "email": "admin@example.com",
    "password": "SecurePass123!",
    "organization_name": "My Team"
  }'
```

Log in at `http://localhost:5173` (or `http://localhost:8080/admin` if you built the static dashboard).

#### Step 4: Create an API key

Navigate to **API Keys** in the dashboard and click **Create Key**. Copy the key — it's shown only once.

---

### Configure Providers

Providers can be configured in three ways:

#### Option A: Environment variables (SOLO mode)

```bash
export OPENAI_API_KEY="sk-..."
export ANTHROPIC_API_KEY="sk-ant-..."
export GEMINI_API_KEY="..."
export OLLAMA_BASE_URL="http://localhost:11434"
```

#### Option B: Dashboard (TEAM mode)

Go to **Providers → Add Provider**, select the provider kind, enter the API key, and click **Test Connection**.

#### Option C: Config file

Create `gateway.toml` in the project root:

```toml
port = 8080
profile = "balanced"

# Provider API keys (SOLO mode fallback)
openai_api_key = "sk-..."
anthropic_api_key = "sk-ant-..."
gemini_api_key = "..."
ollama_base_url = "http://localhost:11434"

# TEAM mode settings
database_url = "postgres://gateway:gateway@localhost:5432/gateway"
redis_url = "redis://localhost:6379"
master_key = "0000000000000000000000000000000000000000000000000000000000000000"
```

---

### Make API Requests

Once a provider is configured, OpenCook is a drop-in replacement for the OpenAI API.

#### Chat completions

```bash
curl -X POST http://localhost:8080/v1/chat/completions \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer sk_gw_your_api_key_here" \
  -d '{
    "model": "gpt-4o",
    "messages": [
      {"role": "system", "content": "You are a helpful assistant."},
      {"role": "user", "content": "Explain quantum computing in one sentence."}
    ],
    "temperature": 0.7,
    "max_tokens": 100
  }'
```

**Headers added by the gateway:**
| Header | Meaning |
|--------|---------|
| `X-Gateway-Mock-Response: true` | No provider configured; returning mock |
| `X-Cache: HIT` | Response served from cache |
| `X-Cache: MISS` | Response fetched from provider |
| `X-Request-Cost-USD: 0.0012` | Estimated cost of this request |

#### List models

```bash
curl http://localhost:8080/v1/models \
  -H "Authorization: Bearer sk_gw_your_api_key_here"
```

#### Streaming (SSE)

```bash
curl -N http://localhost:8080/v1/chat/completions \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer sk_gw_your_api_key_here" \
  -d '{
    "model": "gpt-4o-mini",
    "messages": [{"role": "user", "content": "Count to 5"}],
    "stream": true
  }'
```

#### Python / OpenAI SDK

```python
from openai import OpenAI

client = OpenAI(
    base_url="http://localhost:8080/v1",
    api_key="sk_gw_your_api_key_here",  # any non-empty string works in SOLO mode
)

response = client.chat.completions.create(
    model="gpt-4o",
    messages=[{"role": "user", "content": "Hello!"}],
)
print(response.choices[0].message.content)
```

---

### Use the Dashboard

The admin dashboard gives you full visibility and control.

| Page | What you can do |
|------|-----------------|
| **Overview** | KPI cards (requests, cost, cache hit rate), recent requests, active providers |
| **Providers** | Add/edit providers, run health checks, view latency & error rates |
| **API Keys** | Create, revoke, copy keys; view per-key usage |
| **Users** | Invite members, assign roles (owner/admin/member/viewer), remove access |
| **Routing** | Configure routing rules, priorities, fallback chains |
| **Analytics** | Cost breakdowns by model/provider, token usage, cache performance |
| **Quotas** | Set budget caps, rate limits, warning thresholds |
| **Request Logs** | Filterable log of every request with cost, latency, status |

**Build the dashboard for production:**

```bash
cd frontend
npm install
npm run build
# Static files are output to frontend/dist/
# The backend serves them automatically at /admin
```

---

## Features

| Feature | Description |
|---------|-------------|
| **11 LLM Providers** | OpenAI, Anthropic, Gemini, Ollama, Qwen, Kimi, Tencent, Groq, Mistral, Cohere, Azure |
| **OpenAI-Compatible API** | Drop-in `/v1/chat/completions` and `/v1/models` — works with existing SDKs |
| **Dual-Mode Architecture** | SOLO (SQLite, zero config) ↔ TEAM (PostgreSQL, full auth) |
| **Semantic Caching** | Embedding-based cache cuts costs 20–40% on repeated queries |
| **L1 + L2 Caching** | In-process (moka) + Redis with configurable TTLs |
| **Intelligent Routing** | Privacy-first, frugal, speed, quality, balanced, offline profiles |
| **Circuit Breaker** | Auto-failover when a provider goes down |
| **Hard Budget Caps** | Per-org, per-key, per-model quotas that block when exceeded |
| **6-Layer Rate Limiting** | Global, org, API key, token, provider, IP |
| **Request Logging** | Full audit trail with cost attribution and PII redaction |
| **Prometheus Metrics** | `/metrics` endpoint for Grafana dashboards |
| **RBAC + API Keys** | 4 roles, 31 permissions, SHA-256 hashed keys |
| **React Admin Dashboard** | Real-time health, usage analytics, key management |

---

## Supported Providers

| Provider | Kind | Models | Streaming | Embeddings |
|----------|------|--------|-----------|------------|
| OpenAI | `openai` | gpt-4o, gpt-4o-mini, gpt-4-turbo | ✅ | ✅ |
| Anthropic | `anthropic` | claude-3-5-sonnet, claude-3-opus | ✅ | ❌ |
| Gemini | `gemini` | gemini-1.5-flash, gemini-1.5-pro | ✅ | ✅ |
| Ollama | `ollama` | llama3.2, mistral, codellama | ✅ | ✅ |
| Qwen | `qwen` | qwen-max, qwen-plus, qwen-turbo | ✅ | ❌ |
| Kimi | `kimi` | moonshot-v1-8k, moonshot-v1-32k | ✅ | ❌ |
| Tencent | `tencent` | hunyuan-lite, hunyuan-standard, hunyuan-pro | ✅ | ❌ |
| Groq | `groq` | llama-3.1-70b, mixtral-8x7b | ✅ | ❌ |
| Mistral | `mistral` | mistral-large, mistral-medium | ✅ | ❌ |
| Cohere | `cohere` | command-r, command-r-plus | ✅ | ❌ |
| Azure OpenAI | `azure` | gpt-4o, gpt-4-turbo | ✅ | ✅ |

---

## Configuration Reference

### Environment variables

| Variable | Required | Default | Description |
|----------|----------|---------|-------------|
| `DATABASE_URL` | TEAM only | — | `postgres://` or `sqlite://` connection string |
| `REDIS_URL` | TEAM only | `redis://localhost:6379` | Redis connection |
| `GATEWAY_MASTER_KEY` | TEAM recommended | random | 64-char hex encryption key |
| `GATEWAY_JWT_PRIVATE_KEY` | TEAM recommended | random HS256 | RS256 private key PEM |
| `GATEWAY_JWT_PUBLIC_KEY` | TEAM recommended | random HS256 | RS256 public key PEM |
| `OPENAI_API_KEY` | Optional | — | Provider API key |
| `ANTHROPIC_API_KEY` | Optional | — | Provider API key |
| `GATEWAY_ALLOWED_ORIGINS` | Optional | — | CORS origins (comma-separated) |
| `RUST_LOG` | Optional | `info` | Log level (`error`, `warn`, `info`, `debug`, `trace`) |

### Config file (`gateway.toml`)

```toml
port = 8080
profile = "balanced"          # privacy-first | balanced | speed | frugal | quality | offline
database_url = "postgres://gateway:gateway@localhost:5432/gateway"
redis_url = "redis://localhost:6379"
master_key = "64-char-hex"
jwt_private_key_pem = "-----BEGIN PRIVATE KEY-----\n..."
jwt_public_key_pem = "-----BEGIN PUBLIC KEY-----\n..."
allowed_origins = "http://localhost:5173,http://localhost:8080"
semantic_cache_enabled = false
semantic_cache_threshold = 0.95
embedding_base_url = "https://api.openai.com"
embedding_api_key = "sk-..."
```

---

## Development

```bash
# Clone
git clone https://github.com/ai-gateway/ai-gateway.git
cd ai-gateway

# Run unit tests (no external services)
cargo test --workspace --lib

# Run all tests (requires PostgreSQL + Redis)
export DATABASE_URL=postgres://gateway:gateway@localhost:5432/gateway
export REDIS_URL=redis://localhost:6379
cargo test --workspace

# Format & lint
cargo fmt
cargo clippy --workspace --all-targets -- -D warnings

# Run SOLO mode locally
cargo run --bin gateway-solo -- serve

# Run TEAM mode locally (requires DB + Redis)
cargo run --bin gateway-api -- serve

# Dashboard development
cd frontend
npm install
npm run dev
```

### Project Structure

```
├── crates/
│   ├── gateway-api/           # TEAM mode API server (Axum, port 8080)
│   ├── gateway-solo/          # SOLO mode API server (zero config)
│   ├── gateway-core/          # Routing, circuit breaker, retry, fallback
│   ├── gateway-providers/     # LLM provider adapters
│   ├── gateway-cache/         # L1 (moka) + L2 (Redis) + semantic cache
│   ├── gateway-quota/         # Rate limiting, budget caps, usage tracking
│   ├── gateway-auth/          # JWT, API keys, RBAC, Argon2
│   ├── gateway-db/            # sqlx repos, migrations, dual-db pool
│   └── gateway-observability/ # Prometheus metrics, tracing, request logs
├── frontend/                  # React + Vite + Tailwind + shadcn/ui
├── migrations/                # 28 sqlx PostgreSQL migrations
├── docker/                    # Dockerfiles
├── docs/                      # Full project documentation
└── tasks/                     # Implementation task registry
```

---

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                         Clients                             │
│  OpenAI SDK  │  curl  │  LangChain  │  OpenCode  │  Web UI  │
└──────────────────────────┬──────────────────────────────────┘
                           │  OpenAI-compatible API
┌──────────────────────────▼──────────────────────────────────┐
│                      Axum Router                            │
│  CORS → Body Limit → Trace → Rate Limit → Auth → Handler    │
└──────────────────────────┬──────────────────────────────────┘
                           │
        ┌──────────────────┼──────────────────┐
        ▼                  ▼                  ▼
   ┌─────────┐      ┌──────────┐      ┌─────────────┐
   │  Cache  │      │  Quota   │      │   Router    │
   │ L1 + L2 │      │  Engine  │      │  + Fallback │
   └────┬────┘      └────┬─────┘      └──────┬──────┘
        │                │                   │
        └────────────────┼───────────────────┘
                         ▼
              ┌────────────────────┐
              │  Provider Adapters │
              │ OpenAI / Anthropic │
              │ Gemini / Ollama +7 │
              └────────────────────┘
```

**Stack:** Rust 1.78+ · Axum · Tokio · PostgreSQL 16 / SQLite · Redis 7 · React 18 · Vite

---

## License

MIT OR Apache-2.0

---

<p align="center">
  Built with 🦀· <a href="https://github.com/ai-gateway/ai-gateway/issues">Report an issue</a>
</p>
