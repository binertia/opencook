# Gateway Security Audit Checklist

This document tracks the security posture of the AI Gateway against the OWASP Top 10 and production-hardening best practices.

## OWASP Top 10 Mitigations

| # | Risk | Mitigation | Status |
|---|------|------------|--------|
| A01 | Broken Access Control | RBAC (`Owner/Admin/Member/Viewer`) enforced on all admin endpoints; tenant isolation via `org_id` scoping in every query | ✅ |
| A02 | Cryptographic Failures | AES-256-GCM for provider API keys; JWT HS256/RS256 for sessions; TLS 1.2+ via rustls; master key from env | ✅ |
| A03 | Injection | `ValidatedJson` extractor with `validator`; regex-based SQL-injection pattern detection; input sanitization helpers | ✅ |
| A04 | Insecure Design | Immutable audit log; fail-closed auth middleware; circuit breaker + timeout on all provider calls | ✅ |
| A05 | Security Misconfiguration | `GATEWAY_ENVIRONMENT=production` blocks wildcard CORS; security headers middleware (CSP, HSTS, etc.) | ✅ |
| A06 | Vulnerable and Outdated Components | `cargo audit` run on every build; `validator` upgraded to 0.20 to resolve `idna` RUSTSEC-2024-0421 | ✅ |
| A07 | Identification and Authentication Failures | Password hashing with Argon2; JWT expiry & refresh tokens; API key prefix + hash storage; session auth required for admin | ✅ |
| A08 | Software and Data Integrity Failures | Webhook HMAC-SHA256 signatures; signed JWT claims; provider config encrypted at rest | ✅ |
| A09 | Security Logging and Monitoring Failures | Immutable audit log records all security-relevant actions; timing middleware emits latency metrics; health checks | ✅ |
| A10 | Server-Side Request Forgery (SSRF) | Provider base URLs validated with `validator::url`; no arbitrary URL fetching from user input | ✅ |

## Authentication & Authorization

- [x] Passwords hashed with Argon2id (`gateway_auth::PasswordHasherService`)
- [x] JWT access tokens expire after 15 minutes; refresh tokens rotate
- [x] API keys stored as hashes (SHA-256) with prefixes; raw key shown exactly once
- [x] `AuthContext` injected after every successful auth; no handler bypasses it
- [x] `AuditRead` permission restricted to Owner/Admin roles
- [x] Cross-org access attempts return 403 (no implicit trust)

## Input Validation

- [x] `ValidatedJson` extractor rejects malformed bodies with field-level errors
- [x] `sanitize_input` strips control characters and normalizes whitespace
- [x] `validate_provider_kind` allows only known provider types
- [x] Rate limiting: 10 req/min/IP on auth endpoints; configurable RPS per API key
- [x] Body limit: 10 MB max request size

## Cryptography

- [x] Master key loaded from `GATEWAY_MASTER_KEY` (64-char hex); random dev fallback warns loudly
- [x] Provider API keys encrypted with AES-256-GCM using master key
- [x] TLS 1.2+ served via `axum-server` with rustls when `tls_cert`/`tls_key` configured
- [x] JWT signed with RS256 when PEM keys provided, HS256 dev fallback with warning

## Transport & Headers

- [x] CORS configured via `GATEWAY_ALLOWED_ORIGINS`; wildcard blocked in production
- [x] Preflight cached for 24 hours (`Access-Control-Max-Age: 86400`)
- [x] Credentials enabled for explicit origin lists
- [x] Security headers middleware adds:
  - `X-Content-Type-Options: nosniff`
  - `X-Frame-Options: DENY`
  - `Content-Security-Policy: default-src 'self'`
  - `X-XSS-Protection: 1; mode=block`
  - `Referrer-Policy: strict-origin-when-cross-origin`
  - `Strict-Transport-Security` (when served over HTTPS or behind HTTPS proxy)

## CSRF Protection

- [x] Double-submit cookie pattern for `/api/v1/*` state-changing requests
- [x] `csrf_token` cookie set on login/refresh (`SameSite=Strict`)
- [x] `X-CSRF-Token` header required for POST/PUT/DELETE/PATCH under `/api/v1/*`
- [x] Mismatch returns 403 with `csrf_token_missing_or_invalid`

## Error Handling

- [x] 5xx errors return generic message: "An internal error occurred. Please try again later."
- [x] Stack traces and internal details never leak to clients
- [x] PII redaction (`[REDACTED:email]`, `[REDACTED:api_key]`) applied to error messages
- [x] `request_id` included in every error response for support correlation

## Audit & Monitoring

- [x] Immutable `audit_log` table (no UPDATE/DELETE methods in repo)
- [x] All security-relevant actions recorded: login, key lifecycle, provider changes, user changes, quota changes, webhooks
- [x] Audit entries include actor, IP, user-agent, request ID, before/after values
- [x] Timing middleware emits `X-Gateway-Request-ID`, `X-Total-Latency-Ms`, `X-Gateway-Latency-Ms`, `X-Provider-Latency-Ms`

## Dependency Audit

### cargo audit results (latest run)

| Crate | Advisory | Severity | Status | Notes |
|-------|----------|----------|--------|-------|
| `idna` 0.5.0 | RUSTSEC-2024-0421 | — | **Fixed** | Upgraded `validator` → 0.20.0 |
| `rsa` 0.9.10 | RUSTSEC-2023-0071 | Medium (5.9) | **Accepted** | Transitive via `sqlx-mysql`; we use PostgreSQL/SQLite only |
| `paste` 1.0.15 | RUSTSEC-2024-0436 | Info | **Accepted** | Unmaintained; used by `ratatui` (TUI dashboard) |
| `rustls-pemfile` 2.2.0 | unmaintained | Info | **Accepted** | Used for TLS cert loading; migration to `rustls-pki-types` deferred |

## Deployment Hardening

- [x] No hardcoded secrets in codebase (master key, JWT keys, DB passwords from env)
- [x] Debug endpoints (`/health`, `/ready`, `/metrics`) are read-only and public by design
- [x] `GATEWAY_ENVIRONMENT=production` required for production behavior
- [x] TLS certificates configurable via env/file

## Remaining / Deferred Items

| Item | Priority | Ticket |
|------|----------|--------|
| Replace `rustls-pemfile` with `rustls-pki-types` PEM parsing | Low | — |
| Monitor `rsa` crate for fixed release | Medium | — |
| `cargo audit` in CI pipeline | High | TASK-0003 |
| `clippy::unwrap_used` / `clippy::expect_used` lint enforcement | Medium | — |
| Automatic S3 archive for audit log > 90 days | Low | TASK-0084-deferred |
| SIGHUP key reload without restart | Low | TASK-0085 |
