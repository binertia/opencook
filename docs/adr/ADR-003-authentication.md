# ADR-003: Authentication

## Status
Accepted

## Context
The AI Gateway serves two fundamentally different client types with distinct security and lifecycle requirements:

1. **API Consumers:** Applications, scripts, and services making LLM API calls programmatically. They need long-lived, revocable credentials with high-throughput validation.
2. **Dashboard Users:** Human administrators managing organizations, API keys, quotas, and viewing usage analytics. They need session-based authentication with interactive login flows.

Key forces:
- API key auth must add <1ms overhead per request (validated on every LLM call)
- Dashboard sessions must survive gateway restarts (admin actions should not be interrupted by deploys)
- No external identity provider dependency (Auth0, Okta) to keep deployment self-contained
- Key revocation must propagate within <100ms
- Password storage must follow OWASP recommendations
- The system must support tenant isolation from the auth layer down

## Decision
We will operate **two independent authentication systems**:

**System A: API Key Authentication (for LLM API consumers)**
- API key format: `gk_{environment}_{random_base58}{checksum}` (47 characters, Base58 for URL safety)
- Keys are hashed with SHA-256 for database storage; only the hash is persisted, never the plaintext key
- Redis caches the `key_hash -> org_context` mapping with a 5-minute TTL for sub-millisecond lookups
- Keys support scoping: allowed models, IP allowlists, rate limits, and budget caps
- Revocation is immediate: `DEL cache:{hash}` + pub/sub event propagation

**System B: Session Authentication (for dashboard users)**
- JWT tokens stored in httpOnly cookies (not localStorage, to prevent XSS theft)
- Access tokens are short-lived (24 hours); refresh tokens enable session renewal
- JWT verification is stateless (no DB lookup per request)
- Redis stores session data for logout/invalidation support
- Passwords hashed with Argon2id (OWASP-recommended: t=3, m=64MB, p=4)

**Why not OAuth-first:**
OAuth2/OIDC requires an external identity provider (Google, GitHub, corporate SSO) or running a self-hosted IdP (Keycloak, Dex). External IdPs add operational dependencies that may not be available to all customers (e.g., self-hosted deployments without internet access). OAuth flows are also significantly more complex (authorization codes, PKCE, token exchange, redirect URIs) than API key + JWT session auth. OAuth SSO is supported as an optional add-on, not the primary auth mechanism.

## Alternatives Considered

### Alternative 1: Single Auth System for Both APIs and Dashboard
- **Description:** Use JWT sessions for both LLM API requests and dashboard access.
- **Why rejected:** API consumers (scripts, backend services) are poorly suited to session-based auth with cookies and refresh tokens. API keys are the industry standard for machine-to-machine LLM API authentication. JWT sessions require periodic refresh which complicates long-running scripts. Conversely, dashboard users need interactive login with password reset and MFA, which API keys do not support.

### Alternative 2: OAuth2 as Mandatory Primary Authentication
- **Description:** Require all users to authenticate via external OAuth2 providers (Google, GitHub, etc.).
- **Why rejected:** Adds a hard dependency on external identity providers that may be unavailable in self-hosted or air-gapped deployments. Not all target customers have corporate identity providers. OAuth2 flows are significantly more complex to implement and debug. Violates the "self-contained deployment" principle.

### Alternative 3: API Keys in Plaintext
- **Description:** Store API keys in plaintext in the database for easy retrieval and display.
- **Why rejected:** If the database is compromised, all customer API keys are immediately exposed. Storing only SHA-256 hashes means a DB breach does not leak usable credentials. Keys are shown exactly once at creation time; after that, only the prefix is visible in the UI.

### Alternative 4: HS256 for JWT Signing
- **Description:** Use HMAC-SHA256 (symmetric key) for JWT token signing instead of RS256.
- **Why rejected:** HS256 requires the same secret key for signing and verification, which complicates key rotation (all sessions must be invalidated simultaneously). RS256 uses asymmetric keys; the public key can be distributed for verification without exposing the private signing key. HS256 is acceptable for single-instance deployments but RS256 is preferred for future-proofing.

## Tradeoffs

### What We Gain
- **Independent optimization:** API key auth is optimized for speed (<1ms overhead); session auth is optimized for security (MFA, audit logging).
- **No external dependencies:** Self-contained deployment works without internet access or third-party services.
- **Industry-standard API keys:** Developers expect `Authorization: Bearer <key>` for LLM APIs; zero learning curve.
- **Fast revocation:** API key revocation propagates in <100ms via Redis cache invalidation.
- **Security in depth:** Argon2id for passwords, SHA-256 hashes for API keys, httpOnly cookies for JWT, RBAC for authorization.

### What We Give Up
- **Unified user experience:** Users have separate credentials for API access and dashboard login.
- **SSO complexity:** Organizations wanting SAML/OIDC SSO need the optional add-on module.
- **Operational overhead:** Two auth systems to maintain, monitor, and secure.
- **No API key "user identity":** API keys are tied to organizations, not individual users. Audit trails for API actions show the key used, not the person.

## Consequences
- The `api_keys` table stores `key_hash` (SHA-256), `key_prefix` (first 8 chars for display), and scoping rules. Plaintext keys exist only ephemerally at creation time.
- API key validation path: extract key -> compute hash -> Redis cache lookup (L1, <0.5ms) -> PostgreSQL fallback on miss -> validate status/scopes/rate limits -> attach `AuthContext` to request.
- Session validation path: read httpOnly cookie -> verify JWT signature (RS256) -> check expiry -> attach user context. No database lookup on the hot path.
- Login implements rate limiting (10 attempts per hour per email), account locking (5 failures = 30-minute lock), and optional TOTP-based MFA.
- RBAC defines four roles per organization: `owner`, `admin`, `member`, `viewer`. Roles determine dashboard access permissions and API key management capabilities.
- Superadmin role exists for platform-level operations (managing organizations, global settings) and is separate from organization roles.
- Password requirements enforce 12-character minimum, complexity rules (uppercase, lowercase, digit, special character), and rejection of common passwords via Bloom filter.

## Related Decisions
- **ADR-005 (Tenant Model):** Auth context carries `org_id`; all downstream operations (cache, quota, billing) are scoped to the authenticated tenant.
- **ADR-004 (Rate Limiting):** Rate limits are applied after authentication; the auth system provides the `rate_limit_tier` and `key_id` used by the rate limiter.

## Notes
- API key format uses Base58 (excludes `0`, `O`, `I`, `l`) to prevent transcription errors when keys are copied manually.
- Key rotation creates a new key with the same permissions, revokes the old key with a 7-day grace period.
- Session data in Redis includes IP address binding and user-agent hash for session theft detection.
- Future work: OIDC SSO connector for enterprise customers wanting to federate with their corporate identity provider.
- JWT tokens include `jti` (JWT ID) claim to support token revocation without full session invalidation.
