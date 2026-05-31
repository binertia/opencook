# Authentication & Authorization Specification — AI Gateway

**Version:** 1.0.0
**Status:** Implementation-Ready
**Last Updated:** 2025-01-15

---

## Table of Contents

1. [Authentication Systems Overview](#1-authentication-systems-overview)
2. [API Key Authentication (System A)](#2-api-key-authentication-system-a)
3. [Session Authentication (System B)](#3-session-authentication-system-b)
4. [Authorization (RBAC)](#4-authorization-rbac)
5. [Tenant Isolation](#5-tenant-isolation)
6. [Superadmin](#6-superadmin)
7. [Security Controls](#7-security-controls)
8. [Error Handling](#8-error-handling)

---

## 1. Authentication Systems Overview

### 1.1 Architecture

The AI Gateway operates two independent authentication systems serving different client types:

| System | Client Type | Auth Method | State | Primary Use Case |
|--------|-------------|-------------|-------|-----------------|
| **System A** | API Consumers | API Key | Stateless | AI API requests (chat completions, embeddings, etc.) |
| **System B** | Dashboard Users | Session (JWT) | Stateful | Admin dashboard access, org management |

### 1.2 High-Level Flow

```
┌─────────────────────────────────────────────────────────────────────┐
│                         AI Gateway                                   │
│                                                                      │
│  ┌──────────────────┐          ┌──────────────────┐                 │
│  │  System A        │          │  System B        │                 │
│  │  API Key Auth    │          │  Session Auth    │                 │
│  │                  │          │                  │                 │
│  │  Authorization:  │          │  Cookie:         │                 │
│  │  Bearer gk_...   │          │  session=JWT     │                 │
│  │        │         │          │        │         │                 │
│  │        ▼         │          │        ▼         │                 │
│  │  ┌───────────┐   │          │  ┌───────────┐   │                 │
│  │  │  Redis    │   │          │  │  JWT      │   │                 │
│  │  │  Cache    │   │          │  │  Verify   │   │                 │
│  │  └───────────┘   │          │  └───────────┘   │                 │
│  │        │         │          │        │         │                 │
│  │        ▼         │          │        ▼         │                 │
│  │  Org Context     │          │  User Context    │                 │
│  └──────────────────┘          └──────────────────┘                 │
│           │                              │                          │
│           └──────────────┬───────────────┘                          │
│                          ▼                                          │
│                   ┌────────────┐                                    │
│                   │  RBAC      │                                    │
│                   │  Engine    │                                    │
│                   └────────────┘                                    │
└─────────────────────────────────────────────────────────────────────┘
```

### 1.3 Key Design Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Two separate auth systems | Yes | API keys and human sessions have fundamentally different lifecycle, performance, and security requirements. Merging them creates complexity without benefit. |
| No external IdP required | Yes | Self-contained auth reduces operational dependency. OIDC SSO is supported as optional add-on. |
| JWT for sessions | Yes | Stateless verification eliminates session DB lookups on every request. Refresh tokens enable revocation. |
| API keys hashed in DB | Yes | Never store plaintext API keys. Hash with SHA-256 for lookup; verify full key against prefix+hash. |
| Redis for API key cache | Yes | Sub-millisecond lookups. Cache key hash → org context mapping. |
| RBAC over ABAC | Yes | Simple role-based permissions are sufficient. ABAC adds complexity without clear benefit for this domain. |
| Argon2id for passwords | Yes | OWASP-recommended password hashing. Memory-hard resistance to GPU/ASIC attacks. |

### 1.4 Alternatives Considered

| Alternative | Why Rejected |
|-------------|-------------|
| Single auth system for both APIs and dashboard | Different security profiles and lifecycle requirements |
| OAuth2 as primary (mandatory external IdP) | Adds operational dependency; not all customers have corporate IdP |
| ABAC (Attribute-Based Access Control) | Overkill for current permission model; RBAC is simpler and sufficient |
| HS256 for JWT signing | RS256 preferred for key rotation and distributed verification; HS256 acceptable for single-instance deployments |
| localStorage for JWT storage | XSS-vulnerable; httpOnly cookies are secure by default |

---

## 2. API Key Authentication (System A)

### 2.1 Key Format

#### 2.1.1 Structure

```
gk_live_<random><checksum>
gk_test_<random><checksum>
```

| Component | Description | Length | Example |
|-----------|-------------|--------|---------|
| `gk_` | Product prefix (gateway) | 3 chars | `gk_` |
| `live` or `test` | Environment indicator | 4 chars | `live` |
| `_` | Separator | 1 char | `_` |
| `<random>` | Cryptographically secure random | 32 chars (Base58) | `aBcDeFgHiJkLmNoPqRsTuVwXyZaBcDeF` |
| `<checksum>` | CRC-32 truncated to 6 chars | 6 chars (Base58) | `XyZaBc` |

**Total length:** 3 + 1 + 4 + 1 + 32 + 6 = **47 characters**

**Full example:** `gk_live_aBcDeFgHiJkLmNoPqRsTuVwXyZaBcDeFXyZaBc`

#### 2.1.2 Format Validation (Regex)

```regex
^gk_(live|test)_[A-Za-z0-9]{38}$
```

Or with checksum validation (pseudocode):
```python
import re
import base58

API_KEY_REGEX = re.compile(r'^gk_(live|test)_([A-Za-z0-9]{32})([A-Za-z0-9]{6})$')

def validate_key_format(key: str) -> bool:
    """Validate API key format and checksum."""
    match = API_KEY_REGEX.match(key)
    if not match:
        return False
    
    env, random_part, checksum = match.groups()
    computed = compute_checksum(random_part)
    return checksum == computed
```

#### 2.1.3 Base58 Alphabet

```
123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz
```

Excludes: `0` (zero), `O` (capital o), `I` (capital i), `l` (lowercase L) to prevent transcription errors.

### 2.2 Key Generation

#### 2.2.1 Algorithm

```python
import secrets
import base58
import crc32c

KEY_PREFIX = "gk"
RANDOM_BYTES = 24  # 32 chars in Base58
CHECKSUM_LENGTH = 6

def generate_api_key(environment: str = "live") -> tuple[str, str]:
    """
    Generate a new API key.
    
    Returns:
        tuple: (full_key, key_hash)
               full_key: the plaintext key to show once
               key_hash: SHA-256 hash for database storage
    
    Security: Uses secrets.token_bytes (os.urandom CSPRNG)
    """
    # 1. Generate cryptographically secure random bytes
    random_bytes = secrets.token_bytes(RANDOM_BYTES)
    
    # 2. Encode to Base58
    random_b58 = base58.b58encode(random_bytes).decode('ascii')
    
    # 3. Compute checksum (CRC-32C of random part, first 6 chars of Base58)
    checksum_raw = crc32c.crc32c(random_bytes)
    checksum_b58 = base58.b58encode_int(checksum_raw)[:CHECKSUM_LENGTH]
    
    # 4. Assemble full key
    full_key = f"{KEY_PREFIX}_{environment}_{random_b58}{checksum_b58}"
    
    # 5. Compute hash for storage (show only prefix in UI)
    key_hash = hashlib.sha256(full_key.encode()).hexdigest()
    
    return full_key, key_hash
```

#### 2.2.2 Generation Requirements

| Requirement | Specification |
|-------------|---------------|
| RNG source | `secrets.token_bytes` (Python) or `/dev/urandom` — CSPRNG |
| Entropy | 192 bits (24 bytes) |
| Collision probability | ~2^-192 (negligible) |
| Key space | 58^32 ≈ 2^187 |
| Generation rate limit | Max 10 keys/minute per organization |
| Max keys per org | 100 (configurable) |

### 2.3 Key Storage

#### 2.3.1 Database Schema

```sql
CREATE TABLE api_keys (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id          UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    key_hash        VARCHAR(64) NOT NULL,        -- SHA-256 of full key
    key_prefix      VARCHAR(12) NOT NULL,        -- First 8 chars for display: "gk_live_aB"
    environment     VARCHAR(10) NOT NULL,         -- 'live' or 'test'
    name            VARCHAR(255),                -- User-provided label
    description     TEXT,
    
    -- Scoping
    allowed_models  VARCHAR(100)[],              -- NULL = all models
    ip_allowlist    INET[],                      -- NULL = any IP
    expires_at      TIMESTAMP WITH TIME ZONE,    -- NULL = no expiration
    
    -- Status
    status          VARCHAR(20) NOT NULL DEFAULT 'active',
                    -- enum: active, revoked, expired
    revoked_at      TIMESTAMP WITH TIME ZONE,
    revoked_reason  VARCHAR(255),
    
    -- Quotas
    rate_limit_rps  INTEGER DEFAULT 100,         -- Requests per second
    monthly_budget  DECIMAL(12,2),               -- USD budget cap (NULL = unlimited)
    
    -- Metadata
    created_by      UUID NOT NULL REFERENCES users(id),
    created_at      TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    last_used_at    TIMESTAMP WITH TIME ZONE,
    updated_at      TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    
    -- Constraints
    CONSTRAINT valid_status CHECK (status IN ('active', 'revoked', 'expired')),
    CONSTRAINT valid_env CHECK (environment IN ('live', 'test')),
    UNIQUE(key_hash)
);

-- Indexes
CREATE INDEX idx_api_keys_org ON api_keys(org_id);
CREATE INDEX idx_api_keys_hash ON api_keys(key_hash);
CREATE INDEX idx_api_keys_status ON api_keys(status) WHERE status = 'active';
```

#### 2.3.2 Storage Rules

| Rule | Implementation |
|------|---------------|
| **Never store plaintext** | Only SHA-256 hash is stored. Full key shown exactly once at creation. |
| **Display prefix only** | UI shows `gk_live_aB...` (first 8 chars + ellipsis) for identification |
| **Hash algorithm** | SHA-256 (fast enough for lookup, collision-resistant) |
| **One-way storage** | No mechanism exists to recover plaintext from stored hash |

#### 2.3.3 Audit Record

```sql
CREATE TABLE api_key_audit (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    key_id      UUID NOT NULL REFERENCES api_keys(id),
    action      VARCHAR(50) NOT NULL,
                -- created, used, revoked, rotated, quota_exceeded
    actor_id    UUID REFERENCES users(id),         -- NULL for API usage
    ip_address  INET,
    user_agent  VARCHAR(512),
    metadata    JSONB,
    created_at  TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);
```

### 2.4 Key Validation (Per-Request Flow)

#### 2.4.1 Sequence Diagram

```
┌─────────┐     ┌──────────────┐     ┌─────────────┐     ┌──────────┐
│ Client  │     │ API Gateway  │     │ Redis Cache │     │ Database │
└────┬────┘     └──────┬───────┘     └──────┬──────┘     └────┬─────┘
     │                 │                    │                  │
     │ Authorization:  │                    │                  │
     │ Bearer gk_...   │                    │                  │
     │────────────────>│                    │                  │
     │                 │                    │                  │
     │                 │ 1. Extract key     │                  │
     │                 │ 2. Compute hash    │                  │
     │                 │    hash = SHA256(key)                  │
     │                 │                    │                  │
     │                 │ 3. Lookup cache    │                  │
     │                 │    GET cache:{hash}│                  │
     │                 │───────────────────>│                  │
     │                 │                    │                  │
     │                 │ 4. Cache hit?      │                  │
     │                 │<───────────────────│                  │
     │                 │                    │                  │
     │                 │ ── HIT ──> 7. Return org context      │
     │                 │                    │                  │
     │                 │ ── MISS ─> 5. Query DB                 │
     │                 │───────────────────────────────────────>│
     │                 │                    │                  │
     │                 │ 6. Store in cache  │                  │
     │                 │    SET cache:{hash} {context} EX 300   │
     │                 │───────────────────>│                  │
     │                 │                    │                  │
     │                 │ 7. Return org context                  │
     │                 │                    │                  │
     │                 │ 8. Validate scope                     │
     │                 │    - not revoked                       │
     │                 │    - not expired                       │
     │                 │    - IP allowed                        │
     │                 │    - model allowed                     │
     │                 │                    │                  │
     │                 │ 9. Check quota    │                   │
     │                 │    - rate limit   │                   │
     │                 │    - budget       │                   │
     │                 │                    │                  │
     │                 │ 10. Attach context │                  │
     │                 │    to request      │                  │
     │                 │                    │                  │
     │                 │ 11. Proxy to upstream                  │
     │                 │────────────────────────────────────────>│
```

#### 2.4.2 Validation Pseudocode

```python
import hashlib
import time
from datetime import datetime, timezone

# Cache key prefix for tenant isolation in Redis
CACHE_PREFIX = "auth:apikey"
CACHE_TTL_SECONDS = 300  # 5 minutes

async def authenticate_api_key(request: Request) -> AuthContext:
    """
    Authenticate an incoming API request.
    Target: <1ms overhead (excluding cache miss)
    """
    start_time = time.monotonic()
    
    # 1. Extract key from Authorization header
    auth_header = request.headers.get("Authorization", "")
    if not auth_header.startswith("Bearer "):
        raise AuthError("INVALID_KEY_FORMAT", "Authorization header must be 'Bearer <key>'")
    
    api_key = auth_header[7:]  # Strip "Bearer "
    
    # 2. Validate format
    if not validate_key_format(api_key):
        raise AuthError("INVALID_KEY_FORMAT", "API key format is invalid")
    
    # 3. Compute hash for lookup
    key_hash = hashlib.sha256(api_key.encode()).hexdigest()
    
    # 4. Check cache (Redis)
    cache_key = f"{CACHE_PREFIX}:{key_hash}"
    cached = await redis.get(cache_key)
    
    if cached:
        key_data = json.loads(cached)
        source = "cache"
    else:
        # 5. Cache miss → query database
        key_data = await db.fetchrow(
            "SELECT * FROM api_keys WHERE key_hash = $1 AND status = 'active'",
            key_hash
        )
        if not key_data:
            raise AuthError("INVALID_KEY", "API key not found")
        
        # 6. Store in cache (serialize to JSON)
        await redis.setex(
            cache_key,
            CACHE_TTL_SECONDS,
            json.dumps(dict(key_data))
        )
        source = "database"
    
    # 7. Validate key status
    if key_data["status"] != "active":
        if key_data["status"] == "revoked":
            raise AuthError("REVOKED_KEY", "This API key has been revoked")
        elif key_data["status"] == "expired":
            raise AuthError("EXPIRED_KEY", "This API key has expired")
    
    # 8. Check expiration date
    expires_at = key_data.get("expires_at")
    if expires_at and datetime.now(timezone.utc) > expires_at:
        # Update status to expired
        await db.execute(
            "UPDATE api_keys SET status = 'expired' WHERE id = $1",
            key_data["id"]
        )
        await redis.delete(cache_key)
        raise AuthError("EXPIRED_KEY", "This API key has expired")
    
    # 9. Check IP allowlist
    ip_allowlist = key_data.get("ip_allowlist")
    if ip_allowlist:
        client_ip = ipaddress.ip_address(request.client_ip)
        if client_ip not in [ipaddress.ip_network(ip) for ip in ip_allowlist]:
            raise AuthError("IP_NOT_ALLOWED", "Request IP not in allowlist")
    
    # 10. Check model restrictions
    allowed_models = key_data.get("allowed_models")
    if allowed_models:
        requested_model = request.body.get("model", "")
        if requested_model not in allowed_models:
            raise AuthError("MODEL_NOT_ALLOWED", f"Model '{requested_model}' not allowed for this key")
    
    # 11. Check rate limit
    rate_limit_key = f"ratelimit:{key_data['id']}:{int(time.time())}"
    current = await redis.incr(rate_limit_key)
    if current == 1:
        await redis.expire(rate_limit_key, 1)  # 1-second window
    if current > key_data.get("rate_limit_rps", 100):
        raise AuthError("RATE_LIMIT_EXCEEDED", "Rate limit exceeded for this API key")
    
    # 12. Check budget (monthly)
    budget = key_data.get("monthly_budget")
    if budget:
        spent = await get_monthly_spend(key_data["org_id"])
        if spent >= budget:
            raise AuthError("QUOTA_EXCEEDED", "Monthly budget exceeded")
    
    # 13. Build auth context
    auth_context = AuthContext(
        auth_type="api_key",
        key_id=key_data["id"],
        org_id=key_data["org_id"],
        environment=key_data["environment"],  # 'live' or 'test'
        allowed_models=allowed_models,
        rate_limit_rps=key_data.get("rate_limit_rps", 100),
        source=source
    )
    
    # 14. Update last_used (async, non-blocking)
    asyncio.create_task(update_last_used(key_data["id"]))
    
    # Performance logging
    elapsed_ms = (time.monotonic() - start_time) * 1000
    logger.debug(f"API key auth: {elapsed_ms:.2f}ms (source: {source})")
    
    return auth_context

async def update_last_used(key_id: UUID):
    """Update last_used timestamp (fire and forget)."""
    await db.execute(
        "UPDATE api_keys SET last_used_at = NOW() WHERE id = $1",
        key_id
    )
```

### 2.5 Key Revocation

#### 2.5.1 Revocation Flow

```python
async def revoke_api_key(key_id: UUID, reason: str, revoked_by: UUID):
    """
    Revoke an API key immediately.
    Revocation is atomic and propagates within 100ms.
    """
    # 1. Update database
    key_data = await db.fetchrow(
        """UPDATE api_keys 
           SET status = 'revoked', 
               revoked_at = NOW(), 
               revoked_reason = $1,
               updated_at = NOW()
           WHERE id = $2
           RETURNING key_hash""",
        reason, key_id
    )
    
    if not key_data:
        raise NotFoundError("API key not found")
    
    # 2. Invalidate cache immediately
    cache_key = f"{CACHE_PREFIX}:{key_data['key_hash']}"
    await redis.delete(cache_key)
    
    # 3. Publish revocation event (for distributed cache invalidation)
    await redis.publish("apikey:revoked", json.dumps({
        "key_hash": key_data["key_hash"],
        "key_id": str(key_id),
        "timestamp": datetime.now(timezone.utc).isoformat()
    }))
    
    # 4. Audit log
    await audit_log.record(
        action="revoked",
        key_id=key_id,
        actor_id=revoked_by,
        reason=reason
    )
```

#### 2.5.2 Cache Invalidation Strategy

| Scenario | Action | Latency |
|----------|--------|---------|
| Key revoked | `DEL cache:{hash}` + publish event | <10ms |
| Key expired (time-based) | TTL naturally expires | Up to CACHE_TTL_SECONDS |
| Key updated (rate limit, name) | `DEL cache:{hash}` | <10ms |
| Budget/quota change | `DEL cache:{hash}` | <10ms |
| Emergency (revoke all org keys) | `DEL auth:apikey:*` (flush by pattern) + broadcast | <100ms |

### 2.6 Key Rotation

#### 2.6.1 Rotation Flow

```python
async def rotate_api_key(key_id: UUID, rotated_by: UUID) -> str:
    """
    Rotate an API key: create new key, deprecate old key.
    
    Returns:
        The new plaintext API key (shown once)
    """
    async with db.transaction():
        # 1. Get existing key details
        old_key = await db.fetchrow(
            "SELECT * FROM api_keys WHERE id = $1", key_id
        )
        if not old_key:
            raise NotFoundError("API key not found")
        
        # 2. Generate new key with same permissions
        new_full_key, new_hash = generate_api_key(old_key["environment"])
        
        # 3. Create new key record
        new_key_id = await db.fetchval(
            """INSERT INTO api_keys 
                (org_id, key_hash, key_prefix, environment, name, 
                 allowed_models, ip_allowlist, rate_limit_rps, monthly_budget, created_by)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
               RETURNING id""",
            old_key["org_id"], new_hash, new_full_key[:8], old_key["environment"],
            f"{old_key['name']} (rotated)",
            old_key["allowed_models"], old_key["ip_allowlist"],
            old_key["rate_limit_rps"], old_key["monthly_budget"],
            rotated_by
        )
        
        # 4. Revoke old key
        await revoke_api_key(key_id, "Rotated", rotated_by)
        
        # 5. Audit log
        await audit_log.record(
            action="rotated",
            key_id=new_key_id,
            old_key_id=key_id,
            actor_id=rotated_by
        )
    
    return new_full_key  # Show once to user
```

#### 2.6.2 Rotation Policy

| Policy | Default | Description |
|--------|---------|-------------|
| Auto-rotation period | 90 days | Keys older than this trigger warning |
| Rotation grace period | 7 days | Old key continues working after rotation |
| Max age before mandatory rotation | 180 days | Key automatically revoked |
| Rotation notification | Email to admins | 30, 14, 7, 1 days before expiration |

### 2.7 Performance Targets

| Metric | Target | Measurement |
|--------|--------|-------------|
| Cache hit latency | <0.5ms | Redis `GET` operation |
| Cache miss latency | <5ms | DB query + cache write |
| Overall auth overhead | <1ms (p99) | End-to-end per request |
| Revocation propagation | <100ms | Time to invalidate all caches |
| Cache hit ratio | >95% | Monitored continuously |

---

## 3. Session Authentication (System B)

### 3.1 User Model

#### 3.1.1 Database Schema

```sql
CREATE TABLE users (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    email           VARCHAR(255) NOT NULL UNIQUE,
    password_hash   VARCHAR(255) NOT NULL,      -- Argon2id hash
    
    -- Profile
    first_name      VARCHAR(100),
    last_name       VARCHAR(100),
    display_name    VARCHAR(255),               -- Auto-generated or user-set
    
    -- Status
    status          VARCHAR(20) NOT NULL DEFAULT 'pending',
                    -- enum: pending, active, suspended, deactivated
    email_verified  BOOLEAN NOT NULL DEFAULT FALSE,
    email_verified_at TIMESTAMP WITH TIME ZONE,
    
    -- Security
    mfa_enabled     BOOLEAN NOT NULL DEFAULT FALSE,
    mfa_secret      VARCHAR(255),               -- Encrypted TOTP secret
    failed_login_attempts INTEGER NOT NULL DEFAULT 0,
    locked_until    TIMESTAMP WITH TIME ZONE,
    
    -- Session management
    password_changed_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    last_login_at   TIMESTAMP WITH TIME ZONE,
    last_login_ip   INET,
    
    -- Metadata
    created_at      TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at      TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    
    -- Constraints
    CONSTRAINT valid_user_status CHECK (status IN ('pending', 'active', 'suspended', 'deactivated'))
);

CREATE INDEX idx_users_email ON users(email);
CREATE INDEX idx_users_status ON users(status);
```

#### 3.1.2 Organization Membership

```sql
CREATE TABLE organization_members (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id          UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    user_id         UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    role            VARCHAR(20) NOT NULL DEFAULT 'member',
                    -- enum: owner, admin, member, viewer
    invited_by      UUID REFERENCES users(id),
    joined_at       TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at      TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    
    UNIQUE(org_id, user_id)
);

CREATE INDEX idx_org_members_org ON organization_members(org_id);
CREATE INDEX idx_org_members_user ON organization_members(user_id);
```

### 3.2 Password Security

#### 3.2.1 Hashing: Argon2id

```python
from argon2 import PasswordHasher
from argon2.low_level import Type

# Argon2id parameters (OWASP recommended minimum)
ph = PasswordHasher(
    time_cost=3,        # iterations (t)
    memory_cost=65536,  # 64 MB (m)
    parallelism=4,      # parallel threads (p)
    hash_len=32,        # output hash length
    salt_len=16,        # salt length
    type=Type.ID        # Argon2id (hybrid: resistant to GPU and side-channel)
)

def hash_password(password: str) -> str:
    """Hash a password for storage. Returns encoded hash string."""
    return ph.hash(password)

def verify_password(password: str, hash_string: str) -> bool:
    """Verify a password against a stored hash."""
    try:
        ph.verify(hash_string, password)
        return True
    except argon2.exceptions.VerifyMismatchError:
        return False

def needs_rehash(hash_string: str) -> bool:
    """Check if hash needs upgrading to newer parameters."""
    return ph.check_needs_rehash(hash_string)
```

#### 3.2.2 Password Requirements

| Requirement | Specification | Rationale |
|-------------|-------------|-----------|
| Minimum length | 12 characters | NIST SP 800-63B recommends minimum 8; 12 for stronger security |
| Maximum length | 128 characters | Prevent DoS via extremely long passwords |
| Uppercase required | At least 1 | Increases character space |
| Lowercase required | At least 1 | Increases character space |
| Digit required | At least 1 | Increases character space |
| Special character | At least 1 from `!@#$%^&*()-_=+[]{}|;:,.<>?` | Increases character space |
| Common password check | Reject top 10,000 common passwords | Prevent dictionary attacks |
| Credential stuffing check | Check against Have I Been Pwned API (optional) | Prevent reuse of leaked passwords |
| Unicode support | Yes (normalized per NFC) | International password support |

#### 3.2.3 Password Validation

```python
import re
import unicodedata

PASSWORD_MIN_LENGTH = 12
PASSWORD_MAX_LENGTH = 128
PASSWORD_PATTERN = re.compile(
    r'^(?=.*[a-z])'      # At least one lowercase
    r'(?=.*[A-Z])'       # At least one uppercase
    r'(?=.*\d)'          # At least one digit
    r'(?=.*[!@#$%^&*()\-_=+\[\]{}|;:,.<>?])'  # At least one special
    r'[\S]{' + str(PASSWORD_MIN_LENGTH) + r',' + str(PASSWORD_MAX_LENGTH) + r'}$'
)

# Load common passwords into a Bloom filter for O(1) lookups
COMMON_PASSWORDS_BLOOM = load_common_passwords_bloom_filter()

def validate_password(password: str) -> tuple[bool, list[str]]:
    """
    Validate password against policy.
    Returns (is_valid, list_of_errors).
    """
    errors = []
    
    # Normalize Unicode (NFC)
    password = unicodedata.normalize('NFC', password)
    
    # Check length
    if len(password) < PASSWORD_MIN_LENGTH:
        errors.append(f"Password must be at least {PASSWORD_MIN_LENGTH} characters")
    if len(password) > PASSWORD_MAX_LENGTH:
        errors.append(f"Password must be at most {PASSWORD_MAX_LENGTH} characters")
    
    # Check complexity
    if not re.search(r'[a-z]', password):
        errors.append("Password must contain at least one lowercase letter")
    if not re.search(r'[A-Z]', password):
        errors.append("Password must contain at least one uppercase letter")
    if not re.search(r'\d', password):
        errors.append("Password must contain at least one digit")
    if not re.search(r'[!@#$%^&*()\-_=+\[\]{}|;:,.<>?]', password):
        errors.append("Password must contain at least one special character")
    
    # Check against common passwords
    if password.lower() in COMMON_PASSWORDS_BLOOM:
        errors.append("Password is too common. Please choose a more unique password.")
    
    return len(errors) == 0, errors
```

### 3.3 Registration Flow

#### 3.3.1 Sequence Diagram

```
┌────────┐     ┌──────────┐     ┌──────────────┐     ┌─────────┐     ┌──────┐
│ Client │     │ Gateway  │     │ Auth Service │     │ Database│     │ Email│
└────┬───┘     └────┬─────┘     └──────┬───────┘     └────┬────┘     └──────┘
     │              │                  │                  │          │
     │ POST /register│                 │                  │          │
     │ {email, pwd,  │                 │                  │          │
     │  name, org}    │                 │                  │          │
     │─────────────>│                  │                  │          │
     │              │                  │                  │          │
     │              │ 1. Validate input│                  │          │
     │              │    - email format│                  │          │
     │              │    - password    │                  │          │
     │              │    - org name    │                  │          │
     │              │                  │                  │          │
     │              │ 2. Check email exists               │          │
     │              │─────────────────────────────────────>│          │
     │              │                  │                  │          │
     │              │ 3. Hash password (Argon2id)         │          │
     │              │                  │                  │          │
     │              │ 4. Create user + org (transaction)  │          │
     │              │─────────────────────────────────────>│          │
     │              │                  │                  │          │
     │              │ 5. Generate verification token      │          │
     │              │                  │                  │          │
     │              │ 6. Send verification email          │          │
     │              │─────────────────────────────────────────────────>│
     │              │                  │                  │          │
     │  201 Created │                  │                  │          │
     │  {user_id,   │                  │                  │          │
     │   status:    │                  │                  │          │
     │   pending}   │                  │                  │          │
     │<─────────────│                  │                  │          │
     │              │                  │                  │          │
     │ [Later: click email link]       │                  │          │
     │ GET /verify?token=...           │                  │          │
     │─────────────>│                  │                  │          │
     │              │ 7. Verify token  │                  │          │
     │              │ 8. Activate user │                  │          │
     │              │─────────────────────────────────────>│          │
     │              │                  │                  │          │
     │  302 Redirect│                  │                  │          │
     │  /login      │                  │                  │          │
     │<─────────────│                  │                  │          │
```

#### 3.3.2 Registration Endpoint

```python
from pydantic import BaseModel, EmailStr, validator

class RegistrationRequest(BaseModel):
    email: EmailStr
    password: str
    first_name: str = Field(..., min_length=1, max_length=100)
    last_name: str = Field(..., min_length=1, max_length=100)
    organization_name: str = Field(..., min_length=1, max_length=255)
    
    @validator('password')
    def validate_password_strength(cls, v):
        is_valid, errors = validate_password(v)
        if not is_valid:
            raise ValueError(f"Password does not meet requirements: {'; '.join(errors)}")
        return v

async def register(request: RegistrationRequest) -> RegistrationResponse:
    """
    Register a new user and create their organization.
    User becomes the organization owner.
    """
    # 1. Check if email already exists
    existing = await db.fetchval(
        "SELECT id FROM users WHERE email = $1", request.email
    )
    if existing:
        # Return same response to prevent email enumeration
        raise AuthError("EMAIL_EXISTS", 
            "If this email is not registered, you will receive a verification email.")
    
    # 2. Hash password
    password_hash = hash_password(request.password)
    
    async with db.transaction():
        # 3. Create user
        user_id = await db.fetchval(
            """INSERT INTO users 
                (email, password_hash, first_name, last_name, display_name, status)
               VALUES ($1, $2, $3, $4, $5, 'pending')
               RETURNING id""",
            request.email.lower().strip(),
            password_hash,
            request.first_name,
            request.last_name,
            f"{request.first_name} {request.last_name}"
        )
        
        # 4. Create organization
        org_id = await db.fetchval(
            """INSERT INTO organizations 
                (name, slug, owner_id, status)
               VALUES ($1, $2, $3, 'active')
               RETURNING id""",
            request.organization_name,
            generate_org_slug(request.organization_name),
            user_id
        )
        
        # 5. Add user as owner
        await db.execute(
            """INSERT INTO organization_members (org_id, user_id, role)
               VALUES ($1, $2, 'owner')""",
            org_id, user_id
        )
        
        # 6. Create default org settings
        await db.execute(
            """INSERT INTO organization_settings (org_id)
               VALUES ($1)""",
            org_id
        )
    
    # 7. Generate and send verification email (async)
    verification_token = generate_secure_token(32)
    await redis.setex(f"verify:{verification_token}", 86400, str(user_id))  # 24h expiry
    await send_verification_email(request.email, verification_token)
    
    return RegistrationResponse(
        user_id=user_id,
        status="pending",
        message="Registration successful. Please check your email to verify your account."
    )
```

#### 3.3.3 Email Verification Token

```python
def generate_secure_token(length: int = 32) -> str:
    """Generate a cryptographically secure random token."""
    return secrets.token_urlsafe(length)

async def verify_email(token: str):
    """Verify user email address."""
    user_id = await redis.get(f"verify:{token}")
    if not user_id:
        raise AuthError("INVALID_TOKEN", "Verification token is invalid or expired")
    
    await db.execute(
        """UPDATE users 
           SET email_verified = TRUE, 
               email_verified_at = NOW(),
               status = 'active'
           WHERE id = $1""",
        UUID(user_id)
    )
    
    await redis.delete(f"verify:{token}")
    return {"status": "verified"}
```

### 3.4 Login Flow

#### 3.4.1 Login Endpoint

```python
class LoginRequest(BaseModel):
    email: EmailStr
    password: str
    mfa_code: Optional[str] = None  # If MFA enabled

class LoginResponse(BaseModel):
    access_token: str      # JWT (short-lived)
    token_type: str        # "bearer"
    expires_in: int        # seconds
    refresh_token: str     # Long-lived token for renewal

async def login(request: LoginRequest, client_info: ClientInfo) -> LoginResponse:
    """
    Authenticate user and issue JWT tokens.
    Implements rate limiting and account locking.
    """
    # 1. Rate limit by email
    rate_key = f"login_attempts:{request.email.lower()}"
    attempts = await redis.incr(rate_key)
    if attempts == 1:
        await redis.expire(rate_key, 3600)  # 1-hour window
    
    if attempts > 10:
        raise AuthError("TOO_MANY_ATTEMPTS", 
            "Too many login attempts. Please try again later.",
            status_code=429,
            retry_after=await redis.ttl(rate_key)
        )
    
    # 2. Find user
    user = await db.fetchrow(
        "SELECT * FROM users WHERE email = $1", request.email.lower().strip()
    )
    
    # 3. Constant-time verification (prevent timing attacks)
    if not user:
        # Perform dummy hash to maintain constant time
        ph.hash("dummy_password_for_timing")
        raise AuthError("INVALID_CREDENTIALS", "Invalid email or password")
    
    # 4. Check account status
    if user["status"] == "suspended":
        raise AuthError("ACCOUNT_SUSPENDED", "Account has been suspended")
    if user["status"] == "deactivated":
        raise AuthError("ACCOUNT_DEACTIVATED", "Account has been deactivated")
    if user["status"] == "pending":
        raise AuthError("EMAIL_NOT_VERIFIED", "Please verify your email before logging in")
    
    # 5. Check account lock
    if user["locked_until"] and datetime.now(timezone.utc) < user["locked_until"]:
        raise AuthError("ACCOUNT_LOCKED", 
            "Account is temporarily locked due to too many failed attempts",
            status_code=423,
            locked_until=user["locked_until"].isoformat()
        )
    
    # 6. Verify password
    if not verify_password(request.password, user["password_hash"]):
        # Increment failed attempts
        new_failed = user["failed_login_attempts"] + 1
        lock_until = None
        
        if new_failed >= 5:
            # Lock for 30 minutes after 5 failures
            lock_until = datetime.now(timezone.utc) + timedelta(minutes=30)
        
        await db.execute(
            """UPDATE users 
               SET failed_login_attempts = $1, locked_until = $2
               WHERE id = $3""",
            new_failed, lock_until, user["id"]
        )
        
        raise AuthError("INVALID_CREDENTIALS", "Invalid email or password")
    
    # 7. Check MFA if enabled
    if user["mfa_enabled"]:
        if not request.mfa_code:
            return {"mfa_required": True, "mfa_methods": ["totp"]}
        
        if not verify_totp(user["mfa_secret"], request.mfa_code):
            raise AuthError("INVALID_MFA_CODE", "Invalid MFA code")
    
    # 8. Reset failed attempts and update last login
    await db.execute(
        """UPDATE users 
           SET failed_login_attempts = 0,
               locked_until = NULL,
               last_login_at = NOW(),
               last_login_ip = $1,
               password_changed_at = COALESCE(password_changed_at, NOW())
           WHERE id = $2""",
        client_info.ip_address, user["id"]
    )
    
    # 9. Check if password rehash needed
    if needs_rehash(user["password_hash"]):
        new_hash = hash_password(request.password)
        await db.execute(
            "UPDATE users SET password_hash = $1 WHERE id = $2",
            new_hash, user["id"]
        )
    
    # 10. Generate tokens
    tokens = await create_session(user["id"], client_info)
    
    # 11. Audit log
    await audit_log.record(
        action="login",
        user_id=user["id"],
        ip_address=client_info.ip_address,
        user_agent=client_info.user_agent
    )
    
    return tokens
```

### 3.5 JWT Design

#### 3.5.1 Token Structure

**Signing Algorithm:** RS256 (RSA + SHA-256)

Rationale: RS256 allows the auth service to sign tokens with a private key while any service can verify them with the public key. This enables distributed verification without sharing secrets.

Alternative: HS256 is acceptable for single-instance deployments but requires shared secret management in distributed systems.

#### 3.5.2 Key Pair Management

```python
# RSA key pair generation (one-time setup)
from cryptography.hazmat.primitives import serialization
from cryptography.hazmat.primitives.asymmetric import rsa

def generate_jwt_keypair():
    """Generate RSA key pair for JWT signing."""
    private_key = rsa.generate_private_key(
        public_exponent=65537,
        key_size=2048  # Minimum recommended; 4096 for higher security
    )
    
    private_pem = private_key.private_bytes(
        encoding=serialization.Encoding.PEM,
        format=serialization.PrivateFormat.PKCS8,
        encryption_algorithm=serialization.NoEncryption()
    )
    
    public_pem = private_key.public_key().public_bytes(
        encoding=serialization.Encoding.PEM,
        format=serialization.PublicFormat.SubjectPublicKeyInfo
    )
    
    return private_pem, public_pem

# Store private key securely (e.g., environment variable or secret manager)
# Store public key in all services that need to verify JWTs
```

#### 3.5.3 JWT Claims

**Access Token:**

| Claim | Type | Description |
|-------|------|-------------|
| `sub` | UUID (string) | User ID |
| `org_id` | UUID (string) | Current active organization ID |
| `role` | string | User's role in the organization (`owner`, `admin`, `member`, `viewer`) |
| `email` | string | User's email address |
| `iat` | integer | Issued at (Unix timestamp) |
| `exp` | integer | Expiration (Unix timestamp) — 15 minutes from iat |
| `jti` | UUID (string) | Unique token ID for revocation tracking |
| `type` | string | Always `"access"` |

**Refresh Token:**

| Claim | Type | Description |
|-------|------|-------------|
| `sub` | UUID (string) | User ID |
| `jti` | UUID (string) | Unique token ID |
| `iat` | integer | Issued at |
| `exp` | integer | Expiration — 7 days from iat |
| `type` | string | Always `"refresh"` |
| `session_id` | UUID (string) | Session identifier |

#### 3.5.4 Token Generation

```python
import jwt
from datetime import datetime, timezone, timedelta
import uuid

# Configuration
ACCESS_TOKEN_LIFETIME = 900      # 15 minutes
REFRESH_TOKEN_LIFETIME = 604800  # 7 days
JWT_ALGORITHM = "RS256"

# Load keys from environment/secret manager
JWT_PRIVATE_KEY = os.environ["JWT_PRIVATE_KEY"]
JWT_PUBLIC_KEY = os.environ["JWT_PUBLIC_KEY"]

async def create_session(user_id: UUID, client_info: ClientInfo) -> LoginResponse:
    """Create access and refresh tokens for a user session."""
    
    # Get user's primary organization and role
    membership = await db.fetchrow(
        """SELECT om.org_id, om.role, o.name as org_name
           FROM organization_members om
           JOIN organizations o ON om.org_id = o.id
           WHERE om.user_id = $1
           ORDER BY om.joined_at ASC
           LIMIT 1""",
        user_id
    )
    
    if not membership:
        raise AuthError("NO_ORGANIZATION", "User is not a member of any organization")
    
    # Get user details
    user = await db.fetchrow(
        "SELECT email FROM users WHERE id = $1", user_id
    )
    
    now = datetime.now(timezone.utc)
    
    # Create access token
    access_jti = str(uuid.uuid4())
    access_payload = {
        "sub": str(user_id),
        "org_id": str(membership["org_id"]),
        "role": membership["role"],
        "email": user["email"],
        "iat": int(now.timestamp()),
        "exp": int((now + timedelta(seconds=ACCESS_TOKEN_LIFETIME)).timestamp()),
        "jti": access_jti,
        "type": "access"
    }
    
    access_token = jwt.encode(
        access_payload, JWT_PRIVATE_KEY, algorithm=JWT_ALGORITHM
    )
    
    # Create refresh token
    refresh_jti = str(uuid.uuid4())
    session_id = str(uuid.uuid4())
    refresh_payload = {
        "sub": str(user_id),
        "jti": refresh_jti,
        "session_id": session_id,
        "iat": int(now.timestamp()),
        "exp": int((now + timedelta(seconds=REFRESH_TOKEN_LIFETIME)).timestamp()),
        "type": "refresh"
    }
    
    refresh_token = jwt.encode(
        refresh_payload, JWT_PRIVATE_KEY, algorithm=JWT_ALGORITHM
    )
    
    # Store refresh token hash in database (for revocation)
    refresh_hash = hashlib.sha256(refresh_token.encode()).hexdigest()
    await db.execute(
        """INSERT INTO refresh_tokens 
            (id, user_id, token_hash, expires_at, ip_address, user_agent)
           VALUES ($1, $2, $3, $4, $5, $6)""",
        session_id, user_id, refresh_hash,
        now + timedelta(seconds=REFRESH_TOKEN_LIFETIME),
        client_info.ip_address, client_info.user_agent
    )
    
    # Add access token JTI to revocation list with TTL = access token lifetime
    # This enables immediate logout without waiting for token expiry
    await redis.setex(f"revoked:{access_jti}", ACCESS_TOKEN_LIFETIME, "1")
    
    return LoginResponse(
        access_token=access_token,
        token_type="bearer",
        expires_in=ACCESS_TOKEN_LIFETIME,
        refresh_token=refresh_token
    )
```

#### 3.5.5 Token Verification

```python
async def verify_access_token(token: str) -> TokenPayload:
    """
    Verify and decode an access token.
    Used on every authenticated dashboard request.
    """
    try:
        # 1. Decode and verify signature/expiry
        payload = jwt.decode(
            token,
            JWT_PUBLIC_KEY,
            algorithms=[JWT_ALGORITHM],
            options={"require": ["sub", "exp", "iat", "jti", "type"]}
        )
    except jwt.ExpiredSignatureError:
        raise AuthError("TOKEN_EXPIRED", "Access token has expired")
    except jwt.InvalidTokenError:
        raise AuthError("INVALID_TOKEN", "Token is invalid")
    
    # 2. Check token type
    if payload.get("type") != "access":
        raise AuthError("INVALID_TOKEN_TYPE", "Expected access token")
    
    # 3. Check revocation list
    jti = payload["jti"]
    if await redis.exists(f"revoked:{jti}"):
        # Token has been revoked (user logged out)
        raise AuthError("TOKEN_REVOKED", "Token has been revoked")
    
    # 4. Check user status
    user_id = UUID(payload["sub"])
    user_status = await redis.get(f"user:{user_id}:status")
    if not user_status:
        user = await db.fetchrow(
            "SELECT status FROM users WHERE id = $1", user_id
        )
        if not user:
            raise AuthError("USER_NOT_FOUND", "User no longer exists")
        user_status = user["status"]
        await redis.setex(f"user:{user_id}:status", 300, user_status)
    
    if user_status == "suspended":
        raise AuthError("ACCOUNT_SUSPENDED", "Account has been suspended")
    if user_status == "deactivated":
        raise AuthError("ACCOUNT_DEACTIVATED", "Account has been deactivated")
    
    # 5. Verify organization membership
    org_id = UUID(payload["org_id"])
    role = payload["role"]
    
    membership_valid = await redis.get(f"membership:{user_id}:{org_id}")
    if not membership_valid:
        member = await db.fetchrow(
            """SELECT role FROM organization_members 
               WHERE user_id = $1 AND org_id = $2""",
            user_id, org_id
        )
        if not member:
            raise AuthError("ORG_ACCESS_DENIED", "User is not a member of this organization")
        await redis.setex(f"membership:{user_id}:{org_id}", 300, member["role"])
        role = member["role"]
    
    return TokenPayload(
        user_id=user_id,
        org_id=org_id,
        role=role,
        email=payload["email"],
        jti=jti
    )
```

### 3.6 Token Refresh

```python
async def refresh_access_token(refresh_token: str) -> LoginResponse:
    """
    Exchange a valid refresh token for a new access token pair.
    """
    try:
        payload = jwt.decode(
            refresh_token,
            JWT_PUBLIC_KEY,
            algorithms=[JWT_ALGORITHM],
            options={"require": ["sub", "jti", "type", "session_id"]}
        )
    except jwt.ExpiredSignatureError:
        raise AuthError("REFRESH_TOKEN_EXPIRED", "Refresh token has expired. Please log in again.")
    except jwt.InvalidTokenError:
        raise AuthError("INVALID_REFRESH_TOKEN", "Refresh token is invalid")
    
    if payload["type"] != "refresh":
        raise AuthError("INVALID_TOKEN_TYPE", "Expected refresh token")
    
    # Verify refresh token in database
    session_id = UUID(payload["session_id"])
    token_hash = hashlib.sha256(refresh_token.encode()).hexdigest()
    
    session = await db.fetchrow(
        """SELECT rt.*, u.status as user_status
           FROM refresh_tokens rt
           JOIN users u ON rt.user_id = u.id
           WHERE rt.id = $1 AND rt.token_hash = $2 AND rt.expires_at > NOW()
           AND rt.revoked_at IS NULL""",
        session_id, token_hash
    )
    
    if not session:
        raise AuthError("INVALID_REFRESH_TOKEN", "Refresh token is invalid or revoked")
    
    if session["user_status"] != "active":
        raise AuthError("ACCOUNT_INACTIVE", "Account is not active")
    
    # Rotate refresh token (security best practice)
    # Revoke old refresh token and issue new one
    await db.execute(
        "UPDATE refresh_tokens SET revoked_at = NOW() WHERE id = $1",
        session_id
    )
    
    # Create new session
    client_info = ClientInfo(
        ip_address=session["ip_address"],
        user_agent=session["user_agent"]
    )
    return await create_session(UUID(payload["sub"]), client_info)
```

### 3.7 Token Transport

#### 3.7.1 httpOnly Cookie (Recommended)

```http
Set-Cookie: session=<access_token>; HttpOnly; Secure; SameSite=Strict; Path=/; Max-Age=900
Set-Cookie: refresh=<refresh_token>; HttpOnly; Secure; SameSite=Strict; Path=/api/auth/refresh; Max-Age=604800
```

| Attribute | Value | Rationale |
|-----------|-------|-----------|
| `HttpOnly` | true | Prevents JavaScript access (XSS protection) |
| `Secure` | true | Only sent over HTTPS |
| `SameSite=Strict` | true | Prevents CSRF attacks |
| `Path` | `/` (access), `/api/auth/refresh` (refresh) | Scope tokens appropriately |
| `Max-Age` | 900 (access), 604800 (refresh) | Match token lifetimes |

#### 3.7.2 Why Not localStorage?

| Concern | localStorage | httpOnly Cookie |
|---------|-------------|-----------------|
| XSS vulnerability | Vulnerable — JS can read token | Protected — JS cannot access |
| CSRF vulnerability | Not applicable | Mitigated via `SameSite=Strict` + CSRF tokens |
| Token transmission | Manual (JS sets header) | Automatic (browser handles) |
| XSS vs CSRF tradeoff | XSS is more dangerous for auth tokens | Preferred: XSS protection > CSRF risk |

**Recommendation:** Use httpOnly cookies for session auth. The XSS risk of localStorage outweighs the CSRF risk of cookies, especially with `SameSite=Strict`.

### 3.8 Logout

```python
async def logout(access_token: str, refresh_token: Optional[str] = None):
    """
    Logout: revoke tokens and clear session.
    """
    try:
        payload = jwt.decode(access_token, JWT_PUBLIC_KEY, algorithms=[JWT_ALGORITHM])
        jti = payload.get("jti")
        user_id = payload.get("sub")
        
        if jti:
            # Revoke access token (add to revocation list)
            ttl = max(0, payload["exp"] - int(datetime.now(timezone.utc).timestamp()))
            await redis.setex(f"revoked:{jti}", ttl, "1")
        
        if refresh_token:
            # Revoke refresh token in database
            try:
                refresh_payload = jwt.decode(
                    refresh_token, JWT_PUBLIC_KEY, algorithms=[JWT_ALGORITHM]
                )
                session_id = refresh_payload.get("session_id")
                if session_id:
                    await db.execute(
                        "UPDATE refresh_tokens SET revoked_at = NOW() WHERE id = $1",
                        UUID(session_id)
                    )
            except jwt.InvalidTokenError:
                pass  # Refresh token invalid, still clear cookie
        
        # Clear all cookies
        response.delete_cookie("session")
        response.delete_cookie("refresh")
        
        # Audit log
        if user_id:
            await audit_log.record(
                action="logout",
                user_id=UUID(user_id)
            )
        
    except jwt.InvalidTokenError:
        # Even if token is invalid, clear cookies
        response.delete_cookie("session")
        response.delete_cookie("refresh")
```

### 3.9 Password Reset

```python
async def request_password_reset(email: str):
    """Request a password reset email."""
    # Always return success to prevent email enumeration
    
    user = await db.fetchrow(
        "SELECT id FROM users WHERE email = $1 AND status = 'active'",
        email.lower().strip()
    )
    
    if user:
        # Generate reset token
        token = generate_secure_token(32)
        await redis.setex(
            f"pwdreset:{token}", 3600,  # 1 hour expiry
            str(user["id"])
        )
        
        await send_password_reset_email(email, token)
    
    # Same response regardless of whether user exists
    return {"message": "If an account with that email exists, you will receive a password reset link."}

async def reset_password(token: str, new_password: str):
    """Reset password using token from email."""
    user_id = await redis.get(f"pwdreset:{token}")
    if not user_id:
        raise AuthError("INVALID_TOKEN", "Password reset token is invalid or expired")
    
    # Validate new password
    is_valid, errors = validate_password(new_password)
    if not is_valid:
        raise AuthError("INVALID_PASSWORD", f"Password requirements: {'; '.join(errors)}")
    
    # Hash and update
    password_hash = hash_password(new_password)
    
    await db.execute(
        """UPDATE users 
           SET password_hash = $1,
               password_changed_at = NOW(),
               failed_login_attempts = 0,
               locked_until = NULL
           WHERE id = $2""",
        password_hash, UUID(user_id)
    )
    
    # Invalidate token
    await redis.delete(f"pwdreset:{token}")
    
    # Revoke all sessions (force re-login)
    await revoke_all_user_sessions(UUID(user_id))
    
    await audit_log.record(
        action="password_reset",
        user_id=UUID(user_id)
    )
    
    return {"status": "success", "message": "Password has been reset. Please log in with your new password."}
```

### 3.10 Session Management

#### 3.10.1 Active Sessions

```sql
CREATE TABLE refresh_tokens (
    id              UUID PRIMARY KEY,
    user_id         UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    token_hash      VARCHAR(64) NOT NULL,        -- SHA-256 of refresh token
    expires_at      TIMESTAMP WITH TIME ZONE NOT NULL,
    revoked_at      TIMESTAMP WITH TIME ZONE,
    ip_address      INET,
    user_agent      VARCHAR(512),
    created_at      TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

CREATE INDEX idx_refresh_tokens_user ON refresh_tokens(user_id);
CREATE INDEX idx_refresh_tokens_active ON refresh_tokens(user_id) WHERE revoked_at IS NULL;
```

#### 3.10.2 "Logout Everywhere"

```python
async def revoke_all_user_sessions(user_id: UUID, except_session_id: Optional[UUID] = None):
    """
    Revoke all refresh tokens for a user.
    Used on password change, security breach, or admin action.
    """
    # 1. Revoke all refresh tokens in database
    if except_session_id:
        await db.execute(
            """UPDATE refresh_tokens 
               SET revoked_at = NOW() 
               WHERE user_id = $1 AND id != $2 AND revoked_at IS NULL""",
            user_id, except_session_id
        )
    else:
        await db.execute(
            """UPDATE refresh_tokens 
               SET revoked_at = NOW() 
               WHERE user_id = $1 AND revoked_at IS NULL""",
            user_id
        )
    
    # 2. Increment session version to invalidate all access tokens
    await redis.incr(f"user:{user_id}:session_version")
    
    # 3. Set TTL on session version (new tokens must check this)
    await redis.expire(f"user:{user_id}:session_version", 86400 * 7)  # 7 days
```

---

## 4. Authorization (RBAC)

### 4.1 Role Definitions

| Role | Description | Use Case |
|------|-------------|----------|
| **owner** | Full access to the organization. Can delete org, manage billing, manage all users. | Organization founder |
| **admin** | Can manage API keys, invite users, manage settings. Cannot delete org or manage billing. | Team lead |
| **member** | Can view usage, create API keys (with limits), view configs. Cannot manage users or billing. | Developer |
| **viewer** | Read-only access. Can view dashboards and usage. Cannot modify anything. | Stakeholder, auditor |

### 4.2 Permission Matrix

| Action | Resource | Owner | Admin | Member | Viewer |
|--------|----------|:-----:|:-----:|:------:|:------:|
| **Organization** |||||
| View org details | organization | Y | Y | Y | Y |
| Edit org settings | organization | Y | Y | N | N |
| Delete organization | organization | Y | N | N | N |
| Manage billing | organization | Y | N | N | N |
| View invoices | organization | Y | Y | N | Y |
| **Users & Members** |||||
| View members | organization_members | Y | Y | Y | Y |
| Invite member | organization_members | Y | Y | N | N |
| Remove member | organization_members | Y | Y | N | N |
| Change member role | organization_members | Y | Y | N | N |
| Remove owner | organization_members | N | N | N | N |
| **API Keys** |||||
| View own keys | api_keys | Y | Y | Y | Y |
| View all org keys | api_keys | Y | Y | Y | Y |
| Create API key | api_keys | Y | Y | Y* | N |
| Revoke any API key | api_keys | Y | Y | N** | N |
| Rotate any API key | api_keys | Y | Y | N** | N |
| Set key budgets | api_keys | Y | Y | N | N |
| Set key IP allowlist | api_keys | Y | Y | N | N |
| Set key model restrictions | api_keys | Y | Y | Y* | N |
| **Models** |||||
| View available models | models | Y | Y | Y | Y |
| Configure model access | models | Y | Y | N | N |
| Set org-wide model allowlist | models | Y | Y | N | N |
| **Usage & Analytics** |||||
| View usage dashboard | usage | Y | Y | Y | Y |
| View cost breakdown | usage | Y | Y | Y | Y |
| View request logs | logs | Y | Y | Y*** | N |
| Export usage data | usage | Y | Y | Y | N |
| **Settings** |||||
| View org settings | settings | Y | Y | Y | Y |
| Edit rate limits | settings | Y | Y | N | N |
| Edit webhook URLs | settings | Y | Y | N | N |
| Configure SSO | settings | Y | N | N | N |
| View audit log | audit_log | Y | Y | N | Y |
| **Security** |||||
| View security settings | security | Y | Y | Y | Y |
| Enable MFA | security | Y | Y | Y | Y |
| Force password reset | security | Y | Y | N | N |
| Revoke all sessions | security | Y | Y (own) | Y (own) | Y (own) |

**Notes:**
- `*` Member can create keys with default settings only (no custom rate limits, budgets, or IP allowlists)
- `**` Member can only revoke/rotate keys they created
- `***` Member can view request logs but with PII redacted (no prompt/response content)

### 4.3 Permission System Implementation

#### 4.3.1 Permission Enum

```python
from enum import Enum, auto

class Permission(str, Enum):
    """All permissions in the system."""
    
    # Organization
    ORG_READ = "org:read"
    ORG_UPDATE = "org:update"
    ORG_DELETE = "org:delete"
    BILLING_MANAGE = "billing:manage"
    BILLING_READ = "billing:read"
    
    # Members
    MEMBER_READ = "member:read"
    MEMBER_INVITE = "member:invite"
    MEMBER_REMOVE = "member:remove"
    MEMBER_ROLE_UPDATE = "member:role:update"
    
    # API Keys
    APIKEY_READ = "apikey:read"
    APIKEY_CREATE = "apikey:create"
    APIKEY_REVOKE = "apikey:revoke"
    APIKEY_ROTATE = "apikey:rotate"
    APIKEY_BUDGET_SET = "apikey:budget:set"
    APIKEY_SCOPE_SET = "apikey:scope:set"
    
    # Models
    MODEL_READ = "model:read"
    MODEL_CONFIGURE = "model:configure"
    
    # Usage
    USAGE_READ = "usage:read"
    USAGE_EXPORT = "usage:export"
    LOGS_READ = "logs:read"
    
    # Settings
    SETTINGS_READ = "settings:read"
    SETTINGS_UPDATE = "settings:update"
    SSO_CONFIGURE = "sso:configure"
    AUDIT_READ = "audit:read"
    
    # Security
    SECURITY_READ = "security:read"
    MFA_MANAGE = "mfa:manage"
    SESSION_REVOKE_ALL = "session:revoke:all"
```

#### 4.3.2 Role-to-Permission Mapping

```python
ROLE_PERMISSIONS = {
    "owner": [
        # All permissions
        Permission.ORG_READ, Permission.ORG_UPDATE, Permission.ORG_DELETE,
        Permission.BILLING_MANAGE, Permission.BILLING_READ,
        Permission.MEMBER_READ, Permission.MEMBER_INVITE, 
        Permission.MEMBER_REMOVE, Permission.MEMBER_ROLE_UPDATE,
        Permission.APIKEY_READ, Permission.APIKEY_CREATE,
        Permission.APIKEY_REVOKE, Permission.APIKEY_ROTATE,
        Permission.APIKEY_BUDGET_SET, Permission.APIKEY_SCOPE_SET,
        Permission.MODEL_READ, Permission.MODEL_CONFIGURE,
        Permission.USAGE_READ, Permission.USAGE_EXPORT,
        Permission.LOGS_READ,
        Permission.SETTINGS_READ, Permission.SETTINGS_UPDATE,
        Permission.SSO_CONFIGURE, Permission.AUDIT_READ,
        Permission.SECURITY_READ, Permission.MFA_MANAGE,
        Permission.SESSION_REVOKE_ALL,
    ],
    "admin": [
        Permission.ORG_READ, Permission.ORG_UPDATE,
        Permission.BILLING_READ,
        Permission.MEMBER_READ, Permission.MEMBER_INVITE, Permission.MEMBER_REMOVE,
        Permission.APIKEY_READ, Permission.APIKEY_CREATE,
        Permission.APIKEY_REVOKE, Permission.APIKEY_ROTATE,
        Permission.APIKEY_SCOPE_SET,
        Permission.MODEL_READ, Permission.MODEL_CONFIGURE,
        Permission.USAGE_READ, Permission.USAGE_EXPORT, Permission.LOGS_READ,
        Permission.SETTINGS_READ, Permission.SETTINGS_UPDATE,
        Permission.AUDIT_READ,
        Permission.SECURITY_READ, Permission.MFA_MANAGE,
        Permission.SESSION_REVOKE_ALL,
    ],
    "member": [
        Permission.ORG_READ,
        Permission.MEMBER_READ,
        Permission.APIKEY_READ, Permission.APIKEY_CREATE,
        Permission.MODEL_READ,
        Permission.USAGE_READ,
        Permission.SETTINGS_READ,
        Permission.SECURITY_READ, Permission.MFA_MANAGE,
        Permission.SESSION_REVOKE_ALL,
    ],
    "viewer": [
        Permission.ORG_READ,
        Permission.MEMBER_READ,
        Permission.APIKEY_READ,
        Permission.MODEL_READ,
        Permission.USAGE_READ,
        Permission.BILLING_READ,
        Permission.SETTINGS_READ,
        Permission.AUDIT_READ,
        Permission.SECURITY_READ,
        Permission.SESSION_REVOKE_ALL,
    ],
}

def has_permission(role: str, permission: Permission) -> bool:
    """Check if a role has a specific permission."""
    return permission in ROLE_PERMISSIONS.get(role, [])
```

#### 4.3.3 Middleware / Decorator

```python
from functools import wraps
from fastapi import Request, HTTPException

class RequirePermission:
    """Permission check decorator/middleware."""
    
    def __init__(self, permission: Permission):
        self.permission = permission
    
    async def __call__(self, request: Request):
        # Auth context is set by auth middleware
        auth_context = request.state.auth_context
        
        if not auth_context:
            raise HTTPException(status_code=401, detail="Authentication required")
        
        role = auth_context.role
        
        if not has_permission(role, self.permission):
            raise HTTPException(
                status_code=403,
                detail={
                    "error": "INSUFFICIENT_PERMISSIONS",
                    "message": f"Role '{role}' does not have permission '{self.permission.value}'",
                    "required_permission": self.permission.value,
                    "current_role": role
                }
            )
        
        return auth_context

# Usage in route handlers:
@app.get("/api/keys")
async def list_keys(
    auth: AuthContext = Depends(RequirePermission(Permission.APIKEY_READ))
):
    # Only users with apikey:read permission reach here
    return await get_org_keys(auth.org_id)
```

#### 4.3.4 Ownership Check for API Keys

```python
async def can_revoke_key(user_id: UUID, user_role: str, key_id: UUID) -> bool:
    """
    Check if user can revoke a specific API key.
    Owner/Admin can revoke any key in org.
    Member can only revoke keys they created.
    """
    if has_permission(user_role, Permission.APIKEY_REVOKE):
        return True
    
    # Member: check if they created this key
    key = await db.fetchrow(
        "SELECT created_by FROM api_keys WHERE id = $1", key_id
    )
    if not key:
        return False
    
    return key["created_by"] == user_id
```

### 4.4 Organization Isolation

#### 4.4.1 Isolation Rules

1. **Every database query must include `org_id` filter** — No global queries without org_id
2. **Users can only access data for their current active organization** — Switching orgs requires new token
3. **Cross-organization access is forbidden** — Except for superadmin
4. **API keys are scoped to exactly one organization** — No shared keys across orgs

#### 4.4.2 org_id Validation

```python
def require_org_access(auth_context: AuthContext, requested_org_id: UUID):
    """
    Verify the user's current organization matches the requested org.
    Prevents cross-organization access via parameter tampering.
    """
    if auth_context.org_id != requested_org_id:
        # Log potential tenant escape attempt
        logger.warning(
            "cross_org_access_attempt",
            user_id=str(auth_context.user_id),
            user_org=str(auth_context.org_id),
            requested_org=str(requested_org_id)
        )
        raise HTTPException(
            status_code=403,
            detail={
                "error": "ORG_ACCESS_DENIED",
                "message": "You do not have access to this organization"
            }
        )

# All service-layer functions must accept and filter by org_id:
async def get_api_keys(org_id: UUID) -> list[APIKey]:
    """Get all API keys for an organization."""
    return await db.fetch(
        "SELECT * FROM api_keys WHERE org_id = $1",  # org_id filter is mandatory
        org_id
    )
```

#### 4.4.3 Multi-Organization Users

Users can belong to multiple organizations. The active organization is determined by:

1. **JWT claim `org_id`** — Set at login time (primary org)
2. **Organization switch endpoint** — Issues new access token with different `org_id`
3. **Role is per-organization** — Same user can be `admin` in one org and `member` in another

```python
async def switch_organization(user_id: UUID, new_org_id: UUID, refresh_token: str) -> LoginResponse:
    """
    Switch active organization.
    Validates membership and issues new tokens.
    """
    # Verify user is member of target org
    membership = await db.fetchrow(
        """SELECT role FROM organization_members 
           WHERE user_id = $1 AND org_id = $2""",
        user_id, new_org_id
    )
    
    if not membership:
        raise AuthError("ORG_ACCESS_DENIED", "You are not a member of this organization")
    
    # Issue new tokens with new org_id and role
    client_info = ClientInfo(ip_address=None, user_agent=None)  # From request
    return await create_session(user_id, client_info, force_org_id=new_org_id)
```

---

## 5. Tenant Isolation

### 5.1 Isolation Architecture

Tenant isolation ensures that data from one organization is never accessible to another. Isolation is enforced at multiple layers:

```
┌─────────────────────────────────────────────────────────────┐
│                    Tenant Isolation Layers                    │
├─────────────────────────────────────────────────────────────┤
│ Layer 1: Authentication    │ org_id embedded in auth context │
├────────────────────────────┼─────────────────────────────────┤
│ Layer 2: API Gateway       │ Route validation by org_id      │
├────────────────────────────┼─────────────────────────────────┤
│ Layer 3: Application       │ Service-layer org_id filtering  │
├────────────────────────────┼─────────────────────────────────┤
│ Layer 4: Database          │ WHERE org_id = $1 on every query│
├────────────────────────────┼─────────────────────────────────┤
│ Layer 5: Cache             │ Key prefixing by org_id         │
├────────────────────────────┼─────────────────────────────────┤
│ Layer 6: Logs              │ org_id on every log entry       │
└────────────────────────────┴─────────────────────────────────┘
```

### 5.2 Tenant Context Flow

```python
class TenantContext:
    """Immutable tenant context attached to every request."""
    org_id: UUID
    org_slug: str
    user_id: Optional[UUID]     # Set for session auth
    key_id: Optional[UUID]      # Set for API key auth
    auth_type: str              # 'api_key' or 'session'
    role: Optional[str]         # Set for session auth
    environment: Optional[str]  # 'live' or 'test' for API keys

class RequestContext:
    """Request-scoped context carrying tenant and auth information."""
    tenant: TenantContext
    request_id: UUID
    timestamp: datetime
    
    @property
    def org_id(self) -> UUID:
        """Convenience accessor — every request must have org context."""
        return self.tenant.org_id
```

### 5.3 Database-Level Isolation

#### 5.3.1 Mandatory org_id Filter

**Rule:** Every SELECT, UPDATE, DELETE query on tenant-scoped tables MUST include `WHERE org_id = $org_id`.

**Enforcement:** Use query builders or repository patterns that automatically inject org_id.

```python
class TenantRepository:
    """Base repository that enforces tenant isolation."""
    
    def __init__(self, tenant_context: TenantContext):
        self.org_id = tenant_context.org_id
    
    async def query(self, sql: str, *args) -> list[Record]:
        """
        Execute a query with automatic org_id injection.
        SQL must contain {org_filter} placeholder.
        """
        org_filter = f"org_id = '{self.org_id}'"
        final_sql = sql.format(org_filter=org_filter)
        return await db.fetch(final_sql, *args)
    
    async def get(self, table: str, id: UUID) -> Optional[Record]:
        """Get a record by ID, scoped to current org."""
        return await db.fetchrow(
            f"SELECT * FROM {table} WHERE id = $1 AND org_id = $2",
            id, self.org_id
        )

# Usage:
repo = TenantRepository(request.state.tenant)
api_keys = await repo.query(
    "SELECT * FROM api_keys WHERE {org_filter} ORDER BY created_at DESC"
)
```

#### 5.3.2 Tables with org_id

All of the following tables have `org_id` column with foreign key constraint:

```sql
-- Core tenant tables
api_keys, organization_members, organization_settings,
-- Usage and billing tables
requests_log, usage_metrics, invoices, billing_events,
-- Configuration
custom_model_configs, webhook_endpoints, alert_rules
```

**Global tables** (no org_id — superadmin only):
```sql
users, organizations, system_settings, admin_audit_log
```

### 5.4 Cache-Level Isolation

#### 5.4.1 Key Prefixing

All cache keys include the organization ID as prefix:

```python
CACHE_PREFIX = "t:{org_id}"  # Short prefix to minimize memory

def cache_key(org_id: UUID, resource: str, identifier: str = "") -> str:
    """Generate a tenant-isolated cache key."""
    if identifier:
        return f"t:{org_id}:{resource}:{identifier}"
    return f"t:{org_id}:{resource}"

# Examples:
cache_key(org_id, "apikey", key_hash)     # "t:550e8400-e29b-41d4-a716-446655440000:apikey:abc123"
cache_key(org_id, "settings")              # "t:550e8400-e29b-41d4-a716-446655440000:settings"
cache_key(org_id, "usage", "2024-01")      # "t:550e8400-e29b-41d4-a716-446655440000:usage:2024-01"
```

#### 5.4.2 Cache Isolation Rules

| Rule | Implementation |
|------|---------------|
| Never use global cache keys for tenant data | Always prefix with `t:{org_id}` |
| Shared config cached separately | `system:config` for global, `t:{org_id}:config` for tenant |
| Cache eviction on org deletion | Pattern delete `t:{org_id}:*` |
| Cross-tenant cache hit is impossible | Different prefixes = different keys |

### 5.5 Log-Level Isolation

#### 5.5.1 Structured Logging

Every log entry includes `org_id` for tenant identification:

```python
import structlog

logger = structlog.get_logger()

async def log_request(request: Request, tenant: TenantContext):
    """Log a request with full tenant context."""
    logger.info(
        "api_request",
        org_id=str(tenant.org_id),
        request_id=str(request.state.request_id),
        user_id=str(tenant.user_id) if tenant.user_id else None,
        key_id=str(tenant.key_id) if tenant.key_id else None,
        method=request.method,
        path=request.url.path,
        model=request.body.get("model"),
        tokens_used=request.response.get("usage", {}).get("total_tokens"),
        latency_ms=request.state.latency_ms,
    )
```

#### 5.5.2 Log Output Format

```json
{
  "timestamp": "2025-01-15T10:30:00Z",
  "level": "info",
  "event": "api_request",
  "org_id": "550e8400-e29b-41d4-a716-446655440000",
  "request_id": "f47ac10b-58cc-4372-a567-0e02b2c3d479",
  "key_id": "key-uuid-here",
  "method": "POST",
  "path": "/v1/chat/completions",
  "model": "gpt-4",
  "tokens_used": 512,
  "latency_ms": 145.2
}
```

### 5.6 Prevention of Tenant Escape

#### 5.6.1 Validation Layers

| Layer | Check | Failure Action |
|-------|-------|---------------|
| **Authentication** | Verify API key or session belongs to valid org | Reject request |
| **URL Parameter** | `org_id` in path matches auth context org_id | Log warning, reject request |
| **Query Parameter** | Any `org_id` param matches auth context | Reject request |
| **Request Body** | `org_id` in JSON body matches auth context | Reject request |
| **Database Query** | WHERE clause always includes org_id | Data leak prevented |
| **Cache Key** | All cache keys prefixed with org_id | Cross-tenant miss (safe) |
| **Response** | Ensure no org_id from other tenants in response | Filter before sending |

#### 5.6.2 Automated Testing

```python
# Security test: tenant isolation
def test_cannot_access_other_org_data():
    """Verify user cannot access data from another organization."""
    # Create two orgs with API keys
    org1_key = create_api_key(org1_id)
    org2_key = create_api_key(org2_id)
    
    # Create data in org1
    response1 = api_request(org1_key, "/v1/chat/completions", data={...})
    request_id = response1["request_id"]
    
    # Try to access org1's data with org2 key (should fail)
    response2 = api_request(org2_key, f"/v1/requests/{request_id}")
    assert response2.status_code == 404  # Not found (not 403, to avoid info leak)
    
    # Try org_id parameter tampering
    response3 = api_request(org1_key, f"/v1/requests?org_id={org2_id}")
    assert response3.status_code == 403
```

---

## 6. Superadmin

### 6.1 Definition

Superadmin is a special system-level role with access to all organizations and system configuration. Superadmins are **not** stored in the `organization_members` table.

### 6.2 Capabilities

| Capability | Description |
|------------|-------------|
| View all organizations | Read-only access to all org data |
| Manage organizations | Create, suspend, delete organizations |
| View system metrics | Global usage, health, performance |
| Manage system settings | Feature flags, global rate limits |
| Access audit logs | All auth events across all orgs |
| User impersonation | Temporarily act as any org user (with audit trail) |
| Emergency key revocation | Revoke any API key system-wide |
| Manage superadmins | Add/remove superadmin accounts |

### 6.3 Configuration

Superadmins are configured via environment variables (not in database UI):

```bash
# Comma-separated list of superadmin emails
SUPERADMIN_EMAILS="admin@company.com,support@company.com"
```

Superadmin status is determined at login time:

```python
async def check_superadmin(email: str) -> bool:
    """Check if email is in superadmin list."""
    superadmin_emails = os.environ.get("SUPERADMIN_EMAILS", "").split(",")
    return email.lower().strip() in [e.lower().strip() for e in superadmin_emails]
```

### 6.4 Superadmin JWT

Superadmin access tokens include an additional claim:

```python
# In create_session():
if is_superadmin:
    access_payload["is_superadmin"] = True
    access_payload["org_id"] = "*"  # Wildcard — all orgs
```

Superadmin authentication skips org-specific checks:

```python
async def verify_access_token(token: str) -> TokenPayload:
    payload = jwt.decode(token, JWT_PUBLIC_KEY, algorithms=[JWT_ALGORITHM])
    
    is_superadmin = payload.get("is_superadmin", False)
    
    if is_superadmin:
        return TokenPayload(
            user_id=UUID(payload["sub"]),
            org_id=None,  # Must be specified per-request
            role="superadmin",
            is_superadmin=True
        )
    
    # Normal user validation continues...
```

### 6.5 Superadmin Access Patterns

```python
# Superadmin must explicitly specify org_id in request
@app.get("/admin/organizations/{org_id}/apikeys")
async def admin_list_keys(
    org_id: UUID,
    auth: AuthContext = Depends(RequireSuperadmin)
):
    """Superadmin endpoint to list keys for any org."""
    return await get_org_keys(org_id)  # Direct org_id, no auth check

# Impersonation
@app.post("/admin/impersonate/{user_id}")
async def impersonate(
    user_id: UUID,
    auth: AuthContext = Depends(RequireSuperadmin)
):
    """Generate a temporary token to act as another user."""
    target_user = await get_user(user_id)
    
    # Create impersonation token (short-lived: 1 hour)
    token = create_impersonation_token(
        superadmin_id=auth.user_id,
        target_user_id=user_id,
        target_org_id=target_user.primary_org_id,
        lifetime=3600
    )
    
    await audit_log.record(
        action="impersonation_started",
        superadmin_id=auth.user_id,
        target_user_id=user_id,
        target_org_id=target_user.primary_org_id
    )
    
    return {"impersonation_token": token}
```

### 6.6 Security Considerations

| Control | Implementation |
|---------|---------------|
| Immutable via UI | Superadmin list only changeable via env var / config file |
| Audit all actions | Every superadmin action is logged with full context |
| No self-elevation | Existing superadmins cannot add new superadmins through UI |
| Impersonation audit | All impersonation sessions are logged and visible to org owners |
| Rate limiting | Superadmin endpoints have stricter rate limits |
| IP allowlist | Optional: superadmin endpoints restricted to office IPs |

---

## 7. Security Controls

### 7.1 Rate Limiting

#### 7.1.1 Rate Limit Tiers

| Tier | Scope | Limit | Window | Implementation |
|------|-------|-------|--------|----------------|
| **Global IP** | Per IP address | 100 req/min | 60s | Middleware, Redis counter |
| **Login attempts** | Per email | 5 attempts | 15min | Redis counter, account lock |
| **Registration** | Per IP | 5 accounts/hour | 3600s | Redis counter |
| **API Key** | Per key | Configurable (default 100 rps) | 1s | Redis sliding window |
| **Organization** | Per org | Configurable (default 10,000 rps) | 1s | Redis sliding window |
| **Password reset** | Per email | 3 requests/hour | 3600s | Redis counter |

#### 7.1.2 Sliding Window Implementation

```python
import math

class SlidingWindowRateLimiter:
    """Redis-backed sliding window rate limiter."""
    
    def __init__(self, redis_client):
        self.redis = redis_client
    
    async def is_allowed(self, key: str, limit: int, window: int) -> tuple[bool, dict]:
        """
        Check if request is allowed under rate limit.
        
        Returns:
            (allowed, metadata) where metadata includes remaining and reset
        """
        now = time.time()
        window_start = now - window
        
        pipe = self.redis.pipeline()
        
        # Remove expired entries
        pipe.zremrangebyscore(key, 0, window_start)
        
        # Count current entries
        pipe.zcard(key)
        
        # Add current request
        pipe.zadd(key, {str(now): now})
        
        # Set expiry on the key
        pipe.expire(key, window)
        
        results = await pipe.execute()
        current_count = results[1]  # Count before adding current request
        
        allowed = current_count < limit
        
        if not allowed:
            # Remove the entry we just added
            await self.redis.zrem(key, str(now))
        
        # Calculate reset time (oldest entry in window)
        oldest = await self.redis.zrange(key, 0, 0, withscores=True)
        reset_at = oldest[0][1] + window if oldest else now + window
        
        remaining = max(0, limit - current_count - (1 if allowed else 0))
        
        return allowed, {
            "limit": limit,
            "remaining": remaining,
            "reset": math.ceil(reset_at),
            "window": window
        }
```

#### 7.1.3 Rate Limit Headers

Every API response includes rate limit headers:

```http
X-RateLimit-Limit: 100
X-RateLimit-Remaining: 97
X-RateLimit-Reset: 1705312800
X-RateLimit-Window: 1
```

When rate limit exceeded:

```http
HTTP/1.1 429 Too Many Requests
Retry-After: 60
X-RateLimit-Limit: 100
X-RateLimit-Remaining: 0
X-RateLimit-Reset: 1705312800

{
    "error": "RATE_LIMIT_EXCEEDED",
    "message": "Rate limit exceeded. Please slow down.",
    "retry_after": 60
}
```

### 7.2 Brute Force Protection

#### 7.2.1 Login Protection

```
Failed Attempts    Action
─────────────────────────────────────────
1-4                Increment counter, allow retry
5                  Lock account for 30 minutes
5+                 Extend lock, log security event
After successful   Reset counter to 0
login
```

#### 7.2.2 Distributed Brute Force Detection

```python
async def detect_distributed_attack(ip_prefix: str) -> bool:
    """
    Detect distributed brute force (many IPs, same target pattern).
    Looks for high volume of failed logins across IP range.
    """
    failed_logins = await redis.get(f"distributed_failures:{ip_prefix}")
    if int(failed_logins or 0) > 100:  # 100 failures from /24 in 5 minutes
        # Block the entire /24 for 1 hour
        await redis.setex(f"blocked_range:{ip_prefix}", 3600, "1")
        await alert_security_team("distributed_brute_force", ip_prefix)
        return True
    return False
```

### 7.3 Session Invalidation

#### 7.3.1 "Logout Everywhere"

Triggered by:
- Password change
- Suspicious activity detected
- Admin action
- User request

Implementation: See Section 3.10.2.

#### 7.3.2 Session Version Check

Every token includes a session version that must match the server's version:

```python
async def check_session_version(user_id: UUID, token_version: int) -> bool:
    """Verify the token's session version matches current version."""
    current_version = await redis.get(f"user:{user_id}:session_version")
    return int(current_version or 0) <= token_version
```

### 7.4 Audit Logging

#### 7.4.1 Events to Log

| Event | Data Logged | Retention |
|-------|------------|-----------|
| User login | user_id, ip, user_agent, timestamp, success/fail | 1 year |
| User logout | user_id, timestamp | 1 year |
| Password change | user_id, timestamp, ip | 1 year |
| Password reset request | user_id/email, ip, timestamp | 1 year |
| API key created | key_id, org_id, user_id, timestamp | 1 year |
| API key revoked | key_id, org_id, user_id, reason | 1 year |
| API key used (failed auth) | key_prefix, org_id, ip, reason | 90 days |
| Role changed | target_user_id, old_role, new_role, changed_by | 1 year |
| Member invited | email, org_id, invited_by, role | 1 year |
| Member removed | user_id, org_id, removed_by | 1 year |
| MFA enabled/disabled | user_id, timestamp | 1 year |
| Superadmin impersonation | superadmin_id, target_user_id, duration | 2 years |
| Organization deleted | org_id, deleted_by | 2 years |

#### 7.4.2 Audit Log Schema

```sql
CREATE TABLE audit_log (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    timestamp       TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    event_type      VARCHAR(50) NOT NULL,
    severity        VARCHAR(10) NOT NULL DEFAULT 'info',
                    -- debug, info, warning, error, critical
    
    -- Actor
    actor_id        UUID,  -- NULL for unauthenticated actions
    actor_type      VARCHAR(20),  -- user, api_key, system, superadmin
    actor_role      VARCHAR(20),
    
    -- Target
    target_type     VARCHAR(50),  -- user, api_key, organization, member
    target_id       UUID,
    
    -- Context
    org_id          UUID,
    ip_address      INET,
    user_agent      VARCHAR(512),
    
    -- Details
    action          VARCHAR(50),
    details         JSONB,  -- Flexible structured data
    
    -- Result
    success         BOOLEAN,
    error_code      VARCHAR(50)
);

CREATE INDEX idx_audit_timestamp ON audit_log(timestamp);
CREATE INDEX idx_audit_org ON audit_log(org_id);
CREATE INDEX idx_audit_actor ON audit_log(actor_id);
CREATE INDEX idx_audit_event ON audit_log(event_type);
```

### 7.5 MFA (Multi-Factor Authentication)

#### 7.5.1 Status: Implemented (Recommended)

MFA is implemented via TOTP (Time-based One-Time Password) per RFC 6238.

#### 7.5.2 TOTP Implementation

```python
import pyotp
import qrcode
import qrcode.image.svg
import io
import base64

class TOTPService:
    """TOTP MFA service."""
    
    ISSUER = "AI Gateway"
    
    @staticmethod
    def generate_secret() -> str:
        """Generate a new TOTP secret."""
        return pyotp.random_base32()
    
    @staticmethod
    def get_provisioning_uri(secret: str, email: str) -> str:
        """Generate otpauth:// URI for QR code."""
        totp = pyotp.TOTP(secret)
        return totp.provisioning_uri(
            name=email,
            issuer_name=TOTPService.ISSUER
        )
    
    @staticmethod
    def generate_qr_code(uri: str) -> str:
        """Generate QR code as base64 data URI."""
        factory = qrcode.image.svg.SvgImage
        img = qrcode.make(uri, image_factory=factory)
        buffer = io.BytesIO()
        img.save(buffer)
        svg_data = base64.b64encode(buffer.getvalue()).decode()
        return f"data:image/svg+xml;base64,{svg_data}"
    
    @staticmethod
    def verify(secret: str, code: str) -> bool:
        """Verify a TOTP code."""
        totp = pyotp.TOTP(secret)
        # Allow 1 time step window (±30 seconds)
        return totp.verify(code, valid_window=1)

async def setup_mfa(user_id: UUID):
    """Start MFA setup for a user."""
    secret = TOTPService.generate_secret()
    
    # Store encrypted secret temporarily (not activated yet)
    await redis.setex(f"mfa_setup:{user_id}", 600, secret)  # 10 min to complete
    
    user = await db.fetchrow("SELECT email FROM users WHERE id = $1", user_id)
    uri = TOTPService.get_provisioning_uri(secret, user["email"])
    qr_code = TOTPService.generate_qr_code(uri)
    
    return {
        "secret": secret,  # For manual entry
        "qr_code": qr_code,
        "message": "Scan QR code with authenticator app and verify to activate"
    }

async def verify_and_activate_mfa(user_id: UUID, code: str):
    """Verify MFA code and activate MFA for user."""
    secret = await redis.get(f"mfa_setup:{user_id}")
    if not secret:
        raise AuthError("SETUP_EXPIRED", "MFA setup has expired. Please start again.")
    
    if not TOTPService.verify(secret, code):
        raise AuthError("INVALID_CODE", "Invalid verification code")
    
    # Encrypt secret before storing
    encrypted_secret = encrypt_with_kms(secret)
    
    await db.execute(
        "UPDATE users SET mfa_enabled = TRUE, mfa_secret = $1 WHERE id = $2",
        encrypted_secret, user_id
    )
    
    await redis.delete(f"mfa_setup:{user_id}")
    
    # Generate backup codes
    backup_codes = generate_backup_codes(10)
    await store_backup_codes(user_id, backup_codes)
    
    await audit_log.record(action="mfa_enabled", user_id=user_id)
    
    return {
        "status": "enabled",
        "backup_codes": backup_codes  # Show once
    }
```

#### 7.5.3 MFA Enforcement Policy

| Setting | Default | Description |
|---------|---------|-------------|
| MFA optional | Yes (default) | Users can enable MFA voluntarily |
| MFA required for admins | No (configurable) | Force MFA for admin+ roles |
| MFA required org-wide | No (configurable) | Force MFA for all members |
| Remember device | 30 days | Skip MFA on trusted devices |

### 7.6 Additional Security Headers

```http
Strict-Transport-Security: max-age=31536000; includeSubDomains; preload
X-Content-Type-Options: nosniff
X-Frame-Options: DENY
Content-Security-Policy: default-src 'self'; script-src 'self' 'unsafe-inline'
X-XSS-Protection: 1; mode=block
Referrer-Policy: strict-origin-when-cross-origin
Permissions-Policy: camera=(), microphone=(), geolocation=()
```

### 7.7 API Key Security Best Practices

| Control | Implementation |
|---------|---------------|
| HTTPS only | Reject plaintext HTTP requests |
| Key in header only | Never accept API key in query params or body |
| Key prefix in logs | Only log first 8 chars (`gk_live_aB...`) |
| No key in error messages | Never return API key in any error response |
| Auto-revoke on breach | Integration with known breach databases (optional) |
| Key age warnings | Alert when keys are > 60 days old |

---

## 8. Error Handling

### 8.1 Standard Error Response Format

All authentication and authorization errors use this response format:

```json
{
    "error": "ERROR_CODE",
    "message": "Human-readable description",
    "detail": {},
    "request_id": "uuid-for-tracing",
    "timestamp": "2025-01-15T10:30:00Z"
}
```

### 8.2 Auth Error Catalog

#### 8.2.1 API Key Errors

| Error Code | HTTP Status | Message | Trigger |
|------------|-------------|---------|---------|
| `MISSING_KEY` | 401 | "API key is required" | No Authorization header |
| `INVALID_KEY_FORMAT` | 401 | "API key format is invalid" | Key doesn't match regex |
| `INVALID_KEY` | 401 | "API key not found" | Key hash not in database |
| `REVOKED_KEY` | 401 | "This API key has been revoked" | Key status = 'revoked' |
| `EXPIRED_KEY` | 401 | "This API key has expired" | Key past expiration date |
| `IP_NOT_ALLOWED` | 403 | "Request IP not in allowlist" | Client IP not in allowed list |
| `MODEL_NOT_ALLOWED` | 403 | "Model 'X' not allowed for this key" | Requested model not in allowed_models |
| `RATE_LIMIT_EXCEEDED` | 429 | "Rate limit exceeded" | Key-level rate limit hit |
| `QUOTA_EXCEEDED` | 429 | "Monthly budget exceeded" | Key/org monthly budget exhausted |

```python
# Example error response for revoked key
{
    "error": "REVOKED_KEY",
    "message": "This API key has been revoked",
    "detail": {
        "key_prefix": "gk_live_aB",
        "revoked_at": "2025-01-10T08:00:00Z"
    },
    "request_id": "f47ac10b-58cc-4372-a567-0e02b2c3d479",
    "timestamp": "2025-01-15T10:30:00Z"
}
```

#### 8.2.2 Session Auth Errors

| Error Code | HTTP Status | Message | Trigger |
|------------|-------------|---------|---------|
| `INVALID_CREDENTIALS` | 401 | "Invalid email or password" | Wrong email/password combo |
| `ACCOUNT_SUSPENDED` | 403 | "Account has been suspended" | User status = 'suspended' |
| `ACCOUNT_DEACTIVATED` | 403 | "Account has been deactivated" | User status = 'deactivated' |
| `EMAIL_NOT_VERIFIED` | 403 | "Please verify your email" | User status = 'pending' |
| `ACCOUNT_LOCKED` | 423 | "Account temporarily locked" | Too many failed login attempts |
| `TOO_MANY_ATTEMPTS` | 429 | "Too many login attempts" | Rate limit on login endpoint |
| `TOKEN_EXPIRED` | 401 | "Access token has expired" | JWT past exp claim |
| `TOKEN_REVOKED` | 401 | "Token has been revoked" | Token in revocation list |
| `INVALID_TOKEN` | 401 | "Token is invalid" | Bad signature, malformed JWT |
| `INVALID_REFRESH_TOKEN` | 401 | "Refresh token invalid or revoked" | Bad/expired/revoked refresh token |
| `MFA_REQUIRED` | 403 | "Multi-factor authentication required" | MFA enabled but no code provided |
| `INVALID_MFA_CODE` | 401 | "Invalid MFA code" | Wrong TOTP code |
| `SETUP_EXPIRED` | 400 | "MFA setup has expired" | MFA setup window passed |

#### 8.2.3 Authorization Errors

| Error Code | HTTP Status | Message | Trigger |
|------------|-------------|---------|---------|
| `INSUFFICIENT_PERMISSIONS` | 403 | "Role 'X' does not have permission 'Y'" | Missing required permission |
| `ORG_ACCESS_DENIED` | 403 | "You do not have access to this organization" | Cross-org access attempt |
| `NOT_ORG_MEMBER` | 403 | "User is not a member of this organization" | Valid user, wrong org |
| `OWNER_REQUIRED` | 403 | "This action requires owner privileges" | Non-owner attempting owner action |

#### 8.2.4 General Errors

| Error Code | HTTP Status | Message | Trigger |
|------------|-------------|---------|---------|
| `UNAUTHENTICATED` | 401 | "Authentication required" | No auth provided for protected endpoint |
| `INTERNAL_ERROR` | 500 | "Internal authentication error" | Unexpected server error |
| `SERVICE_UNAVAILABLE` | 503 | "Authentication service unavailable" | Redis/database connectivity issue |

### 8.3 Error Response Implementation

```python
from fastapi import HTTPException
from pydantic import BaseModel
from typing import Optional, Any
import uuid
from datetime import datetime, timezone

class AuthErrorDetail(BaseModel):
    """Standard auth error response."""
    error: str
    message: str
    detail: dict[str, Any] = {}
    request_id: str
    timestamp: str

class AuthError(Exception):
    """Custom auth exception with structured error info."""
    
    def __init__(
        self,
        error_code: str,
        message: str,
        status_code: int = 401,
        detail: dict = None,
        headers: dict = None
    ):
        self.error_code = error_code
        self.message = message
        self.status_code = status_code
        self.detail = detail or {}
        self.headers = headers or {}
        super().__init__(message)
    
    def to_response(self, request_id: str) -> dict:
        """Convert to standard error response dict."""
        return {
            "error": self.error_code,
            "message": self.message,
            "detail": self.detail,
            "request_id": request_id,
            "timestamp": datetime.now(timezone.utc).isoformat()
        }

# FastAPI exception handler
@app.exception_handler(AuthError)
async def auth_error_handler(request: Request, exc: AuthError):
    request_id = getattr(request.state, 'request_id', str(uuid.uuid4()))
    
    headers = dict(exc.headers)
    if exc.status_code == 429:
        headers.setdefault("Retry-After", str(exc.detail.get("retry_after", 60)))
    
    return JSONResponse(
        status_code=exc.status_code,
        headers=headers,
        content=exc.to_response(request_id)
    )
```

### 8.4 Security-Sensitive Error Handling

| Scenario | Response | Rationale |
|----------|----------|-----------|
| Invalid email on login | Generic "Invalid credentials" | Prevents email enumeration |
| Invalid password on login | Generic "Invalid credentials" | Prevents username enumeration |
| Password reset for non-existent email | Same success response | Prevents email enumeration |
| Organization not found | 404 (no org info) | No information about other orgs |
| Key from different org on lookup | 401 "Invalid key" | Don't reveal key exists under different org |

---

## Appendix A: Configuration Reference

```yaml
# Auth Configuration
auth:
  # API Key settings
  api_key:
    prefix: "gk"
    random_length: 32
    checksum_length: 6
    default_rate_limit_rps: 100
    default_monthly_budget_usd: null  # unlimited
    max_keys_per_org: 100
    cache_ttl_seconds: 300
    rotation_warning_days: [30, 14, 7, 1]
    mandatory_rotation_days: 180
    grace_period_days: 7
  
  # JWT settings
  jwt:
    algorithm: "RS256"
    access_token_lifetime_seconds: 900      # 15 minutes
    refresh_token_lifetime_seconds: 604800  # 7 days
    rsa_key_size: 2048
    require_email_verification: true
  
  # Password settings
  password:
    min_length: 12
    max_length: 128
    require_uppercase: true
    require_lowercase: true
    require_digit: true
    require_special: true
    check_common_passwords: true
    argon2_time_cost: 3
    argon2_memory_cost: 65536
    argon2_parallelism: 4
  
  # Rate limiting
  rate_limit:
    global_ip_rpm: 100
    login_attempts_per_email: 5
    login_lockout_minutes: 30
    registration_per_ip_per_hour: 5
    password_reset_per_email_per_hour: 3
  
  # MFA settings
  mfa:
    enabled: true
    required_for_admins: false
    required_org_wide: false
    remember_device_days: 30
    totp_window: 1
    backup_codes_count: 10
  
  # Session
  session:
    max_concurrent_sessions: 10
    cookie_http_only: true
    cookie_secure: true
    cookie_same_site: "strict"
  
  # Superadmin
  superadmin:
    emails: []  # Set via env var
    require_ip_allowlist: false
    impersonation_max_duration_minutes: 60
```

## Appendix B: Database Migration Checklist

1. [ ] Create `users` table
2. [ ] Create `organizations` table
3. [ ] Create `organization_members` table
4. [ ] Create `api_keys` table
5. [ ] Create `refresh_tokens` table
6. [ ] Create `audit_log` table
7. [ ] Create `api_key_audit` table
8. [ ] Create `organization_settings` table
9. [ ] Create indexes on all foreign keys
10. [ ] Create indexes on lookup columns (email, key_hash, status)
11. [ ] Set up row-level security policies (PostgreSQL RLS)
12. [ ] Generate RSA key pair for JWT signing
13. [ ] Configure superadmin emails
14. [ ] Seed initial application settings

## Appendix C: Security Checklist

- [ ] API keys stored as SHA-256 hashes, never plaintext
- [ ] Passwords hashed with Argon2id (OWASP parameters)
- [ ] JWT signed with RS256, private key protected
- [ ] httpOnly, Secure, SameSite=Strict cookies
- [ ] Rate limiting on all auth endpoints
- [ ] Account lockout after failed login attempts
- [ ] Email verification required for registration
- [ ] org_id filter on every tenant-scoped database query
- [ ] Cache keys prefixed with tenant org_id
- [ ] Audit logging for all auth events
- [ ] Superadmin access restricted and audited
- [ ] No sensitive data in error messages
- [ ] Constant-time password verification
- [ ] HTTPS-only for all auth endpoints
- [ ] Security headers on all responses
- [ ] CORS properly configured (whitelist origins)
- [ ] Input validation on all auth parameters
- [ ] SQL injection prevention (parameterized queries)
- [ ] XSS prevention (no user input in responses without encoding)
- [ ] CSRF protection (SameSite cookies + CSRF tokens for state-changing ops)
