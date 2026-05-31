# Decision Log

> **Append-only record of architectural decisions made during implementation.**
> Format: `YYYY-MM-DD — Decision (context) — Rationale`
> Never delete or modify old entries. Add new ones at the top.

## 2026-05-31 — API key format changed from `gk_live_*` to `sk_gw_*`
**Context:** Original spec used `gk_live_*` format. TASK-0014 spec updated to match OpenAI conventions.
**Rationale:** Developers already know `sk-` prefix from OpenAI. Reduces cognitive load. Format: `sk_gw_{32_base58}{6_base58_checksum}` = 44 chars.
**Impact:** AUTH.md still references old `gk_live_*` format in diagrams. Docs need sync.

## 2026-05-31 — Checksum computed on base58 string, not raw bytes
**Context:** CRC32C checksum for API key verification.
**Rationale:** Using the base58-encoded string (not raw 24 bytes) handles truncation edge cases consistently. The random part is 24 bytes → base58 → 32 chars. Checksum is on those 32 chars.
**Impact:** Key generation and verification must use same approach. Tests verify this.

## 2026-05-31 — Circular dependency resolved: types in `gateway-core`
**Context:** `gateway-core` originally depended on `gateway-providers` for types, but `gateway-providers` needed `gateway-core` for errors.
**Rationale:** Canonical OpenAI-compatible types belong in `gateway-core` (the contract layer). `gateway-providers` implements the contract. `gateway-api` composes both.
**Impact:** Provider trait is in `gateway-providers`, types in `gateway-core`. All imports updated.

## 2026-05-31 — Partitioned tables have no parent-level PRIMARY KEY
**Context:** PostgreSQL requires partition columns in PRIMARY KEY / UNIQUE constraints.
**Rationale:** Removed `PRIMARY KEY` from parent tables: `requests`, `responses`, `usage_records`, `webhook_deliveries`, `audit_log`. Child partitions enforce uniqueness via partition bounds. `responses.request_id` FK to `requests(id)` also removed.
**Impact:** Application must enforce referential integrity if needed. DB queries use `id` with `WHERE org_id = $1` for uniqueness.

## 2026-05-31 — sqlx 0.9 uses `.up.sql` / `.down.sql` files, not combined format
**Context:** Migration framework decision.
**Rationale:** sqlx 0.9 dropped the `--! down` separator. Each migration needs separate files.
**Impact:** All 22 migrations follow this format. Future migrations must too.

## 2026-05-31 — Mock provider fallback for development
**Context:** Chat completions endpoint needs to work without OPENAI_API_KEY.
**Rationale:** Return mock response with `gateway.provider = "mock"` when API key unset. Enables development and CI without live API calls.
**Impact:** `POST /v1/chat/completions` works out of the box. Set OPENAI_API_KEY for real provider calls.

## 2026-05-31 — sqlx upgraded from 0.7 to 0.8
**Context:** `cargo check` warned that `sqlx-postgres v0.7.4` contains code that will be rejected by a future Rust version (never-type fallback issue).
**Rationale:** Upgrade now while codebase is small. sqlx 0.8.6 compiles cleanly with Rust 1.96, zero code changes required.
**Impact:** Workspace `Cargo.toml` updated. All crates compile without warnings.

## 2026-05-31 — RLS as defense-in-depth, not primary isolation
**Context:** Tenant isolation strategy.
**Rationale:** Application queries MUST include `WHERE org_id = $1`. RLS policies are safety net only. Connection pool sets default `app.org_id` via `after_connect` for superuser paths.
**Impact:** Every repository query must filter by org_id. Code review should grep for queries without org_id.

## 2026-05-31 — Server middleware stack: CORS → body limit → trace
**Context:** Axum middleware ordering.
**Rationale:** CORS first (handles preflight before body read), body limit second (prevents large payloads from reaching handlers), trace last (sees the actual response).
**Impact:** Auth middleware should be placed after trace/body-limit but before route handlers when wired.
