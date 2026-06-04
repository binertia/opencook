# OpenCook — Open Source AI Gateway

> A lightweight, self-hosted AI gateway that unifies access to 11+ LLM providers through a single OpenAI-compatible API. Built in Rust for performance and reliability.

---

## What It Is

OpenCook is an open-source AI gateway designed for teams who want to use multiple LLM providers without the operational complexity. It sits between your application and your AI providers — handling authentication, routing, caching, rate limiting, cost tracking, and failover — so your code only talks to one API.

You can deploy it on a single VPS in under 10 minutes. No Kubernetes. No DevOps team required.

---

## Core Features

### 1. Unified OpenAI-Compatible API

Change one line in your application — the base URL — and instantly route to OpenAI, Anthropic, Gemini, Ollama, or any of 11 supported providers. The API surface is fully OpenAI-compatible, so existing SDKs and tools work without modification.

```bash
# Before: talking directly to OpenAI
curl https://api.openai.com/v1/chat/completions ...

# After: talking through OpenCook
curl http://localhost:8080/v1/chat/completions ...
```

### 2. 11+ Provider Ecosystem

| Provider | Region | Notable Models |
|----------|--------|----------------|
| OpenAI | US | gpt-4o, gpt-4o-mini, gpt-4-turbo |
| Anthropic | US | claude-3-5-sonnet, claude-3-opus |
| Gemini | US | gemini-1.5-flash, gemini-1.5-pro |
| Groq | US | llama-3.1-70b-versatile, mixtral-8x7b |
| Mistral | EU | mistral-large, mistral-medium |
| Cohere | US | command-r, command-r-plus |
| Azure OpenAI | Global | gpt-4o, gpt-4-turbo |
| Ollama | Self-hosted | llama3.2, mistral, codellama |
| Qwen (Alibaba) | China | qwen-max, qwen-plus, qwen-turbo |
| Kimi (Moonshot) | China | moonshot-v1-8k, moonshot-v1-32k |
| Tencent (Hunyuan) | China | hunyuan-lite, hunyuan-standard, hunyuan-pro |

You are not locked into any single provider. Switch models, compare providers, or route different workloads to different backends — all through the same API.

### 3. Semantic Caching

Repeated or semantically similar requests are served from cache instead of hitting the provider API. OpenCook uses embedding-based similarity to recognize when a new question is close enough to a previously cached one — cutting AI spend by 20–40% on typical workloads.

- **Exact-match cache**: SHA-256 request hashing for identical queries
- **Semantic cache**: Cosine similarity on embeddings for near-identical queries
- **Configurable threshold**: Tune sensitivity per use case
- **Cost tracking**: See exactly how much caching saves you

### 4. Circuit Breaker & Automatic Failover

When a provider goes down or hits rate limits, OpenCook automatically reroutes to the next healthy provider in your fallback chain. No manual intervention. No 3 AM pages.

- Health checks every 30 seconds
- Circuit breaker trips after 5 consecutive failures
- 60-second recovery window before retry
- Retry with exponential backoff + jitter

### 5. Built-in React Admin Dashboard

A full-featured dashboard ships with the gateway — no separate observability tool required.

- **Dashboard Overview**: KPI cards, recent requests, active providers, quick actions
- **Provider Management**: Add, edit, test, and monitor provider health
- **API Keys**: Create scoped keys, set rate limits, revoke instantly
- **User Management**: Invite team members, assign roles (owner/admin/member/viewer)
- **Analytics**: Usage breakdowns by model, status, token volume, and cost
- **Request Logs**: Filterable, searchable log of every request

### 6. Dual Deployment Mode

**SOLO Mode** (default): Run locally with zero configuration. SQLite database auto-creates. Perfect for indie developers, side projects, and prototyping.

```bash
opencook serve
```

**TEAM Mode**: Full PostgreSQL + Redis + RBAC for production teams. Multi-organization support, JWT session auth, and granular permissions.

```bash
# Environment variables for TEAM mode
DATABASE_URL="postgres://..."
REDIS_URL="redis://..."
GATEWAY_MASTER_KEY="..."
```

### 7. Six-Layer Rate Limiting

Prevent runaway costs and abuse with granular rate limits at every layer:

1. **Global**: Protect the gateway itself
2. **Organization**: Per-team quotas
3. **API Key**: Per-key request and token limits
4. **Token**: Per-minute token budgets
5. **Provider**: Respect upstream provider rate limits
6. **IP**: Basic abuse prevention

Rate limits use Redis-backed sliding windows with Lua scripts for atomicity. They fail open on Redis errors — your service stays available.

### 8. Request Logging & Cost Tracking

Every request is logged with full metadata: tokens in/out, latency, provider used, model routed, cost in USD, cache hit/miss status, and error details.

- **Per-request cost calculation**: Based on per-model pricing tables
- **Aggregated analytics**: Daily/hourly rollups by model, provider, and API key
- **Export**: CSV and JSON export for external analysis
- **Retention**: Configurable; data lives in your database, not a third-party cloud

### 9. OpenCode Compatible

Works out of the box with AI coding agents and tools that expect an OpenAI-compatible endpoint — including opencode.ai and similar platforms.

---

## What Makes It Better

### Deploys in Under 10 Minutes on a Single VPS

Most AI gateway solutions require Kubernetes clusters, multiple containers, or dedicated DevOps expertise to run in production. OpenCook is designed for the opposite: one VPS, one `docker-compose up`, and you are routing requests.

- Single Docker Compose file includes PostgreSQL, Redis, and the gateway
- SQLite mode requires zero external dependencies
- No Kubernetes. No Helm charts. No Terraform modules.

### Zero Markup on Provider Costs

Some gateways charge a percentage on every request you make. OpenCook does not. You bring your own API keys, you pay providers directly, and you keep 100% of the savings from caching and smart routing.

### Hard Budget Caps — Not Just Alerts

Many tools send you a Slack message when you hit 80% of budget. OpenCook can **stop** requests when the budget is exhausted. A runaway loop or viral feature will not generate a surprise $5,000 bill at the end of the month.

- Soft warnings at configurable thresholds (50%, 75%, 90%)
- Hard cutoff at 100% of budget
- Per-organization, per-key, and per-user granularity

### Built-in Observability — No External Tools Required

Self-hosted infrastructure should not require another SaaS subscription just to see what is happening. The dashboard gives you real-time visibility into costs, latency, errors, and cache performance without integrating Datadog, Langfuse, or Prometheus.

### Rust Backend for Predictable Performance

Built on Axum + Tokio, the gateway adds <5ms of latency overhead per request. The backend compiles to a single binary with minimal resource usage — suitable for a $5/month VPS running side projects or a dedicated production box handling thousands of requests per second.

### Semantic Caching Included — Not Enterprise-Locked

Embedding-based semantic caching is available in the open-source version, not hidden behind a $500/month enterprise tier. You get cost reduction features on day one.

### Actively Maintained Open-Core

The project is under active development with a public roadmap, tracked tasks, and regular releases. You are not adopting a project that was acquired and put into maintenance mode.

---

## The Problems We Solve

### "Our AI bill was 3× what we expected."

**Problem**: Token-based pricing has no ceiling. A runaway agent loop or viral feature can burn through a monthly budget overnight.

**Solution**: Hard budget caps with automatic request blocking. Semantic caching cuts repetitive costs by 20–40%. Cost-aware routing sends simple queries to cheaper models.

### "We have no idea which team or project is spending what."

**Problem**: Multiple teams share the same provider API keys. Finance sees one lump-sum bill with zero attribution.

**Solution**: Virtual API keys with per-key cost tracking. Dashboard shows breakdown by key, model, provider, and time period. Export to CSV for finance teams.

### "Managing 4 different API integrations is exhausting."

**Problem**: Each provider has different auth methods, response formats, SDKs, rate limits, and billing portals.

**Solution**: One API key. One endpoint. One response format. OpenCook normalizes everything to OpenAI-compatible JSON, handles provider-specific transformations internally.

### "Our app went down because OpenAI had an outage."

**Problem**: Single provider = single point of failure. Most small teams have no fallback strategy.

**Solution**: Automatic health monitoring + circuit breaker + fallback chain. If OpenAI is down, requests route to Anthropic or Gemini transparently.

### "We are paying to generate the same response over and over."

**Problem**: Support bots, documentation Q&A, and internal tools receive many similar or identical questions.

**Solution**: Exact-match + semantic caching. Previously generated responses are served from cache at zero provider cost. Hit rates of 40–60% are common for Q&A workloads.

### "Setting up LiteLLM took two days and a Kubernetes cluster."

**Problem**: Existing open-source gateways are powerful but operationally complex. They require DevOps expertise most SMEs do not have.

**Solution**: `docker-compose up` or `opencook serve`. Ten minutes. One person. No specialized knowledge required.

### "I do not want my prompts passing through a third-party cloud."

**Problem**: Managed gateway services see all your traffic. For sensitive applications, this is a compliance and security risk.

**Solution**: Self-hosted on your infrastructure. Your data never leaves your server. Your prompts stay private.

---

## Architecture at a Glance

| Layer | Technology | Why |
|-------|-----------|-----|
| Gateway Backend | Rust (Axum + Tokio) | Performance, safety, single binary |
| Admin Dashboard | React + TypeScript + Tailwind | Modern UI, fast iteration |
| Database (TEAM) | PostgreSQL 16 | ACID transactions, audit trails |
| Database (SOLO) | SQLite | Zero config, auto-created |
| Cache | Redis 7 | Rate limiting, response cache |
| Deployment | Docker Compose | Single VPS, one command |

---

## Quick Start

```bash
# Install
curl -fsSL https://raw.githubusercontent.com/ai-gateway/ai-gateway/main/install.sh | bash

# Run (SOLO mode — zero config)
opencook serve

# Or with Docker Compose (TEAM mode with PostgreSQL + Redis)
docker-compose -f docker-compose.dev.yml up
```

The gateway starts on `http://localhost:8080` with the dashboard available at the same URL.

---

## License

MIT OR Apache-2.0

---

*OpenCook is actively developed. See the [roadmap](ROADMAP.md) and [task index](../tasks/INDEX.md) for what is being built next.*
