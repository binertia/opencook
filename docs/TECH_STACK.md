# AI Gateway - Technology Stack & Deployment Document

> **Product**: AI Gateway - Modular monolith AI gateway on a single VPS
> **Target**: Deploy in <10 minutes by a non-DevOps person
> **Last Updated**: 2025-01-14

---

## Table of Contents

1. [Technology Stack](#1-technology-stack)
2. [Development Environment](#2-development-environment)
3. [Production Deployment](#3-production-deployment)
4. [Environment Configuration](#4-environment-configuration)
5. [Resource Requirements](#5-resource-requirements)
6. [Health Checks & Readiness](#6-health-checks--readiness)
7. [Operational Runbooks](#7-operational-runbooks)
8. [Security Defaults](#8-security-defaults)

---

## 1. Technology Stack

### 1.1 Backend Stack

| Component | Choice | Version | Purpose |
|-----------|--------|---------|---------|
| Language | Rust | 1.78+ (MSRV) | High-performance, memory-safe systems language |
| Web Framework | axum | 0.7 | HTTP server, routing, middleware, WebSocket support |
| HTTP Client | reqwest | 0.12 | Outbound HTTP requests to LLM provider APIs |
| Async Runtime | tokio | 1.37 | Async runtime - multi-thread scheduler |
| Database Driver | sqlx | 0.7 | Compile-time checked SQL with connection pooling |
| Redis Client | redis (tokio-comp) | 0.25 | Async Redis operations |
| Serialization | serde | 1.0 | JSON serialization/deserialization |
| Config | envy + figment | 0.4 / 0.10 | Environment-variable configuration with validation |
| Logging | tracing + tracing-subscriber | 0.1 | Structured logging with JSON output in production |
| Auth | jsonwebtoken | 9.3 | JWT token generation and validation |
| Validation | validator | 0.18 | Input validation (emails, URLs, ranges) |
| Testing | tokio-test + httptest | 0.5 / 0.15 | Async test runtime and HTTP mocking |
| Password Hashing | argon2 | 0.5 | Secure password hashing (OWASP recommended) |
| Process Manager | cargo-watch | 8.5 | Hot reload during development |
| Build Tool | cargo + rustc | 1.78 | Native Rust build toolchain |
| OpenAPI | utoipa | 4.2 | Auto-generated OpenAPI/Swagger documentation |

#### Backend Decision Details

**Rust 1.78+**
- **What**: Systems programming language with zero-cost abstractions and memory safety guarantees.
- **Why chosen**: Maximum performance for I/O-bound gateway workloads (proxying LLM requests); memory safety eliminates entire classes of production bugs; single binary deployment artifact.
- **Alternatives rejected**:
  - *Go*: Lacks Rust's zero-cost async and compile-time safety guarantees; larger binary, less memory control.
  - *Node.js*: Single-threaded event loop becomes bottleneck under high concurrency; memory overhead unacceptable for VPS constraints.
  - *Python*: GIL limits concurrency; memory footprint too high for single-VPS deployment.

**axum 0.7**
- **What**: Ergonomic, modular web framework built on tokio and hyper.
- **Why chosen**: Native tokio integration; tower middleware ecosystem (rate limiting, timeout, compression); excellent WebSocket support for LLM streaming responses; built by Tokio team.
- **Alternatives rejected**:
  - *actix-web*: Excellent performance but uses its own actor runtime; less ergonomic middleware composition.
  - *rocket*: Requires nightly Rust; heavier runtime with less async-native design.

**reqwest 0.12**
- **What**: High-level HTTP client with async support, connection pooling, and timeout handling.
- **Why chosen**: Built on hyper; handles connection reuse to LLM APIs automatically; streaming response body support for SSE.
- **Alternatives rejected**:
  - *hyper (raw)*: Too low-level; requires manual connection management.
  - *awc (actix-web client)*: Tied to actix runtime.

**tokio 1.37 (multi-thread)**
- **What**: Rust's premier async runtime with work-stealing scheduler.
- **Why chosen**: Industry standard; axum's native runtime; handles thousands of concurrent connections efficiently.
- **Alternatives rejected**:
  - *async-std*: Less ecosystem support; fewer integrations.
  - *smol*: Good for embedded, insufficient for high-throughput gateway.

**sqlx 0.7**
- **What**: Async SQL toolkit with compile-time query verification.
- **Why chosen**: Compile-time checked SQL catches schema mismatches at build time; no ORM overhead; dead-simple connection pooling via `sqlx::Pool`.
- **Alternatives rejected**:
  - *diesel*: Sync-only; heavy compile times; ORM-centric design adds complexity.
  - *sea-orm*: Full ORM with migration overhead; unnecessary abstraction layer for gateway queries.
  - *tokio-postgres (raw)*: No compile-time checking; more boilerplate.

**redis (tokio-comp) 0.25**
- **What**: Official Redis client with tokio compatibility.
- **Why chosen**: Native async; multiplexed connections; supports Redis Cluster if needed later.
- **Alternatives rejected**:
  - *fred*: Good but less documentation; smaller community.
  - *bb8-redis*: Just a pool wrapper, not a full client.

**serde 1.0**
- **What**: Serialization framework for Rust.
- **Why chosen**: Universal standard; derive macros eliminate boilerplate; supports JSON, YAML, TOML.
- **Alternatives rejected**: *None* - serde is the undisputed standard.

**envy + figment 0.4 / 0.10**
- **What**: envy parses env vars into structs; figment provides layered config (env > file > defaults).
- **Why chosen**: Type-safe config parsing at startup; clear error messages for missing vars; supports `.env` files in development.
- **Alternatives rejected**:
  - *dotenvy alone*: No type safety, manual parsing required.
  - *config-rs*: Heavier, more complex than needed for single-VPS deployment.

**tracing + tracing-subscriber 0.1**
- **What**: Structured, contextual logging framework.
- **Why chosen**: Structured JSON logs in production; distributed tracing support for future; log levels per module.
- **Alternatives rejected**:
  - *log crate*: Unstructured, no context propagation.
  - *slog*: More complex API; tracing is the modern standard.

**jsonwebtoken 9.3**
- **What**: JWT encode/decode library.
- **Why chosen**: Pure Rust, no OpenSSL dependency; supports RS256/HS256; actively maintained.
- **Alternatives rejected**: *None* - clear leader in the Rust JWT space.

**validator 0.18**
- **What**: Input validation with derive macros.
- **Why chosen**: Declarative validation (`#[validate(email)]`, `#[validate(length(min = 1))]`); integrates with axum extractors.
- **Alternatives rejected**: *garde*: Newer, less ecosystem maturity.

**utoipa 4.2**
- **What**: OpenAPI documentation generator with Swagger UI.
- **Why chosen**: Derive macros generate docs from code; built-in Scalar UI; always in sync with implementation.
- **Alternatives rejected**: *aide*: More complex, requires handler wrapping.

---

### 1.2 Frontend Stack

| Component | Choice | Version | Purpose |
|-----------|--------|---------|---------|
| Framework | React | 18.3 | UI component library |
| Language | TypeScript | 5.4 | Type-safe JavaScript |
| Bundler | Vite | 5.2 | Fast dev server and production build |
| UI Components | shadcn/ui | latest | Accessible, customizable component primitives |
| Component Base | Radix UI | 1.0 | Headless, accessible UI primitives |
| Styling | Tailwind CSS | 3.4 | Utility-first CSS |
| State Management | Zustand | 4.5 | Lightweight global state |
| Server State | TanStack Query | 5.28 | Server data fetching, caching, synchronization |
| Routing | React Router | 6.22 | Client-side routing |
| Charts | Recharts | 2.12 | Cost analytics and usage dashboards |
| Forms | React Hook Form | 7.51 | Performant form handling |
| Validation | Zod | 3.22 | Schema validation (shared with backend) |
| HTTP Client | ky | 1.2 | Modern fetch wrapper with retry logic |
| Icons | Lucide React | 0.356 | Consistent icon set |
| Notifications | Sonner | 1.4 | Toast notifications |

#### Frontend Decision Details

**React 18 + TypeScript 5.4**
- **What**: Component-based UI library with type safety.
- **Why chosen**: Largest ecosystem; single-page admin dashboard fits React's model well; Strict Mode catches issues early.
- **Alternatives rejected**:
  - *Vue 3*: Good but smaller type-safe ecosystem; team's expertise favors React.
  - *Svelte*: Less mature testing tooling; smaller package ecosystem.
  - *Solid*: Excellent performance but too niche for team familiarity.

**Vite 5.2**
- **What**: ES Modules-based dev server and bundler.
- **Why chosen**: Dev server starts in <300ms; HMR nearly instant; simpler config than webpack; no hidden bundler complexity.
- **Alternatives rejected**:
  - *webpack*: Complex config; slower HMR; requires extensive plugin ecosystem knowledge.
  - *esbuild (direct)*: Fast but lacks dev server and HMR.
  - *Turbopack*: Still maturing; Vite is proven and stable.

**shadcn/ui + Radix UI**
- **What**: Copy-pasteable components built on Radix primitives.
- **Why chosen**: Full ownership of component code (no npm dependency); Tailwind-native styling; accessible out of the box.
- **Alternatives rejected**:
  - *MUI*: Heavy bundle size; theming complexity; opinionated design.
  - *Chakra UI*: Good but v3 migration churn; heavier runtime.
  - *Ant Design*: Too opinionated for gateway admin UI; large bundle.

**Zustand 4.5**
- **What**: Minimal state management with hooks API.
- **Why chosen**: No providers needed; 1KB bundle; TypeScript-native; avoids Redux boilerplate for simple admin state.
- **Alternatives rejected**:
  - *Redux Toolkit*: Excessive boilerplate for gateway admin needs.
  - *Jotai*: Atom-based model unnecessary for our flat state shape.
  - *React Context*: Performance issues with frequent updates.

**TanStack Query 5.28**
- **What**: Server state management with caching, deduping, and background refetching.
- **Why chosen**: Eliminates manual fetch/useEffect patterns; automatic cache invalidation; built-in error handling and retries.
- **Alternatives rejected**:
  - *SWR*: Similar but TanStack Query has better TypeScript support and devtools.
  - *RTK Query*: Tied to Redux; unnecessary coupling.

**Recharts 2.12**
- **What**: Composable charting library built on D3.
- **Why chosen**: React-native API; sufficient for cost/usage dashboards; smaller bundle than full D3.
- **Alternatives rejected**:
  - *D3 (direct)*: Too low-level; steep learning curve.
  - *Chart.js*: Imperative API; React wrapper adds complexity.
  - *Victory*: Heavier; more suited for data science dashboards.

**ky 1.2**
- **What**: Lightweight HTTP client built on fetch.
- **Why chosen**: Smaller than axios; fetch-based (no XMLHttpRequest); built-in retry and timeout; TypeScript-native.
- **Alternatives rejected**:
  - *axios*: Larger bundle; XMLHttpRequest-based; unnecessary feature overlap.
  - *fetch (raw)*: No timeout/retry; verbose error handling.

---

### 1.3 Database Stack

| Component | Choice | Version | Purpose |
|-----------|--------|---------|---------|
| Database | PostgreSQL | 16 | Primary data store |
| Migration Tool | sqlx-cli + sqlx-migrate | 0.7 | Schema migrations with compile-time verification |
| Connection Pool | sqlx::Pool (built-in) | 0.7 | Async connection pooling |
| Admin Access | pgAdmin or psql | - | Direct database inspection |

#### Database Decision Details

**PostgreSQL 16**
- **What**: Production-grade open-source relational database.
- **Why chosen**: ACID compliance for financial/billing data; JSONB for flexible metadata; proven at scale; excellent Docker support.
- **Alternatives rejected**:
  - *SQLite*: Insufficient for concurrent gateway workloads; no built-in replication/backup tooling.
  - *MySQL 8*: Comparable but PostgreSQL has superior JSON support and stricter data integrity.
  - *CockroachDB*: Overkill for single-node deployment.

**sqlx-cli (migrate)**
- **What**: Migration runner integrated with sqlx's compile-time checking.
- **Why chosen**: Migrations are plain SQL (no lock-in); compile-time verification catches schema drift; reversible migrations.
- **Alternatives rejected**:
  - *Flyway*: Java dependency; adds container bloat.
  - *Liquibase*: XML/YAML-based; heavier than needed.
  - *refinery*: Good but less integration with sqlx's query checking.

---

### 1.4 Cache Stack

| Component | Choice | Version | Purpose |
|-----------|--------|---------|---------|
| Cache | Redis | 7.2 | In-memory data store |
| Persistence | RDB + AOF | - | Data durability with configurable trade-offs |

#### Redis Use Cases

| Use Case | Key Pattern | TTL | Reason |
|----------|-------------|-----|--------|
| Response Cache | `cache:{provider}:{hash}` | 1-5 min | Avoid redundant LLM API calls for identical prompts |
| Rate Limit Counters | `ratelimit:{key}:{window}` | Window duration | Sliding window rate limiting |
| Session Store | `session:{token}` | 24 hours | Authenticated user sessions |
| Provider Status | `status:{provider}` | 30 sec | Cached health check results |
| Metrics Buffer | `metrics:{minute}` | 1 hour | Batched metrics before DB write |

#### Redis Decision Details

**Redis 7.2**
- **What**: In-memory key-value store with persistence options.
- **Why chosen**: Sub-millisecond operations; native data structures (strings, hashes, sorted sets); battle-tested; excellent Rust client support.
- **Alternatives rejected**:
  - *Memcached*: No persistence; fewer data types; less operational tooling.
  - *KeyDB*: Interesting multi-threading but smaller community.
  - *Valkey (AWS fork)*: Too new; ecosystem still maturing.

---

### 1.5 Infrastructure Stack

| Component | Choice | Version | Purpose |
|-----------|--------|---------|---------|
| Container Runtime | Docker Engine | 26.0 | Container packaging and execution |
| Orchestration | Docker Compose | 2.27 | Single-node multi-container management |
| Reverse Proxy | Caddy | 2.8 | Automatic HTTPS, reverse proxy, static file serving |
| SSL/TLS | Let's Encrypt (via Caddy) | - | Free, automated certificate provisioning |
| OS | Ubuntu LTS | 24.04 | Server operating system |
| Process Supervision | systemd | - | Docker daemon and host-level process management |

#### Infrastructure Decision Details

**Docker + Docker Compose**
- **What**: Container platform with declarative multi-container configuration.
- **Why chosen**: `docker compose up -d` is the simplest possible deployment; reproducible across environments; no Kubernetes complexity.
- **Alternatives rejected**:
  - *Podman + podman-compose*: Good but less documentation; Compose support lags.
  - *Kubernetes*: Massive overkill for single VPS; requires dedicated expertise.
  - *Nomad*: HashiCorp ecosystem; more complex than Compose.

**Caddy 2.8**
- **What**: Modern reverse proxy with automatic HTTPS.
- **Why chosen**: Automatic Let's Encrypt (no certbot needed); Caddyfile is readable; HTTP/3 support; WebSocket passthrough works out of the box.
- **Alternatives rejected**:
  - *nginx*: Requires manual SSL cert management; config syntax error-prone; needs certbot companion.
  - *Traefik*: Good but overkill for single-node; Docker socket exposure is a security risk.
  - *Apache*: Legacy; memory-heavy; unnecessary feature set.

**Let's Encrypt**
- **What**: Free certificate authority.
- **Why chosen**: Zero cost; Caddy handles entire lifecycle (request, renew, auto-reload); trusted by all browsers.
- **Alternatives rejected**:
  - *Buy certificates*: Unnecessary cost for this use case.
  - *self-signed*: Browser warnings unacceptable for production.

---

## 2. Development Environment

### 2.1 Prerequisites

```
# Required
Docker Engine 26.0+    (docker --version)
Docker Compose 2.27+   (docker compose version)
Rust 1.78+             (rustc --version)
Node.js 20+            (node --version)
pnpm 8+                (pnpm --version)
sqlx-cli 0.7+          (cargo install sqlx-cli)
just 1.25+             (just --version)  [optional task runner]
```

### 2.2 Repository Structure

```
ai-gateway/
├── backend/                    # Rust workspace
│   ├── Cargo.toml              # Workspace definition
│   ├── migrations/             # sqlx migration files
│   │   ├── 0001_init.sql
│   │   └── 0002_add_users.sql
│   └── src/
│       ├── main.rs             # Application entry point
│       ├── config.rs           # Environment configuration
│       ├── routes/             # API route handlers
│       ├── models/             # Database models
│       ├── middleware/         # Auth, rate limiting, logging
│       └── providers/          # LLM provider integrations
├── frontend/                   # React + TypeScript
│   ├── package.json
│   ├── vite.config.ts
│   ├── tsconfig.json
│   └── src/
│       ├── main.tsx
│       ├── App.tsx
│       ├── components/
│       ├── pages/
│       ├── hooks/
│       └── stores/
├── docker/
│   ├── Dockerfile.backend
│   ├── Dockerfile.frontend
│   └── Caddyfile
├── docker-compose.yml          # Production compose file
├── docker-compose.dev.yml      # Development compose file
├── .env.example                # Template environment variables
├── .env                        # Local environment (gitignored)
└── scripts/
    ├── backup.sh               # Database backup script
    ├── restore.sh              # Database restore script
    └── setup.sh                # Initial VPS setup
```

### 2.3 Development Docker Compose

```yaml
# docker-compose.dev.yml
version: "3.8"

services:
  postgres:
    image: postgres:16-alpine
    container_name: ag-postgres
    environment:
      POSTGRES_USER: aigateway
      POSTGRES_PASSWORD: devpassword
      POSTGRES_DB: aigateway_dev
    ports:
      - "5432:5432"
    volumes:
      - postgres_dev_data:/var/lib/postgresql/data
    healthcheck:
      test: ["CMD-SHELL", "pg_isready -U aigateway -d aigateway_dev"]
      interval: 5s
      timeout: 5s
      retries: 5

  redis:
    image: redis:7.2-alpine
    container_name: ag-redis
    ports:
      - "6379:6379"
    command: redis-server --appendonly yes
    volumes:
      - redis_dev_data:/data
    healthcheck:
      test: ["CMD", "redis-cli", "ping"]
      interval: 5s
      timeout: 3s
      retries: 5

  backend:
    build:
      context: ./backend
      dockerfile: ../docker/Dockerfile.backend.dev
    container_name: ag-backend
    environment:
      DATABASE_URL: postgres://aigateway:devpassword@postgres:5432/aigateway_dev
      REDIS_URL: redis://redis:6379
      RUST_LOG: debug
      RUST_BACKTRACE: 1
    ports:
      - "8080:8080"
    volumes:
      - ./backend:/app
      - cargo_cache:/usr/local/cargo/registry
      - target_cache:/app/target
    depends_on:
      postgres:
        condition: service_healthy
      redis:
        condition: service_healthy
    command: cargo watch -x run

  frontend:
    build:
      context: ./frontend
      dockerfile: ../docker/Dockerfile.frontend.dev
    container_name: ag-frontend
    ports:
      - "5173:5173"
    volumes:
      - ./frontend:/app
      - node_modules:/app/node_modules
    environment:
      VITE_API_URL: http://localhost:8080
    command: pnpm dev --host

volumes:
  postgres_dev_data:
  redis_dev_data:
  cargo_cache:
  target_cache:
  node_modules:
```

### 2.4 Development Startup

```bash
# 1. Clone and enter repository
git clone <repo-url> && cd ai-gateway

# 2. Copy environment template
cp .env.example .env

# 3. Start infrastructure (Postgres + Redis)
docker compose -f docker-compose.dev.yml up -d postgres redis

# 4. Run migrations
cd backend && cargo sqlx migrate run

# 5. Start backend (with hot reload)
cargo watch -x run

# 6. In another terminal, start frontend
cd frontend && pnpm install && pnpm dev

# 7. Open browser
# Frontend: http://localhost:5173
# API Docs:  http://localhost:8080/docs
# API:       http://localhost:8080
```

### 2.5 Hot Reload Configuration

**Backend (cargo-watch)**:
```bash
# Watches src/ for changes, recompiles and restarts
cargo watch -q -c -w src/ -x run

# -q: quiet
# -c: clear screen on restart
# -w: watch directory
# -x: execute command
```

**Frontend (Vite HMR)**:
```typescript
// vite.config.ts - HMR is enabled by default
export default defineConfig({
  server: {
    port: 5173,
    host: true,           # Allow external connections
    strictPort: true,     # Fail if port in use
    proxy: {
      '/api': {
        target: 'http://localhost:8080',
        changeOrigin: true,
      },
    },
  },
});
```

### 2.6 Test Database Setup

```bash
# Create isolated test database
docker exec ag-postgres psql -U aigateway -c "CREATE DATABASE aigateway_test;"

# Run migrations against test DB
cd backend && DATABASE_URL=postgres://aigateway:devpassword@localhost:5432/aigateway_test cargo sqlx migrate run

# Run tests
cd backend && cargo test

# Integration tests use a separate test harness that spins up
# testcontainers (ephemeral Postgres/Redis instances)
```

### 2.7 Required Environment Variables (Development)

| Variable | Value | Description |
|----------|-------|-------------|
| `DATABASE_URL` | `postgres://aigateway:devpassword@localhost:5432/aigateway_dev` | Postgres connection string |
| `REDIS_URL` | `redis://localhost:6379` | Redis connection string |
| `RUST_LOG` | `debug` | Log level (trace/debug/info/warn/error) |
| `JWT_SECRET` | `dev-secret-change-in-production` | JWT signing key |
| `OPENAI_API_KEY` | (your key) | OpenAI API key for testing |
| `VITE_API_URL` | `http://localhost:8080` | Frontend API base URL |

---

## 3. Production Deployment

### 3.1 Target Environments

| Priority | Environment | Infrastructure | Notes |
|----------|-------------|----------------|-------|
| 1 (Primary) | Single VPS | DigitalOcean Droplet / Hetzner Cloud / Linode | <10 min deployment target |
| 2 (Secondary) | Self-hosted | Customer's own Linux server | Same Docker Compose stack |

**Recommended VPS Providers**:
- **Hetzner Cloud**: Best price/performance (CPX31: 4 vCPU, 8GB RAM, ~EUR 12/mo)
- **DigitalOcean**: Good documentation, managed backups (Basic Droplet: 4 vCPU, 8GB RAM, ~$48/mo)
- **Linode**: Reliable, good support (Shared 8GB: 4 vCPU, 8GB RAM, ~$48/mo)

### 3.2 Docker Compose Production Configuration

```yaml
# docker-compose.yml - Production
version: "3.8"

services:
  # ── Database ──────────────────────────────────────
  postgres:
    image: postgres:16-alpine
    container_name: ag-postgres
    restart: unless-stopped
    environment:
      POSTGRES_USER: ${DB_USER}
      POSTGRES_PASSWORD: ${DB_PASSWORD}
      POSTGRES_DB: ${DB_NAME}
      PGDATA: /var/lib/postgresql/data/pgdata
    volumes:
      - postgres_data:/var/lib/postgresql/data/pgdata
      - ./backups:/backups
    ports: []  # Not exposed externally
    networks:
      - aigateway
    healthcheck:
      test: ["CMD-SHELL", "pg_isready -U ${DB_USER} -d ${DB_NAME}"]
      interval: 10s
      timeout: 5s
      retries: 5
      start_period: 10s
    deploy:
      resources:
        limits:
          memory: 1G

  # ── Cache ──────────────────────────────────────────
  redis:
    image: redis:7.2-alpine
    container_name: ag-redis
    restart: unless-stopped
    command: >
      redis-server
      --appendonly yes
      --appendfsync everysec
      --maxmemory 512mb
      --maxmemory-policy allkeys-lru
      --requirepass ${REDIS_PASSWORD}
    volumes:
      - redis_data:/data
    ports: []  # Not exposed externally
    networks:
      - aigateway
    healthcheck:
      test: ["CMD", "redis-cli", "-a", "${REDIS_PASSWORD}", "ping"]
      interval: 10s
      timeout: 5s
      retries: 5
      start_period: 5s
    deploy:
      resources:
        limits:
          memory: 512M

  # ── Backend API ────────────────────────────────────
  backend:
    build:
      context: ./backend
      dockerfile: ../docker/Dockerfile.backend
    container_name: ag-backend
    restart: unless-stopped
    environment:
      DATABASE_URL: postgres://${DB_USER}:${DB_PASSWORD}@postgres:5432/${DB_NAME}
      REDIS_URL: redis://:${REDIS_PASSWORD}@redis:6379
      JWT_SECRET: ${JWT_SECRET}
      JWT_EXPIRY_HOURS: ${JWT_EXPIRY_HOURS:-24}
      RUST_LOG: ${RUST_LOG:-info}
      RUST_BACKTRACE: 0
      APP_ENV: production
      PORT: 8080
      # LLM Provider Keys (injected at runtime)
      OPENAI_API_KEY: ${OPENAI_API_KEY:-}
      ANTHROPIC_API_KEY: ${ANTHROPIC_API_KEY:-}
      COHERE_API_KEY: ${COHERE_API_KEY:-}
      # Rate Limiting
      RATE_LIMIT_RPM: ${RATE_LIMIT_RPM:-60}
      RATE_LIMIT_BURST: ${RATE_LIMIT_BURST:-10}
      # Observability
      LOG_FORMAT: json
      METRICS_ENABLED: ${METRICS_ENABLED:-true}
    ports: []  # Accessed via Caddy reverse proxy
    networks:
      - aigateway
    depends_on:
      postgres:
        condition: service_healthy
      redis:
        condition: service_healthy
    healthcheck:
      test: ["CMD", "wget", "--no-verbose", "--tries=1", "--spider", "http://localhost:8080/health"]
      interval: 15s
      timeout: 5s
      retries: 3
      start_period: 30s
    deploy:
      resources:
        limits:
          memory: 512M

  # ── Frontend (static files) ────────────────────────
  frontend:
    build:
      context: ./frontend
      dockerfile: ../docker/Dockerfile.frontend
    container_name: ag-frontend
    restart: unless-stopped
    environment:
      VITE_API_URL: /api  # Relative URL - proxied through Caddy
    networks:
      - aigateway
    # Static files served by Caddy; no ports exposed

  # ── Reverse Proxy ──────────────────────────────────
  caddy:
    image: caddy:2.8-alpine
    container_name: ag-caddy
    restart: unless-stopped
    ports:
      - "80:80"
      - "443:443"
    volumes:
      - ./docker/Caddyfile:/etc/caddy/Caddyfile:ro
      - caddy_data:/data
      - caddy_config:/config
      - frontend_dist:/srv/frontend:ro  # Static files volume
    networks:
      - aigateway
    depends_on:
      - backend
      - frontend
    deploy:
      resources:
        limits:
          memory: 128M

  # ── Backup Scheduler ───────────────────────────────
  backup:
    image: postgres:16-alpine
    container_name: ag-backup
    restart: unless-stopped
    environment:
      POSTGRES_USER: ${DB_USER}
      POSTGRES_PASSWORD: ${DB_PASSWORD}
      POSTGRES_DB: ${DB_NAME}
      POSTGRES_HOST: postgres
    volumes:
      - ./backups:/backups
      - ./scripts/backup.sh:/backup.sh:ro
    entrypoint: >
      sh -c 'echo "0 2 * * * /backup.sh" | crontab - && crond -f'
    networks:
      - aigateway
    depends_on:
      postgres:
        condition: service_healthy

volumes:
  postgres_data:
  redis_data:
  caddy_data:
  caddy_config:
  frontend_dist:
    # Built frontend output shared with Caddy
    driver: local

networks:
  aigateway:
    driver: bridge
    ipam:
      config:
        - subnet: 172.20.0.0/16
```

### 3.3 Dockerfile - Backend

```dockerfile
# docker/Dockerfile.backend
# ── Builder Stage ────────────────────────────────────
FROM rust:1.78-slim-bookworm AS builder

WORKDIR /app

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Install sqlx-cli for migrations
RUN cargo install sqlx-cli --no-default-features --features native-tls,postgres

# Cache dependencies layer
COPY backend/Cargo.toml backend/Cargo.lock ./
RUN mkdir src && echo "fn main() {}" > src/main.rs
RUN cargo build --release && rm -rf src

# Build application
COPY backend/ .
RUN cargo sqlx migrate run --database-url "${DATABASE_URL}" || true
RUN cargo build --release

# ── Runtime Stage ────────────────────────────────────
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    ca-certificates \
    wget \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=builder /app/target/release/ai-gateway /app/ai-gateway
COPY --from=builder /usr/local/cargo/bin/sqlx /usr/local/bin/sqlx
COPY backend/migrations ./migrations

EXPOSE 8080

# Run migrations then start
CMD ["sh", "-c", "sqlx migrate run --database-url \"$DATABASE_URL\" && ./ai-gateway"]
```

### 3.4 Dockerfile - Frontend

```dockerfile
# docker/Dockerfile.frontend
FROM node:20-slim AS builder

WORKDIR /app

# Install pnpm
RUN corepack enable && corepack prepare pnpm@8 --activate

COPY frontend/package.json frontend/pnpm-lock.yaml ./
RUN pnpm install --frozen-lockfile

COPY frontend/ .
RUN pnpm build

# Output goes to /app/dist
# Caddy serves this from the volume
```

### 3.5 Caddyfile (Reverse Proxy Configuration)

```
# docker/Caddyfile
{
    auto_https {
        # Production: use Let's Encrypt
        # For local testing, Caddy generates local certs automatically
    }
    email {$ACME_EMAIL:admin@example.com}
}

# Frontend - static files and SPA routing
{$DOMAIN:localhost} {
    # Health check endpoint (bypass logging)
    handle /health {
        respond "ok" 200
    }

    # API requests → backend
    handle /api/* {
        reverse_proxy backend:8080 {
            # WebSocket support for streaming responses
            header_up Connection {>Connection}
            header_up Upgrade {>Upgrade}

            # Timeouts for LLM requests (can be long)
            transport http {
                read_timeout 5m
                write_timeout 5m
            }
        }
    }

    # Static frontend files
    handle {
        root * /srv/frontend
        try_files {path} /index.html
        file_server
        encode gzip zstd
    }

    # Logging
    log {
        output file /data/access.log {
            roll_size 100MiB
            roll_keep 5
            roll_keep_for 30d
        }
        format json
    }

    # Security headers
    header {
        Strict-Transport-Security "max-age=31536000; includeSubDomains; preload"
        X-Content-Type-Options "nosniff"
        X-Frame-Options "DENY"
        Referrer-Policy "strict-origin-when-cross-origin"
        Permissions-Policy "camera=(), microphone=(), geolocation=()"
    }
}
```

### 3.6 Service Dependencies & Startup Order

```
Startup Sequence:
1. Network layer (aigateway bridge) - created first
2. postgres      - with healthcheck (pg_isready)
3. redis         - with healthcheck (PING)
4. backend       - waits for postgres + redis healthy; runs migrations; starts API
5. frontend      - build completes; static output to volume
6. caddy         - waits for backend + frontend; starts reverse proxy
7. backup        - waits for postgres; starts cron scheduler

Internal Communication:
  Caddy (80/443) → backend:8080     (HTTP API)
  Caddy (80/443) → frontend:static  (static files)
  backend        → postgres:5432    (SQL queries)
  backend        → redis:6379       (cache operations)
  backup         → postgres:5432    (pg_dump)
```

### 3.7 Port Mappings

| Port | Protocol | Service | Description |
|------|----------|---------|-------------|
| 80 | TCP | Caddy | HTTP (redirects to HTTPS) |
| 443 | TCP | Caddy | HTTPS (production traffic) |
| 8080 | TCP | Backend | Internal API (not exposed externally) |
| 5432 | TCP | Postgres | Internal database (not exposed) |
| 6379 | TCP | Redis | Internal cache (not exposed) |

**No external ports except 80/443.** All internal services communicate on the Docker bridge network.

### 3.8 Volume Mounts & Persistence

| Volume | Service | Path | Contents | Backup |
|--------|---------|------|----------|--------|
| `postgres_data` | postgres | `/var/lib/postgresql/data` | All application data | Daily pg_dump |
| `redis_data` | redis | `/data` | AOF persistence | Included in PG backup |
| `caddy_data` | caddy | `/data` | TLS certificates, config | Let Caddy regenerate |
| `caddy_config` | caddy | `/config` | Caddy runtime config | Ephemeral |
| `./backups` | host | `./backups` | Dump files | Copied offsite |

### 3.9 Network Configuration

```yaml
# Internal Docker network
networks:
  aigateway:
    driver: bridge
    subnet: 172.20.0.0/16
    gateway: 172.20.0.1

# Service IPs (dynamic via Docker DNS):
#   postgres → resolves to 172.20.0.x
#   redis    → resolves to 172.20.0.x
#   backend  → resolves to 172.20.0.x
#   caddy    → resolves to 172.20.0.x
```

### 3.10 Database Operations

#### Backup Strategy

```bash
# scripts/backup.sh - runs daily at 02:00 via cron
#!/bin/bash
set -euo pipefail

BACKUP_DIR="/backups"
DATE=$(date +%Y%m%d_%H%M%S)
DB_NAME="${POSTGRES_DB}"
RETENTION_DAYS=7

# Create backup
pg_dump \
  --host="${POSTGRES_HOST}" \
  --username="${POSTGRES_USER}" \
  --dbname="${DB_NAME}" \
  --format=custom \
  --file="${BACKUP_DIR}/backup_${DATE}.dump"

# Compress
gzip -f "${BACKUP_DIR}/backup_${DATE}.dump"

# Clean old backups (>7 days)
find "${BACKUP_DIR}" -name "backup_*.dump.gz" -mtime +${RETENTION_DAYS} -delete

echo "Backup completed: backup_${DATE}.dump.gz"
```

#### Restore Procedure

```bash
# scripts/restore.sh
#!/bin/bash
set -euo pipefail

BACKUP_FILE="$1"  # e.g., backups/backup_20240114_020000.dump.gz

# Stop backend
docker compose stop backend

# Drop and recreate database
docker compose exec postgres psql -U "${DB_USER}" -c "DROP DATABASE IF EXISTS ${DB_NAME};"
docker compose exec postgres psql -U "${DB_USER}" -c "CREATE DATABASE ${DB_NAME};"

# Restore
gunzip -c "${BACKUP_FILE}" | docker compose exec -T postgres pg_restore \
  --username="${DB_USER}" \
  --dbname="${DB_NAME}" \
  --clean \
  --if-exists

# Restart
docker compose start backend

echo "Restore completed from ${BACKUP_FILE}"
```

#### Migration Execution

Migrations run automatically at container startup:
1. Backend container starts
2. `sqlx migrate run` executes before application start
3. Application starts only if migrations succeed
4. Migrations are idempotent and ordered by timestamp prefix

#### Connection Pool Sizing

```rust
// sqlx::Pool configuration (in backend/src/db.rs)
let pool = PgPoolOptions::new()
    .max_connections(20)        // Max concurrent DB connections
    .min_connections(5)         // Maintain warm connections
    .acquire_timeout(Duration::from_secs(5))
    .idle_timeout(Duration::from_secs(300))
    .max_lifetime(Duration::from_secs(1800))
    .connect(&database_url)
    .await?;
```

Pool sizing rationale:
- **20 max connections**: Postgres on 1GB RAM can comfortably handle 50-100; we reserve 20 for the app.
- **5 min connections**: Avoid connection creation latency for steady-state traffic.
- **5s acquire timeout**: Fail fast if pool exhausted; triggers 503 to client.

### 3.11 Redis Operations

#### Persistence Configuration

```
# Redis command-line options (in docker-compose.yml)
--appendonly yes          # Enable AOF persistence
--appendfsync everysec    # fsync every second (balance of speed/durability)
--maxmemory 512mb         # Hard memory limit
--maxmemory-policy allkeys-lru  # Evict least-recently-used keys when full
--requirepass <password>  # Authentication required
```

RDB snapshots are also enabled by default (save every 60s if 1 key changed).

#### Key Naming Convention

```
Format:   {category}:{identifier}[:subkey]
Examples:
  cache:openai:a1b2c3d4       # Cached LLM response (hash of prompt)
  ratelimit:api_key_123:60    # Rate limit counter (key:window_seconds)
  session:abcdef123456        # User session data
  status:openai               # Provider health status
  metrics:202401141200        # Aggregated metrics bucket (YYYYMMDDHHMM)
```

#### Memory Management

| Setting | Value | Rationale |
|---------|-------|-----------|
| maxmemory | 512MB | Hard limit on container memory |
| maxmemory-policy | allkeys-lru | Evict oldest unused keys; cache should never block |
| ttl cache keys | 60-300s | Short TTL prevents stale cache; long enough for hit rate |
| ttl session keys | 86400s | 24-hour sessions |
| ttl rate limit | window duration | Auto-cleanup after window expires |

---

## 4. Environment Configuration

### 4.1 Complete Environment Variable Reference

#### Database

| Variable | Required | Default | Description |
|----------|----------|---------|-------------|
| `DB_USER` | Yes | - | PostgreSQL username |
| `DB_PASSWORD` | Yes | - | PostgreSQL password (min 16 chars in production) |
| `DB_NAME` | Yes | `aigateway` | Database name |
| `DATABASE_URL` | Yes | - | Full connection string (auto-built if components provided) |

#### Redis

| Variable | Required | Default | Description |
|----------|----------|---------|-------------|
| `REDIS_PASSWORD` | Yes | - | Redis AUTH password |
| `REDIS_URL` | Yes | - | Full Redis connection string |

#### Application Core

| Variable | Required | Default | Description |
|----------|----------|---------|-------------|
| `APP_ENV` | No | `production` | Environment identifier (development/staging/production) |
| `PORT` | No | `8080` | Backend HTTP port (internal) |
| `RUST_LOG` | No | `info` | Log level (trace/debug/info/warn/error) |
| `LOG_FORMAT` | No | `json` | Output format: `json` (production) or `pretty` (dev) |
| `RUST_BACKTRACE` | No | `0` | Stack traces on panic (1=on, 0=off) |
| `REQUEST_TIMEOUT_SECS` | No | `300` | Max request duration (5 min for LLM calls) |

#### Authentication

| Variable | Required | Default | Description |
|----------|----------|---------|-------------|
| `JWT_SECRET` | Yes | - | JWT signing key (min 32 bytes, HS256) |
| `JWT_EXPIRY_HOURS` | No | `24` | Token lifetime in hours |
| `ENABLE_REGISTRATION` | No | `false` | Allow new user registration |

#### LLM Provider Keys

| Variable | Required | Default | Description |
|----------|----------|---------|-------------|
| `OPENAI_API_KEY` | No | - | OpenAI API key |
| `OPENAI_BASE_URL` | No | `https://api.openai.com` | Custom OpenAI-compatible endpoint |
| `ANTHROPIC_API_KEY` | No | - | Anthropic API key |
| `COHERE_API_KEY` | No | - | Cohere API key |

#### Rate Limiting

| Variable | Required | Default | Description |
|----------|----------|---------|-------------|
| `RATE_LIMIT_RPM` | No | `60` | Requests per minute per API key |
| `RATE_LIMIT_BURST` | No | `10` | Burst allowance (concurrent) |

#### Observability

| Variable | Required | Default | Description |
|----------|----------|---------|-------------|
| `METRICS_ENABLED` | No | `true` | Enable Prometheus metrics endpoint |
| `METRICS_PORT` | No | `9090` | Internal metrics port |
| `TRACING_ENABLED` | No | `true` | Enable distributed tracing |

#### SSL/TLS (Caddy)

| Variable | Required | Default | Description |
|----------|----------|---------|-------------|
| `DOMAIN` | Yes | - | Public domain (e.g., gateway.example.com) |
| `ACME_EMAIL` | Yes | - | Let's Encrypt account email |

### 4.2 Example Production `.env` File

```bash
# === Database ===
DB_USER=aigateway
DB_PASSWORD=change-this-to-32-char-random-string
DB_NAME=aigateway

# === Redis ===
REDIS_PASSWORD=change-this-to-different-32-char-random

# === Application ===
APP_ENV=production
RUST_LOG=info
LOG_FORMAT=json
JWT_SECRET=use-openssl-rand-base64-32-output-here
JWT_EXPIRY_HOURS=24
ENABLE_REGISTRATION=false

# === LLM Providers (at least one required) ===
OPENAI_API_KEY=sk-...
ANTHROPIC_API_KEY=sk-ant-...

# === Rate Limiting ===
RATE_LIMIT_RPM=60
RATE_LIMIT_BURST=10

# === SSL ===
DOMAIN=gateway.yourdomain.com
ACME_EMAIL=admin@yourdomain.com
```

---

## 5. Resource Requirements

### 5.1 VPS Specifications

| Resource | Minimum | Recommended | Notes |
|----------|---------|-------------|-------|
| **CPU** | 2 vCPU | 4 vCPU | Rust is efficient; 2 vCPU handles moderate LLM proxy load. 4 vCPU recommended for concurrent streaming. |
| **RAM** | 4 GB | 8 GB | Postgres (1GB) + Redis (512MB) + Backend (512MB) + OS overhead (~2GB) = ~4GB minimum. 8GB comfortable. |
| **Disk** | 40 GB SSD | 80 GB SSD | OS (10GB) + containers (5GB) + Postgres growth (10GB) + backups (15GB). SSD required for Postgres I/O. |
| **Network** | 100 Mbps | 1 Gbps | LLM streaming is bandwidth-intensive. 100Mbps handles ~50 concurrent streams. |
| **OS** | Ubuntu 22.04 LTS | Ubuntu 24.04 LTS | Any Linux with Docker support works. Ubuntu LTS for 5-year support cycle. |

### 5.2 Service Resource Limits

| Service | Memory Limit | CPU Limit | Rationale |
|---------|-------------|-----------|-----------|
| postgres | 1G | 1.0 | Database is the bottleneck; give it room |
| redis | 512M | 0.5 | In-memory ops are fast; limited by design |
| backend | 512M | 1.0 | Rust is memory-efficient; handles load well |
| caddy | 128M | 0.25 | Proxy is lightweight |
| backup | 256M | 0.25 | Only runs during backup window |

### 5.3 Cost Estimates (Monthly)

| Provider | Plan | Specs | Monthly Cost |
|----------|------|-------|-------------|
| **Hetzner** | CPX31 | 4 vCPU, 8GB, 160GB | ~EUR 13.60 |
| **Hetzner** | CPX41 | 8 vCPU, 16GB, 240GB | ~EUR 25.70 |
| **DigitalOcean** | Basic (Intel) | 4 vCPU, 8GB, 160GB | ~$48 |
| **Linode** | Shared 8GB | 4 vCPU, 8GB, 160GB | ~$48 |
| **AWS Lightsail** | 8GB | 2 vCPU, 8GB, 160GB | ~$40 |

**Recommendation**: Hetzner CPX31 for best price/performance. Upgrade to CPX41 if monitoring shows memory pressure.

---

## 6. Health Checks & Readiness

### 6.1 Health Check Endpoints

| Endpoint | Method | Auth | Returns | Description |
|----------|--------|------|---------|-------------|
| `GET /health` | GET | No | `{"status":"ok"}` | Liveness probe - always returns 200 if process running |
| `GET /health/ready` | GET | No | `{"status":"ok","checks":{"database":true,"redis":true}}` | Readiness probe - verifies all dependencies |
| `GET /metrics` | GET | No | Prometheus format | Operational metrics (if METRICS_ENABLED=true) |

### 6.2 Health Check Implementation

```rust
// backend/src/routes/health.rs
use axum::{response::Json, http::StatusCode};
use serde_json::{json, Value};

// GET /health - Liveness (process is running)
pub async fn liveness() -> (StatusCode, Json<Value>) {
    (StatusCode::OK, Json(json!({"status": "ok"})))
}

// GET /health/ready - Readiness (dependencies accessible)
pub async fn readiness(
    State(pool): State<PgPool>,
    State(redis): State<redis::aio::MultiplexedConnection>,
) -> (StatusCode, Json<Value>) {
    let db_healthy = sqlx::query("SELECT 1").fetch_one(&pool).await.is_ok();
    let redis_healthy = redis::cmd("PING").query_async::<_, String>(&mut redis.clone())
        .await.is_ok();

    if db_healthy && redis_healthy {
        (StatusCode::OK, Json(json!({
            "status": "ok",
            "checks": { "database": true, "redis": true }
        })))
    } else {
        (StatusCode::SERVICE_UNAVAILABLE, Json(json!({
            "status": "unhealthy",
            "checks": { "database": db_healthy, "redis": redis_healthy }
        })))
    }
}

// GET /metrics - Prometheus metrics
pub async fn metrics() -> Result<String, StatusCode> {
    // Export registered prometheus metrics
    prometheus::encode_to_string()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}
```

### 6.3 Docker Health Checks

| Service | Check Type | Interval | Timeout | Retries | Start Period |
|---------|-----------|----------|---------|---------|-------------|
| postgres | `pg_isready` | 10s | 5s | 5 | 10s |
| redis | `redis-cli PING` | 10s | 5s | 5 | 5s |
| backend | HTTP GET `/health` | 15s | 5s | 3 | 30s |

### 6.4 Startup Behavior

```
Phase 1: Infrastructure (0-10s)
  - Docker creates network
  - Postgres starts, runs init scripts
  - Redis starts, loads AOF

Phase 2: Dependency Readiness (5-15s)
  - Postgres healthcheck passes
  - Redis healthcheck passes

Phase 3: Application Start (15-30s)
  - Backend container starts
  - sqlx-cli runs migrations
  - Axum server binds to :8080
  - /health returns 200

Phase 4: Proxy Ready (30-45s)
  - Caddy starts
  - Resolves backend DNS
  - Obtains/loads TLS certificate
  - Ready to serve traffic

Total cold-start time: ~30-45 seconds
```

### 6.5 Shutdown Behavior

```
1. SIGTERM received (docker compose stop)
2. Backend: Stops accepting new connections
3. Backend: Waits for in-flight requests (up to REQUEST_TIMEOUT_SECS)
4. Backend: Closes database connections (pool drains)
5. Backend: Exits (SIGKILL after 30s grace period)
6. Postgres: Checkpoint and flush WAL
7. Redis: Save AOF, exit
```

---

## 7. Operational Runbooks

### 7.1 How to Deploy a New Version

```bash
# ── Zero-Downtime Deployment ────────────────────────────

# 1. SSH into the VPS
ssh user@gateway.yourdomain.com

# 2. Navigate to the project
cd /opt/ai-gateway

# 3. Pull latest code
git pull origin main

# 4. Rebuild and restart
docker compose build --no-cache backend
docker compose up -d --no-deps backend

# 5. Verify deployment
curl -s https://gateway.yourdomain.com/health | jq .
# Expected: {"status":"ok"}

# 6. Check logs for errors
docker compose logs --tail=50 backend

# ── Rollback (if needed) ────────────────────────────────
git log --oneline -5                    # Find previous commit
git checkout <previous-commit-hash>      # Checkout known good version
docker compose build --no-cache backend
docker compose up -d --no-deps backend
```

**Downtime**: ~5-10 seconds during container swap. For true zero-downtime, a blue-green deployment on a single VPS requires scripting but is achievable with Caddy's graceful reload.

### 7.2 How to Backup and Restore

```bash
# ── Manual Backup ──────────────────────────────────────
cd /opt/ai-gateway
./scripts/backup.sh
# Output: backups/backup_20240114_143022.dump.gz

# ── List Backups ───────────────────────────────────────
ls -la backups/
# -rw-r--r-- 1 root root 2.4M Jan 14 02:00 backup_20240114_020000.dump.gz
# -rw-r--r-- 1 root root 2.5M Jan 15 02:00 backup_20240115_020000.dump.gz

# ── Restore from Backup ────────────────────────────────
cd /opt/ai-gateway
./scripts/restore.sh backups/backup_20240114_020000.dump.gz

# ── Copy Backup Offsite ────────────────────────────────
# Option A: rsync to another server
rsync -avz backups/ backup-server:/backups/aigateway/

# Option B: Upload to S3-compatible storage
s3cmd sync backups/ s3://my-backup-bucket/aigateway/

# Option C: SCP to local machine
scp user@gateway.yourdomain.com:/opt/ai-gateway/backups/*.dump.gz ./local-backups/
```

### 7.3 How to Rotate Secrets

```bash
# ── Step 1: Generate new secrets ───────────────────────
# JWT Secret
openssl rand -base64 32
# Example output: aB3dE5fG7hI9jK1lM2nO3pQ4rS5tU6vW7xY8zA0bC1=

# Database Password
openssl rand -base64 24
# Example output: xY7zA9bC2dE4fG6hI8jK0lM1nO3pQ5r=

# ── Step 2: Update .env ────────────────────────────────
# Edit /opt/ai-gateway/.env with new values
# Do NOT change DATABASE_URL or REDIS_URL format strings
# Only change the password components

# ── Step 3: Rotate database password ───────────────────
# Connect to Postgres
docker compose exec postgres psql -U aigateway -c \
  "ALTER USER aigateway WITH PASSWORD 'new-password-here';"

# ── Step 4: Rotate Redis password ──────────────────────
# Redis requires restart for password change
docker compose down redis
docker compose up -d redis

# ── Step 5: Restart application ────────────────────────
docker compose up -d

# ── Step 6: Verify ─────────────────────────────────────
docker compose logs backend | tail -20
curl -s https://gateway.yourdomain.com/health
```

### 7.4 How to Scale Vertically

```bash
# ── Monitor current resources ──────────────────────────
docker stats                    # Container-level usage
free -h                         # System memory
df -h                           # Disk usage
top                             # CPU usage

# ── Identify bottleneck ────────────────────────────────
# High CPU?  → Upgrade to more vCPUs
# High RAM?  → Add memory, increase container limits
# High Disk? → Expand block storage or clean old backups
# Slow DB?   → Increase Postgres memory limit

# ── Upgrade VPS (example: Hetzner) ────────────────────
# 1. Power off
docker compose down

# 2. Resize via provider API/console (Hetzner example)
hcloud server change-type <server-id> cpx41

# 3. Power on, start services
docker compose up -d

# 4. Adjust container limits in docker-compose.yml if needed
# Edit limits, then:
docker compose up -d

# ── Adjust Postgres memory ─────────────────────────────
# Edit docker-compose.yml, under postgres service:
#   deploy.resources.limits.memory: 2G  (was 1G)
docker compose up -d postgres
```

### 7.5 How to Check System Health

```bash
# ── Quick Health Check ─────────────────────────────────
curl -s https://gateway.yourdomain.com/health | jq .
curl -s https://gateway.yourdomain.com/health/ready | jq .

# ── Container Status ───────────────────────────────────
docker compose ps
#   NAME        IMAGE           STATUS          PORTS
#   ag-backend  ai-gateway/backend   Up 3 days
#   ag-postgres postgres:16-alpine   Up 3 days (healthy)
#   ag-redis    redis:7.2-alpine     Up 3 days (healthy)
#   ag-caddy    caddy:2.8-alpine     Up 3 days

# ── Resource Usage ─────────────────────────────────────
docker stats --no-stream
# CONTAINER      CPU %    MEM USAGE / LIMIT
# ag-backend     2.14%    89MiB / 512MiB
# ag-postgres    0.05%    156MiB / 1GiB
# ag-redis       0.03%    12.4MiB / 512MiB
# ag-caddy       0.01%    18.2MiB / 128MiB

# ── Recent Logs ────────────────────────────────────────
docker compose logs --tail=100 backend
docker compose logs --tail=50 --follow backend  # Live tail

# ── Database Health ────────────────────────────────────
docker compose exec postgres psql -U aigateway -c \
  "SELECT count(*) as active_connections FROM pg_stat_activity;"

docker compose exec postgres psql -U aigateway -c \
  "SELECT pg_size_pretty(pg_database_size('aigateway'));"

# ── Redis Health ───────────────────────────────────────
docker compose exec redis redis-cli -a "$REDIS_PASSWORD" INFO memory
docker compose exec redis redis-cli -a "$REDIS_PASSWORD" INFO stats

# ── Certificate Status ─────────────────────────────────
docker compose exec caddy caddy list-modules
docker compose exec caddy caddy reload --config /etc/caddy/Caddyfile
```

### 7.6 How to View Logs

```bash
# ── All services ───────────────────────────────────────
docker compose logs

# ── Specific service ───────────────────────────────────
docker compose logs backend
docker compose logs postgres
docker compose logs caddy

# ── With timestamps ────────────────────────────────────
docker compose logs --timestamps backend

# ── Follow (live tail) ─────────────────────────────────
docker compose logs --follow backend

# ── Since specific time ────────────────────────────────
docker compose logs --since 2024-01-14T10:00:00 backend

# ── Search for errors ──────────────────────────────────
docker compose logs backend | grep -i error
docker compose logs backend | grep -i "panic\|error\|warn"

# ── Export logs ────────────────────────────────────────
docker compose logs --timestamps backend > backend_logs_$(date +%Y%m%d).txt

# ── Structured log query (JSON) ────────────────────────
# In production, logs are JSON. Parse with jq:
docker compose logs backend | jq 'select(.level == "ERROR")'
docker compose logs backend | jq 'select(.response_time_ms > 1000)'
```

---

## 8. Security Defaults

### 8.1 Default Security Headers

Caddy automatically applies these headers on all responses:

| Header | Value | Purpose |
|--------|-------|---------|
| `Strict-Transport-Security` | `max-age=31536000; includeSubDomains; preload` | Force HTTPS for 1 year |
| `X-Content-Type-Options` | `nosniff` | Prevent MIME-type sniffing |
| `X-Frame-Options` | `DENY` | Prevent clickjacking |
| `Referrer-Policy` | `strict-origin-when-cross-origin` | Limit referrer leakage |
| `Permissions-Policy` | `camera=(), microphone=(), geolocation=()` | Disable browser features |

Additional headers applied by backend:

| Header | Value | Purpose |
|--------|-------|---------|
| `X-Request-Id` | `<uuid>` | Request tracing |
| `Cache-Control` | `no-store` (API routes) | Prevent sensitive data caching |

### 8.2 CORS Configuration

```rust
// Default CORS: disabled for production (same-origin)
// When running frontend + backend on same domain via Caddy,
// CORS is not needed.

// For API-only deployments (separate frontend domain):
let cors = CorsLayer::new()
    .allow_origin(["https://admin.yourdomain.com".parse().unwrap()])
    .allow_methods([Method::GET, Method::POST, Method::PUT, Method::DELETE])
    .allow_headers([AUTHORIZATION, CONTENT_TYPE])
    .max_age(Duration::from_secs(3600));
```

| Setting | Default | Description |
|---------|---------|-------------|
| CORS enabled | `false` | Only enable if frontend hosted on different domain |
| Allowed origins | `[]` | Must be explicitly configured |
| Allowed methods | `GET, POST, PUT, DELETE` | Standard CRUD |
| Credentials | `false` | JWT in Authorization header, not cookies |

### 8.3 Rate Limiting Defaults

| Layer | Limit | Window | Scope | Behavior |
|-------|-------|--------|-------|----------|
| **Per-API-key** | 60 requests | 60 seconds | API key | Sliding window via Redis |
| **Per-IP** | 100 requests | 60 seconds | IP address | Fallback for unauthenticated |
| **Per-provider** | Provider limit | - | Global | Enforced per LLM provider rules |
| **Burst** | 10 requests | - | API key | Token bucket for traffic spikes |

Rate limit responses:
- **429 Too Many Requests** when limit exceeded
- `Retry-After` header with seconds until reset
- `X-RateLimit-Limit`, `X-RateLimit-Remaining`, `X-RateLimit-Reset` headers

### 8.4 Secret Management in Production

```bash
# ── Secret Generation Standards ────────────────────────

# JWT Secret: 256 bits (32 bytes), base64-encoded
openssl rand -base64 32

# Database Password: 24+ chars, mixed case + numbers
openssl rand -base64 24 | tr -dc 'a-zA-Z0-9' | head -c 32

# API Keys for users: UUID v4 (128 bits entropy)
uuidgen

# ── Storage Best Practices ─────────────────────────────
#
# 1. .env file permissions
chmod 600 /opt/ai-gateway/.env
chown root:root /opt/ai-gateway/.env
#
# 2. Never commit .env to Git
#    .env is in .gitignore by default
#
# 3. Limit read access
docker compose reads .env automatically
# Only root and docker group should read it
#
# 4. For teams sharing a VPS:
#    - Use a password manager to share the .env contents
#    - Or use Docker secrets (swarm mode, overkill for single VPS)
#    - Or use a vault tool like `pass` or `bitwarden-cli`
#
# 5. LLM API keys:
#    - Store only provider keys you actively use
#    - Rotate provider keys quarterly
#    - Monitor provider dashboards for unexpected usage

# ── .env Security Checklist ────────────────────────────
# [ ] All passwords are 24+ characters, randomly generated
# [ ] JWT secret is 32+ bytes of random data
# [ ] .env file permissions are 600 (owner read/write only)
# [ ] .env is listed in .gitignore
# [ ] No secrets in docker-compose.yml (use ${VAR} interpolation)
# [ ] No secrets in container layers (use BuildKit secrets if needed)
# [ ] No secrets in logs (backend masks Authorization headers)
# [ ] Production JWT expiry is <= 24 hours
# [ ] Registration is disabled in production (ENABLE_REGISTRATION=false)
```

### 8.5 Network Security

```yaml
# Docker Compose security: no ports exposed except Caddy
services:
  backend:
    ports: []  # NO external ports
  postgres:
    ports: []  # NO external ports
  redis:
    ports: []  # NO external ports
  caddy:
    ports:
      - "80:80"     # Only HTTP/S exposed
      - "443:443"
```

| Rule | Setting |
|------|---------|
| Internal services | Docker bridge network only |
| External access | Caddy reverse proxy only |
| Database | Not accessible from internet |
| Redis | Password-protected, internal only |
| SSH | Key-based auth, disable password login |
| Firewall (UFW) | Allow 22, 80, 443 only |

### 8.6 SSL/TLS Configuration

| Setting | Value | Description |
|---------|-------|-------------|
| Protocol | TLS 1.2+ | Minimum TLS version |
| Certificate | Let's Encrypt | Free, auto-renewing |
| Auto-renew | Caddy-managed | No manual intervention |
| HSTS | 1 year | Preload-ready |
| OCSP Stapling | Enabled | Caddy default |

### 8.7 Input Validation & Injection Protection

| Layer | Protection |
|-------|-----------|
| SQL Injection | sqlx parameterized queries (compile-time checked) |
| XSS | Output not rendered as HTML; Content-Type headers |
| JSON Injection | Serde deserialization with strict types |
| Request Size | 10MB max body size (Axum default limit) |
| Path Traversal | Static file serving via Caddy (chrooted) |
| Header Injection | Axum sanitizes header values |

---

## Appendix A: Quick Start Checklist

```
Server Setup:
[ ] Provision VPS (Ubuntu 24.04, 4 vCPU, 8GB RAM)
[ ] Create non-root user with sudo
[ ] Install Docker Engine + Docker Compose
[ ] Configure UFW (allow 22, 80, 443)
[ ] Set up SSH key auth, disable password login
[ ] Point DNS A record to VPS IP

Deployment:
[ ] Clone repository to /opt/ai-gateway
[ ] Copy .env.example to .env, fill all values
[ ] Generate strong passwords (openssl rand)
[ ] Set .env permissions to 600
[ ] docker compose up -d
[ ] Verify: curl https://yourdomain.com/health
[ ] Check logs: docker compose logs -f backend

Post-Deploy:
[ ] Create first admin user
[ ] Add LLM provider API keys
[ ] Configure rate limits for your use case
[ ] Set up offsite backup (cron/rsync/S3)
[ ] Verify backup script works: ./scripts/backup.sh
[ ] Monitor for 24 hours
```

## Appendix B: File Locations

| File | Path | Purpose |
|------|------|---------|
| Main Compose | `/opt/ai-gateway/docker-compose.yml` | Production services |
| Dev Compose | `/opt/ai-gateway/docker-compose.dev.yml` | Development services |
| Environment | `/opt/ai-gateway/.env` | Secrets and config |
| Caddyfile | `/opt/ai-gateway/docker/Caddyfile` | Reverse proxy config |
| Backend Dockerfile | `/opt/ai-gateway/docker/Dockerfile.backend` | Backend build |
| Frontend Dockerfile | `/opt/ai-gateway/docker/Dockerfile.frontend` | Frontend build |
| DB Migrations | `/opt/ai-gateway/backend/migrations/` | Schema migrations |
| Backups | `/opt/ai-gateway/backups/` | pg_dump output |
| Logs | `docker compose logs` / Caddy access log | Application logs |
| Scripts | `/opt/ai-gateway/scripts/` | Backup, restore, setup |

---

*Document version: 1.0 | Target deploy time: < 10 minutes | Audience: Solo developers, small teams, non-DevOps operators*
