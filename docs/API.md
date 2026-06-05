# API Reference

> **Quick reference for all AI Gateway endpoints.** For the full specification with complete schemas, see [API_SPEC.md](API_SPEC.md).

---

## Base URLs

| API | Path | Authentication |
|-----|------|----------------|
| Public AI API (OpenAI-compatible) | `https://gateway.example.com/v1` | API Key (`Authorization: Bearer sk_gw_...`) |
| Admin API | `https://gateway.example.com/api/v1` | JWT Session or API Key |
| Health/Metrics | `https://gateway.example.com/health` | None |

---

## Authentication

### API Key

```bash
curl https://gateway.example.com/v1/chat/completions \
  -H "Authorization: Bearer sk_gw_1234567890..."
```

Key format: `sk_gw_{32 base58 chars}{6-char checksum}` (44 characters total)

### JWT Session (Dashboard)

Login via the admin dashboard. The backend sets an HTTP-only cookie containing a JWT access token (RS256, 15-minute expiry) and a refresh token (7-day expiry).

---

## Public AI Endpoints

### Chat Completions

Create a chat completion. OpenAI-compatible.

```bash
POST /v1/chat/completions
```

**Request:**

```json
{
  "model": "gpt-4o-mini",
  "messages": [
    {"role": "system", "content": "You are helpful."},
    {"role": "user", "content": "Hello!"}
  ],
  "temperature": 0.7,
  "max_tokens": 256,
  "stream": false,
  "gateway_routing_strategy": "balanced"
}
```

**Response:**

```json
{
  "id": "chatcmpl-abc123",
  "object": "chat.completion",
  "created": 1717000000,
  "model": "gpt-4o-mini",
  "choices": [{
    "index": 0,
    "message": {"role": "assistant", "content": "Hello there!"},
    "finish_reason": "stop"
  }],
  "usage": {
    "prompt_tokens": 15,
    "completion_tokens": 20,
    "total_tokens": 35
  },
  "gateway": {
    "provider_used": "openai",
    "latency_ms": 420,
    "cached": false
  }
}
```

**Streaming (`stream: true`):**

Returns SSE with `data: {...}` lines per chunk. Final line is `data: [DONE]`.

**Gateway-specific headers:**

| Header | Description |
|--------|-------------|
| `X-Gateway-Request-ID` | Unique request trace ID |
| `X-Gateway-Provider` | Provider that handled the request |
| `X-Gateway-Cached` | `true` if response served from cache |
| `X-Gateway-Mock-Response` | `true` if no provider configured |

### List Models

```bash
GET /v1/models
```

**Response:**

```json
{
  "object": "list",
  "data": [
    {
      "id": "gpt-4o-mini",
      "object": "model",
      "created": 1717000000,
      "owned_by": "openai"
    }
  ]
}
```

---

## Admin Endpoints

### Quotas

Manage per-organization quota and budget caps.

```bash
# List quotas
GET /api/v1/organizations/{org_id}/quotas

# Create quota
POST /api/v1/organizations/{org_id}/quotas
{
  "name": "Monthly Token Budget",
  "metric": "tokens",
  "period": "month",
  "limit": 10000000,
  "action": "block"
}

# Get quota
GET /api/v1/organizations/{org_id}/quotas/{quota_id}

# Update quota
PUT /api/v1/organizations/{org_id}/quotas/{quota_id}
{
  "limit": 20000000,
  "action": "warn"
}

# Delete quota
DELETE /api/v1/organizations/{org_id}/quotas/{quota_id}
```

**Quota fields:**

| Field | Type | Description |
|-------|------|-------------|
| `name` | string | Human-readable name |
| `metric` | enum | `requests`, `tokens`, `cost_usd` |
| `period` | enum | `minute`, `hour`, `day`, `month`, `total` |
| `limit` | integer | Maximum allowed value |
| `action` | enum | `block` (403) or `warn` (header) |

### Usage Analytics

```bash
# Request counts and token usage
GET /api/v1/organizations/{org_id}/usage?start=2025-01-01&end=2025-01-31

# Cost breakdown
GET /api/v1/organizations/{org_id}/costs?start=2025-01-01&end=2025-01-31
```

**Usage response:**

```json
{
  "total_requests": 15420,
  "total_tokens": 3200000,
  "total_cost_usd": "42.50",
  "by_provider": {
    "openai": {"requests": 12000, "tokens": 2800000, "cost": "38.20"},
    "anthropic": {"requests": 3420, "tokens": 400000, "cost": "4.30"}
  },
  "by_day": [
    {"date": "2025-01-01", "requests": 500, "tokens": 100000}
  ]
}
```

---

## Health & Metrics

```bash
# Liveness probe
GET /health
# → {"status":"healthy"}

# Readiness probe (checks DB + Redis)
GET /ready
# → {"status":"ready"}

# Prometheus metrics
GET /metrics
# → # HELP gateway_requests_total Total requests
# → gateway_requests_total{status="200"} 15420
```

---

## Error Codes

All errors return a consistent envelope:

```json
{
  "error": {
    "code": "quota_exceeded",
    "message": "Monthly token budget exceeded",
    "type": "quota_error",
    "param": null,
    "status": 403,
    "request_id": "req_abc123"
  }
}
```

| HTTP | Code | Description |
|------|------|-------------|
| `400` | `invalid_request` | Malformed request body |
| `401` | `unauthorized` | Missing or invalid API key |
| `403` | `quota_exceeded` | Quota or budget limit reached |
| `404` | `not_found` | Resource not found |
| `429` | `rate_limit_exceeded` | Too many requests |
| `500` | `internal_error` | Unexpected server error |
| `502` | `provider_error` | Upstream provider failure |
| `503` | `service_unavailable` | Gateway overloaded or provider unavailable |

---

## Routing Strategies

Pass `gateway_routing_strategy` in the chat completion request body:

| Strategy | Behavior |
|----------|----------|
| `privacy-first` | Prefer on-premise / local models (Ollama) |
| `balanced` | Cost/quality tradeoff |
| `speed` | Lowest latency provider |
| `frugal` | Cheapest provider |
| `quality` | Best model available |
| `offline` | Only local models, no external APIs |

---

## SDK Compatibility

Any OpenAI-compatible SDK works by changing the base URL:

### Python

```python
from openai import OpenAI

client = OpenAI(
    base_url="http://localhost:8080/v1",
    api_key="sk_gw_1234567890..."  # Your gateway API key
)

response = client.chat.completions.create(
    model="gpt-4o-mini",
    messages=[{"role": "user", "content": "Hello!"}]
)
```

### Node.js

```javascript
import OpenAI from 'openai';

const client = new OpenAI({
  baseURL: 'http://localhost:8080/v1',
  apiKey: 'sk_gw_1234567890...',
});

const response = await client.chat.completions.create({
  model: 'gpt-4o-mini',
  messages: [{ role: 'user', content: 'Hello!' }],
});
```

---

---

## SSO Authentication (Enterprise)

### OIDC

#### Initiate Login

```bash
GET /api/v1/auth/oidc/authorize?org_id={org_id}
```

Generates a random `state` nonce, stores it in Redis (10-minute TTL), and redirects the browser to the configured OIDC identity provider. The user authenticates with the IdP and is redirected back to the callback URL.

#### Callback

```bash
GET /api/v1/auth/oidc/callback?code={code}&state={state}
```

Verifies the `state` parameter against Redis (one-time use), exchanges the authorization code for tokens, provisions the user if necessary, and redirects to the configured `allowed_origins` URL.

### SAML 2.0

#### Initiate Login

```bash
GET /api/v1/auth/saml/authorize?org_id={org_id}
```

Generates a random `RelayState` nonce, stores it in Redis (10-minute TTL), and redirects the browser to the configured SAML identity provider with an AuthnRequest.

#### Assertion Consumer Service (ACS)

```bash
POST /api/v1/auth/saml/acs
Content-Type: application/x-www-form-urlencoded

SAMLResponse={base64_response}&RelayState={relay_state}
```

Verifies the `RelayState` against Redis (one-time use), parses the SAML assertion, provisions the user if necessary, and redirects to the configured `allowed_origins` URL.

### SSO Admin Configuration

All SSO admin endpoints require a JWT session with `settings:read` or `settings:write` permission. The path `org_id` must match the caller's active organization.

| Method | Endpoint | Permission | Description |
|--------|----------|------------|-------------|
| GET | `/api/v1/organizations/:org_id/sso` | `settings:read` | List configured SSO providers |
| POST | `/api/v1/organizations/:org_id/sso` | `settings:write` | Create or update SSO config |
| DELETE | `/api/v1/organizations/:org_id/sso/:provider_type` | `settings:write` | Delete SSO config (`saml` or `oidc`) |

For complete request/response schemas, webhook formats, and SSE streaming details, see **[API_SPEC.md](API_SPEC.md)**.
