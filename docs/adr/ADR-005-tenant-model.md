# ADR-005: Tenant Model

## Status
Accepted

## Context
The AI Gateway is a multi-tenant system: multiple organizations share a single gateway deployment, each with their own API keys, provider configurations, quotas, usage data, and cached responses. Tenant isolation is a security-critical concern; a breach or misconfiguration in one organization must not affect another.

Key forces:
- Single VPS deployment serves multiple organizations
- PostgreSQL is the sole persistence layer; no distributed databases
- The team is <5 engineers; operational complexity must be minimal
- Tenant isolation must be enforceable at the database, cache, and observability layers
- Cross-tenant data leakage would be a catastrophic security and business failure
- Some operations (provider configs, model pricing) are shared; most (API keys, usage, cache) are per-tenant

## Decision
We will implement **organization-based tenancy** using a combination of **row-level security (RLS) policies in PostgreSQL** and **application-level filtering** for cache keys, queries, and observability data.

Every table that stores organization-scoped data includes an `org_id` column (UUID, foreign key to `organizations`). All queries include `WHERE org_id = $1` as the first filter condition. The `AuthContext` produced by authentication carries the `org_id`, and all downstream operations use it to scope data access.

**RLS policies on PostgreSQL:**
```sql
-- Enable RLS on all tenant-scoped tables
ALTER TABLE api_keys ENABLE ROW LEVEL SECURITY;
ALTER TABLE provider_configs ENABLE ROW LEVEL SECURITY;
ALTER TABLE usage_records ENABLE ROW LEVEL SECURITY;

-- Policy: users can only see their own organization's data
CREATE POLICY org_isolation ON api_keys
    USING (org_id = current_setting('app.current_org_id')::UUID);
```

The application sets `app.current_org_id` on each database connection before executing queries. RLS acts as a safety net; the primary isolation mechanism is application-level `WHERE` clauses.

**Why not schema-per-tenant:**
Schema-per-tenant would require creating a new PostgreSQL schema for each organization, duplicating table definitions, and managing schema migrations across N schemas. At 100+ tenants, migration operations become slow and error-prone. Schema-per-tenant also prevents efficient cross-tenant queries needed for platform analytics and billing aggregation.

**Why not database-per-tenant:**
Database-per-tenant provides the strongest isolation but is operationally infeasible on a single VPS. Each database requires separate connection pools, backup schedules, and disk space. Connection limits on PostgreSQL (typically 100) would cap the number of tenants at ~20-30.

**Tenant isolation in cache:**
Cache keys are prefixed with the tenant ID: `llm:exact:{tenant_id}:{model}:{hash}`. Cross-tenant cache poisoning is structurally impossible because even a hash collision would mismatch the tenant prefix.

**Tenant isolation in observability:**
All usage metrics, request logs, and billing records include `org_id`. Dashboard queries filter by `org_id`. Prometheus metrics include `org_id` label for per-tenant alerting.

**Shared resources:**
Provider configurations (base URLs, API keys for upstream providers) can be shared across tenants via a `is_shared` flag, or per-tenant via `org_id`. Model pricing defaults are global but overridable per-organization.

## Alternatives Considered

### Alternative 1: Schema-Per-Tenant
- **Description:** Each organization gets its own PostgreSQL schema with isolated tables.
- **Why rejected:** Migrations must run against N schemas; at 100 tenants this is slow and failure-prone. Connection pool sizing becomes complex (pools per schema). Cross-tenant analytics require `UNION ALL` across schemas. No benefit over RLS + `org_id` filtering for the target workload.

### Alternative 2: Database-Per-Tenant
- **Description:** Each organization gets its own PostgreSQL database.
- **Why rejected:** Operationally infeasible on a single VPS. Each database consumes connections, memory, and disk. Backup/restore operations multiply. The connection pool limit (~100) caps tenants at ~20-30. Over-engineered for SME customers.

### Alternative 3: Application-Only Filtering (No RLS)
- **Description:** Rely solely on `WHERE org_id = $1` in application queries without PostgreSQL RLS.
- **Why rejected:** Application bugs (a missing `WHERE` clause, an SQL injection vulnerability, a developer running ad-hoc queries) can expose cross-tenant data. RLS is a defense-in-depth safety net that makes such breaches structurally impossible at the database level.

### Alternative 4: Shared-Nothing Micro-Tenancy
- **Description:** Run a separate gateway instance per tenant, each with its own database.
- **Why rejected:** Violates the core product differentiator of "deploy in <10 minutes on a single VPS." The operational overhead of N gateway instances, N databases, and N Redis instances is unsustainable for a <5-person team.

## Tradeoffs

### What We Gain
- **Operational simplicity:** One database, one schema, standard migrations. Any engineer with PostgreSQL knowledge can operate it.
- **Scalable to hundreds of tenants:** `org_id` filtering scales linearly with proper indexing; no schema proliferation.
- **Cross-tenant analytics:** Platform-level queries (total revenue, provider utilization, top models) are simple aggregations over the same tables.
- **Defense in depth:** Application-level filtering + RLS policies means two independent mechanisms must fail for a cross-tenant leak.
- **Minimal resource overhead:** A single UUID column per row adds 16 bytes; negligible compared to typical row sizes.

### What We Give Up
- **Weaker isolation than schema/db-per-tenant:** A PostgreSQL superuser or RLS bypass bug could theoretically expose cross-tenant data. Mitigated by least-privilege database roles.
- **No tenant-specific schema customization:** All tenants share the same table structure; tenant-specific fields must use JSONB columns.
- **Query performance:** Every query includes `org_id` in the WHERE clause; indexes must be composite (`org_id, created_at`). This is standard multi-tenant indexing practice.
- **Tenant data portability:** Extracting one tenant's data requires filtering exports, not simply dumping a schema.

## Consequences
- Every tenant-scoped table includes `org_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE`.
- Composite indexes on `(org_id, created_at DESC)` are required for all timestamp-ordered queries.
- Cache keys always include the tenant ID prefix; tenant purge operations use `SCAN llm:*:{tenant_id}:*` + `UNLINK`.
- The `organizations` table stores tenant metadata: name, slug, settings, created_at. Deleting an organization cascades to all associated data.
- Provider configurations can be global (shared) or tenant-specific via `org_id NULLABLE` with `is_shared BOOLEAN`.
- The admin dashboard enforces tenant isolation by injecting the user's `org_id` into all API queries.
- PostgreSQL RLS policies are enabled but application queries still include explicit `org_id` filters. RLS is the safety net, not the primary mechanism.
- Superadmin users can view cross-tenant data via a separate `superadmin` role that bypasses RLS for platform operations.

## Related Decisions
- **ADR-003 (Authentication):** Auth context carries `org_id`; all downstream tenant scoping depends on it.
- **ADR-002 (Cache Strategy):** Cache keys include tenant ID for isolation; tenant purge invalidates all associated cache entries.
- **ADR-006 (Observability):** Metrics and logs include `org_id` for per-tenant dashboards and alerts.

## Notes
- Row-level security is enabled on all tenant-scoped tables but the application does not rely on it exclusively. Queries always include explicit `WHERE org_id = $1` filters. RLS is defense-in-depth.
- The `org_id` is set on the database connection via `SET app.current_org_id = '...'` before each query. A middleware resets it after the query to prevent leakage in connection pooling.
- Tenant deletion is a soft delete (`status = 'deleted'`) for 30 days, then hard delete via a background job. This allows recovery from accidental deletions.
- Future work: Consider schema-per-tenant only if a single enterprise customer demands dedicated schema isolation and is willing to pay for the operational overhead.
- Index strategy: All hot-path queries use composite indexes leading with `org_id`. Example: `CREATE INDEX idx_api_keys_org_hash ON api_keys(org_id, key_hash)`.
