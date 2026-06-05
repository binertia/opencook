# AI Gateway — Consolidated Security Strategy Document

**Version:** 1.0
**Status:** Implementation-Ready
**Classification:** Internal Use
**Owner:** Security Lead / CISO
**Review Cycle:** Quarterly
**Last Updated:** 2025-01-15

---

## Table of Contents

1. [Security Architecture Overview](#1-security-architecture-overview)
2. [Authentication & Authorization](#2-authentication--authorization)
3. [Data Protection](#3-data-protection)
4. [Input Validation & Injection Prevention](#4-input-validation--injection-prevention)
5. [Rate Limiting & Abuse Prevention](#5-rate-limiting--abuse-prevention)
6. [Audit & Logging](#6-audit--logging)
7. [Vulnerability Management](#7-vulnerability-management)
8. [Incident Response](#8-incident-response)
9. [Security Checklist (Implementation Order)](#9-security-checklist-implementation-order)
10. [Security Decision Log](#10-security-decision-log)

---

## 1. Security Architecture Overview

### 1.1 Defense in Depth Layers

The AI Gateway employs five concentric trust boundaries with escalating security controls at each layer:

```
+-------------------------+ Trust Boundary 1: Internet-facing (Untrusted)
|   Nginx Reverse Proxy   |  - TLS 1.3 termination, HSTS, request size limits
|   - Rate limiting       |  - Slowloris protection, connection timeouts
|   - WAF rules           |  - IP allowlisting, X-Forwarded-For stripping
+------------+------------+
             |
+-------------------------+ Trust Boundary 2: Application layer (Partially trusted)
|   React + TS Frontend   |  - CSP headers, SRI, HttpOnly cookies
|   (Admin Dashboard)     |  - No secrets in localStorage, backend-enforced RBAC
|                         |
|   Rust Backend          |  - Parameterized queries, JWT validation, tenant scoping
|   - Auth middleware     |  - Request-scoped tenant context, deny-by-default RBAC
|   - Provider proxy      |  - SSRF protection, URL whitelist, certificate pinning
|   - Cache layer         |  - Tenant-isolated keys, PII detection, content integrity
+------------+------------+
             |
+-------------------------+ Trust Boundary 3: Data layer (Trusted)
|   PostgreSQL 14+        |  - AES-256 encrypted at rest, Row-Level Security
|   - Tenant data         |  - Least-privilege DB user, parameterized queries only
|   - Audit logs          |  - Append-only audit table, hash chain integrity
|                         |
|   Redis 7+              |  - AUTH + ACL (command whitelist), no persistence for cache
|   - Response cache      |  - Tenant-isolated key namespaces, memory limits
|   - Rate limit state    |  - Lua-script atomic operations, denied-command monitoring
|   - Session store       |
+------------+------------+
             |
+-------------------------+ Trust Boundary 4: Infrastructure (Highly trusted)
|   Docker Compose        |  - Distroless runtime images, read-only root fs
|   - Host VPS            |  - Non-root user, no-new-privileges, seccomp profile
|   - Backups             |  - GPG-encrypted, immutable object storage, separate account
|   - Secrets             |  - Docker Secrets (/run/secrets/), never env vars
+-------------------------+

Trust Boundary 5: External AI Providers (Trusted third-party with verification)
   - Certificate pinning (TOFU), response integrity monitoring, canary requests
```

### 1.2 Trust Boundaries

| Boundary | Components | Trust Level | Key Controls |
|----------|-----------|-------------|--------------|
| TB-1: Internet-facing | Nginx, Public API endpoints, TLS termination | Untrusted | TLS 1.3, HSTS, rate limiting, request size limits, connection timeouts |
| TB-2: Application | Rust backend, React frontend, auth middleware | Partially trusted | RBAC, JWT validation, tenant scoping, parameterized queries, input validation |
| TB-3: Data | PostgreSQL, Redis | Trusted | Encryption at rest, RLS, ACL, least-privilege users, append-only audit logs |
| TB-4: Infrastructure | Docker daemon, Host filesystem, Backups | Highly trusted | Distroless images, read-only fs, Docker Secrets, backup encryption, network isolation |
| TB-5: External providers | OpenAI, Anthropic, Google APIs | Trusted with verification | Certificate pinning, health monitoring, multi-provider failover, canary requests |

### 1.3 Security-Critical Components

| Component | Criticality | Failure Mode | Primary Mitigation | Threat IDs |
|-----------|-------------|--------------|-------------------|------------|
| API key storage & handling | Critical | Complete tenant compromise | AES-256-GCM encryption, zeroize memory, no logging | T-003, T-009 |
| Tenant isolation middleware | Critical | Cross-tenant data breach | Request-scoped org_id, RLS, deny-by-default auth | T-004, T-010, T-013 |
| JWT signing & validation | Critical | Authentication bypass | RS256 only, reject alg:none, explicit algorithm enforcement | T-009 |
| Cost tracking & circuit breaker | Critical | Financial destruction | Per-org hard caps, real-time cost tracking, 429 at budget limit | T-007 |
| Cache key generation | High | Cache poisoning, cross-tenant leak | SHA-256 hash keys, tenant_id prefix from auth context | T-001, T-004 |
| Provider URL routing | High | SSRF, internal network access | URL whitelist, IP blocklist, DNS resolution before request | T-005 |
| Secret management | Critical | Complete system compromise | Docker Secrets, no env vars for secrets, no debug endpoints | T-015 |
| Audit logging pipeline | High | Undetected breach, compliance failure | Structured logging, append-only storage, hash chain integrity | T-011 |

### 1.4 Security Ownership

| Role | Responsibility | Primary | Backup |
|------|---------------|---------|--------|
| **Security Lead** | Overall security strategy, threat modeling, incident response, vulnerability management | Security Lead | CTO |
| **Platform Lead** | Infrastructure security, container hardening, network isolation, backup encryption | Platform Lead | Senior Engineer |
| **Backend Lead** | Application security, auth implementation, input validation, secure coding practices | Backend Lead | Senior Backend Engineer |
| **Compliance Lead** | Regulatory compliance (SOC 2, GDPR), audit coordination, evidence collection | Compliance Lead | DPO |
| **DPO (Data Protection Officer)** | GDPR/data privacy, DSR handling, DPIA, sub-processor governance | DPO | Compliance Lead |
| **CISO** | Risk acceptance, security budget, executive escalation, third-party security | CISO | CEO |
| **All Engineers** | Secure coding, code review for security, dependency updates, security test authorship | Self | Security Lead |

---

## 2. Authentication & Authorization

### 2.1 Dual Authentication Systems

The gateway operates two independent authentication systems serving different client types. They must not share secrets, keys, or validation code.

| System | Client Type | Auth Method | Token Format | Stateless? |
|--------|-------------|-------------|--------------|------------|
| **System A** | API Consumers (programmatic) | API Key | `gk_live_<random><checksum>` (47 chars) | Yes |
| **System B** | Dashboard Users (human) | JWT Session | RS256-signed JWT in httpOnly cookie | Yes (stateless verification) |

### 2.2 API Key Security (System A)

#### 2.2.1 Key Generation

| Requirement | Specification | Enforcement |
|-------------|-------------|-------------|
| RNG source | `secrets.token_bytes` via `/dev/urandom` (CSPRNG) | Code review + unit test |
| Entropy | 192 bits (24 bytes random) | Generation logic audit |
| Format | `gk_live_<32-char Base58><6-char CRC32 checksum>` | Regex validation: `^gk_(live|test)_[A-Za-z0-9]{38}$` |
| Key space | 58^32 ≈ 2^187 | Mathematical analysis |
| Generation rate limit | Max 10 keys/minute per organization | Redis counter per org_id |
| Max keys per org | 100 (configurable) | Database constraint |
| Collision probability | ~2^-192 (negligible) | Documented, accepted |

#### 2.2.2 Key Storage Rules

- **NEVER store plaintext API keys.** Store only SHA-256 hash for lookup. Full key displayed exactly once at creation.
- **Display prefix only** in UI: `gk_live_aB...` (first 8 chars + ellipsis)
- **No mechanism exists** to recover plaintext from stored hash
- **Cache key lookup in Redis:** `auth:apikey:{hash}` with 5-minute TTL, scoped to tenant

#### 2.2.3 Per-Request Validation Flow

1. Extract key from `Authorization: Bearer <key>` header
2. Validate format against regex
3. Compute SHA-256 hash of full key
4. Check Redis cache: `GET auth:apikey:{hash}` (sub-millisecond lookup)
5. On cache miss: query `SELECT * FROM api_keys WHERE key_hash = $1 AND status = 'active'`
6. Store result in Redis with 5-minute TTL
7. Validate: not revoked, not expired, IP in allowlist, model allowed
8. Check rate limit: sliding window counter in Redis (per key)
9. Check monthly budget: reject with 429 if exceeded
10. Attach `AuthContext` to request (org_id, key_id, role, environment)
11. **Target overhead:** <1ms p99 per request

#### 2.2.4 Key Revocation

Revocation must propagate within 100ms:

1. Update database: `UPDATE api_keys SET status = 'revoked', revoked_at = NOW() WHERE id = $1`
2. Invalidate Redis cache immediately: `DEL auth:apikey:{hash}`
3. Publish revocation event on Redis pub/sub for distributed cache invalidation
4. Record audit log entry with actor_id, reason, timestamp

#### 2.2.5 Key Rotation

| Policy | Default | Implementation |
|--------|---------|---------------|
| Auto-rotation warning | 90 days | Dashboard notification + email to admins |
| Rotation grace period | 7 days | Old key continues working after rotation |
| Mandatory rotation | 180 days | Key automatically revoked at 180 days |
| Rotation notifications | 30, 14, 7, 1 days before expiration | Email to organization admins |
| Self-service rotation | Available in dashboard | Any owner/admin can rotate |

#### 2.2.6 Constant-Time Key Comparison

```rust
// Use subtle crate for constant-time comparison
use subtle::ConstantTimeEq;

fn verify_api_key(provided: &[u8], stored_hash: &[u8]) -> bool {
    // Compute hash of provided key
    let provided_hash = sha2::Sha256::digest(provided);
    // Constant-time comparison to prevent timing attacks
    provided_hash.as_slice().ct_eq(stored_hash).into()
}
```

**Response uniformity:** Identical HTTP 401 response body and timing for: invalid format, non-existent key, revoked key, expired key. Do not differentiate.

### 2.3 Session Authentication (System B)

#### 2.3.1 JWT Configuration

| Parameter | Value | Rationale |
|-----------|-------|-----------|
| Signing algorithm | **RS256 only** | Asymmetric: private key signs, public key verifies; enables key rotation without redeploy |
| Key size | 2048-bit RSA (minimum) | 4096-bit preferred for new deployments |
| Access token lifetime | 15 minutes | Short-lived reduces window for token theft |
| Refresh token lifetime | 7 days | Long-lived for UX, rotatable for security |
| Algorithm enforcement | Hardcoded RS256 | Reject any `alg` value other than RS256, including `none` |
| Required claims | `sub`, `exp`, `iat`, `jti`, `type` | Missing claim = immediate rejection |
| Claim validation | Verify `iss`, `aud`, `exp`, `iat` | Clock skew tolerance: 60 seconds |

#### 2.3.2 Token Transport (httpOnly Cookies)

```
Set-Cookie: session=<access_token>; HttpOnly; Secure; SameSite=Strict; Path=/; Max-Age=900
Set-Cookie: refresh=<refresh_token>; HttpOnly; Secure; SameSite=Strict; Path=/api/auth/refresh; Max-Age=604800
```

| Attribute | Value | Security Purpose |
|-----------|-------|-----------------|
| `HttpOnly` | true | Prevents JavaScript access (XSS protection) |
| `Secure` | true | HTTPS only |
| `SameSite=Strict` | true | Prevents CSRF attacks |
| `Path` | `/` (access), `/api/auth/refresh` (refresh) | Scope restriction |

**Why not localStorage:** XSS vulnerability of localStorage outweighs the CSRF risk of cookies. With `SameSite=Strict`, CSRF risk is minimal.

#### 2.3.3 Refresh Token Rotation

1. On token refresh: revoke old refresh token, issue new access + refresh token pair
2. Store refresh token hash (SHA-256) in database for revocation tracking
3. Alert on refresh token reuse (indicates token theft)
4. Max 3 concurrent sessions per user

#### 2.3.4 Password Security

| Requirement | Specification |
|-------------|---------------|
| Hashing algorithm | Argon2id (OWASP-recommended, memory-hard) |
| Parameters | time_cost=3, memory_cost=65536 (64MB), parallelism=4 |
| Minimum password length | 12 characters |
| Maximum password length | 128 characters |
| Complexity requirements | 1 uppercase, 1 lowercase, 1 digit, 1 special character |
| Common password check | Reject top 10,000 common passwords (Bloom filter) |
| Credential stuffing check | Optional HIBP API verification |
| Account lockout | Lock 30 minutes after 5 failed attempts |
| Login rate limit | Max 10 attempts per email per hour |
| Constant-time verification | Perform dummy hash on non-existent users to prevent timing attacks |

### 2.4 RBAC Model

#### 2.4.1 Role Definitions

| Role | Scope | Key Permissions |
|------|-------|-----------------|
| **owner** | Organization founder | Full access: org management, billing, user management, all API keys |
| **admin** | Team lead | Manage API keys, invite users, settings, view logs. Cannot delete org or manage billing |
| **member** | Developer | View usage, create own API keys (default settings only), view configs. Cannot manage users or billing |
| **viewer** | Stakeholder/auditor | Read-only: dashboards, usage, billing read |
| **superadmin** | Platform operator (gateway staff) | Cross-organization access for platform support. Requires MFA + manual approval |

#### 2.4.2 Permission-to-Endpoint Mapping

Every API endpoint declares its required permission. The RBAC middleware checks permissions before handler execution. **Default: deny access if no permission is declared.**

Example permission requirements:
- `POST /api/v1/chat/completions` — requires `api_key` authentication (System A)
- `GET /api/v1/admin/keys` — requires `apikey:read` permission
- `POST /api/v1/admin/keys` — requires `apikey:create` permission
- `DELETE /api/v1/admin/keys/:id` — requires `apikey:revoke` permission
- `GET /api/v1/admin/users` — requires `member:read` permission
- `POST /api/v1/admin/users` — requires `member:invite` permission
- `PATCH /api/v1/admin/users/:id/role` — requires `member_role:update` permission

#### 2.4.3 Critical RBAC Enforcement Rules

1. **Deny-by-default:** Every endpoint has explicit permission requirement. No endpoint is accessible without a declared permission.
2. **Organization scoping on every request:** Admin endpoints verify `admin.org_id == resource.org_id` on every request. Deny if mismatch.
3. **No mass assignment:** Use DTOs with explicit field allowlists. Reject unexpected fields. Separate update structs for user vs. admin operations.
4. **Backend-enforced only:** Frontend RBAC is for UX convenience only. All authorization decisions happen server-side.
5. **Cross-tenant access is forbidden** except for superadmin with explicit approval.

### 2.5 Tenant Isolation Guarantees

#### 2.5.1 Isolation Layers

Tenant isolation is enforced at 6 independent layers. Any single layer failing is caught by the others:

| Layer | Enforcement Mechanism | Verification |
|-------|----------------------|--------------|
| Layer 1: Authentication | org_id embedded in auth context (API key lookup or JWT claim) | Unit tests for auth flow |
| Layer 2: API Gateway | Route validation: every route handler requires org_id parameter | Integration tests |
| Layer 3: Application | Service-layer org_id filtering on every query | Code review: grep for queries without org_id |
| Layer 4: Database | Row-Level Security policies: `CREATE POLICY tenant_isolation ON requests USING (org_id = current_setting('app.current_org_id')::uuid)` | Database audit |
| Layer 5: Cache | All cache keys prefixed with `llm:exact:{tenant_id}:` — structurally prevents cross-tenant poisoning | Static analysis |
| Layer 6: Logs | org_id on every log entry; log access scoped to own tenant | Audit log review |

#### 2.5.2 Mandatory Isolation Rules

- Every database query on tenant-scoped tables MUST include `WHERE org_id = $1`
- Users can only access data for their current active organization (determined by JWT claim)
- Cross-organization access is forbidden except for superadmin
- API keys are scoped to exactly one organization — no shared keys
- Organization switching requires new token issuance with re-verification of membership

#### 2.5.3 Tenant Isolation Verification

- **Automated integration tests:** Random org_id mutations in requests must always result in HTTP 403
- **Property-based tests:** Run in CI on every build
- **Decoy organizations:** Set up orgs with no real users; alert on any access
- **Code review requirement:** Every PR touching database queries must be reviewed for org_id inclusion

### 2.6 SSO Security (SAML 2.0 & OIDC)

#### 2.6.1 CSRF Protection for Identity Provider Flows

Both SAML and OIDC login flows use cryptographically random nonces stored in Redis with a 10-minute TTL and one-time use semantics.

| Flow | Nonce Mechanism | Storage | Verification |
|------|----------------|---------|--------------|
| **OIDC** | `state` parameter — 32 bytes of randomness, hex-encoded | `sso:oidc:state:{nonce}` → `org_id` | `/api/v1/auth/oidc/callback` atomically reads and deletes the Redis key (`GETDEL`), validates the `org_id`, and rejects replays |
| **SAML** | `RelayState` parameter — 32 bytes of randomness, hex-encoded | `sso:saml:relay:{nonce}` → `org_id` | `/api/v1/auth/saml/acs` atomically reads and deletes the Redis key (`GETDEL`), validates the `org_id`, and rejects replays |

**Attack prevented:** An attacker cannot trick a user into completing an IdP login against a victim organization. Even if the attacker knows the target `org_id` (UUID), they cannot forge a valid `state`/`RelayState` nonce.

#### 2.6.2 Post-Login Redirects

Both SAML ACS and OIDC callback redirect the browser to the first URL configured in `allowed_origins` rather than a hardcoded path. This prevents open-redirect vulnerabilities and ensures the user lands on the legitimate dashboard origin.

#### 2.6.3 SSO Configuration Access Control

Admin endpoints for viewing and modifying SSO configuration enforce RBAC:

| Endpoint | Required Permission |
|----------|---------------------|
| `GET /api/v1/organizations/:org_id/sso` | `settings:read` |
| `POST /api/v1/organizations/:org_id/sso` | `settings:write` |
| `DELETE /api/v1/organizations/:org_id/sso/:provider_type` | `settings:write` |

Additionally, every request verifies `auth.org_id == path_org_id` to prevent cross-organization SSO tampering.

---

## 3. Data Protection

### 3.1 Encryption at Rest

| Asset | Encryption Method | Key Management |
|-------|------------------|----------------|
| Customer AI provider API keys (A-1) | AES-256-GCM | Master key in Docker Secrets (`/run/secrets/master_key`); master key never logged |
| JWT signing private key (A-7) | File-based, 0400 permissions | Docker Secrets; rotate every 90 days |
| PostgreSQL database files | LUKS full-disk encryption | VPS provider encryption + application-layer AES-256 |
| PostgreSQL backups | GPG encryption (AES-256) | Backup encryption key stored separately from database key |
| Redis cache | No persistence (memory only) | `save ""`, `appendonly no` — no data written to disk |
| Audit logs | Append-only with SHA-256 hash chain | Separate database user for audit writes |

#### 3.1.1 Provider Key Encryption Implementation

```rust
use aes_gcm::Aes256Gcm;
use aes_gcm::aead::{Aead, KeyInit};

// Master key loaded from Docker Secrets at startup
const MASTER_KEY_PATH: &str = "/run/secrets/master_key";

fn encrypt_provider_key(plaintext: &[u8], master_key: &[u8; 32]) -> Vec<u8> {
    let cipher = Aes256Gcm::new_from_slice(master_key).unwrap();
    let nonce = aes_gcm::Nonce::from_slice(&rand::random::<[u8; 12]>());
    cipher.encrypt(nonce, plaintext).unwrap()
}
```

- Master key: 32 bytes (256 bits), loaded from Docker Secrets at container startup
- Nonce: 12 bytes random, unique per encryption operation
- Ciphertext format: `nonce (12 bytes) || ciphertext || auth_tag (16 bytes)`

### 3.2 Encryption in Transit

| Connection | Minimum TLS | Preferred | Certificate Validation |
|------------|-------------|-----------|----------------------|
| Client → Nginx | TLS 1.2 | TLS 1.3 | Standard CA validation |
| Nginx → Backend | TLS 1.2 (internal) | mTLS (future) | Internal CA |
| Backend → PostgreSQL | TLS 1.2 | TLS 1.3 | Verify server certificate |
| Backend → Redis | TLS 1.2 | TLS 1.3 | Verify server certificate |
| Backend → AI Providers | TLS 1.2 | TLS 1.3 | **Certificate pinning (TOFU)** |

#### 3.2.1 TLS Configuration (Nginx)

```nginx
# nginx ssl configuration
ssl_protocols TLSv1.2 TLSv1.3;
ssl_ciphers ECDHE-ECDSA-AES128-GCM-SHA256:ECDHE-RSA-AES128-GCM-SHA256:ECDHE-ECDSA-AES256-GCM-SHA384:ECDHE-RSA-AES256-GCM-SHA384;
ssl_prefer_server_ciphers off;
ssl_session_cache shared:SSL:10m;
ssl_session_timeout 1d;
ssl_session_tickets off;

# HSTS
add_header Strict-Transport-Security "max-age=63072000; includeSubDomains; preload" always;
```

#### 3.2.2 Certificate Pinning for AI Providers

1. On first connection to each provider, pin the certificate/public key fingerprint
2. Alert on any certificate change
3. Grace period: both old and new certificates accepted for 7 days during rotation
4. New certificate fetched out-of-band (different network path) for verification

### 3.3 Secret Management

#### 3.3.1 Secret Storage Rules

| Rule | Implementation | Verification |
|------|---------------|------------|
| **Never use environment variables for secrets** | Docker Secrets (`/run/secrets/<name>`) mounted as files with 0400 permissions | CI check: scan Dockerfile/compose for `ENV` with secret patterns |
| **No debug endpoints in production** | Debug endpoints compiled only with `cfg(debug_assertions)` | CI check: grep for debug endpoints in release build artifacts |
| **No secrets in logs** | Automated redaction: replace `sk-\w+` with `[REDACTED]`, mask PII patterns | Daily log scan for secret patterns |
| **No secrets in code** | Pre-commit hook blocks commits matching secret regexes | Git pre-commit hook + CI secret scanning (truffleHog) |
| **No secrets in error responses** | Production error handlers return generic messages only | Integration tests verify error responses contain no secrets |

#### 3.3.2 Required Docker Secrets

```yaml
# docker-compose.yml secrets section
secrets:
  master_key:
    file: ./secrets/master_key
  jwt_private_key:
    file: ./secrets/jwt_private_key
  jwt_public_key:
    file: ./secrets/jwt_public_key
  database_password:
    file: ./secrets/database_password
  redis_password:
    file: ./secrets/redis_password
```

#### 3.3.3 Secret Rotation Schedule

| Secret Type | Rotation Frequency | Rotation Procedure |
|-------------|-------------------|-------------------|
| Master encryption key | On suspected compromise or annually | 1. Generate new key 2. Re-encrypt all provider keys 3. Update Docker Secret 4. Restart containers |
| JWT signing key pair | Every 90 days | 1. Generate new RSA key pair 2. Add public key to active keys list 3. Wait 24h for token propagation 4. Remove old key |
| Database password | Every 90 days | 1. Update PostgreSQL user password 2. Update Docker Secret 3. Rolling container restart |
| Redis AUTH password | Every 90 days | 1. Update Redis ACL 2. Update Docker Secret 3. Rolling container restart |
| Provider API keys (gateway's own) | On provider rotation or compromise | 1. Generate new key at provider 2. Update encrypted storage 3. Revoke old key at provider |

### 3.4 Key Rotation Procedures

#### 3.4.1 API Key Rotation (Customer-Facing)

```rust
async fn rotate_api_key(key_id: Uuid, rotated_by: Uuid) -> Result<String> {
    let mut tx = db.begin().await?;
    
    // 1. Get existing key details
    let old_key = sqlx::query!("SELECT * FROM api_keys WHERE id = $1", key_id)
        .fetch_one(&mut *tx).await?;
    
    // 2. Generate new key with same permissions
    let (new_full_key, new_hash) = generate_api_key(old_key.environment);
    
    // 3. Create new key record
    let new_key_id = sqlx::query_scalar!(
        "INSERT INTO api_keys (org_id, key_hash, key_prefix, environment, name, allowed_models, ip_allowlist, rate_limit_rps, monthly_budget, created_by)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10) RETURNING id",
        old_key.org_id, new_hash, &new_full_key[..8], old_key.environment,
        format!("{} (rotated)", old_key.name), old_key.allowed_models,
        old_key.ip_allowlist, old_key.rate_limit_rps, old_key.monthly_budget, rotated_by
    ).fetch_one(&mut *tx).await?;
    
    // 4. Revoke old key
    sqlx::query!("UPDATE api_keys SET status = 'revoked', revoked_at = NOW() WHERE id = $1", key_id)
        .execute(&mut *tx).await?;
    
    tx.commit().await?;
    
    // 5. Invalidate cache
    redis.del(format!("auth:apikey:{}", old_key.key_hash)).await?;
    
    // 6. Audit log
    audit_log.record("api_key_rotated", new_key_id, rotated_by, json!({"old_key_id": key_id})).await?;
    
    Ok(new_full_key) // Show ONCE to user
}
```

#### 3.4.2 JWT Key Rotation

1. Generate new RSA key pair
2. Add new public key to the active verification keys list (supports multiple active keys)
3. New tokens signed with new key
4. Old tokens continue to validate for 24-hour grace period
5. After grace period, remove old public key from verification list
6. All tokens signed with old key now fail validation (users must re-authenticate)

### 3.5 Data Classification and Handling

| Classification | Definition | Examples | Handling Requirements |
|---------------|------------|----------|----------------------|
| **Public** | Approved for public disclosure | Marketing materials, public API docs | None; freely distributable |
| **Internal** | Business use only; no external sharing | Analytics, configs, non-sensitive logs | Authenticated access; no public URLs |
| **Confidential** | Sensitive business/customer data | API keys, customer request data, support tickets | AES-256 at rest; TLS 1.2+ in transit; RBAC + need-to-know; audit logging |
| **Restricted** | Highest sensitivity; disclosure causes significant harm | Audit logs, billing data, PII, PHI | AES-256 at rest + transit; MFA required; strict access control; immutable logs; DLP monitoring |

#### 3.5.1 Customer Data Sensitivity by Type

| Customer Type | Typical Data Sensitivity | Gateway Treatment |
|--------------|-------------------------|-------------------|
| General SaaS | Internal - Confidential | Standard encryption; no body logging |
| Healthcare | Confidential - Restricted | BAA required; no PHI in logs; enhanced audit |
| Financial Services | Confidential - Restricted | Enhanced encryption; no PII in logs; SOC 2+ required |
| Government | Restricted | Highest tier; air-gapped options |
| Education | Confidential | FERPA considerations; student data protection |

---

## 4. Input Validation & Injection Prevention

### 4.1 SQL Injection Prevention

#### 4.1.1 Mandatory Requirements

| Control | Implementation | Verification |
|---------|---------------|------------|
| **Parameterized queries exclusively** | Use `sqlx` query macros (`query!`, `query_as!`) with bound parameters only | Static analysis in CI: fail build if `format!` used in SQL query construction |
| **Zero raw string concatenation** for user input | Query builder pattern enforced | `cargo audit` + semgrep rules |
| **Query allowlist for sort/filter** | Sort columns: explicit allowlist (`created_at`, `model`, `status` only); order: `ASC` or `DESC` only | Input validation layer rejects unexpected values |
| **Least privilege database user** | Application DB user: `SELECT`, `INSERT`, `UPDATE` on specific tables only; no `DELETE` on users/keys; no `DROP`, `CREATE`, `COPY`, `pg_read_file` | Database user permission audit |

#### 4.1.2 PostgreSQL Row-Level Security

```sql
-- Enable RLS on all tenant-scoped tables
ALTER TABLE requests ENABLE ROW LEVEL FORCE;
ALTER TABLE api_keys ENABLE ROW LEVEL FORCE;
ALTER TABLE organization_settings ENABLE ROW LEVEL FORCE;

-- Create tenant isolation policy
CREATE POLICY tenant_isolation ON requests
    USING (org_id = current_setting('app.current_org_id')::uuid);

-- Application sets tenant context before each query
SET app.current_org_id = '550e8400-e29b-41d4-a716-446655440000';
```

**RLS is a defense-in-depth layer.** Application must still include `WHERE org_id = $1` in every query. RLS catches any query that forgets the filter.

#### 4.1.3 SQL Injection Detection

| Detection Method | Alert Trigger |
|-----------------|---------------|
| Query execution time | >5x normal (indicates slow UNION-based extraction) |
| Error response patterns | PostgreSQL error codes in HTTP 500 responses |
| Result set size | Queries returning >10,000 rows (normal: <100) |
| Denied SQL operations | `COPY`, `pg_read_file`, `DROP` attempted |
| WAF rules | SQLi signature matches in request payload |

### 4.2 Cache Poisoning Prevention

#### 4.2.1 Tenant-Isolated Cache Keys

```rust
// CORRECT: tenant_id in key (from authenticated context)
let key = format!("llm:exact:{}:{}:{}", tenant_id, model, hash);

// WRONG: shared key space (vulnerable to cross-tenant poisoning)
// let key = format!("llm:exact:{}:{}", model, hash);
```

| Control | Implementation |
|---------|---------------|
| **Tenant ID from auth context** | The `tenant_id` used in cache keys MUST come from the verified auth context (API key lookup or JWT claim), NEVER from user-provided headers or request body |
| **SHA-256 hash keys** | All cache keys use SHA-256 hex digests — prevents information leakage and enumeration |
| **Cache key validation** | Validate tenant_id is UUID format; model slug is alphanumeric + dash only; hash is 64-char hex |
| **Cache TTL** | Chat completions: 60-300s (short poison window); Embeddings: 3600s (deterministic) |
| **Cache invalidation on key rotation** | Flush all `llm:*:{org_id}:*` when API key rotated |

#### 4.2.2 PII Detection for Cache Skip

```rust
fn contains_pii(text: &str) -> bool {
    lazy_static! {
        static ref PII_PATTERNS: Vec<Regex> = vec![
            regex!(r"\b\d{3}-\d{2}-\d{4}\b"),           // US SSN
            regex!(r"\b(?:4\d{3}|5[1-5]\d{2})[\s-]?\d{4}[\s-]?\d{4}[\s-]?\d{4}\b"), // Credit cards
            regex!(r"\b[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Z|a-z]{2,}\b"), // Email
            regex!(r"\b\(?\d{3}\)?[\s.-]?\d{3}[\s.-]?\d{4}\b"), // Phone
            regex!(r"\b(sk-[a-zA-Z0-9]{48})\b"),           // OpenAI-style keys
        ];
    }
    PII_PATTERNS.iter().any(|re| re.is_match(text))
}

// In cache write path: skip caching if PII detected
if contains_pii(&request_body_text) {
    return CacheDecision::Skip;
}
```

**Client opt-out:** `X-Cache-No-Store: true` header bypasses all caching for sensitive requests.

### 4.3 SSRF Prevention (Provider and Webhook URL Validation)

#### 4.3.1 URL Whitelist and Validation

Applies to both provider base URLs and webhook URLs configured by organizations.

| Control | Implementation |
|---------|---------------|
| **URL whitelist** | Only pre-approved provider domains: `api.openai.com`, `api.anthropic.com`, `generativelanguage.googleapis.com`, etc. Reject all others |
| **Scheme restriction** | Only `http://` and `https://` schemes are accepted. `file://`, `ftp://`, `gopher://`, etc. are rejected |
| **Internal IP blocklist** | Reject URLs resolving to: `127.0.0.0/8`, `10.0.0.0/8`, `172.16.0.0/12`, `192.168.0.0/16`, `169.254.0.0/16`, `::1/128`, `fc00::/7`, `fe80::/10`, IPv4-mapped IPv6 (`::ffff:127.0.0.1`) |
| **Internal hostname blocklist** | Reject well-known internal hostnames: `localhost`, `localhost.localdomain`, `ip6-localhost`, `ip6-loopback` |
| **DNS resolution before request** | Resolve hostname to IP, validate against blocklist, then connect. Prevents DNS rebinding attacks |
| **Disable redirect following** | Configure HTTP client (`reqwest`) to not follow HTTP redirects. Treat 3xx responses as errors |
| **URL canonicalization** | Parse with `url`/`reqwest::Url` crate, extract host, validate against blocklist. Reject if parse fails |
| **Validation enforcement** | `CreateWebhookRequest`, `UpdateWebhookRequest`, `CreateProviderRequest`, and `TestConnectionRequest` all enforce `validate_url_not_internal` via the `validator` crate |

#### 4.3.2 SSRF Detection

| Detection Method | Alert Trigger |
|-----------------|---------------|
| Outbound destination IP | Any non-whitelist IP from backend container |
| DNS lookups | Internal hostname lookups from backend |
| Sequential requests | Rapid requests to adjacent IP addresses (network scanning) |
| HTTP errors from non-standard ports | Indicates internal service interaction |

### 4.4 Prompt Injection Handling

#### 4.4.1 Gateway's Role and Limitations

**The gateway cannot reliably detect all prompt injection payloads** because:
- Injection payloads are semantically indistinguishable from legitimate user requests
- Encoded/escaped injection (base64, unicode homoglyphs, markdown obfuscation) bypasses naive pattern matching
- The gateway does not have access to the LLM's internal state or token-level attention weights

**The gateway's role is containment and damage limitation, not detection.**

#### 4.4.2 Containment Controls

| Control | Priority | Implementation |
|---------|----------|---------------|
| **No action based on LLM response** | P0 | Gateway never takes action based on LLM response content — no URL prefetching, no response-activated webhooks, no tool execution |
| **System prompt isolation** | P0 | System prompt maintained in separate field; user role messages cannot precede system prompt |
| **Strip tools/functions by default** | P1 | Gateway strips `tools`/`functions` parameters unless explicitly enabled per-organization |
| **Response content filtering** | P1 | Scan outgoing responses for patterns: literal system prompt text, internal IP addresses, JWT token patterns (`eyJ...`) |
| **Log sanitization** | P1 | Escape control characters (newlines, null bytes) in LLM responses before logging |
| **Outbound firewall** | P2 | Backend container: outbound access to provider IPs and internal DB/cache only. Default deny all other outbound |

#### 4.4.3 Prompt Injection Detection

| Detection Method | Alert Trigger |
|-----------------|---------------|
| Response content patterns | Internal IPs, domain names, URL schemas in LLM responses |
| Response entropy | Unusually high entropy responses (possible encoded exfiltration) |
| Request keywords | Requests containing "system prompt", "ignore previous", "override" |
| Repeated patterns | Multiple requests with injection signatures from same IP/tenant |

### 4.5 Request Size Limits

| Limit | Value | Enforcement Layer |
|-------|-------|-------------------|
| Max request body | 1 MB | Nginx `client_max_body_size 1m` + backend validation |
| Max messages array | 50 messages | Backend deserialization validation |
| Max individual message | 100 KB | Backend validation |
| Max total tokens per request | 128K (model-dependent) | Backend validation against model limits |
| Nginx connection timeouts | Body: 10s, Header: 10s, Keepalive: 30s | Nginx configuration |
| Backend total timeout | 60s | Application timeout |

---

## 5. Rate Limiting & Abuse Prevention

### 5.1 Multi-Layer Rate Limiting

Rate limiting is enforced at three layers with increasing granularity:

```
Layer 1: Nginx (edge)          → IP-based, connection limits, request rate
Layer 2: Application           → Per-API-key, per-organization, token-based
Layer 3: Provider passthrough  → Surface provider rate limits to clients
```

#### 5.1.1 Nginx Layer (Edge)

```nginx
# Rate limiting zones
limit_req_zone $binary_remote_addr zone=api:10m rate=10r/s;
limit_req_zone $binary_remote_addr zone=auth:10m rate=5r/m;
limit_conn_zone $binary_remote_addr zone=conn:10m;

# Apply to API endpoints
location /v1/ {
    limit_req zone=api burst=20 nodelay;
    limit_conn conn 50;
    limit_req_status 429;
}

# Stricter for auth endpoints
location /api/auth/ {
    limit_req zone=auth burst=5 nodelay;
    limit_req_status 429;
}
```

#### 5.1.2 Application Layer (Per-Organization)

| Limit Type | Default Value | Scope | Redis Key Pattern |
|------------|--------------|-------|-------------------|
| Requests per minute | 100 | Per API key | `ratelimit:{key_id}:rpm` |
| Requests per hour | 10,000 | Per API key | `ratelimit:{key_id}:rph` |
| Tokens per minute | 100,000 | Per organization | `ratelimit:{org_id}:tpm` |
| Tokens per day | 10,000,000 | Per organization | `ratelimit:{org_id}:tpd` |
| Max cost per hour | Configurable | Per organization | `cost:{org_id}:hourly` |
| Concurrent requests | 50 | Per API key | Semaphore in Redis |
| Concurrent streams | 10 | Per API key | Semaphore in Redis |

**Sliding window implementation:** Redis sorted sets with Lua script for atomic check-and-record. Removes entries outside the window, counts current, adds new request.

#### 5.1.3 Token Bucket (Burst Tolerance)

| Parameter | Default | Purpose |
|-----------|---------|---------|
| Bucket capacity | 20 requests | Maximum burst size |
| Refill rate | 10 requests/second | Sustained throughput |
| Key | `burst:{tenant_id}:{provider}` | Per-tenant, per-provider |

### 5.2 Cost Abuse Prevention (Denial of Wallet)

#### 5.2.1 Cost-Based Circuit Breaker

| Threshold | Action | Implementation |
|-----------|--------|---------------|
| 80% of daily budget | Warning alert | Real-time alert to organization admins |
| 100% of daily budget | Hard stop (HTTP 429) | Reject all requests with `429 Too Many Requests` + `Retry-After` header. Requires explicit admin override to resume |
| 5x historical usage | Anomaly alert | Automatic alert + temporary rate reduction |

#### 5.2.2 Request Cost Controls

| Control | Implementation |
|---------|---------------|
| Streaming rate limits | Max 5 concurrent streams per key; max stream duration 120 seconds; force-close exceeded |
| Model allowlisting | Per-organization allowed model list; reject requests for non-allowed models |
| Max tokens enforcement | Validate `max_tokens` against remaining budget before forwarding to provider |
| Embedding batch limits | Max batch size 100; max input length 8192 tokens per item |

#### 5.2.3 Cost Abuse Detection

| Detection Method | Alert Trigger |
|-----------------|---------------|
| Token consumption | >5x vs. 7-day rolling average for any organization |
| Model upgrade pattern | Sudden shift from cheap to expensive models |
| Concurrent streams | Stream count approaching limit for single API key |
| Request body size | Shift toward maximum-size requests |
| Same key from multiple IPs | >10 different IPs using same key within 1 hour |
| Canary API keys | Unused keys that see any traffic |

### 5.3 DDoS Mitigation

| Layer | Control | Implementation |
|-------|---------|---------------|
| Nginx | Connection limits | `limit_conn` per IP; max 50 concurrent |
| Nginx | Rate limiting | `limit_req` 10r/s with burst 20 |
| Nginx | Slowloris protection | `client_body_timeout 10s`, `client_header_timeout 10s` |
| Application | Semaphore-based concurrency | Max 50 concurrent requests per API key |
| Application | Load shedding | Return 503 when queue depth exceeds threshold |
| Application | IP reputation | Temporary block IPs with excessive 401/429 rates |
| Infrastructure | VPS-level | Provider DDoS protection (e.g., Cloudflare if used) |

### 5.4 Brute Force Protection

| Endpoint | Protection Method | Threshold |
|----------|------------------|-----------|
| Login (`POST /api/auth/login`) | Account lockout + IP rate limit | Lock 30 min after 5 failed attempts; max 10 attempts per IP per hour |
| API key validation | Constant-time comparison + identical responses | No differentiation between invalid formats |
| Password reset | Token rate limit | Max 3 reset requests per email per hour |
| Registration | IP rate limit | Max 5 registrations per IP per hour |

#### 5.4.2 Rate Limit Response Headers

Every response includes rate limit status:

```
X-RateLimit-Limit: 100
X-RateLimit-Remaining: 87
X-RateLimit-Reset: 1704067200
X-RateLimit-Window: 60
```

---

## 6. Audit & Logging

### 6.1 Security Event Logging

#### 6.1.1 Required Security Events

| Event Category | Event Type | Log Level | Fields Required |
|----------------|------------|-----------|----------------|
| **Authentication** | Login success/failure | INFO/WARN | user_id, email, ip, user_agent, success boolean |
| Authentication | API key usage | INFO | key_id (hash only), org_id, ip, model |
| Authentication | MFA challenge | INFO | user_id, method, success |
| Authentication | Account lockout | WARN | user_id, email, ip, attempt_count |
| **Authorization** | Permission denied | WARN | user_id, resource, action, required_permission |
| Authorization | Cross-tenant access attempt | CRITICAL | user_id, source_org, target_org, ip, endpoint |
| Authorization | Role change | INFO | actor_id, target_id, old_role, new_role |
| **Data Access** | Provider key decryption | INFO | key_id, org_id, requesting_user, timestamp |
| Data Access | Tenant data export | INFO | user_id, org_id, records_exported |
| **Admin Action** | API key rotation | INFO | actor_id, key_id, org_id |
| Admin Action | User invite/remove | INFO | actor_id, target_id, org_id |
| Admin Action | Billing update | INFO | actor_id, org_id, changes_summary |
| Admin Action | Settings change | INFO | actor_id, org_id, key, old_value, new_value |
| **Security** | Rate limit exceeded | WARN | tenant_id, ip, endpoint, limit, actual |
| Security | Cost circuit breaker triggered | CRITICAL | org_id, current_cost, budget_limit |
| Security | Suspicious pattern detected | WARN | tenant_id, pattern_type, confidence, details |
| Security | DLP trigger | CRITICAL | tenant_id, pattern_type, action_taken, request_id |
| **System** | Configuration change | INFO | actor_id, key, old_value, new_value |
| System | Deployment | INFO | version, actor_id, environment |
| System | Backup completed/failed | INFO/WARN | status, size, duration, error_if_any |

#### 6.1.2 Events That Must NOT Be Logged

| Data | Reason | Alternative |
|------|--------|-------------|
| API key plaintext | Key exposure risk | Log key hash prefix only (`gk_live_aB...`) |
| Provider API keys | Key exposure risk | Log key ID reference only |
| Request/response bodies | PII/confidential data exposure | Log metadata: token count, status, duration |
| JWT tokens | Session hijacking risk | Log `jti` claim only |
| Passwords (even hashed) | Credential exposure | Never log authentication credentials |
| Personal identifiable information | GDPR compliance | Log pseudonymized identifiers only |

### 6.2 Audit Trail Requirements

#### 6.2.1 Log Format Specification

Every log entry must include:

| Field | Required | Description | Example |
|-------|----------|-------------|---------|
| `timestamp` | Yes | ISO 8601 UTC | `2025-01-15T10:30:00.000Z` |
| `event_id` | Yes | Unique UUID | `evt_a1b2c3d4...` |
| `event_type` | Yes | Event type code | `user_login` |
| `severity` | Yes | INFO/WARN/CRITICAL | `WARN` |
| `actor_id` | Yes | ID of actor | `user_123` / `api_key_hash` |
| `actor_type` | Yes | Type of actor | `user` / `api_key` / `system` |
| `tenant_id` | Conditional | Target tenant (if applicable) | `org_456` |
| `ip_address` | Yes | Source IP (hashed if GDPR risk) | `hash:abc123` |
| `action` | Yes | Action performed | `create` / `read` / `update` / `delete` |
| `resource` | Yes | Resource affected | `/api/v1/keys/abc` |
| `status` | Yes | Outcome | `success` / `failure` / `denied` |
| `duration_ms` | No | Request duration | `45` |
| `metadata` | No | JSON metadata | `{"model":"gpt-4o"}` |
| `integrity_hash` | Yes | SHA-256 hash of log entry | `sha256:abc123...` |
| `chain_hash` | Yes | Hash of previous log entry | `sha256:prev123...` |

#### 6.2.2 Log Retention

| Log Type | Retention | Storage | Justification |
|----------|-----------|---------|---------------|
| Authentication logs | 1 year | Hot: 30 days → Cold: 335 days | SOC 2 requirement; incident investigation |
| Authorization logs | 1 year | Hot: 30 days → Cold: 335 days | SOC 2 requirement |
| Admin action logs | 3 years | Hot: 90 days → Cold: remainder | Change tracking; accountability |
| Security event logs | 3 years | Hot: 90 days → Cold: remainder | Forensic analysis |
| API access logs (metadata only) | 90 days | Hot: 30 days → Cold: 60 days | Usage analysis; rate limiting |
| System event logs | 90 days | Hot: 30 days → Cold: 60 days | Operational troubleshooting |
| Compliance audit logs | 7 years | Immutable archive | Legal/regulatory requirement |

### 6.3 Log Integrity Protection

| Control | Implementation | Verification |
|---------|---------------|------------|
| Append-only storage | Audit log table: write-once policy; no UPDATE or DELETE permissions on audit table | Database permission audit |
| Hash chain | Each entry includes SHA-256 of previous entry's hash; tamper detection script runs daily | `integrity_hash` and `chain_hash` fields |
| Separate DB user for audit | `audit_writer` role: INSERT only on audit table; no other permissions | Permission matrix review |
| External log streaming | Forward all security logs to external SIEM in real-time | Stream health monitoring |
| WORM storage for compliance | Object storage with object lock for 7-year retention | Storage config validation |

### 6.4 Access Logging

#### 6.4.1 API Access Log Format

```json
{
  "timestamp": "2025-01-15T10:30:00.000Z",
  "event_id": "evt_a1b2c3d4",
  "event_type": "api_request",
  "severity": "INFO",
  "actor_type": "api_key",
  "key_id": "hash:abc123",
  "tenant_id": "org_456",
  "ip_address": "hash:def456",
  "method": "POST",
  "path": "/v1/chat/completions",
  "status": 200,
  "duration_ms": 1200,
  "model": "gpt-4o",
  "tokens_in": 150,
  "tokens_out": 500,
  "cache_hit": false,
  "provider": "openai",
  "cost_cents": 2.5
}
```

**Note:** Request body content, response body content, and `Authorization` header are NEVER logged.

---

## 7. Vulnerability Management

### 7.1 Dependency Scanning

#### 7.1.1 Cargo Audit (Rust)

| Control | Frequency | Implementation |
|---------|-----------|---------------|
| `cargo audit` in CI | Every build (PR + merge) | Fails build on crates with known CVEs |
| Weekly full scan | Every Monday | Automated scan + report to Security Lead |
| SBOM generation | Every release | `cargo tree --format {p} > sbom.txt`; stored with release artifacts |
| Dependency allowlist | New deps require review | All new dependencies require Security Lead approval |

#### 7.1.2 Dependency Management Rules

- `Cargo.lock` committed to version control — exact versions for all builds
- No floating versions in `Cargo.toml` — use exact versions or `=x.y.z`
- Vetted dependency allowlist: document justification for each dependency
- Transitive dependency monitoring: `cargo audit` covers full dependency tree
- Vendor dependencies for air-gapped builds (optional)

### 7.2 Container Image Scanning

| Control | Tool | Frequency | Action on Finding |
|---------|------|-----------|-------------------|
| Image vulnerability scan | Trivy | Every build | Fail build on CRITICAL/HIGH findings |
| Runtime image minimalism | Distroless/scratch | Every build | No shell, no package manager, no SSH |
| Image signing | Cosign | Every build | Sign all images; verify before deployment |
| Image digest pinning | SHA256 | Every build | Use `image@sha256:...` not tags |
| Base image updates | Trivy scan | Weekly | Automated alert on new CVEs in base images |

#### 7.2.2 Container Security Hardening

| Control | Implementation |
|---------|---------------|
| Non-root user | Run as `gateway` user (UID 1000) |
| Read-only root filesystem | `read_only: true` in Docker Compose |
| No new privileges | `security_opt: no-new-privileges:true` |
| Drop all capabilities | `cap_drop: [ALL]` |
| Seccomp profile | Default Docker seccomp + custom restrictions |
| No privileged mode | Never use `--privileged` or equivalent |
| User namespaces | Enable user namespace remapping |
| Resource limits | `mem_limit`, `cpus`, `pids_limit` on all containers |

### 7.3 Security Update Process

| Severity | Patch SLA | Process |
|----------|-----------|---------|
| **Critical** | 7 calendar days | Emergency patch: security team + platform lead coordinate immediate fix |
| **High** | 30 calendar days | Standard patch: scheduled into next sprint |
| **Medium** | 90 calendar days | Planned patch: scheduled maintenance window |
| **Low** | Next release | Include with regular feature release |

#### 7.3.1 Patch Process

1. **Detection:** `cargo audit`, Trivy scans, or external vulnerability report
2. **Assessment:** Security Lead assesses applicability to gateway architecture
3. **Prioritization:** Severity mapped to SLA above
4. **Implementation:** Developer updates dependency, runs tests
5. **Verification:** Security regression tests + integration tests
6. **Deployment:** Standard CI/CD pipeline with canary deployment
7. **Confirmation:** Post-deployment scan confirms vulnerability resolved
8. **Documentation:** Update vulnerability tracking log

### 7.4 Penetration Testing Schedule

| Test Type | Frequency | Scope | Vendor |
|-----------|-----------|-------|--------|
| External penetration test | Annual | Public API endpoints, admin dashboard | Third-party security vendor |
| Internal penetration test | Annual | Internal network, container escape, privilege escalation | Third-party security vendor |
| Bug bounty review | Annual | Review bug bounty submissions (if program active) | Security team |
| Threat model review | Quarterly | Update threat model based on new features, architecture changes | Security Lead + Backend Lead |
| Code security audit | Quarterly | Automated + manual review of security-critical code | Security team |

### 7.5 Security Review Gates

| Gate | Trigger | Reviewer | Checklist |
|------|---------|----------|-----------|
| **Pre-launch** | Before production deployment | Security Lead | All P0 controls implemented, penetration test complete, no open Critical/High vulnerabilities |
| **Major feature** | New feature touching auth, billing, or tenant isolation | Security Lead + Backend Lead | Threat model update, security test cases, input validation review |
| **Dependency addition** | New crate added to Cargo.toml | Security Lead | Justification documented, `cargo audit` clean, license compatible |
| **Architecture change** | Changes to deployment, data flow, or trust boundaries | Security Lead | STRIDE analysis, trust boundary review, detection rule update |

---

## 8. Incident Response

### 8.1 Security Incident Classification

| Severity | Definition | Examples | Response Time |
|----------|------------|----------|---------------|
| **P1 - Critical** | Active data breach; service unusable; complete compromise | RCE exploited, DB exposed, ransomware active, confirmed key theft at scale | 15 minutes |
| **P2 - High** | Major feature degraded; confirmed vulnerability exploited | Auth bypass active, major provider outage, large-scale cost abuse | 1 hour |
| **P3 - Medium** | Partial degradation; potential vulnerability not yet exploited | Rate limiting issues, minor provider outage, low-severity vulnerability discovered | 4 hours |
| **P4 - Low** | Cosmetic; informational; no immediate impact | UI glitch, documentation issue, information disclosure with no sensitive data | 1 business day |

### 8.2 Response Procedures

#### 8.2.1 Response Workflow

| Phase | Timeline | Actions | Owner |
|-------|----------|---------|-------|
| **Detection** | 0-15 min | Automated alert or manual report; on-call paged | Monitoring / Reporter |
| **Triage** | 15-30 min | Validate incident; classify severity; assign IR lead | On-call Engineer |
| **Containment** | 30 min - 2h | Short-term: stop bleeding (block IPs, rotate keys, disable feature, trigger circuit breaker) | IR Lead |
| **Investigation** | 2h - 24h | Determine scope, root cause, affected tenants; preserve evidence | IR Lead + Security |
| **Eradication** | 1h - 48h | Remove threat; patch vulnerability; fix root cause | Engineering |
| **Recovery** | 1h - 72h | Restore service; verify integrity; monitor for recurrence | Engineering |
| **Post-Incident** | 5 business days | Post-mortem; action items; communication | IR Lead |
| **Closure** | 10 business days | All action items assigned; incident closed | Compliance Lead |

#### 8.2.2 Containment Playbooks

**API Key Theft (T-003):**
1. Immediately revoke compromised API key
2. Rotate all keys for the affected organization
3. Check access logs for unauthorized usage patterns (last 30 days)
4. Block source IP addresses at Nginx level
5. Notify organization administrator
6. Forensic analysis: determine compromise vector

**Tenant Escape (T-004):**
1. Immediately disable attacker's account and all API keys
2. Audit all access by compromised account (last 30 days)
3. Determine scope: which organizations' data was accessed
4. Notify affected organizations within 24 hours
5. Review authorization middleware for bypass vulnerability
6. Emergency patch deployment

**Denial of Wallet (T-007):**
1. Trigger emergency circuit breaker for affected organization
2. Revoke compromised API keys
3. Analyze request patterns to identify attack source
4. Block attacking IP addresses at Nginx level
5. Coordinate with provider on rate limit enforcement
6. Review billing impact; prepare customer credit/refund

**Authentication Bypass (T-009):**
1. Identify bypass method and affected endpoints
2. Temporarily disable affected authentication mechanism if needed
3. Force all active sessions to re-authenticate (increment session version)
4. Review JWT validation code for algorithm confusion or none-alg acceptance
5. Emergency patch
6. Audit all admin actions during exposure window

### 8.3 Escalation Paths

| Severity | Initial Response | Escalation Path |
|----------|-----------------|-----------------|
| P1 (Critical) | On-call Engineer (15 min) | On-call → Security Lead + Platform Lead (30 min) → CTO + CEO + Legal (1 hour) |
| P2 (High) | On-call Engineer (1 hour) | On-call → Security Lead (2 hours) → Engineering Lead (4 hours) |
| P3 (Medium) | On-call Engineer (4 hours) | On-call → Engineering Lead (next business day) |
| P4 (Low) | Track in backlog | Engineering Lead (weekly review) |

### 8.4 Communication Templates

#### 8.4.1 Customer Notification (Critical Incident)

```
Subject: Security Incident Notification - [Organization Name]

Dear [Customer] Administrator,

We are writing to inform you of a security incident affecting our AI Gateway service.

INCIDENT SUMMARY:
- What: [Brief description]
- When: [Date/time, timezone]
- Duration: [Duration of impact]
- Severity: [Critical/High/Medium]

IMPACT ASSESSMENT:
- Your organization's data: [Affected/Not affected/Under investigation]
- API key status: [Revoked/Active/Rotated]
- Recommended action: [Rotate keys, review usage, no action required]

ACTIONS TAKEN:
- [Containment measures]
- [Investigation status]
- [Remediation steps]

We will provide an update within [24 hours].

Contact: security@[company].com
```

#### 8.4.2 Regulatory Notification Timeline

| Regulation | Trigger | Timeline | Recipient |
|------------|---------|----------|-----------|
| GDPR Art. 33 | Personal data breach likely to result in risk to rights | 72 hours to DPA | Supervisory Authority |
| GDPR Art. 34 | High risk to rights | Without undue delay | Affected data subjects |
| CCPA/CPRA | Unauthorized access to unencrypted personal information | Without undue delay | Affected CA residents |
| HIPAA Breach Rule | Breach of unsecured PHI | 60 days to HHS; 60 days to individuals | HHS + affected individuals |

### 8.5 Incident Response Roles

| Role | Responsibility | Primary | Backup |
|------|---------------|---------|--------|
| **Incident Commander** | Overall IR coordination; decision authority | Security Lead | CTO |
| **Technical Lead** | Technical investigation; containment; recovery | Platform Lead | Senior Engineer |
| **Communications Lead** | Internal + external communications | Compliance Lead | CEO |
| **Legal Advisor** | Regulatory notification; legal risk assessment | Legal Counsel | External counsel |
| **Customer Liaison** | Customer communication; DSR coordination | Customer Success Lead | Support Lead |
| **Forensics Lead** | Evidence preservation; forensic analysis | Security Lead | External forensics |

---

## 9. Security Checklist (Implementation Order)

### Must Have Before Launch (P0)

| # | Priority | Control | Effort | Status | Reference |
|---|----------|---------|--------|--------|-----------|
| 1 | P0 | Parameterized queries via sqlx on all database queries; zero raw SQL concatenation | 3d | Pending | T-013, T-004 |
| 2 | P0 | PostgreSQL Row-Level Security enabled on all tenant-scoped tables | 1d | Pending | T-004, R-PROD-004 |
| 3 | P0 | Request-scoped tenant context: org_id stored in Axum extensions, verified on every request | 2d | Pending | T-004, T-010 |
| 4 | P0 | Deny-by-default RBAC: every endpoint declares required permission; default reject | 2d | Pending | T-010, STR-BE-012 |
| 5 | P0 | Organization scoping on every admin action: verify admin.org_id == resource.org_id | 1d | Complete | T-004, T-010 |
| 6 | P0 | RS256 JWT only: hardcoded algorithm; reject `none`, `HS256`, and all unexpected `alg` values | 1d | Pending | T-009, STR-BE-001 |
| 7 | P0 | Constant-time API key comparison using `subtle::ConstantTimeEq`; identical responses for all invalid keys | 1d | Pending | T-009, T-003 |
| 8 | P0 | Secure API key generation: CSPRNG 192-bit entropy, `gk_live_` prefix, Base58 encoding, CRC32 checksum | 1d | Pending | T-009, T-003 |
| 9 | P0 | Tenant ID prefix on all cache keys: `llm:exact:{org_id}:{model}:{hash}` | 1d | Pending | T-001, T-004 |
| 10 | P0 | AES-256-GCM encryption for all customer provider keys at rest; master key in Docker Secrets | 2d | Pending | T-003, R-TECH-004 |
| 11 | P0 | Zeroize memory: clear key material immediately after use using `zeroize` crate | 1d | Pending | T-003, STR-BE-006 |
| 12 | P0 | No debug endpoints in production: gate with `cfg(debug_assertions)`; CI check | 1d | Pending | T-015, STR-BE-007 |
| 13 | P0 | All secrets in Docker Secrets (`/run/secrets/`); never environment variables | 2d | Pending | T-015, STR-DOCKER-004 |
| 14 | P0 | Cost-based circuit breaker: per-organization hard cap; HTTP 429 when budget exceeded | 3d | Pending | T-007, R-FIN-001 |
| 15 | P0 | Tiered rate limiting: per-org requests/minute, tokens/minute, tokens/day, max cost/day | 2d | Pending | T-007, T-006 |
| 16 | P0 | Request size limits: max body 1MB, max 50 messages, max 100KB per message | 1d | Pending | STR-BE-010 |
| 17 | P0 | URL whitelist for providers: only pre-approved domains; reject all others | 1d | Complete | T-005 |
| 18 | P0 | Internal IP blocklist for SSRF: reject 127.0.0.0/8, 10.0.0.0/8, 172.16.0.0/12, 192.168.0.0/16, 169.254.0.0/16 | 1d | Complete | T-005 |
| 19 | P0 | No action based on LLM response: no URL prefetching, no response-activated webhooks | 1d | Pending | T-002 |
| 20 | P0 | Structured logging with redaction: replace `sk-\w+` with `[REDACTED]`; never log bodies | 1d | Pending | T-011, R-TECH-006 |
| 21 | P0 | TLS 1.3 preferred, TLS 1.2 minimum for all external connections; HSTS header | 1d | Pending | R-TECH-005 |
| 22 | P0 | Argon2id password hashing: time_cost=3, memory_cost=65536, parallelism=4 | 1d | Pending | AUTH-SPEC |
| 23 | P0 | Account lockout: 30-minute lock after 5 failed login attempts | 1d | Pending | T-009 |
| 24 | P0 | HttpOnly Secure SameSite=Strict cookies for JWT session transport | 1d | Pending | STR-FE-001 |
| 25 | P0 | Container hardening: non-root user, read-only fs, no-new-privileges, drop all capabilities | 2d | Pending | STR-DOCKER-007 |
| 25a | P0 | OIDC state parameter CSRF protection: random nonce in Redis, 10-min TTL, one-time use | 1d | Complete | T-0096 |
| 25b | P0 | SAML RelayState CSRF protection: random nonce in Redis, 10-min TTL, one-time use | 1d | Complete | T-0096 |
| 25c | P0 | SSO admin endpoints enforce RBAC (`settings:read` / `settings:write`) + org_id verification | 1d | Complete | T-0096 |

### Must Have Within 30 Days (P1)

| # | Priority | Control | Effort | Status | Reference |
|---|----------|---------|--------|--------|-----------|
| 26 | P1 | Cache content integrity: store SHA-256 hash alongside cached response; verify on retrieval | 1d | Pending | T-001, STR-REDIS-002 |
| 27 | P1 | Cache TTL policy: chat 60-300s, embeddings 3600s; limits poison window | 1d | Pending | T-001 |
| 28 | P1 | Cache invalidation on key rotation: flush `llm:*:{org_id}:*` pattern | 1d | Pending | T-001 |
| 29 | P1 | PII detection before caching: regex patterns for SSN, credit cards, emails; skip cache if detected | 1d | Pending | CACHE-SEC, R-COMP-001 |
| 30 | P1 | Response content filtering: scan for system prompt text, internal IPs, JWT patterns in responses | 1d | Pending | T-002 |
| 31 | P1 | Strip tools/functions parameters unless explicitly enabled per-organization | 1d | Pending | T-002 |
| 32 | P1 | Log sanitization: escape control characters in LLM responses before logging | 1d | Pending | T-002, T-011 |
| 33 | P1 | Certificate pinning (TOFU) for AI providers; alert on certificate change | 2d | Pending | T-008 |
| 34 | P1 | `cargo audit` in CI: fail build on known CVEs; weekly automated scans | 1d | Pending | T-014 |
| 35 | P1 | Distroless runtime images: no shell, no package manager | 1d | Pending | T-014, STR-DOCKER-008 |
| 36 | P1 | Docker image signing with Cosign; verify before deployment | 1d | Pending | T-014 |
| 37 | P1 | Redis ACL: command whitelist (GET, SET, EXPIRE, INCR); deny FLUSHALL, CONFIG, DEBUG, SLAVEOF | 1d | Pending | T-012, STR-REDIS-009 |
| 38 | P1 | API key access audit: log every key decryption with tenant_id, timestamp, requesting_user | 1d | Pending | T-003 |
| 39 | P1 | Self-service key rotation UI with 7-day grace period | 2d | Pending | R-TECH-004 |
| 40 | P1 | Idempotency key canonicalization: normalize request body before hash computation | 1d | Pending | T-006 |
| 41 | P1 | Admin action audit logging: append-only table with integrity hash chain | 1d | Pending | T-010 |
| 42 | P1 | Connection timeouts: Nginx body 10s/header 10s, backend 60s total | 1d | Pending | T-007 |
| 43 | P1 | Concurrent request limits: max 50 per API key, max 10 concurrent streams | 1d | Pending | T-007 |
| 44 | P1 | Token-based rate limiting: limit on output tokens, not just request count | 1d | Pending | T-007 |
| 45 | P1 | DNS resolution before HTTP request: validate resolved IP against blocklist | 1d | Pending | T-005 |
| 46 | P1 | Disable HTTP redirect following in HTTP client (reqwest) | 1d | Pending | T-005 |
| 47 | P1 | Disable core dumps: `ulimit -c 0` in container; core_pattern to /dev/null | 1d | Pending | T-003, STR-BE-006 |
| 48 | P1 | No key material in logs: automated CI scanning for `sk-` patterns in log statements | 1d | Pending | T-011 |
| 49 | P1 | Configuration validation at startup: reject if debug mode detected, env vars contain secrets | 1d | Pending | T-015 |
| 50 | P1 | Automated secret scanning in CI: truffleHog or GitHub secret scanning on every push | 1d | Pending | T-015 |

### Must Have Within 90 Days (P2)

| # | Priority | Control | Effort | Status | Reference |
|---|----------|---------|--------|--------|-----------|
| 51 | P2 | Usage anomaly detection: >5x deviation from 7-day average triggers alert + rate reduction | 2d | Pending | T-007, R-TECH-004 |
| 52 | P2 | Multi-provider failover: health checks every 30s; automatic routing to backup provider | 3d | Pending | T-008, R-TECH-007 |
| 53 | P2 | Provider health monitoring: latency, error rate, response format anomaly detection | 2d | Pending | T-008 |
| 54 | P2 | Canary requests: periodic requests with known expected responses; alert on deviation | 1d | Pending | T-008 |
| 55 | P2 | HSM or external key management (HashiCorp Vault) for master key | 2d | Pending | T-003 |
| 56 | P2 | Backup encryption: GPG-encrypt all PostgreSQL backups; separate backup encryption key | 1d | Pending | R-TECH-001 |
| 57 | P2 | Automated backup restore tests: weekly automated restore + integrity verification | 2d | Pending | R-TECH-001 |
| 58 | P2 | Centralized logging: deploy Loki/Grafana or ELK stack for log aggregation | 2d | Pending | COMPLIANCE-AU |
| 59 | P2 | Anomaly detection for cost/usage: ML-based or statistical baseline per organization | 2d | Pending | T-007 |
| 60 | P2 | File integrity monitoring on critical files: alert on unauthorized changes | 1d | Pending | COMPLIANCE-CC7 |
| 61 | P2 | WAF deployment: ModSecurity/nginx with SQLi and injection rule sets | 2d | Pending | T-013 |
| 62 | P2 | MFA for admin dashboard: TOTP (Google Authenticator/Authy) | 2d | Pending | COMPLIANCE-AC-002 |
| 63 | P2 | Concurrent session limits: max 3 sessions per admin user | 1d | Pending | T-009 |
| 64 | P2 | JWT key rotation: automated 90-day rotation with grace period | 2d | Pending | T-009 |
| 65 | P2 | Email verification required before login | 1d | Pending | AUTH-SPEC |
| 66 | P2 | Password reset flow: secure token (32 bytes CSPRNG), 1-hour expiry, single use | 1d | Pending | AUTH-SPEC |
| 67 | P2 | DLP scanning on logs: automated scan for sensitive patterns; alert and quarantine | 2d | Pending | T-011, R-TECH-006 |
| 68 | P2 | Cross-border data transfer controls: EU region deployment option, SCCs in DPAs | 2d | Pending | R-COMP-004 |
| 69 | P2 | Sub-processor governance: published list, 30-day notification for additions | 1d | Pending | COMPLIANCE-DP-002 |
| 70 | P2 | Data retention enforcement: automated purging per retention schedule | 2d | Pending | COMPLIANCE-DS |

### Ongoing (Continuous)

| # | Priority | Control | Frequency | Reference |
|---|----------|---------|-----------|-----------|
| 71 | Ongoing | Quarterly access reviews: all admin users, last login, actions taken | Quarterly | COMPLIANCE-AC-004 |
| 72 | Ongoing | Vulnerability scanning: `cargo audit` + Trivy image scans | Weekly | COMPLIANCE-CM-003 |
| 73 | Ongoing | Penetration testing: third-party external + internal | Annual | COMPLIANCE-CM-004 |
| 74 | Ongoing | Security awareness training for all employees | Annual | COMPLIANCE-TS-001 |
| 75 | Ongoing | Phishing simulations | Quarterly | COMPLIANCE-TS-001 |
| 76 | Ongoing | Disaster recovery tests | Quarterly | COMPLIANCE-BC-001 |
| 77 | Ongoing | Risk register review | Quarterly | RISKS-REGISTER |
| 78 | Ongoing | Threat model review | Quarterly or after architecture changes | THREAT-MODEL |
| 79 | Ongoing | Log integrity verification: hash chain tamper detection | Daily | COMPLIANCE-AU-002 |
| 80 | Ongoing | Secret rotation: JWT keys, DB password, Redis password | 90 days | SEC-SECRETS |
| 81 | Ongoing | Dependency review: assess all new dependencies before addition | Per change | T-014 |
| 82 | Ongoing | Backup restore verification | Weekly | R-TECH-001 |
| 83 | Ongoing | Provider certificate monitoring: alert on fingerprint changes | Continuous | T-008 |
| 84 | Ongoing | Canary request monitoring: alert on response deviation | Continuous | T-008 |

---

## 10. Security Decision Log

### 10.1 Why API Keys Over mTLS for Initial Release

**Decision:** Use random API keys (System A) instead of mutual TLS for programmatic authentication.

**Rationale:**
- mTLS requires certificate infrastructure (CA, issuance, rotation, revocation) that adds 2-3 weeks to launch timeline
- API keys are standard in AI API ecosystem; customers expect Bearer token authentication
- Key rotation is simpler: revoke and regenerate vs. certificate revocation list management
- mTLS adds significant operational complexity for a single-VPS deployment

**Trade-offs:**
- API keys can be stolen if leaked by customer; mTLS keys are non-exportable
- API keys require secure storage and rotation discipline

**Future:** mTLS offered as enterprise tier option for high-security customers.

**References:** AUTH.md Section 1.3, T-003, R-TECH-004

### 10.2 Why Row-Level Security + Application Filtering

**Decision:** Use both PostgreSQL RLS policies AND application-layer `WHERE org_id = $1` on every query.

**Rationale:**
- RLS alone: defense in depth if application forgets filter, but adds per-query overhead
- Application filtering alone: fast but vulnerable to query construction errors
- Combined: application catches most cases with minimal overhead; RLS catches any missed queries as safety net
- RLS policies use `current_setting('app.current_org_id')` set per-request by application

**Trade-offs:**
- Slight performance overhead from RLS policy evaluation
- Requires careful management of the `app.current_org_id` session variable

**References:** T-004, STR-DB-006, AUTH.md Section 5.3

### 10.3 Why JWT Sessions Over OAuth

**Decision:** Use RS256-signed JWT sessions with httpOnly cookies instead of OAuth 2.0 / OpenID Connect.

**Rationale:**
- Self-contained auth eliminates operational dependency on external identity provider uptime
- Not all customers have corporate identity providers; self-hosted auth works for all
- RS256 allows stateless verification: no database lookup on every request
- Refresh token rotation enables session revocation without database dependency
- 15-minute access token lifetime limits theft window

**Trade-offs:**
- No single sign-on with corporate identity providers (planned as optional add-on)
- JWT revocation requires revocation list check (Redis `EXISTS revoked:{jti}`)
- Session invalidation across all devices requires session version increment

**Future:** OIDC SSO integration as optional add-on for enterprise customers.

**References:** AUTH.md Section 1.3, T-009, STR-FE-001

### 10.4 Why No MFA in MVP

**Decision:** Do not require multi-factor authentication for initial release.

**Rationale:**
- MFA adds friction to onboarding; MVP priority is validating product-market fit
- TOTP requires email/SMS infrastructure for recovery flows
- Account lockout (5 failed attempts → 30 min lock) provides baseline brute force protection
- Argon2id password hashing provides strong credential storage

**Trade-offs:**
- Account takeover risk if password is weak or reused
- SOC 2 CC6.3 requires MFA for admin accounts
- GDPR Art. 32 expects strong authentication

**Mitigation:** MFA scheduled for P1 (30-day post-launch). Optional MFA available for early adopters.

**Future:** TOTP-based MFA required for all admin accounts by default; WebAuthn for enterprise tier.

**References:** COMPLIANCE.md AC-002, T-009, R-COMP-003

### 10.5 Why Exact + Semantic Cache Over CDN Edge Caching

**Decision:** Use L1 in-process (moka) + L2 Redis caching with exact and semantic matching. No CDN edge caching.

**Rationale:**
- AI responses are non-deterministic and user-specific; CDN hit rate would be <2%
- Request diversity (different system prompts per tenant, different parameters) makes edge caching ineffective
- CDN cache invalidation is coarse (path-based); LLM cache needs fine-grained semantic invalidation
- CDN egress costs often exceed savings from cache hits
- Semantic caching increases hit rate 3-10x over exact match alone by catching rephrasings

**Trade-offs:**
- Single VPS deployment limits cache to one Redis instance
- Semantic cache requires embedding computation (~5-10ms overhead on miss)
- Cache warming is not possible; cold start means all cache misses

**Future:** Redis Cluster or dedicated cache layer for multi-node deployments.

**References:** CACHE.md Section 1.3, T-001, STR-REDIS-005

### 10.6 Why API Key Prefix Shows `gk_` Not Embedded org_id

**Decision:** API key format is `gk_live_<random><checksum>` with no embedded organization identifier.

**Rationale:**
- Embedding org_id in key (e.g., `ag_42_a1b2c3`) enables enumeration: iterate org_ids to find valid keys
- Constant-time lookup with uniform response prevents timing attacks
- 192 bits of random entropy provides sufficient key space (2^187 combinations)
- Prefix serves branding/identification only; no security function

**Trade-offs:**
- Requires database lookup to map key to organization (cached in Redis, <1ms overhead)
- No way to route to correct org without lookup (vs. embedded org_id enabling direct routing)

**References:** AUTH.md Section 2.1, T-009

### 10.7 Why Single VPS for Initial Deployment

**Decision:** Deploy on a single VPS via Docker Compose instead of Kubernetes or multi-AZ.

**Rationale:**
- Single VPS is operationally simpler for a small team; no Kubernetes expertise required
- Cost-efficient for pre-revenue stage
- Docker Compose provides adequate service orchestration for 4-container stack
- Vertical scaling (larger VPS) handles initial growth

**Trade-offs:**
- Single point of failure: VPS outage brings entire service down
- No auto-scaling: manual intervention required for traffic spikes
- No data residency options (single region only)
- Limited by single-node cache performance

**Mitigation:** Health checks with auto-restart, daily automated backups, documented migration path to multi-node.

**Future:** Kubernetes migration documented; triggered by scaling needs or enterprise customer requirements.

**References:** R-TECH-002, R-OPS-002, R-PROD-003

### 10.8 Why No Request/Response Body Logging

**Decision:** Never log request or response bodies in production. Log metadata only.

**Rationale:**
- AI request/response bodies frequently contain PII, PHI, proprietary business data, trade secrets
- Log files are copied to multiple locations (backup, centralized logging, crash dumps)
- Once logged, sensitive data is nearly impossible to fully remove
- Compliance requirements (GDPR Art. 32, HIPAA) mandate data minimization
- Metadata (token count, model, status, duration) is sufficient for debugging and billing

**Trade-offs:**
- Harder to debug certain issues without body content
- Customer support cannot see exact request/response for troubleshooting

**Mitigation:** Optional opt-in body logging for specific customers with shorter retention (7 days), encrypted storage, explicit consent.

**References:** T-011, R-TECH-006, COMPLIANCE.md Section 4.1

### 10.9 Why Cargo.lock Committed (Dependency Pinning)

**Decision:** Commit `Cargo.lock` to version control; use exact versions for all dependencies.

**Rationale:**
- Reproducible builds: same `Cargo.lock` + same Rust version = same binary
- Prevents supply chain attacks via dependency version squatting
- `cargo audit` can scan exact dependency tree for CVEs
- Enables binary integrity verification (compare deployed hash with CI-built hash)

**Trade-offs:**
- Dependency updates require explicit PR and review
- Security patches require manual intervention

**Mitigation:** Weekly `cargo audit` scans; Dependabot-style alerts for new CVEs in pinned versions.

**References:** T-014, STR-DOCKER-001

### 10.10 Why Argon2id Over bcrypt

**Decision:** Use Argon2id for password hashing instead of bcrypt.

**Rationale:**
- Argon2id is the OWASP-recommended password hashing algorithm (as of 2023)
- Memory-hard function: resistance to GPU/ASIC attacks
- Configurable memory cost (64MB default) makes parallel cracking expensive
- Winner of Password Hashing Competition (2015)

**Parameters:** time_cost=3, memory_cost=65536 (64MB), parallelism=4, hash_len=32, salt_len=16

**Trade-offs:**
- Higher memory usage per hash verification than bcrypt
- Slightly more complex configuration

**References:** AUTH.md Section 3.2, NIST SP 800-63B

---

## Document Control

| Attribute | Value |
|-----------|-------|
| **Document ID** | SEC-AIGW-001 |
| **Version** | 1.0 |
| **Owner** | Security Lead / CISO |
| **Classification** | Internal Use |
| **Review Cycle** | Quarterly |
| **Last Updated** | 2025-01-15 |
| **Next Review** | 2025-04-15 |

### Source Document References

| Document | File | Threat IDs | Risk IDs |
|----------|------|------------|----------|
| Threat Model | `THREAT_MODEL.md` | T-001 through T-015 | — |
| Compliance Requirements | `COMPLIANCE.md` | — | R-COMP-001 through R-COMP-006 |
| Risk Register | `RISKS.md` | — | R-TECH-001 through R-PROD-005 |
| Authentication & Authorization | `AUTH.md` | T-009, T-010, T-004 | — |
| Cache Architecture | `CACHE.md` | T-001, T-012 | — |

---

*Document generated: Security Strategy v1.0*
*Review cycle: Quarterly or after significant architecture changes*
*Next review date: 90 days from publication*
