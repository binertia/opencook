# OpenCook

![CI](https://github.com/ai-gateway/ai-gateway/actions/workflows/ci.yml/badge.svg)
![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue)
![Rust](https://img.shields.io/badge/rust-1.78%2B-orange?logo=rust)

> Cook locally. GPT when the stacktrace speaking Thai.

A lightweight, self-hosted AI gateway that unifies access to 11+ LLM providers through a single OpenAI-compatible API. Built in Rust for performance and reliability.

## Quick Start

### One-liner Install (macOS / Linux)

```bash
curl -fsSL https://raw.githubusercontent.com/ai-gateway/ai-gateway/main/install.sh | bash
```

Or with Cargo:

```bash
cargo install --git https://github.com/ai-gateway/ai-gateway --bin opencook
```

### Start the Gateway

```bash
# Set your API key (any provider)
export OPENAI_API_KEY="sk-..."

# Start the server
opencook serve
```

The gateway will start on `http://localhost:8080` with a built-in admin dashboard.

### Make Your First Request

```bash
curl -X POST http://localhost:8080/v1/chat/completions \
  -H "Content-Type: application/json" \
  -d '{
    "model": "gpt-4o",
    "messages": [{"role": "user", "content": "Hello!"}]
  }'
```

## Features

- **11 Providers** — OpenAI, Anthropic, Gemini, Ollama, Qwen, Kimi, Tencent, Groq, Mistral, Cohere, Azure
- **OpenAI-Compatible API** — Drop-in replacement for `/v1/chat/completions` and `/v1/models`
- **Semantic Caching** — Embedding-based response caching cuts costs 20-40%
- **Circuit Breaker + Fallback** — Automatic failover between providers
- **React Admin Dashboard** — Real-time provider health, usage analytics, API key management
- **Dual Mode** — SOLO (SQLite, zero config) or TEAM (PostgreSQL, full auth)
- **Request Logging** — Per-request cost tracking and analytics
- **Rate Limiting** — 6-layer rate limiting (global, org, key, provider, IP)
- **OpenCode Compatible** — Works with opencode.ai and other AI coding agents

## Providers

| Provider | Kind | Models |
|----------|------|--------|
| OpenAI | `openai` | gpt-4o, gpt-4o-mini, gpt-4-turbo |
| Anthropic | `anthropic` | claude-3-5-sonnet, claude-3-opus |
| Gemini | `gemini` | gemini-1.5-flash, gemini-1.5-pro |
| Ollama | `ollama` | llama3.2, mistral, codellama |
| Qwen (Alibaba) | `qwen` | qwen-max, qwen-plus, qwen-turbo |
| Kimi (Moonshot) | `kimi` | moonshot-v1-8k, moonshot-v1-32k |
| Tencent (Hunyuan) | `tencent` | hunyuan-lite, hunyuan-standard, hunyuan-pro |
| Groq | `groq` | llama-3.1-70b-versatile, mixtral-8x7b |
| Mistral | `mistral` | mistral-large, mistral-medium |
| Cohere | `cohere` | command-r, command-r-plus |
| Azure OpenAI | `azure` | gpt-4o, gpt-4-turbo |

## Admin Dashboard

The React admin dashboard is available at `http://localhost:8080` and includes:

- **Dashboard** — KPI cards, recent requests, active providers
- **Providers** — CRUD, health checks, test connections
- **API Keys** — Create, revoke, delete keys with copy-to-clipboard
- **Users** — Invite, manage roles, remove members
- **Analytics** — Usage breakdowns by model and status

## Configuration

### SOLO Mode (default)

No configuration needed. SQLite database is auto-created. Just run:

```bash
opencook serve
```

### TEAM Mode

Set environment variables:

```bash
export DATABASE_URL="postgres://user:pass@localhost:5432/gateway"
export REDIS_URL="redis://localhost:6379"
export GATEWAY_MASTER_KEY="64-char-hex-string"
export GATEWAY_JWT_PRIVATE_KEY="-----BEGIN PRIVATE KEY-----..."
export GATEWAY_JWT_PUBLIC_KEY="-----BEGIN PUBLIC KEY-----..."
```

### Semantic Cache

```bash
export GATEWAY_SEMANTIC_CACHE_ENABLED=true
export GATEWAY_SEMANTIC_CACHE_THRESHOLD=0.95
export EMBEDDING_API_KEY="sk-..."
```

## Docker

```bash
docker-compose -f docker-compose.dev.yml up
```

## Development

```bash
# Run tests
cargo test --workspace

# Run E2E tests
cargo test -p gateway-api --test e2e_auth --test e2e_chat_completion

# Run SOLO mode locally
cargo run --bin opencook -- serve
```

## Architecture

- **Rust** + Axum + Tokio — Async HTTP server
- **PostgreSQL / SQLite** — Dual-database support
- **Redis** — Caching, rate limiting, health status
- **React + Vite** — Admin dashboard

## License

MIT OR Apache-2.0
