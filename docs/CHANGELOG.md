# Changelog

All notable changes to the AI Gateway project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [1.0.0] — 2026-06-04

### Added

- **Core Gateway** — OpenAI-compatible chat completions API with unified provider routing
- **Multi-Provider Support** — OpenAI, Anthropic, Gemini, and Ollama providers with circuit breaker, retry, and fallback
- **Dual-Database Architecture** — PostgreSQL 16 (TEAM mode) and SQLite (SOLO mode) with `DbBackend` abstraction
- **Authentication** — API key auth (`sk_gw_*` format, 44 chars, SHA-256 stored) and JWT session auth (RS256, 15min access / 7day refresh)
- **RBAC** — 4 roles (owner, admin, member, viewer) with 31 granular permissions
- **Rate Limiting** — 6 layers: global, org, key requests, key tokens, provider, IP via Redis Lua scripts
- **Quota & Budget Caps** — Pre-request cost estimation against configurable limits (requests, tokens, cost_usd) with block/warn actions
- **Multi-Layer Caching** — In-process L1 (`moka`, 60s TTL, 10K entries) + Redis L2 with semantic caching support
- **Observability** — Prometheus metrics (`/metrics`), structured JSON logging (`tracing`), request timing headers
- **Admin Dashboard** — React 18 + Vite + Tailwind CSS + shadcn/ui SPA with:
  - Responsive sidebar layout with dark mode
  - Dashboard overview with KPIs and recent requests table
  - Provider list, add/edit wizard, and detail pages with health charts
  - Organization settings and user management
  - Static file serving from Axum backend at `/admin/*` with SPA fallback
- **SOLO Mode** — Standalone `gateway-solo` binary with SQLite, no auth, interactive config wizard, and user-configurable quotas
- **E2E Integration Tests** — 13 tests across 4 suites using SQLite in-memory, Redis, and Wiremock mock providers
- **Frontend Tests** — 34 tests across 7 test files using Vitest + jsdom + Testing Library
- **Rust Unit Tests** — 10 tests across backend crates
- **Docker Compose Dev Environment** — PostgreSQL, Redis, backend (cargo-watch), frontend (Vite HMR)
- **Migrations** — 40+ SQLx migrations for TEAM mode schema

### Architecture

- 9 Rust workspace crates: `gateway-api`, `gateway-core`, `gateway-providers`, `gateway-cache`, `gateway-quota`, `gateway-auth`, `gateway-db`, `gateway-observability`, `gateway-solo`
- Axum 0.7 + tower-http 0.5 middleware stack: CORS → body limit → trace → rate limit → auth → handler
- SSE streaming support with `LoggingStream` wrapper for DB persistence
- Dynamic model registry with static fallback
- Request-scoped tenant isolation via `org_id`

### Security

- Argon2id password hashing with zxcvbn strength checking
- API key format with CRC32C checksum for typo detection
- Tenant-scoped queries with `org_id` filtering
- SSRF protection via URL whitelist
- TLS 1.2+ recommended for production (see [DEPLOYMENT.md](DEPLOYMENT.md))

### DevEx

- Hot-reload backend with `cargo-watch`
- Vite HMR frontend dev server with proxy to backend
- sqlx compile-time SQL checking
- `cargo fmt` + `cargo clippy` enforced

### Known Limitations

- **API Key Middleware** — Currently validates format only (no DB lookup); uses `DEFAULT_ORG_ID`. Full verification planned for v1.1.
- **Session Auth** — Dashboard login flow is stubbed; JWT sessions work but user registration API is minimal.
- **Webhook Events** — Schema defined but not yet wired to event dispatch.
- **Semantic Cache** — Architecture designed; L1/L2 operational but semantic similarity matching not yet implemented.
- **Ollama Health Checks** — Relies on `/api/tags` endpoint; may not reflect model-specific availability.
- **E2E Tests** — Must run with `--test-threads=1` due to env var interference (`OPENAI_BASE_URL`, `OPENAI_API_KEY`).
- **RBAC Enforcement** — Roles and permissions are defined but middleware enforcement is partial.

---

## [Unreleased]

### Security

- **SSRF Protection** — Webhook and provider base URLs are validated against private IP ranges (RFC 1918), loopback, link-local, multicast, and well-known internal hostnames (`localhost`). Only `http`/`https` schemes are allowed.
- **OIDC CSRF Protection** — `/api/v1/auth/oidc/authorize` now generates a cryptographically random 256-bit state nonce stored in Redis (10-min TTL). `/api/v1/auth/oidc/callback` verifies the state and deletes it after one-time use.
- **SAML CSRF Protection** — New `/api/v1/auth/saml/authorize` endpoint generates a random RelayState stored in Redis. `/api/v1/auth/saml/acs` verifies RelayState before processing the SAML response.
- **SSO Admin RBAC** — `GET/POST /organizations/:org_id/sso` and `DELETE /organizations/:org_id/sso/:provider_type` now require `SettingsRead` / `SettingsWrite` permissions.
- **Cross-Organization Access Control** — Added `auth.org_id != org_id` checks to quotas, usage, and SSO admin endpoints to prevent cross-tenant data access.

### Changed

- **Zero Clippy Warnings** — `cargo clippy --workspace --all-targets --all-features` now passes with 0 warnings. This includes boxing `ApiError` in helper functions to satisfy `result_large_err` lint and refactoring `gateway-solo` into a library with separate binary entry points.
- **SSO Redirects** — SAML ACS and OIDC callback no longer hardcode `/`; they redirect to the first configured `allowed_origins` URL.

### Planned

- Full API key DB verification middleware
- Dashboard user registration and login flow
- Webhook event dispatch
- Semantic cache implementation
- Billing integration (Stripe)
- Horizontal scaling guide
- Grafana dashboards
- OpenAPI / Swagger documentation

---

## Release Notes Format

```
## [X.Y.Z] — YYYY-MM-DD

### Added
- New features

### Changed
- Changes in existing functionality

### Deprecated
- Soon-to-be removed features

### Removed
- Removed features

### Fixed
- Bug fixes

### Security
- Security improvements
```
