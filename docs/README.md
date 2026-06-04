# AI Gateway Documentation

> **Welcome.** This directory contains all project documentation for the AI Gateway — a modular, self-hosted gateway for LLM APIs with unified billing, routing, caching, and observability.

---

## Quick Navigation

| Document | Purpose | Audience |
|----------|---------|----------|
| **[QUICKSTART.md](QUICKSTART.md)** | Get the gateway running in <5 minutes | New users, evaluators |
| **[DEPLOYMENT.md](DEPLOYMENT.md)** | Production deployment options (Docker, K8s, bare metal) | DevOps, SRE |
| **[API.md](API.md)** | API endpoint reference with curl examples | Developers integrating the gateway |
| **[TROUBLESHOOTING.md](TROUBLESHOOTING.md)** | Common issues, error messages, fixes | Operators, on-call |
| **[CHANGELOG.md](CHANGELOG.md)** | Release history and breaking changes | Everyone |
| **[ARCHITECTURE.md](ARCHITECTURE.md)** | System design, principles, component diagrams | Contributors, architects |
| **[API_SPEC.md](API_SPEC.md)** | Full API specification (OpenAI-compatible + Admin) | Developers |
| **[DATABASE.md](DATABASE.md)** | Schema design, migrations, naming conventions | Backend developers |
| **[AUTH.md](AUTH.md)** | Authentication & authorization deep dive | Security reviewers |
| **[SECURITY.md](SECURITY.md)** | Security strategy, threat model, controls | Security engineers |
| **[CACHE.md](CACHE.md)** | Caching architecture (L1/L2, semantic cache) | Backend developers |
| **[OBSERVABILITY.md](OBSERVABILITY.md)** | Metrics, logging, tracing, alerting | Operators |
| **[TECH_STACK.md](TECH_STACK.md)** | Technology choices and versions | New contributors |
| **[CURRENT_STATE.md](CURRENT_STATE.md)** | Snapshot of what's actually built right now | Maintainers |

---

## Document Categories

### 🚀 Getting Started
- [QUICKSTART.md](QUICKSTART.md) — Docker Compose up, first API key, first chat request
- [DEPLOYMENT.md](DEPLOYMENT.md) — Production deployment guides

### 📖 Reference
- [API.md](API.md) — Quick API reference
- [API_SPEC.md](API_SPEC.md) — Complete specification with request/response schemas
- [DATABASE.md](DATABASE.md) — Database schema and migration guide

### 🏗️ Design & Architecture
- [ARCHITECTURE.md](ARCHITECTURE.md) — High-level architecture
- [TECH_STACK.md](TECH_STACK.md) — Stack rationale
- [ADR/](adr/) — Architecture Decision Records

### 🔐 Security
- [SECURITY.md](SECURITY.md) — Security strategy
- [AUTH.md](AUTH.md) — Authentication & authorization
- [THREAT_MODEL.md](THREAT_MODEL.md) — Threat model

### ⚙️ Operations
- [TROUBLESHOOTING.md](TROUBLESHOOTING.md) — Common issues and fixes
- [OBSERVABILITY.md](OBSERVABILITY.md) — Monitoring and observability
- [CACHE.md](CACHE.md) — Caching operational guide

### 📋 Planning
- [ROADMAP.md](ROADMAP.md) — Development roadmap
- [CURRENT_STATE.md](CURRENT_STATE.md) — Implementation status
- [CHANGELOG.md](CHANGELOG.md) — Release history

---

## Project Overview

The AI Gateway is a **single-node-first**, self-hosted gateway that unifies access to multiple LLM providers (OpenAI, Anthropic, Gemini, Ollama) behind a single OpenAI-compatible API. It provides:

- **Unified API** — Drop-in OpenAI compatibility
- **Intelligent Routing** — Privacy-first, balanced, speed, frugal, quality, offline profiles
- **Multi-layer Caching** — In-process (L1) + Redis (L2) with semantic caching
- **Quota & Budget Caps** — Per-organization limits with pre-request cost estimation
- **Observability** — Prometheus metrics, structured logging, request tracing
- **Admin Dashboard** — React-based SPA for provider management, usage analytics, org settings

### Modes

| Mode | Database | Auth | Use Case |
|------|----------|------|----------|
| **TEAM** (gateway-api) | PostgreSQL 15+ | JWT + API keys | Multi-tenant SaaS deployment |
| **SOLO** (gateway-solo) | SQLite | None | Personal/local use |

---

## Repository Layout

```
├── crates/              # Rust workspace crates
│   ├── gateway-api/     # Axum API server (TEAM mode)
│   ├── gateway-solo/    # Standalone server (SOLO mode)
│   ├── gateway-core/    # Routing, caching, orchestration
│   ├── gateway-providers/  # LLM provider implementations
│   ├── gateway-auth/    # JWT, API keys, RBAC
│   ├── gateway-db/      # Database layer (Pg + SQLite)
│   ├── gateway-cache/   # L1/L2 cache implementation
│   ├── gateway-quota/   # Quota & budget enforcement
│   └── gateway-observability/  # Metrics, tracing
├── frontend/            # React + Vite admin dashboard
├── migrations/          # SQLx database migrations
├── docker/              # Dockerfiles
├── docs/                # This directory
└── tasks/               # Task tracking (TASK-*.md)
```

---

## Contributing

1. Read [ARCHITECTURE.md](ARCHITECTURE.md) and [CURRENT_STATE.md](CURRENT_STATE.md)
2. Check the [ROADMAP.md](ROADMAP.md) for planned work
3. Review relevant [ADR](adr/) documents for design context
4. Follow existing code style; Rust code uses `cargo fmt` + `cargo clippy`

---

## Support

- **Issues**: Check [TROUBLESHOOTING.md](TROUBLESHOOTING.md) first
- **Questions**: Review [API_SPEC.md](API_SPEC.md) or [API.md](API.md)
- **Deployment**: See [DEPLOYMENT.md](DEPLOYMENT.md)
