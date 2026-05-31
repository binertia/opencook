# Swarm-01 Knowledge Base — Database Schema

> **Database:** PostgreSQL 15+  
> **Product:** AI Gateway  
> **Multi-tenant:** Organization-scoped row-level security  
> **Document Version:** 1.0  

---

## 1. Schema Design Principles

### 1.1 Tenant Isolation Approach

| Aspect | Decision | Rationale |
|--------|----------|-----------|
| Isolation model | **Row-level security (RLS) + `org_id` column** | Combines application-level filtering with database-level enforcement. RLS prevents bugs from leaking data; `org_id` columns enable performant tenant-scoped queries and partitioning. |
| Schema per tenant | **Not used** | Avoids schema explosion, simplifies migrations, and maintains connection pool efficiency. |
| Database per tenant | **Not used** | Overhead too high for SaaS model; connection pooling becomes unmanageable. |
| Enforcement layer | **Dual: application WHERE clauses + RLS policies** | App-level for performance (avoids RLS planning overhead on hot paths); RLS as defense-in-depth for direct DB access and data leakage prevention. |

### 1.2 Naming Conventions

| Element | Convention | Example |
|---------|------------|---------|
| Tables | snake_case, plural nouns | `routing_rules`, `usage_records` |
| Columns | snake_case | `created_at`, `org_id` |
| Primary keys | `id` (UUID) | `id UUID PRIMARY KEY DEFAULT gen_random_uuid()` |
| Foreign keys | `[table]_id` | `org_id`, `user_id`, `api_key_id` |
| Indexes | `idx_[table]_[column(s)]` | `idx_requests_org_id_created_at` |
| Unique constraints | `uk_[table]_[column(s)]` | `uk_api_keys_key_hash` |
| Check constraints | `chk_[table]_[condition]` | `chk_quotas_limit_positive` |
| Triggers | `trg_[table]_[action]` | `trg_requests_updated_at` |
| Functions | `fn_[purpose]` | `fn_update_timestamp` |
| Enums | snake_case, singular | `provider_kind`, `quota_period` |

### 1.3 ID Generation Strategy

| Entity | Strategy | Rationale |
|--------|----------|-----------|
| All primary keys | **UUID v7** (via `pg_uuidv7` extension or application-generated) | Time-ordered UUIDs provide monotonic ordering for index locality on high-write tables (`requests`, `responses`). Avoids hot-spotting on B-tree indexes compared to random UUID v4. |
| External-facing IDs | **ULID** (application layer) | Lexicographically sortable, URL-safe, shorter than UUID. Used for API keys and request trace IDs. |
| Internal sequences | **BIGSERIAL** only for `migration_versions` | Migration tracking only; never for business entities. |

```sql
-- Enable UUIDv7 generation
CREATE EXTENSION IF NOT EXISTS "pgcrypto";
CREATE EXTENSION IF NOT EXISTS "pg_uuidv7";
```

> **Note:** If `pg_uuidv7` extension is unavailable, UUID v4 via `gen_random_uuid()` is acceptable. The application layer may also generate ULIDs for request IDs to ensure sortability.

### 1.4 Timestamp Strategy

| Column | Type | Default | Purpose |
|--------|------|---------|---------|
| `created_at` | `TIMESTAMPTZ` | `now()` | Immutable record creation time |
| `updated_at` | `TIMESTAMPTZ` | `now()` | Last modification time; updated by trigger |
| `deleted_at` | `TIMESTAMPTZ` | `NULL` | Soft delete marker; `NULL` = active record |

All tables include these three columns. Queries filter with `WHERE deleted_at IS NULL` unless explicitly including soft-deleted records. No hard deletes on tenant-visible tables.

### 1.5 Indexing Strategy Overview

| Principle | Decision |
|-----------|----------|
| Every foreign key indexed | FK columns used in JOINs and tenant lookups |
| Every tenant query starts with `org_id` | Composite indexes lead with `org_id` for tenant scoping |
| Time-series tables partitioned by range | `requests`, `responses`, `usage_records` partitioned on `created_at` |
| Partial indexes for soft deletes | `WHERE deleted_at IS NULL` reduces index size and improves query selectivity |
| GIN indexes for JSONB | Metadata, headers, and provider-specific config stored as JSONB |
| Expression indexes for lookups | Hashed API key lookups, lowercased email searches |
| No indexes on high-cardinality unconstrained columns | e.g., `request_body` is not indexed; truncated for storage |

---

## 2. Complete Table Definitions

---

### 2.1 organizations

**Purpose:** Tenant root. Every other table references `org_id` for tenant isolation.

```sql
CREATE TABLE organizations (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name            TEXT NOT NULL,
    slug            TEXT NOT NULL,
    status          TEXT NOT NULL DEFAULT 'active'
                        CHECK (status IN ('active', 'suspended', 'deleted')),
    settings        JSONB NOT NULL DEFAULT '{}',
    billing_email   TEXT,
    plan_tier       TEXT NOT NULL DEFAULT 'free'
                        CHECK (plan_tier IN ('free', 'starter', 'pro', 'enterprise')),
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    deleted_at      TIMESTAMPTZ
);

CREATE UNIQUE INDEX uk_organizations_slug ON organizations(slug) WHERE deleted_at IS NULL;
CREATE INDEX idx_organizations_status ON organizations(status) WHERE deleted_at IS NULL;
```

| Column | Type | Nullable | Default | Notes |
|--------|------|----------|---------|-------|
| `id` | UUID | No | `gen_random_uuid()` | PK, tenant root |
| `name` | TEXT | No | — | Display name |
| `slug` | TEXT | No | — | URL identifier, unique |
| `status` | TEXT | No | `'active'` | active / suspended / deleted |
| `settings` | JSONB | No | `'{}'` | Org-level config (rate limits, features) |
| `billing_email` | TEXT | Yes | — | Invoice/contact email |
| `plan_tier` | TEXT | No | `'free'` | Subscription tier |
| `created_at` | TIMESTAMPTZ | No | `now()` | — |
| `updated_at` | TIMESTAMPTZ | No | `now()` | Trigger-updated |
| `deleted_at` | TIMESTAMPTZ | Yes | NULL | Soft delete |

**Rationale:** `slug` uniqueness enforced via partial index to allow reuse after soft delete. `settings` JSONB avoids schema changes for org-level feature flags. `status` enables suspension without data loss.

---

### 2.2 users

**Purpose:** Dashboard login users. Belongs to one organization.

```sql
CREATE TABLE users (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id          UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    email           TEXT NOT NULL,
    password_hash   TEXT,
    display_name    TEXT,
    role            TEXT NOT NULL DEFAULT 'member'
                        CHECK (role IN ('owner', 'admin', 'member', 'viewer')),
    status          TEXT NOT NULL DEFAULT 'active'
                        CHECK (status IN ('active', 'inactive', 'suspended')),
    last_login_at   TIMESTAMPTZ,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    deleted_at      TIMESTAMPTZ
);

CREATE UNIQUE INDEX uk_users_org_email ON users(org_id, email) WHERE deleted_at IS NULL;
CREATE INDEX idx_users_org_id ON users(org_id) WHERE deleted_at IS NULL;
```

| Column | Type | Nullable | Default | Notes |
|--------|------|----------|---------|-------|
| `id` | UUID | No | `gen_random_uuid()` | PK |
| `org_id` | UUID | No | — | FK → organizations, cascade delete |
| `email` | TEXT | No | — | Scoped unique per org |
| `password_hash` | TEXT | Yes | — | Argon2id hash; NULL for SSO-only users |
| `display_name` | TEXT | Yes | — | Human-readable name |
| `role` | TEXT | No | `'member'` | RBAC role |
| `status` | TEXT | No | `'active'` | Account status |
| `last_login_at` | TIMESTAMPTZ | Yes | — | Last successful authentication |
| `created_at` | TIMESTAMPTZ | No | `now()` | — |
| `updated_at` | TIMESTAMPTZ | No | `now()` | Trigger-updated |
| `deleted_at` | TIMESTAMPTZ | Yes | NULL | Soft delete |

**Rationale:** Email uniqueness is per-organization, not global — two orgs can have `user@example.com`. SSO users may have NULL `password_hash`. Roles are coarse-grained; fine-grained permissions live in `settings` JSONB on organization if needed.

---

### 2.3 api_keys

**Purpose:** API keys for AI API access. Scoped to an organization, optionally to a user.

```sql
CREATE TABLE api_keys (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id          UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    user_id         UUID REFERENCES users(id) ON DELETE SET NULL,
    name            TEXT NOT NULL,
    key_hash        TEXT NOT NULL,
    key_prefix      TEXT NOT NULL,
    scopes          TEXT[] NOT NULL DEFAULT ARRAY['ai:write'],
    rate_limit_rps  INTEGER NOT NULL DEFAULT 10,
    status          TEXT NOT NULL DEFAULT 'active'
                        CHECK (status IN ('active', 'revoked', 'expired')),
    expires_at      TIMESTAMPTZ,
    last_used_at    TIMESTAMPTZ,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    deleted_at      TIMESTAMPTZ
);

CREATE UNIQUE INDEX uk_api_keys_key_hash ON api_keys(key_hash) WHERE deleted_at IS NULL;
CREATE INDEX idx_api_keys_org_id ON api_keys(org_id) WHERE deleted_at IS NULL;
CREATE INDEX idx_api_keys_key_prefix ON api_keys(key_prefix) WHERE deleted_at IS NULL;
```

| Column | Type | Nullable | Default | Notes |
|--------|------|----------|---------|-------|
| `id` | UUID | No | `gen_random_uuid()` | PK |
| `org_id` | UUID | No | — | FK → organizations |
| `user_id` | UUID | Yes | — | FK → users; who created it |
| `name` | TEXT | No | — | Human-readable label |
| `key_hash` | TEXT | No | — | SHA-256 hash of the API key; only the hash is stored |
| `key_prefix` | TEXT | No | — | First 8 chars of key for identification in UI |
| `scopes` | TEXT[] | No | `['ai:write']` | Permission scopes |
| `rate_limit_rps` | INTEGER | No | `10` | Requests per second limit |
| `status` | TEXT | No | `'active'` | active / revoked / expired |
| `expires_at` | TIMESTAMPTZ | Yes | — | Optional expiration |
| `last_used_at` | TIMESTAMPTZ | Yes | — | Last successful API call |
| `created_at` | TIMESTAMPTZ | No | `now()` | — |
| `updated_at` | TIMESTAMPTZ | No | `now()` | Trigger-updated |
| `deleted_at` | TIMESTAMPTZ | Yes | NULL | Soft delete |

**Rationale:** Raw API keys are never stored — only SHA-256 hashes. `key_prefix` enables UI display without exposing the full key. `scopes` as TEXT[] allows multiple permissions. `rate_limit_rps` is per-key; org-level rate limits live in `organizations.settings`. The `uk_api_keys_key_hash` index is the primary lookup path for authenticating every incoming request.

---

### 2.4 provider_configs

**Purpose:** Configured AI providers (OpenAI, Anthropic, Azure, etc.) per organization.

```sql
CREATE TYPE provider_kind AS ENUM (
    'openai', 'anthropic', 'azure_openai', 'google_gemini',
    'cohere', 'mistral', 'groq', 'custom', 'bedrock'
);

CREATE TABLE provider_configs (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id          UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    name            TEXT NOT NULL,
    kind            provider_kind NOT NULL,
    api_base        TEXT,
    api_key_enc     BYTEA NOT NULL,
    default_headers JSONB NOT NULL DEFAULT '{}',
    config          JSONB NOT NULL DEFAULT '{}',
    priority        INTEGER NOT NULL DEFAULT 0,
    status          TEXT NOT NULL DEFAULT 'active'
                        CHECK (status IN ('active', 'inactive', 'error')),
    last_error_at   TIMESTAMPTZ,
    last_error_msg  TEXT,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    deleted_at      TIMESTAMPTZ
);

CREATE UNIQUE INDEX uk_provider_configs_org_name ON provider_configs(org_id, name) WHERE deleted_at IS NULL;
CREATE INDEX idx_provider_configs_org_id ON provider_configs(org_id) WHERE deleted_at IS NULL;
CREATE INDEX idx_provider_configs_kind ON provider_configs(kind);
```

| Column | Type | Nullable | Default | Notes |
|--------|------|----------|---------|-------|
| `id` | UUID | No | `gen_random_uuid()` | PK |
| `org_id` | UUID | No | — | FK → organizations |
| `name` | TEXT | No | — | Org-scoped display name |
| `kind` | provider_kind | No | — | Provider type enum |
| `api_base` | TEXT | Yes | — | Custom base URL if not default |
| `api_key_enc` | BYTEA | No | — | AES-256-GCM encrypted API key |
| `default_headers` | JSONB | No | `'{}'` | Default HTTP headers for this provider |
| `config` | JSONB | No | `'{}'` | Provider-specific config (model mapping, region, etc.) |
| `priority` | INTEGER | No | `0` | Lower = higher priority in routing |
| `status` | TEXT | No | `'active'` | active / inactive / error |
| `last_error_at` | TIMESTAMPTZ | Yes | — | Last failed request timestamp |
| `last_error_msg` | TEXT | Yes | — | Last error message |
| `created_at` | TIMESTAMPTZ | No | `now()` | — |
| `updated_at` | TIMESTAMPTZ | No | `now()` | Trigger-updated |
| `deleted_at` | TIMESTAMPTZ | Yes | NULL | Soft delete |

**Rationale:** `api_key_enc` is encrypted at application layer before storage. `config` JSONB holds provider-specific settings without schema proliferation. `priority` enables simple fallback routing. `status` + error tracking enables automatic provider health monitoring.

---

### 2.5 provider_models

**Purpose:** Models available per provider configuration. Enables model aliasing and routing.

```sql
CREATE TABLE provider_models (
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id              UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    provider_config_id  UUID NOT NULL REFERENCES provider_configs(id) ON DELETE CASCADE,
    model_id            TEXT NOT NULL,
    model_name          TEXT NOT NULL,
    aliases             TEXT[] NOT NULL DEFAULT '{}',
    input_cost_per_1k   NUMERIC(18, 8) NOT NULL DEFAULT 0,
    output_cost_per_1k  NUMERIC(18, 8) NOT NULL DEFAULT 0,
    context_window      INTEGER,
    max_tokens          INTEGER,
    supports_streaming  BOOLEAN NOT NULL DEFAULT true,
    supports_tools      BOOLEAN NOT NULL DEFAULT false,
    supports_vision     BOOLEAN NOT NULL DEFAULT false,
    status              TEXT NOT NULL DEFAULT 'active'
                            CHECK (status IN ('active', 'deprecated', 'disabled')),
    config              JSONB NOT NULL DEFAULT '{}',
    created_at          TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT now(),
    deleted_at          TIMESTAMPTZ
);

CREATE UNIQUE INDEX uk_provider_models_provider_model ON provider_models(provider_config_id, model_id) WHERE deleted_at IS NULL;
CREATE INDEX idx_provider_models_org_id ON provider_models(org_id) WHERE deleted_at IS NULL;
CREATE INDEX idx_provider_models_alias ON provider_models USING GIN (aliases) WHERE deleted_at IS NULL;
```

| Column | Type | Nullable | Default | Notes |
|--------|------|----------|---------|-------|
| `id` | UUID | No | `gen_random_uuid()` | PK |
| `org_id` | UUID | No | — | FK → organizations |
| `provider_config_id` | UUID | No | — | FK → provider_configs |
| `model_id` | TEXT | No | — | Provider's model identifier (e.g., `gpt-4o`) |
| `model_name` | TEXT | No | — | Display name |
| `aliases` | TEXT[] | No | `'{}'` | Alternative names for routing (e.g., `['gpt-4', 'latest']`) |
| `input_cost_per_1k` | NUMERIC(18,8) | No | `0` | Cost per 1K input tokens |
| `output_cost_per_1k` | NUMERIC(18,8) | No | `0` | Cost per 1K output tokens |
| `context_window` | INTEGER | Yes | — | Max context length |
| `max_tokens` | INTEGER | Yes | — | Max generation length |
| `supports_streaming` | BOOLEAN | No | `true` | Streaming support flag |
| `supports_tools` | BOOLEAN | No | `false` | Function calling support |
| `supports_vision` | BOOLEAN | No | `false` | Image input support |
| `status` | TEXT | No | `'active'` | active / deprecated / disabled |
| `config` | JSONB | No | `'{}'` | Model-specific overrides |
| `created_at` | TIMESTAMPTZ | No | `now()` | — |
| `updated_at` | TIMESTAMPTZ | No | `now()` | Trigger-updated |
| `deleted_at` | TIMESTAMPTZ | Yes | NULL | Soft delete |

**Rationale:** Cost fields enable real-time cost tracking without external lookups. `aliases` with GIN index enables flexible routing by model name aliases. `config` JSONB allows per-model parameter overrides (temperature defaults, etc.).

---

### 2.6 routing_rules

**Purpose:** Routing configuration: which model/provider to use under what conditions.

```sql
CREATE TYPE routing_strategy AS ENUM (
    'fallback', 'weighted', 'conditional', 'single'
);

CREATE TABLE routing_rules (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id          UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    name            TEXT NOT NULL,
    description     TEXT,
    strategy        routing_strategy NOT NULL DEFAULT 'single',
    priority        INTEGER NOT NULL DEFAULT 0,
    match_model     TEXT,
    match_tags      TEXT[] NOT NULL DEFAULT '{}',
    conditions      JSONB NOT NULL DEFAULT '{}',
    targets         JSONB NOT NULL DEFAULT '[]',
    timeout_ms      INTEGER NOT NULL DEFAULT 30000,
    retries         INTEGER NOT NULL DEFAULT 1,
    status          TEXT NOT NULL DEFAULT 'active'
                        CHECK (status IN ('active', 'inactive', 'draft')),
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    deleted_at      TIMESTAMPTZ
);

CREATE UNIQUE INDEX uk_routing_rules_org_name ON routing_rules(org_id, name) WHERE deleted_at IS NULL;
CREATE INDEX idx_routing_rules_org_priority ON routing_rules(org_id, priority) WHERE deleted_at IS NULL AND status = 'active';
CREATE INDEX idx_routing_rules_match_model ON routing_rules(org_id, match_model) WHERE deleted_at IS NULL AND status = 'active';
CREATE INDEX idx_routing_rules_conditions ON routing_rules USING GIN (conditions) WHERE deleted_at IS NULL;
```

| Column | Type | Nullable | Default | Notes |
|--------|------|----------|---------|-------|
| `id` | UUID | No | `gen_random_uuid()` | PK |
| `org_id` | UUID | No | — | FK → organizations |
| `name` | TEXT | No | — | Human-readable rule name |
| `description` | TEXT | Yes | — | Optional documentation |
| `strategy` | routing_strategy | No | `'single'` | Routing algorithm |
| `priority` | INTEGER | No | `0` | Evaluation order; lower first |
| `match_model` | TEXT | Yes | — | Exact model name match; NULL = wildcard |
| `match_tags` | TEXT[] | No | `'{}'` | Tags for matching |
| `conditions` | JSONB | No | `'{}'` | Match conditions (JSONPath, headers, etc.) |
| `targets` | JSONB | No | `'[]'` | Ordered list of provider/model targets |
| `timeout_ms` | INTEGER | No | `30000` | Per-request timeout |
| `retries` | INTEGER | No | `1` | Retry count on failure |
| `status` | TEXT | No | `'active'` | active / inactive / draft |
| `created_at` | TIMESTAMPTZ | No | `now()` | — |
| `updated_at` | TIMESTAMPTZ | No | `now()` | Trigger-updated |
| `deleted_at` | TIMESTAMPTZ | Yes | NULL | Soft delete |

**Rationale:** `conditions` and `targets` as JSONB enables complex routing logic without schema changes. `priority` + `match_model` indexes support the hot-path routing lookup. Rules are evaluated in priority order within an org.

---

### 2.7 requests

**Purpose:** Request log — the highest-write table. Records every AI API request.

```sql
CREATE TYPE request_status AS ENUM (
    'pending', 'processing', 'success', 'error', 'timeout', 'cancelled'
);

CREATE TABLE requests (
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id              UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    api_key_id          UUID NOT NULL REFERENCES api_keys(id) ON DELETE SET NULL,
    user_id             UUID REFERENCES users(id) ON DELETE SET NULL,
    provider_config_id  UUID REFERENCES provider_configs(id) ON DELETE SET NULL,
    provider_model_id   UUID REFERENCES provider_models(id) ON DELETE SET NULL,
    routing_rule_id     UUID REFERENCES routing_rules(id) ON DELETE SET NULL,
    
    -- Request identification
    trace_id            TEXT NOT NULL,
    parent_trace_id     TEXT,
    
    -- Request content (truncated)
    method              TEXT NOT NULL DEFAULT 'POST',
    path                TEXT NOT NULL,
    model_requested     TEXT,
    model_routed        TEXT,
    
    -- Body stored with truncation
    request_headers     JSONB NOT NULL DEFAULT '{}',
    request_body        TEXT,
    request_body_truncated BOOLEAN NOT NULL DEFAULT false,
    
    -- Timing
    requested_at        TIMESTAMPTZ NOT NULL DEFAULT now(),
    gateway_received_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    provider_sent_at    TIMESTAMPTZ,
    provider Responded_at TIMESTAMPTZ,
    completed_at        TIMESTAMPTZ,
    
    -- Latency metrics (milliseconds)
    latency_gateway_ms  INTEGER,
    latency_provider_ms INTEGER,
    latency_total_ms    INTEGER,
    
    -- Token usage (from provider response)
    prompt_tokens       INTEGER NOT NULL DEFAULT 0,
    completion_tokens   INTEGER NOT NULL DEFAULT 0,
    total_tokens        INTEGER NOT NULL DEFAULT 0,
    
    -- Cost (computed from provider_model rates)
    input_cost          NUMERIC(18, 8) NOT NULL DEFAULT 0,
    output_cost         NUMERIC(18, 8) NOT NULL DEFAULT 0,
    total_cost          NUMERIC(18, 8) NOT NULL DEFAULT 0,
    
    -- Status and metadata
    status              request_status NOT NULL DEFAULT 'pending',
    status_code         INTEGER,
    error_code          TEXT,
    error_message       TEXT,
    metadata            JSONB NOT NULL DEFAULT '{}',
    
    -- Cache info
    cache_hit           BOOLEAN NOT NULL DEFAULT false,
    cache_key_hash      TEXT,
    
    created_at          TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT now(),
    deleted_at          TIMESTAMPTZ
) PARTITION BY RANGE (created_at);

-- Indexes (created on parent, inherited by partitions)
CREATE INDEX idx_requests_org_created ON requests(org_id, created_at DESC) WHERE deleted_at IS NULL;
CREATE INDEX idx_requests_api_key_created ON requests(api_key_id, created_at DESC) WHERE deleted_at IS NULL;
CREATE INDEX idx_requests_trace_id ON requests(trace_id) WHERE deleted_at IS NULL;
CREATE INDEX idx_requests_status ON requests(status) WHERE deleted_at IS NULL;
CREATE INDEX idx_requests_model ON requests(org_id, model_routed, created_at DESC) WHERE deleted_at IS NULL;
CREATE INDEX idx_requests_cache ON requests(org_id, cache_key_hash) WHERE cache_hit = true AND deleted_at IS NULL;
CREATE INDEX idx_requests_metadata ON requests USING GIN (metadata) WHERE deleted_at IS NULL;
```

> **Note on column name:** `provider_responded_at` — the DDL above has a typo in the word `provider_responded_at` (written as two words). Correct DDL:

```sql
    provider_responded_at TIMESTAMPTZ,
```

| Column | Type | Nullable | Default | Notes |
|--------|------|----------|---------|-------|
| `id` | UUID | No | `gen_random_uuid()` | PK |
| `org_id` | UUID | No | — | FK → organizations; partition key consideration |
| `api_key_id` | UUID | No | — | FK → api_keys; primary lookup key |
| `user_id` | UUID | Yes | — | FK → users; dashboard user if identified |
| `provider_config_id` | UUID | Yes | — | FK → provider_configs; which provider handled it |
| `provider_model_id` | UUID | Yes | — | FK → provider_models; which model served it |
| `routing_rule_id` | UUID | Yes | — | FK → routing_rules; which rule matched |
| `trace_id` | TEXT | No | — | Distributed trace ID; unique |
| `parent_trace_id` | TEXT | Yes | — | Parent trace for multi-turn or chained requests |
| `method` | TEXT | No | `'POST'` | HTTP method |
| `path` | TEXT | No | — | API endpoint path |
| `model_requested` | TEXT | Yes | — | Model client asked for |
| `model_routed` | TEXT | Yes | — | Model actually used |
| `request_headers` | JSONB | No | `'{}'` | Selected headers (auth stripped) |
| `request_body` | TEXT | Yes | — | Request body, potentially truncated |
| `request_body_truncated` | BOOLEAN | No | `false` | Whether body was truncated |
| `requested_at` | TIMESTAMPTZ | No | `now()` | Client request timestamp |
| `gateway_received_at` | TIMESTAMPTZ | No | `now()` | Gateway received timestamp |
| `provider_sent_at` | TIMESTAMPTZ | Yes | — | Sent to provider timestamp |
| `provider_responded_at` | TIMESTAMPTZ | Yes | — | Provider response timestamp |
| `completed_at` | TIMESTAMPTZ | Yes | — | Response sent to client timestamp |
| `latency_gateway_ms` | INTEGER | Yes | — | Internal processing latency |
| `latency_provider_ms` | INTEGER | Yes | — | Provider round-trip latency |
| `latency_total_ms` | INTEGER | Yes | — | End-to-end latency |
| `prompt_tokens` | INTEGER | No | `0` | Input tokens |
| `completion_tokens` | INTEGER | No | `0` | Output tokens |
| `total_tokens` | INTEGER | No | `0` | Total tokens |
| `input_cost` | NUMERIC(18,8) | No | `0` | Computed input cost |
| `output_cost` | NUMERIC(18,8) | No | `0` | Computed output cost |
| `total_cost` | NUMERIC(18,8) | No | `0` | Total cost |
| `status` | request_status | No | `'pending'` | Request lifecycle status |
| `status_code` | INTEGER | Yes | — | HTTP response status |
| `error_code` | TEXT | Yes | — | Structured error code |
| `error_message` | TEXT | Yes | — | Human-readable error |
| `metadata` | JSONB | No | `'{}'` | Provider-specific metadata |
| `cache_hit` | BOOLEAN | No | `false` | Whether served from cache |
| `cache_key_hash` | TEXT | Yes | — | Cache key for deduplication |
| `created_at` | TIMESTAMPTZ | No | `now()` | Partition key |
| `updated_at` | TIMESTAMPTZ | No | `now()` | Trigger-updated |
| `deleted_at` | TIMESTAMPTZ | Yes | NULL | Soft delete |

**Rationale:** 
- Every request is tied to `org_id` and `api_key_id` for tenant scoping and attribution.
- `request_body` is truncated at application layer (default 64KB) to prevent storage bloat; `request_body_truncated` flags this.
- Cost fields are denormalized and computed at write time to avoid JOINs on analytics queries.
- `cache_hit` enables cache effectiveness analysis.
- Partitioned by `created_at` (see Section 4).

---

### 2.8 responses

**Purpose:** Response log. Stored separately from requests to allow different retention, archiving, and access patterns.

```sql
CREATE TABLE responses (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id          UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    request_id      UUID NOT NULL REFERENCES requests(id) ON DELETE CASCADE,
    
    -- Response content (truncated)
    status_code     INTEGER NOT NULL,
    response_headers JSONB NOT NULL DEFAULT '{}',
    response_body   TEXT,
    response_body_truncated BOOLEAN NOT NULL DEFAULT false,
    
    -- Token details (extracted from response)
    prompt_tokens   INTEGER NOT NULL DEFAULT 0,
    completion_tokens INTEGER NOT NULL DEFAULT 0,
    total_tokens    INTEGER NOT NULL DEFAULT 0,
    
    -- Finish reason
    finish_reason   TEXT,
    
    -- Model info (denormalized from request)
    model_used      TEXT,
    
    -- Provider-specific response metadata
    provider_metadata JSONB NOT NULL DEFAULT '{}',
    
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    deleted_at      TIMESTAMPTZ
) PARTITION BY RANGE (created_at);

CREATE INDEX idx_responses_request_id ON responses(request_id) WHERE deleted_at IS NULL;
CREATE INDEX idx_responses_org_created ON responses(org_id, created_at DESC) WHERE deleted_at IS NULL;
CREATE INDEX idx_responses_status ON responses(status_code) WHERE deleted_at IS NULL;
```

| Column | Type | Nullable | Default | Notes |
|--------|------|----------|---------|-------|
| `id` | UUID | No | `gen_random_uuid()` | PK |
| `org_id` | UUID | No | — | FK → organizations |
| `request_id` | UUID | No | — | FK → requests; 1:1 relationship |
| `status_code` | INTEGER | No | — | HTTP status code |
| `response_headers` | JSONB | No | `'{}'` | Response headers |
| `response_body` | TEXT | Yes | — | Response body, potentially truncated |
| `response_body_truncated` | BOOLEAN | No | `false` | Whether body was truncated |
| `prompt_tokens` | INTEGER | No | `0` | Denormalized from response |
| `completion_tokens` | INTEGER | No | `0` | Denormalized from response |
| `total_tokens` | INTEGER | No | `0` | Denormalized from response |
| `finish_reason` | TEXT | Yes | — | stop / length / tool_calls / etc. |
| `model_used` | TEXT | Yes | — | Actual model that generated response |
| `provider_metadata` | JSONB | No | `'{}'` | Provider-specific response data |
| `created_at` | TIMESTAMPTZ | No | `now()` | Partition key |
| `updated_at` | TIMESTAMPTZ | No | `now()` | Trigger-updated |
| `deleted_at` | TIMESTAMPTZ | Yes | NULL | Soft delete |

**Rationale:** Stored separately from `requests` because:
1. Responses may have different retention (e.g., keep requests longer than responses)
2. Response bodies are typically larger; separating them prevents bloating request lookups
3. Enables archiving responses to cold storage independently
4. 1:1 with `requests` via `request_id`

---

### 2.9 usage_records

**Purpose:** Aggregated usage data for fast analytics queries. Updated asynchronously from request logs.

```sql
CREATE TABLE usage_records (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id          UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    api_key_id      UUID REFERENCES api_keys(id) ON DELETE SET NULL,
    provider_config_id UUID REFERENCES provider_configs(id) ON DELETE SET NULL,
    provider_model_id  UUID REFERENCES provider_models(id) ON DELETE SET NULL,
    
    -- Time bucket
    period          TEXT NOT NULL CHECK (period IN ('hourly', 'daily', 'monthly')),
    period_start    TIMESTAMPTZ NOT NULL,
    
    -- Aggregated metrics
    request_count   INTEGER NOT NULL DEFAULT 0,
    request_success INTEGER NOT NULL DEFAULT 0,
    request_error   INTEGER NOT NULL DEFAULT 0,
    
    prompt_tokens       BIGINT NOT NULL DEFAULT 0,
    completion_tokens   BIGINT NOT NULL DEFAULT 0,
    total_tokens        BIGINT NOT NULL DEFAULT 0,
    
    input_cost      NUMERIC(18, 8) NOT NULL DEFAULT 0,
    output_cost     NUMERIC(18, 8) NOT NULL DEFAULT 0,
    total_cost      NUMERIC(18, 8) NOT NULL DEFAULT 0,
    
    -- Latency percentiles (stored as JSONB for flexibility)
    latency_ms_p50  INTEGER,
    latency_ms_p90  INTEGER,
    latency_ms_p99  INTEGER,
    latency_ms_avg  INTEGER,
    
    -- Cache metrics
    cache_hits      INTEGER NOT NULL DEFAULT 0,
    cache_misses    INTEGER NOT NULL DEFAULT 0,
    
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    deleted_at      TIMESTAMPTZ,
    
    UNIQUE(org_id, api_key_id, provider_config_id, provider_model_id, period, period_start)
) PARTITION BY RANGE (period_start);

CREATE INDEX idx_usage_org_period ON usage_records(org_id, period, period_start DESC) WHERE deleted_at IS NULL;
CREATE INDEX idx_usage_org_model ON usage_records(org_id, provider_model_id, period_start DESC) WHERE deleted_at IS NULL;
CREATE INDEX idx_usage_period_start ON usage_records(period_start DESC) WHERE deleted_at IS NULL;
```

| Column | Type | Nullable | Default | Notes |
|--------|------|----------|---------|-------|
| `id` | UUID | No | `gen_random_uuid()` | PK |
| `org_id` | UUID | No | — | FK → organizations |
| `api_key_id` | UUID | Yes | — | FK → api_keys; NULL = aggregated across all keys |
| `provider_config_id` | UUID | Yes | — | FK → provider_configs |
| `provider_model_id` | UUID | Yes | — | FK → provider_models |
| `period` | TEXT | No | — | hourly / daily / monthly |
| `period_start` | TIMESTAMPTZ | No | — | Start of aggregation window |
| `request_count` | INTEGER | No | `0` | Total requests |
| `request_success` | INTEGER | No | `0` | Successful requests |
| `request_error` | INTEGER | No | `0` | Failed requests |
| `prompt_tokens` | BIGINT | No | `0` | Input tokens sum |
| `completion_tokens` | BIGINT | No | `0` | Output tokens sum |
| `total_tokens` | BIGINT | No | `0` | Total tokens sum |
| `input_cost` | NUMERIC(18,8) | No | `0` | Input cost sum |
| `output_cost` | NUMERIC(18,8) | No | `0` | Output cost sum |
| `total_cost` | NUMERIC(18,8) | No | `0` | Total cost sum |
| `latency_ms_p50` | INTEGER | Yes | — | Median latency |
| `latency_ms_p90` | INTEGER | Yes | — | 90th percentile |
| `latency_ms_p99` | INTEGER | Yes | — | 99th percentile |
| `latency_ms_avg` | INTEGER | Yes | — | Average latency |
| `cache_hits` | INTEGER | No | `0` | Cache hit count |
| `cache_misses` | INTEGER | No | `0` | Cache miss count |
| `created_at` | TIMESTAMPTZ | No | `now()` | — |
| `updated_at` | TIMESTAMPTZ | No | `now()` | Trigger-updated |
| `deleted_at` | TIMESTAMPTZ | Yes | NULL | Soft delete |

**Rationale:** Materialized aggregation table. Populated by background workers from `requests` table. Enables sub-100ms analytics dashboards without scanning request logs. The unique constraint prevents double-counting. Partitions by `period_start` for efficient pruning of old aggregated data.

---

### 2.10 quotas

**Purpose:** Quota and budget configuration per organization.

```sql
CREATE TYPE quota_period AS ENUM ('minute', 'hour', 'day', 'month', 'total');
CREATE TYPE quota_metric AS ENUM ('requests', 'tokens', 'cost_usd');

CREATE TABLE quotas (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id          UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    api_key_id      UUID REFERENCES api_keys(id) ON DELETE CASCADE,
    
    name            TEXT NOT NULL,
    description     TEXT,
    
    -- What is being limited
    metric          quota_metric NOT NULL,
    period          quota_period NOT NULL,
    
    -- Limits
    limit_value     NUMERIC(18, 4) NOT NULL,
    warning_threshold NUMERIC(5, 2) NOT NULL DEFAULT 80.00,
    
    -- Scope
    applies_to      TEXT NOT NULL DEFAULT 'all'
                        CHECK (applies_to IN ('all', 'api_key', 'model', 'provider')),
    scope_filter    JSONB NOT NULL DEFAULT '{}',
    
    -- Action on exceeded
    action          TEXT NOT NULL DEFAULT 'block'
                        CHECK (action IN ('block', 'warn', 'throttle')),
    
    status          TEXT NOT NULL DEFAULT 'active'
                        CHECK (status IN ('active', 'inactive')),
    
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    deleted_at      TIMESTAMPTZ
);

CREATE INDEX idx_quotas_org ON quotas(org_id) WHERE deleted_at IS NULL;
CREATE INDEX idx_quotas_org_metric_period ON quotas(org_id, metric, period) WHERE deleted_at IS NULL AND status = 'active';
CREATE INDEX idx_quotas_api_key ON quotas(api_key_id) WHERE deleted_at IS NULL AND api_key_id IS NOT NULL;
```

| Column | Type | Nullable | Default | Notes |
|--------|------|----------|---------|-------|
| `id` | UUID | No | `gen_random_uuid()` | PK |
| `org_id` | UUID | No | — | FK → organizations |
| `api_key_id` | UUID | Yes | — | FK → api_keys; NULL = org-wide |
| `name` | TEXT | No | — | Human-readable label |
| `description` | TEXT | Yes | — | Optional documentation |
| `metric` | quota_metric | No | — | What to measure |
| `period` | quota_period | No | — | Reset window |
| `limit_value` | NUMERIC(18,4) | No | — | Maximum allowed |
| `warning_threshold` | NUMERIC(5,2) | No | `80.00` | Percentage at which to warn |
| `applies_to` | TEXT | No | `'all'` | Scope of restriction |
| `scope_filter` | JSONB | No | `'{}'` | Model/provider filters |
| `action` | TEXT | No | `'block'` | block / warn / throttle |
| `status` | TEXT | No | `'active'` | active / inactive |
| `created_at` | TIMESTAMPTZ | No | `now()` | — |
| `updated_at` | TIMESTAMPTZ | No | `now()` | Trigger-updated |
| `deleted_at` | TIMESTAMPTZ | Yes | NULL | Soft delete |

**Rationale:** Flexible quota system supporting multiple dimensions (requests, tokens, cost). `scope_filter` JSONB enables model-specific or provider-specific quotas without schema changes. `warning_threshold` enables proactive notifications.

---

### 2.11 quota_usage

**Purpose:** Current quota consumption. Updated in real-time or near real-time.

```sql
CREATE TABLE quota_usage (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id          UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    quota_id        UUID NOT NULL REFERENCES quotas(id) ON DELETE CASCADE,
    api_key_id      UUID REFERENCES api_keys(id) ON DELETE CASCADE,
    
    -- Current window
    period_start    TIMESTAMPTZ NOT NULL,
    period_end      TIMESTAMPTZ NOT NULL,
    
    -- Current usage
    current_value   NUMERIC(18, 4) NOT NULL DEFAULT 0,
    
    -- Denormalized for fast lookups
    limit_value     NUMERIC(18, 4) NOT NULL,
    metric          quota_metric NOT NULL,
    
    -- Status
    exceeded_at     TIMESTAMPTZ,
    warned_at       TIMESTAMPTZ,
    
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    deleted_at      TIMESTAMPTZ,
    
    UNIQUE(org_id, quota_id, api_key_id, period_start)
);

CREATE INDEX idx_quota_usage_org_quota ON quota_usage(org_id, quota_id, period_start) WHERE deleted_at IS NULL;
CREATE INDEX idx_quota_usage_api_key ON quota_usage(api_key_id) WHERE deleted_at IS NULL AND api_key_id IS NOT NULL;
```

| Column | Type | Nullable | Default | Notes |
|--------|------|----------|---------|-------|
| `id` | UUID | No | `gen_random_uuid()` | PK |
| `org_id` | UUID | No | — | FK → organizations |
| `quota_id` | UUID | No | — | FK → quotas |
| `api_key_id` | UUID | Yes | — | FK → api_keys |
| `period_start` | TIMESTAMPTZ | No | — | Window start |
| `period_end` | TIMESTAMPTZ | No | — | Window end |
| `current_value` | NUMERIC(18,4) | No | `0` | Consumed amount |
| `limit_value` | NUMERIC(18,4) | No | — | Denormalized from quota |
| `metric` | quota_metric | No | — | Denormalized from quota |
| `exceeded_at` | TIMESTAMPTZ | Yes | — | When quota was exceeded |
| `warned_at` | TIMESTAMPTZ | Yes | — | When warning was sent |
| `created_at` | TIMESTAMPTZ | No | `now()` | — |
| `updated_at` | TIMESTAMPTZ | No | `now()` | Trigger-updated |
| `deleted_at` | TIMESTAMPTZ | Yes | NULL | Soft delete |

**Rationale:** Denormalized `limit_value` and `metric` from `quotas` to avoid JOIN on the hot-path quota check. Updated via `INSERT ... ON CONFLICT DO UPDATE` (upsert) pattern for atomic increment. Rows are pre-created at window start or lazily on first request.

---

### 2.12 webhooks

**Purpose:** Webhook endpoint configurations per organization.

```sql
CREATE TYPE webhook_event AS ENUM (
    'request.completed', 'request.failed',
    'quota.warning', 'quota.exceeded',
    'provider.error', 'provider.recovered'
);

CREATE TABLE webhooks (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id          UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    
    name            TEXT NOT NULL,
    url             TEXT NOT NULL,
    secret_enc      BYTEA,
    
    events          webhook_event[] NOT NULL DEFAULT '{}',
    
    -- Headers to include in delivery
    custom_headers  JSONB NOT NULL DEFAULT '{}',
    
    -- Retry configuration
    max_retries     INTEGER NOT NULL DEFAULT 3,
    retry_interval_seconds INTEGER NOT NULL DEFAULT 60,
    timeout_seconds INTEGER NOT NULL DEFAULT 30,
    
    status          TEXT NOT NULL DEFAULT 'active'
                        CHECK (status IN ('active', 'inactive', 'failing')),
    
    -- Health tracking
    last_delivered_at   TIMESTAMPTZ,
    last_failure_at     TIMESTAMPTZ,
    consecutive_failures INTEGER NOT NULL DEFAULT 0,
    
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    deleted_at      TIMESTAMPTZ
);

CREATE INDEX idx_webhooks_org ON webhooks(org_id) WHERE deleted_at IS NULL;
CREATE INDEX idx_webhooks_status ON webhooks(status) WHERE deleted_at IS NULL;
CREATE INDEX idx_webhooks_events ON webhooks USING GIN (events);
```

| Column | Type | Nullable | Default | Notes |
|--------|------|----------|---------|-------|
| `id` | UUID | No | `gen_random_uuid()` | PK |
| `org_id` | UUID | No | — | FK → organizations |
| `name` | TEXT | No | — | Human-readable label |
| `url` | TEXT | No | — | HTTPS endpoint URL |
| `secret_enc` | BYTEA | Yes | — | Encrypted signing secret |
| `events` | webhook_event[] | No | `'{}'` | Subscribed events |
| `custom_headers` | JSONB | No | `'{}'` | Additional HTTP headers |
| `max_retries` | INTEGER | No | `3` | Max retry attempts |
| `retry_interval_seconds` | INTEGER | No | `60` | Seconds between retries |
| `timeout_seconds` | INTEGER | No | `30` | Request timeout |
| `status` | TEXT | No | `'active'` | active / inactive / failing |
| `last_delivered_at` | TIMESTAMPTZ | Yes | — | Last successful delivery |
| `last_failure_at` | TIMESTAMPTZ | Yes | — | Last failed delivery |
| `consecutive_failures` | INTEGER | No | `0` | Auto-disables at threshold |
| `created_at` | TIMESTAMPTZ | No | `now()` | — |
| `updated_at` | TIMESTAMPTZ | No | `now()` | Trigger-updated |
| `deleted_at` | TIMESTAMPTZ | Yes | NULL | Soft delete |

**Rationale:** `events` array with GIN index enables efficient webhook lookup by event type. `consecutive_failures` enables automatic circuit-breaking. `secret_enc` is used for HMAC-SHA256 signature on webhook payloads.

---

### 2.13 webhook_deliveries

**Purpose:** Webhook delivery log. Tracks every delivery attempt.

```sql
CREATE TABLE webhook_deliveries (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id          UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    webhook_id      UUID NOT NULL REFERENCES webhooks(id) ON DELETE CASCADE,
    
    event_type      webhook_event NOT NULL,
    payload         JSONB NOT NULL,
    
    -- Delivery tracking
    attempt_number  INTEGER NOT NULL DEFAULT 1,
    
    -- Request/response
    request_headers JSONB NOT NULL DEFAULT '{}',
    request_body    TEXT,
    response_status INTEGER,
    response_body   TEXT,
    response_headers JSONB NOT NULL DEFAULT '{}',
    
    -- Result
    status          TEXT NOT NULL
                        CHECK (status IN ('pending', 'delivered', 'failed', 'expired')),
    error_message   TEXT,
    
    -- Timing
    scheduled_at    TIMESTAMPTZ NOT NULL,
    started_at      TIMESTAMPTZ,
    completed_at    TIMESTAMPTZ,
    
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    deleted_at      TIMESTAMPTZ
) PARTITION BY RANGE (created_at);

CREATE INDEX idx_webhook_deliveries_webhook ON webhook_deliveries(webhook_id, created_at DESC) WHERE deleted_at IS NULL;
CREATE INDEX idx_webhook_deliveries_status ON webhook_deliveries(status) WHERE deleted_at IS NULL AND status IN ('pending', 'failed');
CREATE INDEX idx_webhook_deliveries_scheduled ON webhook_deliveries(scheduled_at) WHERE status = 'pending' AND deleted_at IS NULL;
```

| Column | Type | Nullable | Default | Notes |
|--------|------|----------|---------|-------|
| `id` | UUID | No | `gen_random_uuid()` | PK |
| `org_id` | UUID | No | — | FK → organizations |
| `webhook_id` | UUID | No | — | FK → webhooks |
| `event_type` | webhook_event | No | — | What triggered this |
| `payload` | JSONB | No | — | Event payload |
| `attempt_number` | INTEGER | No | `1` | Retry attempt count |
| `request_headers` | JSONB | No | `'{}'` | Headers sent |
| `request_body` | TEXT | Yes | — | Body sent |
| `response_status` | INTEGER | Yes | — | HTTP response status |
| `response_body` | TEXT | Yes | — | HTTP response body |
| `response_headers` | JSONB | No | `'{}'` | Response headers |
| `status` | TEXT | No | — | pending / delivered / failed / expired |
| `error_message` | TEXT | Yes | — | Error details |
| `scheduled_at` | TIMESTAMPTZ | No | — | When to attempt delivery |
| `started_at` | TIMESTAMPTZ | Yes | — | When attempt started |
| `completed_at` | TIMESTAMPTZ | Yes | — | When attempt finished |
| `created_at` | TIMESTAMPTZ | No | `now()` | — |
| `updated_at` | TIMESTAMPTZ | No | `now()` | Trigger-updated |
| `deleted_at` | TIMESTAMPTZ | Yes | NULL | Soft delete |

**Rationale:** Partitioned by `created_at` for retention management. `idx_webhook_deliveries_scheduled` partial index supports the delivery worker polling pattern (find pending jobs ordered by scheduled time).

---

### 2.14 cache_metadata

**Purpose:** Tracks cache entries stored in an external cache (Redis/Memcached). Enables cache analytics and invalidation.

```sql
CREATE TABLE cache_metadata (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id          UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    
    -- Cache key (hashed)
    cache_key_hash  TEXT NOT NULL,
    cache_key_preview TEXT,
    
    -- Content metadata
    model_id        TEXT NOT NULL,
    prompt_preview  TEXT,
    prompt_tokens   INTEGER NOT NULL DEFAULT 0,
    
    -- Storage info
    storage_backend TEXT NOT NULL DEFAULT 'redis',
    ttl_seconds     INTEGER NOT NULL DEFAULT 3600,
    expires_at      TIMESTAMPTZ NOT NULL,
    
    -- Usage tracking
    hit_count       INTEGER NOT NULL DEFAULT 0,
    last_hit_at     TIMESTAMPTZ,
    
    -- Content hash for invalidation
    content_hash    TEXT,
    
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    deleted_at      TIMESTAMPTZ
);

CREATE UNIQUE INDEX uk_cache_metadata_org_hash ON cache_metadata(org_id, cache_key_hash) WHERE deleted_at IS NULL;
CREATE INDEX idx_cache_metadata_expires ON cache_metadata(expires_at) WHERE deleted_at IS NULL;
CREATE INDEX idx_cache_metadata_org_model ON cache_metadata(org_id, model_id) WHERE deleted_at IS NULL;
```

| Column | Type | Nullable | Default | Notes |
|--------|------|----------|---------|-------|
| `id` | UUID | No | `gen_random_uuid()` | PK |
| `org_id` | UUID | No | — | FK → organizations |
| `cache_key_hash` | TEXT | No | — | Hash of cache key |
| `cache_key_preview` | TEXT | Yes | — | Human-readable key preview |
| `model_id` | TEXT | No | — | Cached model |
| `prompt_preview` | TEXT | Yes | — | First 200 chars of prompt |
| `prompt_tokens` | INTEGER | No | `0` | Token count |
| `storage_backend` | TEXT | No | `'redis'` | Cache backend name |
| `ttl_seconds` | INTEGER | No | `3600` | Cache TTL |
| `expires_at` | TIMESTAMPTZ | No | — | Expiration timestamp |
| `hit_count` | INTEGER | No | `0` | Number of cache hits |
| `last_hit_at` | TIMESTAMPTZ | Yes | — | Last cache hit time |
| `content_hash` | TEXT | Yes | — | Content hash for change detection |
| `created_at` | TIMESTAMPTZ | No | `now()` | — |
| `updated_at` | TIMESTAMPTZ | No | `now()` | Trigger-updated |
| `deleted_at` | TIMESTAMPTZ | Yes | NULL | Soft delete |

**Rationale:** Does not store actual cached content (that's in Redis) — only metadata for analytics and management. `hit_count` enables cache effectiveness analysis. `expires_at` supports TTL-based cleanup queries.

---

### 2.15 audit_log

**Purpose:** Immutable audit trail of all significant actions.

```sql
CREATE TYPE audit_action AS ENUM (
    'create', 'update', 'delete', 'login', 'logout',
    'api_key.created', 'api_key.revoked',
    'provider.created', 'provider.updated', 'provider.deleted',
    'quota.exceeded', 'quota.warning',
    'webhook.created', 'webhook.deleted',
    'routing_rule.created', 'routing_rule.updated',
    'settings.updated', 'billing.updated'
);

CREATE TABLE audit_log (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id          UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    user_id         UUID REFERENCES users(id) ON DELETE SET NULL,
    api_key_id      UUID REFERENCES api_keys(id) ON DELETE SET NULL,
    
    action          audit_action NOT NULL,
    entity_type     TEXT NOT NULL,
    entity_id       TEXT,
    
    -- Change details
    old_values      JSONB,
    new_values      JSONB,
    summary         TEXT NOT NULL,
    
    -- Request context
    ip_address      INET,
    user_agent      TEXT,
    request_id      UUID,
    
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    deleted_at      TIMESTAMPTZ
) PARTITION BY RANGE (created_at);

CREATE INDEX idx_audit_org_created ON audit_log(org_id, created_at DESC) WHERE deleted_at IS NULL;
CREATE INDEX idx_audit_action ON audit_log(org_id, action, created_at DESC) WHERE deleted_at IS NULL;
CREATE INDEX idx_audit_entity ON audit_log(entity_type, entity_id) WHERE deleted_at IS NULL;
```

| Column | Type | Nullable | Default | Notes |
|--------|------|----------|---------|-------|
| `id` | UUID | No | `gen_random_uuid()` | PK |
| `org_id` | UUID | No | — | FK → organizations |
| `user_id` | UUID | Yes | — | FK → users; who performed action |
| `api_key_id` | UUID | Yes | — | FK → api_keys; if via API |
| `action` | audit_action | No | — | Action type |
| `entity_type` | TEXT | No | — | Table/entity name |
| `entity_id` | TEXT | Yes | — | Affected entity ID |
| `old_values` | JSONB | Yes | — | Previous state |
| `new_values` | JSONB | Yes | — | New state |
| `summary` | TEXT | No | — | Human-readable description |
| `ip_address` | INET | Yes | — | Source IP |
| `user_agent` | TEXT | Yes | — | Client user agent |
| `request_id` | UUID | Yes | — | Correlated request |
| `created_at` | TIMESTAMPTZ | No | `now()` | Partition key |
| `deleted_at` | TIMESTAMPTZ | Yes | NULL | Soft delete (rarely used) |

**Rationale:** Append-only table. `old_values`/`new_values` capture full change context for compliance. `summary` provides human-readable audit entries. Partitioned by month for retention. No `updated_at` — audit entries are immutable.

---

### 2.16 sessions

**Purpose:** Dashboard session tokens for cookie-based authentication.

```sql
CREATE TABLE sessions (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id          UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    user_id         UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    
    -- Session token (hashed)
    token_hash      TEXT NOT NULL,
    
    -- Metadata
    ip_address      INET,
    user_agent      TEXT,
    
    -- Timing
    expires_at      TIMESTAMPTZ NOT NULL,
    last_active_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    deleted_at      TIMESTAMPTZ
);

CREATE UNIQUE INDEX uk_sessions_token ON sessions(token_hash) WHERE deleted_at IS NULL;
CREATE INDEX idx_sessions_user ON sessions(user_id) WHERE deleted_at IS NULL;
CREATE INDEX idx_sessions_expires ON sessions(expires_at) WHERE deleted_at IS NULL;
```

| Column | Type | Nullable | Default | Notes |
|--------|------|----------|---------|-------|
| `id` | UUID | No | `gen_random_uuid()` | PK |
| `org_id` | UUID | No | — | FK → organizations |
| `user_id` | UUID | No | — | FK → users |
| `token_hash` | TEXT | No | — | SHA-256 of session token |
| `ip_address` | INET | Yes | — | Creation IP |
| `user_agent` | TEXT | Yes | — | Creation UA |
| `expires_at` | TIMESTAMPTZ | No | — | Session expiration |
| `last_active_at` | TIMESTAMPTZ | No | `now()` | Last activity |
| `created_at` | TIMESTAMPTZ | No | `now()` | — |
| `updated_at` | TIMESTAMPTZ | No | `now()` | Trigger-updated |
| `deleted_at` | TIMESTAMPTZ | Yes | NULL | Soft delete (session invalidation) |

**Rationale:** Only stores token hash — raw tokens exist only in client cookies. `last_active_at` enables session freshness tracking. `expires_at` partial index supports efficient cleanup of expired sessions.

---

## 3. Multi-Tenancy Design

### 3.1 Chosen Approach: Row-Level Security + Tenant Column

```
┌─────────────────────────────────────────────────────┐
│              APPLICATION LAYER                       │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐ │
│  │  API Layer  │  │  Dashboard  │  │  Workers    │ │
│  └──────┬──────┘  └──────┬──────┘  └──────┬──────┘ │
│         │                │                │         │
│         └────────────────┼────────────────┘         │
│                          ▼                          │
│           ┌──────────────────────────┐              │
│           │   Always filter by org_id │              │
│           │   in application queries   │              │
│           └──────────────────────────┘              │
└─────────────────────────────────────────────────────┘
                          │
                          ▼
┌─────────────────────────────────────────────────────┐
│            DATABASE LAYER (PostgreSQL)              │
│                                                     │
│  ┌───────────────────────────────────────────────┐  │
│  │  Row-Level Security Policies (defense in depth) │ │
│  │  CREATE POLICY tenant_isolation ON requests     │ │
│  │    USING (org_id = current_setting('app.org_id')│ │
│  │          ::UUID);                               │ │
│  └───────────────────────────────────────────────┘  │
│                                                     │
│  ┌───────────────────────────────────────────────┐  │
│  │  org_id column on every tenant table            │ │
│  │  FK to organizations(id) ON DELETE CASCADE      │ │
│  └───────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────┘
```

### 3.2 Row-Level Security Implementation

```sql
-- Enable RLS on tenant tables
ALTER TABLE users ENABLE ROW LEVEL SECURITY;
ALTER TABLE api_keys ENABLE ROW LEVEL SECURITY;
ALTER TABLE provider_configs ENABLE ROW LEVEL SECURITY;
ALTER TABLE provider_models ENABLE ROW LEVEL SECURITY;
ALTER TABLE routing_rules ENABLE ROW LEVEL SECURITY;
ALTER TABLE requests ENABLE ROW LEVEL SECURITY;
ALTER TABLE responses ENABLE ROW LEVEL SECURITY;
ALTER TABLE usage_records ENABLE ROW LEVEL SECURITY;
ALTER TABLE quotas ENABLE ROW LEVEL SECURITY;
ALTER TABLE quota_usage ENABLE ROW LEVEL SECURITY;
ALTER TABLE webhooks ENABLE ROW LEVEL SECURITY;
ALTER TABLE webhook_deliveries ENABLE ROW LEVEL SECURITY;
ALTER TABLE cache_metadata ENABLE ROW LEVEL SECURITY;
ALTER TABLE audit_log ENABLE ROW LEVEL SECURITY;
ALTER TABLE sessions ENABLE ROW LEVEL SECURITY;

-- Set organization ID per connection/session
SET app.org_id = '00000000-0000-0000-0000-000000000000';

-- Create tenant isolation policy (template)
CREATE POLICY tenant_isolation_users ON users
    USING (org_id = current_setting('app.org_id')::UUID);

CREATE POLICY tenant_isolation_requests ON requests
    USING (org_id = current_setting('app.org_id')::UUID);

CREATE POLICY tenant_isolation_api_keys ON api_keys
    USING (org_id = current_setting('app.org_id')::UUID);

-- Apply pattern to all tenant tables...
```

### 3.3 Tenant Isolation Enforcement

| Layer | Mechanism | Responsibility |
|-------|-----------|----------------|
| Application | `WHERE org_id = $1` on every query | Primary; performance-optimized |
| Connection | `SET app.org_id` on connection acquire | Sets RLS context |
| Database | RLS policy `USING (org_id = current_setting('app.org_id')::UUID)` | Defense in depth |
| Database | FK constraint with `ON DELETE CASCADE` | Data integrity on org deletion |

### 3.4 Query Patterns

**Every tenant-scoped query includes `org_id` as the first filter:**

```sql
-- Correct (uses composite index efficiently)
SELECT * FROM requests
WHERE org_id = '...' AND created_at > now() - interval '1 hour'
ORDER BY created_at DESC
LIMIT 100;

-- Correct (quota check)
SELECT * FROM quota_usage
WHERE org_id = '...' AND quota_id = '...' AND period_start = '...';

-- Anti-pattern (RLS would catch, but avoid)
SELECT * FROM requests WHERE created_at > now() - interval '1 hour';
-- Missing org_id filter — would scan all partitions
```

### 3.5 Performance Implications

| Consideration | Impact | Mitigation |
|---------------|--------|------------|
| RLS planning overhead | ~5-10% on simple queries | App-level filtering is primary; RLS as fallback |
| `org_id` on every table | +16 bytes/row | Minimal; enables tenant scoping and tenant-level partitioning |
| Composite indexes with `org_id` leading | Slightly larger indexes | Required for tenant-scoped query performance |
| `ON DELETE CASCADE` on org_id | Deletes all org data on org deletion | Acceptable; deletion is rare and explicit |

---

## 4. Request Log Design

### 4.1 Storage Strategy

```sql
-- Partitioning: Monthly range partitions on created_at
CREATE TABLE requests_y2024m01 PARTITION OF requests
    FOR VALUES FROM ('2024-01-01') TO ('2024-02-01');
CREATE TABLE requests_y2024m02 PARTITION OF requests
    FOR VALUES FROM ('2024-02-01') TO ('2024-03-01');
-- ... auto-created by application or migration

-- Same for responses and webhook_deliveries
CREATE TABLE responses_y2024m01 PARTITION OF responses
    FOR VALUES FROM ('2024-01-01') TO ('2024-02-01');

CREATE TABLE webhook_deliveries_y2024m01 PARTITION OF webhook_deliveries
    FOR VALUES FROM ('2024-01-01') TO ('2024-02-01');
```

### 4.2 What to Store

| Field | Storage | Max Size | Rationale |
|-------|---------|----------|-----------|
| Request headers | JSONB | 8KB | Selected headers only; auth stripped |
| Request body | TEXT | 64KB | Truncated; full body in object storage if needed |
| Response body | TEXT | 64KB | Truncated; streaming responses store first chunk |
| Token counts | INTEGER columns | 4 bytes each | Denormalized for fast analytics |
| Cost | NUMERIC(18,8) | Variable | Computed at write time |
| Latency breakdown | 3 INTEGER columns | 12 bytes | Gateway, provider, total |
| Metadata | JSONB | 16KB | Provider-specific extras |

### 4.3 Body Truncation Strategy

```sql
-- Application-level truncation before INSERT:
-- 1. Request body > 64KB: truncate, set request_body_truncated = true
-- 2. Response body > 64KB: truncate, set response_body_truncated = true
-- 3. Full bodies stored in object storage (S3) with key: {org_id}/{date}/{trace_id}
-- 4. Object storage URL stored in metadata JSONB if full body archived
```

### 4.4 Partitioning Strategy

```
┌─────────────────────────────────────────────────────────┐
│                    requests (parent)                      │
├─────────────┬─────────────┬─────────────┬───────────────┤
│ 2024-01    │  2024-02    │  2024-03    │  2024-04 ...  │
│ (active)   │  (active)   │  (active)   │  (future)     │
├─────────────┼─────────────┼─────────────┼───────────────┤
│ ~10M rows   │  ~10M rows  │  ~10M rows  │  (empty)      │
│ indexed     │  indexed    │  indexed    │  no index     │
│ queriable   │  queriable  │  queriable  │  not yet born │
└─────────────┴─────────────┴─────────────┴───────────────┘
         │
         ▼ after 3 months
┌─────────────────────────────────────────────────────────┐
│  Old partitions → DETACH → archive to S3 → DROP         │
│  (or ATTACH to cold storage table on slower disk)        │
└─────────────────────────────────────────────────────────┘
```

| Partition Type | Range | Management |
|----------------|-------|------------|
| Monthly | `FOR VALUES FROM (month_start) TO (month_start + 1 month)` | Auto-created 1 month ahead |
| Detach old | After retention period | Background job |
| Archive | Compressed Parquet to S3 | Background job |
| Drop | After archive confirmed | Background job |

### 4.5 Retention Policy

| Data Type | Hot Storage (PostgreSQL) | Warm (Compressed) | Cold (S3) |
|-----------|-------------------------|-------------------|-----------|
| Request logs | 90 days | 90-365 days | 1-7 years |
| Response logs | 30 days | 30-90 days | 90 days - 1 year |
| Usage aggregates | Indefinite | — | — |
| Audit logs | 365 days | 1-3 years | 3-7 years |
| Webhook deliveries | 30 days | 30-90 days | 1 year |

### 4.6 Archiving Strategy

```sql
-- 1. Create archiving function
CREATE OR REPLACE FUNCTION archive_old_partitions(
    p_table TEXT,
    p_older_than TIMESTAMPTZ
) RETURNS void AS $$
DECLARE
    partition_name TEXT;
BEGIN
    -- Find partitions older than threshold
    FOR partition_name IN
        SELECT inhrelid::regclass::text
        FROM pg_inherits
        WHERE inhparent = p_table::regclass
        AND inhrelid::regclass::text < p_table || '_y' || to_char(p_older_than, 'YYYY') || 'm' || to_char(p_older_than, 'MM')
    LOOP
        -- Detach partition
        EXECUTE format('ALTER TABLE %I DETACH PARTITION %I', p_table, partition_name);
        
        -- Export to Parquet via pg_duckdb or COPY TO CSV → convert
        EXECUTE format('COPY %I TO ''/tmp/%s.parquet'' WITH (FORMAT parquet)', partition_name, partition_name);
        
        -- Upload to S3 (done by application)
        -- DROP TABLE partition_name; -- After S3 upload confirmed
    END LOOP;
END;
$$ LANGUAGE plpgsql;

-- 2. Run nightly via pg_cron
SELECT cron.schedule('archive-requests', '0 3 * * *', 
    $$SELECT archive_old_partitions('requests', now() - interval '90 days')$$);
```

### 4.7 Aggregation Pipeline

```
┌──────────────┐     ┌──────────────┐     ┌──────────────┐
│   requests   │────▶│  Aggregator  │────▶│usage_records │
│  (stream)    │     │  (worker)    │     │ (pre-aggreg) │
└──────────────┘     └──────────────┘     └──────────────┘
                            │
                            ▼
                    ┌──────────────┐
                    │  quota_usage  │
                    │  (upsert)    │
                    └──────────────┘
```

```sql
-- Aggregation query run every minute by background worker
INSERT INTO usage_records (
    org_id, api_key_id, provider_config_id, provider_model_id,
    period, period_start, request_count, request_success, request_error,
    prompt_tokens, completion_tokens, total_tokens,
    input_cost, output_cost, total_cost
)
SELECT 
    org_id, api_key_id, provider_config_id, provider_model_id,
    'hourly', date_trunc('hour', created_at),
    COUNT(*),
    COUNT(*) FILTER (WHERE status = 'success'),
    COUNT(*) FILTER (WHERE status = 'error'),
    SUM(prompt_tokens), SUM(completion_tokens), SUM(total_tokens),
    SUM(input_cost), SUM(output_cost), SUM(total_cost)
FROM requests
WHERE created_at >= date_trunc('hour', now() - interval '1 hour')
  AND created_at < date_trunc('hour', now())
  AND aggregated_at IS NULL
GROUP BY org_id, api_key_id, provider_config_id, provider_model_id, date_trunc('hour', created_at)
ON CONFLICT (org_id, api_key_id, provider_config_id, provider_model_id, period, period_start)
DO UPDATE SET
    request_count = usage_records.request_count + EXCLUDED.request_count,
    request_success = usage_records.request_success + EXCLUDED.request_success,
    request_error = usage_records.request_error + EXCLUDED.request_error,
    prompt_tokens = usage_records.prompt_tokens + EXCLUDED.prompt_tokens,
    completion_tokens = usage_records.completion_tokens + EXCLUDED.completion_tokens,
    total_tokens = usage_records.total_tokens + EXCLUDED.total_tokens,
    input_cost = usage_records.input_cost + EXCLUDED.input_cost,
    output_cost = usage_records.output_cost + EXCLUDED.output_cost,
    total_cost = usage_records.total_cost + EXCLUDED.total_cost;
```

---

## 5. Index Strategy

### 5.1 Complete Index Reference

#### organizations

| Index | Type | Columns | Query Pattern | Write Cost |
|-------|------|---------|---------------|------------|
| `PRIMARY KEY` | B-tree | `id` | Single-row lookup | Low |
| `uk_organizations_slug` | B-tree (partial) | `slug` WHERE deleted_at IS NULL | Lookup by slug | Low |
| `idx_organizations_status` | B-tree (partial) | `status` WHERE deleted_at IS NULL | List active orgs | Low |

#### users

| Index | Type | Columns | Query Pattern | Write Cost |
|-------|------|---------|---------------|------------|
| `PRIMARY KEY` | B-tree | `id` | Single-row lookup | Low |
| `uk_users_org_email` | B-tree (partial) | `org_id, email` WHERE deleted_at IS NULL | Login by email | Medium |
| `idx_users_org_id` | B-tree (partial) | `org_id` WHERE deleted_at IS NULL | List org users | Low |

#### api_keys

| Index | Type | Columns | Query Pattern | Write Cost |
|-------|------|---------|---------------|------------|
| `PRIMARY KEY` | B-tree | `id` | Key reference | Low |
| `uk_api_keys_key_hash` | B-tree (partial) | `key_hash` WHERE deleted_at IS NULL | Authenticate request | Medium |
| `idx_api_keys_org_id` | B-tree (partial) | `org_id` WHERE deleted_at IS NULL | List org keys | Low |
| `idx_api_keys_key_prefix` | B-tree (partial) | `key_prefix` WHERE deleted_at IS NULL | UI key search | Low |

#### provider_configs

| Index | Type | Columns | Query Pattern | Write Cost |
|-------|------|---------|---------------|------------|
| `PRIMARY KEY` | B-tree | `id` | Config reference | Low |
| `uk_provider_configs_org_name` | B-tree (partial) | `org_id, name` WHERE deleted_at IS NULL | Named lookup | Low |
| `idx_provider_configs_org_id` | B-tree (partial) | `org_id` WHERE deleted_at IS NULL | List org providers | Low |
| `idx_provider_configs_kind` | B-tree | `kind` | Filter by provider type | Low |

#### provider_models

| Index | Type | Columns | Query Pattern | Write Cost |
|-------|------|---------|---------------|------------|
| `PRIMARY KEY` | B-tree | `id` | Model reference | Low |
| `uk_provider_models_provider_model` | B-tree (partial) | `provider_config_id, model_id` WHERE deleted_at IS NULL | Unique model per provider | Low |
| `idx_provider_models_org_id` | B-tree (partial) | `org_id` WHERE deleted_at IS NULL | List org models | Low |
| `idx_provider_models_alias` | GIN (partial) | `aliases` WHERE deleted_at IS NULL | Route by alias | Medium |

#### routing_rules

| Index | Type | Columns | Query Pattern | Write Cost |
|-------|------|---------|---------------|------------|
| `PRIMARY KEY` | B-tree | `id` | Rule reference | Low |
| `uk_routing_rules_org_name` | B-tree (partial) | `org_id, name` WHERE deleted_at IS NULL | Named lookup | Low |
| `idx_routing_rules_org_priority` | B-tree (partial) | `org_id, priority` WHERE deleted_at IS NULL AND status='active' | Rule evaluation order | Medium |
| `idx_routing_rules_match_model` | B-tree (partial) | `org_id, match_model` WHERE deleted_at IS NULL AND status='active' | Model-specific routing | Medium |
| `idx_routing_rules_conditions` | GIN (partial) | `conditions` WHERE deleted_at IS NULL | Complex condition matching | Medium |

#### requests (highest-write table)

| Index | Type | Columns | Query Pattern | Write Cost |
|-------|------|---------|---------------|------------|
| `PRIMARY KEY` | B-tree | `id` | Row lookup | Low (on partition only) |
| `idx_requests_org_created` | B-tree (partial) | `org_id, created_at DESC` | Recent requests per org | Medium |
| `idx_requests_api_key_created` | B-tree (partial) | `api_key_id, created_at DESC` | Key usage history | Medium |
| `idx_requests_trace_id` | B-tree (partial) | `trace_id` | Distributed tracing | Medium |
| `idx_requests_status` | B-tree (partial) | `status` | Status filtering | Medium |
| `idx_requests_model` | B-tree (partial) | `org_id, model_routed, created_at DESC` | Model usage analytics | Medium |
| `idx_requests_cache` | B-tree (partial) | `org_id, cache_key_hash` WHERE cache_hit=true | Cache hit lookup | Low |
| `idx_requests_metadata` | GIN (partial) | `metadata` WHERE deleted_at IS NULL | Metadata search | High |

**Index Write Tradeoff Analysis:**
- Each index on `requests` adds ~5-10ms to INSERT time
- 8 indexes × 7ms = ~56ms additional INSERT latency
- Mitigation: Use `UNLOGGED` or bulk COPY for backfill; async inserts with queue for highest throughput
- `idx_requests_metadata` GIN is highest cost — consider omitting if metadata search is rare

#### responses

| Index | Type | Columns | Query Pattern | Write Cost |
|-------|------|---------|---------------|------------|
| `PRIMARY KEY` | B-tree | `id` | Row lookup | Low |
| `idx_responses_request_id` | B-tree (partial) | `request_id` WHERE deleted_at IS NULL | Join to requests | Medium |
| `idx_responses_org_created` | B-tree (partial) | `org_id, created_at DESC` | Recent responses | Medium |
| `idx_responses_status` | B-tree (partial) | `status_code` WHERE deleted_at IS NULL | Error analysis | Low |

#### usage_records

| Index | Type | Columns | Query Pattern | Write Cost |
|-------|------|---------|---------------|------------|
| `PRIMARY KEY` | B-tree | `id` | Row lookup | Low |
| `UNIQUE` | B-tree | `org_id, api_key_id, provider_config_id, provider_model_id, period, period_start` | Upsert aggregation | Medium |
| `idx_usage_org_period` | B-tree (partial) | `org_id, period, period_start DESC` | Usage dashboard | Medium |
| `idx_usage_org_model` | B-tree (partial) | `org_id, provider_model_id, period_start DESC` | Model usage | Medium |
| `idx_usage_period_start` | B-tree (partial) | `period_start DESC` | Cross-org analytics | Low |

#### quotas

| Index | Type | Columns | Query Pattern | Write Cost |
|-------|------|---------|---------------|------------|
| `PRIMARY KEY` | B-tree | `id` | Row lookup | Low |
| `idx_quotas_org` | B-tree (partial) | `org_id` WHERE deleted_at IS NULL | List quotas | Low |
| `idx_quotas_org_metric_period` | B-tree (partial) | `org_id, metric, period` WHERE deleted_at IS NULL AND status='active' | Active quota lookup | Medium |
| `idx_quotas_api_key` | B-tree (partial) | `api_key_id` WHERE deleted_at IS NULL AND api_key_id IS NOT NULL | Key-specific quotas | Low |

#### quota_usage

| Index | Type | Columns | Query Pattern | Write Cost |
|-------|------|---------|---------------|------------|
| `PRIMARY KEY` | B-tree | `id` | Row lookup | Low |
| `UNIQUE` | B-tree | `org_id, quota_id, api_key_id, period_start` | Upsert on increment | High (hot path) |
| `idx_quota_usage_org_quota` | B-tree (partial) | `org_id, quota_id, period_start` | Quota status check | High (hot path) |
| `idx_quota_usage_api_key` | B-tree (partial) | `api_key_id` WHERE deleted_at IS NULL | Key usage | Medium |

#### webhook_deliveries

| Index | Type | Columns | Query Pattern | Write Cost |
|-------|------|---------|---------------|------------|
| `PRIMARY KEY` | B-tree | `id` | Row lookup | Low |
| `idx_webhook_deliveries_webhook` | B-tree (partial) | `webhook_id, created_at DESC` | Delivery history | Medium |
| `idx_webhook_deliveries_status` | B-tree (partial) | `status` WHERE deleted_at IS NULL AND status IN ('pending','failed') | Failed delivery retry | Medium |
| `idx_webhook_deliveries_scheduled` | B-tree (partial) | `scheduled_at` WHERE status='pending' | Delivery worker polling | High |

#### audit_log

| Index | Type | Columns | Query Pattern | Write Cost |
|-------|------|---------|---------------|------------|
| `PRIMARY KEY` | B-tree | `id` | Row lookup | Low |
| `idx_audit_org_created` | B-tree (partial) | `org_id, created_at DESC` | Audit trail view | Medium |
| `idx_audit_action` | B-tree (partial) | `org_id, action, created_at DESC` | Filter by action | Medium |
| `idx_audit_entity` | B-tree (partial) | `entity_type, entity_id` | Entity history | Medium |

---

## 6. Migration Strategy

### 6.1 Tool Choice

| Tool | Decision | Rationale |
|------|----------|-----------|
| Primary | **sqlx migrate** (if Rust backend) or **refinery** | Schema versioning with reversible migrations |
| Alternative | **pgroll** (for zero-downtime) | For large table migrations (adding columns to requests) |
| Seeding | **Application-level seed scripts** | Environment-aware seeding |

### 6.2 Migration File Naming

```
migrations/
├── 0001_create_extensions.sql          -- Enable required extensions
├── 0002_create_organizations.sql       -- Tenant root
├── 0003_create_users.sql               -- Auth users
├── 0004_create_api_keys.sql            -- API key management
├── 0005_create_provider_configs.sql    -- Provider setup
├── 0006_create_provider_models.sql     -- Model registry
├── 0007_create_routing_rules.sql       -- Routing configuration
├── 0008_create_requests_partitioned.sql -- Request log (partitioned)
├── 0009_create_responses_partitioned.sql -- Response log
├── 0010_create_usage_records.sql       -- Aggregated analytics
├── 0011_create_quotas.sql              -- Quota configuration
├── 0012_create_quota_usage.sql         -- Quota tracking
├── 0013_create_webhooks.sql            -- Webhook config
├── 0014_create_webhook_deliveries.sql  -- Delivery log
├── 0015_create_cache_metadata.sql      -- Cache tracking
├── 0016_create_audit_log.sql           -- Audit trail
├── 0017_create_sessions.sql            -- Dashboard sessions
├── 0018_create_indexes.sql             -- All indexes
├── 0019_create_triggers.sql            -- Updated_at triggers
├── 0020_create_rls_policies.sql        -- Row-level security
├── 0021_create_partitions.sql          -- Initial partitions
└── 0022_seed_default_data.sql          -- Default enum values
```

### 6.3 Migration Template

```sql
-- migrations/XXXX_migration_name.sql
-- +++ Up Migration +++

-- Description: What this migration does
-- Author: Name
-- Date: YYYY-MM-DD

BEGIN;

-- DDL here

COMMIT;

-- +++ Down Migration +++

BEGIN;

-- Reverse DDL here

COMMIT;
```

### 6.4 Rollback Strategy

| Scenario | Strategy |
|----------|----------|
| Migration fails during deploy | Transaction rollback; migration marked failed; fix and retry |
| Data migration is too slow | Run in batches; use `pg_background` or application worker |
| Column added to large table | Use `pgroll` for zero-downtime; add as nullable with default, backfill, then set NOT NULL |
| Partition management | Detach old partitions before dropping; always verify no active queries |
| Irreversible migration | Create backup table first; document in migration header |

### 6.5 Seeding Strategy

```sql
-- migrations/0022_seed_default_data.sql
BEGIN;

-- Insert default provider kinds (enums are already created)
-- No seed data needed for enums; they are types

-- Seed a system organization for internal use (optional)
INSERT INTO organizations (id, name, slug, plan_tier, status)
VALUES (
    '00000000-0000-0000-0000-000000000001',
    'System',
    'system',
    'enterprise',
    'active'
)
ON CONFLICT DO NOTHING;

COMMIT;
```

### 6.6 Triggers

```sql
-- Auto-update updated_at on all tables
CREATE OR REPLACE FUNCTION fn_update_timestamp()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = now();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Apply to all tables with updated_at
CREATE TRIGGER trg_organizations_updated_at
    BEFORE UPDATE ON organizations
    FOR EACH ROW EXECUTE FUNCTION fn_update_timestamp();

CREATE TRIGGER trg_users_updated_at
    BEFORE UPDATE ON users
    FOR EACH ROW EXECUTE FUNCTION fn_update_timestamp();

CREATE TRIGGER trg_api_keys_updated_at
    BEFORE UPDATE ON api_keys
    FOR EACH ROW EXECUTE FUNCTION fn_update_timestamp();

CREATE TRIGGER trg_provider_configs_updated_at
    BEFORE UPDATE ON provider_configs
    FOR EACH ROW EXECUTE FUNCTION fn_update_timestamp();

CREATE TRIGGER trg_provider_models_updated_at
    BEFORE UPDATE ON provider_models
    FOR EACH ROW EXECUTE FUNCTION fn_update_timestamp();

CREATE TRIGGER trg_routing_rules_updated_at
    BEFORE UPDATE ON routing_rules
    FOR EACH ROW EXECUTE FUNCTION fn_update_timestamp();

CREATE TRIGGER trg_requests_updated_at
    BEFORE UPDATE ON requests
    FOR EACH ROW EXECUTE FUNCTION fn_update_timestamp();

CREATE TRIGGER trg_responses_updated_at
    BEFORE UPDATE ON responses
    FOR EACH ROW EXECUTE FUNCTION fn_update_timestamp();

CREATE TRIGGER trg_usage_records_updated_at
    BEFORE UPDATE ON usage_records
    FOR EACH ROW EXECUTE FUNCTION fn_update_timestamp();

CREATE TRIGGER trg_quotas_updated_at
    BEFORE UPDATE ON quotas
    FOR EACH ROW EXECUTE FUNCTION fn_update_timestamp();

CREATE TRIGGER trg_quota_usage_updated_at
    BEFORE UPDATE ON quota_usage
    FOR EACH ROW EXECUTE FUNCTION fn_update_timestamp();

CREATE TRIGGER trg_webhooks_updated_at
    BEFORE UPDATE ON webhooks
    FOR EACH ROW EXECUTE FUNCTION fn_update_timestamp();

CREATE TRIGGER trg_webhook_deliveries_updated_at
    BEFORE UPDATE ON webhook_deliveries
    FOR EACH ROW EXECUTE FUNCTION fn_update_timestamp();

CREATE TRIGGER trg_cache_metadata_updated_at
    BEFORE UPDATE ON cache_metadata
    FOR EACH ROW EXECUTE FUNCTION fn_update_timestamp();

CREATE TRIGGER trg_sessions_updated_at
    BEFORE UPDATE ON sessions
    FOR EACH ROW EXECUTE FUNCTION fn_update_timestamp();

-- Note: audit_log does not have updated_at (append-only, immutable)
```

---

## 7. Query Patterns

### 7.1 Request Lookup by API Key

```sql
-- Purpose: Find recent requests for a specific API key
-- Frequency: High (dashboard + API)
-- Target latency: <50ms

SELECT 
    id, trace_id, model_routed, status, 
    prompt_tokens, completion_tokens, total_cost,
    latency_total_ms, created_at
FROM requests
WHERE org_id = $1
  AND api_key_id = $2
  AND deleted_at IS NULL
ORDER BY created_at DESC
LIMIT $3 OFFSET $4;

-- Index: idx_requests_api_key_created (api_key_id, created_at DESC)
```

### 7.2 Usage Aggregation by Organization (Daily)

```sql
-- Purpose: Daily usage dashboard
-- Frequency: High (every page load)
-- Target latency: <30ms (from usage_records, not requests)

SELECT 
    period_start,
    SUM(request_count) as requests,
    SUM(prompt_tokens) as prompt_tokens,
    SUM(completion_tokens) as completion_tokens,
    SUM(total_cost) as total_cost,
    AVG(latency_ms_avg)::INTEGER as avg_latency_ms
FROM usage_records
WHERE org_id = $1
  AND period = 'daily'
  AND period_start >= $2
  AND period_start < $3
  AND deleted_at IS NULL
GROUP BY period_start
ORDER BY period_start DESC;

-- Index: idx_usage_org_period (org_id, period, period_start DESC)
```

### 7.3 Cost Calculation Query

```sql
-- Purpose: Real-time cost for current billing period
-- Frequency: Medium (dashboard, billing alerts)
-- Target latency: <50ms

SELECT 
    COALESCE(SUM(total_cost), 0) as period_cost,
    COALESCE(SUM(total_tokens), 0) as period_tokens,
    COALESCE(SUM(request_count), 0) as period_requests
FROM usage_records
WHERE org_id = $1
  AND period = 'monthly'
  AND period_start = date_trunc('month', now())
  AND deleted_at IS NULL;

-- Alternative: Exact cost from requests (slower, for verification)
SELECT 
    SUM(total_cost) as exact_cost,
    SUM(total_tokens) as exact_tokens
FROM requests
WHERE org_id = $1
  AND created_at >= date_trunc('month', now())
  AND status = 'success'
  AND deleted_at IS NULL;

-- Index: idx_requests_org_created
```

### 7.4 Quota Check Query (Hot Path)

```sql
-- Purpose: Check if request should be allowed (per-request check)
-- Frequency: Very high (every incoming request)
-- Target latency: <5ms

SELECT 
    q.id as quota_id,
    q.limit_value,
    q.metric,
    q.period,
    q.action,
    COALESCE(qu.current_value, 0) as current_value,
    q.limit_value - COALESCE(qu.current_value, 0) as remaining,
    CASE WHEN COALESCE(qu.current_value, 0) >= q.limit_value THEN true ELSE false END as exceeded
FROM quotas q
LEFT JOIN quota_usage qu ON q.id = qu.quota_id 
    AND qu.period_start = CASE q.period
        WHEN 'minute' THEN date_trunc('minute', now())
        WHEN 'hour'   THEN date_trunc('hour', now())
        WHEN 'day'    THEN date_trunc('day', now())
        WHEN 'month'  THEN date_trunc('month', now())
        ELSE 'epoch'::timestamptz
    END
    AND qu.deleted_at IS NULL
WHERE q.org_id = $1
  AND q.status = 'active'
  AND q.deleted_at IS NULL
  AND (q.api_key_id = $2 OR q.api_key_id IS NULL);

-- Index: idx_quotas_org_metric_period, idx_quota_usage_org_quota
```

### 7.5 Quota Increment (Upsert)

```sql
-- Purpose: Atomically increment quota usage after successful request
-- Frequency: Very high (every successful request)
-- Target latency: <10ms

INSERT INTO quota_usage (
    org_id, quota_id, api_key_id, period_start, period_end,
    current_value, limit_value, metric
)
SELECT 
    $1, $2, $3,
    CASE $4
        WHEN 'minute' THEN date_trunc('minute', now())
        WHEN 'hour'   THEN date_trunc('hour', now())
        WHEN 'day'    THEN date_trunc('day', now())
        WHEN 'month'  THEN date_trunc('month', now())
    END,
    CASE $4
        WHEN 'minute' THEN date_trunc('minute', now()) + interval '1 minute'
        WHEN 'hour'   THEN date_trunc('hour', now()) + interval '1 hour'
        WHEN 'day'    THEN date_trunc('day', now()) + interval '1 day'
        WHEN 'month'  THEN date_trunc('month', now()) + interval '1 month'
    END,
    $5,  -- increment amount
    $6,  -- limit value (from quotas)
    $7   -- metric (from quotas)
ON CONFLICT (org_id, quota_id, api_key_id, period_start)
DO UPDATE SET
    current_value = quota_usage.current_value + EXCLUDED.current_value,
    updated_at = now()
RETURNING current_value;

-- Index: UNIQUE(org_id, quota_id, api_key_id, period_start) — supports upsert
```

### 7.6 Cache Metadata Query

```sql
-- Purpose: Check cache entry existence and freshness
-- Frequency: High (every cacheable request)
-- Target latency: <5ms

SELECT 
    cache_key_hash, expires_at, hit_count
FROM cache_metadata
WHERE org_id = $1
  AND cache_key_hash = $2
  AND deleted_at IS NULL
  AND expires_at > now();

-- Index: uk_cache_metadata_org_hash
```

### 7.7 Routing Rule Lookup

```sql
-- Purpose: Find matching routing rule for a request
-- Frequency: High (every request)
-- Target latency: <5ms

SELECT 
    id, name, strategy, targets, timeout_ms, retries
FROM routing_rules
WHERE org_id = $1
  AND status = 'active'
  AND deleted_at IS NULL
  AND (match_model = $2 OR match_model IS NULL)
ORDER BY priority ASC
LIMIT 1;

-- Index: idx_routing_rules_org_priority
```

### 7.8 Audit Log Query

```sql
-- Purpose: View audit trail for an organization
-- Frequency: Medium (dashboard)
-- Target latency: <50ms

SELECT 
    action, entity_type, entity_id, 
    old_values, new_values, summary,
    ip_address, created_at
FROM audit_log
WHERE org_id = $1
  AND deleted_at IS NULL
ORDER BY created_at DESC
LIMIT $2 OFFSET $3;

-- Index: idx_audit_org_created
```

### 7.9 Webhook Delivery Polling

```sql
-- Purpose: Find pending webhook deliveries to process
-- Frequency: High (worker polling)
-- Target latency: <20ms

SELECT 
    wd.id, wd.webhook_id, wd.event_type, wd.payload,
    wd.attempt_number, w.url, w.secret_enc, w.timeout_seconds,
    w.max_retries, w.custom_headers
FROM webhook_deliveries wd
JOIN webhooks w ON wd.webhook_id = w.id
WHERE wd.status = 'pending'
  AND wd.scheduled_at <= now()
  AND wd.deleted_at IS NULL
  AND w.status = 'active'
  AND w.deleted_at IS NULL
ORDER BY wd.scheduled_at ASC
LIMIT $1;

-- Index: idx_webhook_deliveries_scheduled
```

### 7.10 Provider Health Check

```sql
-- Purpose: Check recent provider error rate
-- Frequency: Medium (monitoring)
-- Target latency: <100ms

SELECT 
    provider_config_id,
    COUNT(*) FILTER (WHERE status = 'success') as successes,
    COUNT(*) FILTER (WHERE status = 'error') as failures,
    AVG(latency_provider_ms)::INTEGER as avg_latency,
    MAX(created_at) as last_request_at
FROM requests
WHERE org_id = $1
  AND provider_config_id = ANY($2)
  AND created_at > now() - interval '5 minutes'
  AND deleted_at IS NULL
GROUP BY provider_config_id;

-- Index: idx_requests_org_created (partial scan)
```

---

## 8. Data Retention

### 8.1 Retention Matrix

| Data Type | Hot Storage (PostgreSQL) | Warm Storage | Cold Storage (S3) | Deletion |
|-----------|-------------------------|--------------|-------------------|----------|
| **Request logs** | 90 days (partitioned) | 90-365 days (compressed) | 1-7 years (Parquet) | After cold retention |
| **Response logs** | 30 days (partitioned) | 30-90 days (compressed) | 90 days - 1 year | After cold retention |
| **Usage aggregates** | Indefinite | — | — | Never (roll up only) |
| **Audit logs** | 365 days (partitioned) | 1-3 years (compressed) | 3-7 years (Parquet) | After compliance period |
| **Webhook deliveries** | 30 days (partitioned) | 30-90 days (compressed) | 1 year | After cold retention |
| **Cache metadata** | Active entries only | — | — | On TTL expiry |
| **Sessions** | Active + 30 days expired | — | — | After 30 days of expiry |
| **API keys** | Indefinite (soft delete) | — | — | Soft delete only |
| **Quota usage** | Current period + 2 prior | — | — | After 90 days |

### 8.2 Retention Implementation

```sql
-- 1. Automated partition cleanup (requests older than 90 days)
CREATE OR REPLACE FUNCTION drop_old_partitions(
    p_parent_table TEXT,
    p_retention_days INTEGER
) RETURNS INTEGER AS $$
DECLARE
    partition_name TEXT;
    partition_date TEXT;
    cutoff_date TEXT;
    dropped_count INTEGER := 0;
BEGIN
    cutoff_date := to_char(now() - (p_retention_days || ' days')::interval, 'YYYY-MM-DD');
    
    FOR partition_name IN
        SELECT inhrelid::regclass::text
        FROM pg_inherits
        WHERE inhparent = p_parent_table::regclass
    LOOP
        -- Extract date from partition name (assumes _yYYYYmMM suffix)
        partition_date := substring(partition_name from 'y(\d{4})m(\d{2})');
        
        IF partition_date IS NOT NULL AND partition_date < replace(cutoff_date, '-', '') THEN
            -- Verify archived to S3 before dropping
            EXECUTE format('DROP TABLE IF EXISTS %I', partition_name);
            dropped_count := dropped_count + 1;
            RAISE NOTICE 'Dropped partition: %', partition_name;
        END IF;
    END LOOP;
    
    RETURN dropped_count;
END;
$$ LANGUAGE plpgsql;

-- 2. Session cleanup
DELETE FROM sessions 
WHERE expires_at < now() - interval '30 days'
   OR (deleted_at IS NOT NULL AND deleted_at < now() - interval '30 days');

-- 3. Cache metadata cleanup
DELETE FROM cache_metadata WHERE expires_at < now() - interval '7 days';

-- 4. Quota usage cleanup
DELETE FROM quota_usage WHERE period_end < now() - interval '90 days';

-- 5. Soft-deleted entity hard delete (after 30 days grace)
-- Run monthly for each table:
DELETE FROM api_keys WHERE deleted_at < now() - interval '30 days';
DELETE FROM users WHERE deleted_at < now() - interval '30 days';
-- etc.
```

### 8.3 Archiving Workflow

```
┌──────────────┐    ┌──────────────┐    ┌──────────────┐    ┌──────────────┐
│   PostgreSQL  │───▶│   DETACH     │───▶│   Export to  │───▶│   S3 Upload  │
│  (hot partition)│   │  partition   │   │   Parquet    │    │   (verified) │
└──────────────┘    └──────────────┘    └──────────────┘    └──────┬───────┘
                                                                    │
                                                                    ▼
                                                             ┌──────────────┐
                                                             │  DROP TABLE  │
                                                             │  (confirmed) │
                                                             └──────────────┘
```

### 8.4 Compliance Notes

| Requirement | Implementation |
|-------------|----------------|
| GDPR Right to Erasure | Soft delete + 30-day grace; hard delete cascades via `ON DELETE CASCADE` on `org_id` |
| GDPR Right to Access | Export all `org_id` scoped data via API; structured JSON/CSV output |
| SOC 2 Audit Trail | `audit_log` table with immutable entries; 7-year retention |
| Data Residency | `organizations.settings` includes `data_region`; routing to region-specific PostgreSQL read replicas |
| Encryption at Rest | PostgreSQL TDE or cloud-level disk encryption |
| Encryption in Transit | TLS 1.3 for all connections |

---

## Appendix A: Complete DDL Order

```sql
-- 1. Extensions
CREATE EXTENSION IF NOT EXISTS "pgcrypto";
CREATE EXTENSION IF NOT EXISTS "pg_uuidv7";

-- 2. Custom types (enums)
CREATE TYPE provider_kind AS ENUM (...);
CREATE TYPE routing_strategy AS ENUM (...);
CREATE TYPE request_status AS ENUM (...);
CREATE TYPE quota_period AS ENUM (...);
CREATE TYPE quota_metric AS ENUM (...);
CREATE TYPE webhook_event AS ENUM (...);
CREATE TYPE audit_action AS ENUM (...);

-- 3. Tables in dependency order
-- organizations → users → api_keys → provider_configs → provider_models
-- → routing_rules → requests → responses → usage_records
-- → quotas → quota_usage → webhooks → webhook_deliveries
-- → cache_metadata → audit_log → sessions

-- 4. Indexes

-- 5. Triggers

-- 6. RLS policies

-- 7. Initial partitions

-- 8. Seed data
```

## Appendix B: Storage Estimates

| Table | Rows/Month | Row Size | Monthly Growth | Annual |
|-------|-----------|----------|----------------|--------|
| requests | 100M | ~500 bytes | ~50 GB | ~600 GB |
| responses | 100M | ~300 bytes | ~30 GB | ~360 GB |
| usage_records | 10M | ~200 bytes | ~2 GB | ~24 GB |
| audit_log | 5M | ~400 bytes | ~2 GB | ~24 GB |
| webhook_deliveries | 1M | ~500 bytes | ~0.5 GB | ~6 GB |
| All other tables | <1M | ~200 bytes | ~0.2 GB | ~2.4 GB |
| **Total** | — | — | **~85 GB** | **~1 TB** |

> With 90-day hot retention: ~255 GB active data in PostgreSQL. Remainder in S3 at ~$0.023/GB/month.

## Appendix C: Connection Pooling

| Component | Recommendation |
|-----------|----------------|
| Pooler | PgBouncer or built-in pooler (transaction mode) |
| Pool size | 10-20 connections per application instance |
| Max connections | PostgreSQL `max_connections` = 200-500 depending on instance size |
| Prepared statements | Disabled for PgBouncer transaction mode; use simple query protocol |
| RLS context | Set `app.org_id` on connection checkout from pool |

---

*End of Database Schema Document*
