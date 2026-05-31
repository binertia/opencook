# AI Gateway API Specification

## Document Information

| Field | Value |
|---|---|
| Version | 1.0.0 |
| Status | Stable |
| Base URL (Public AI API) | `https://gateway.example.com/v1` |
| Base URL (Admin API) | `https://gateway.example.com/api/v1` |
| Content-Type | `application/json` |

---

## Table of Contents

1. [API Design Principles](#1-api-design-principles)
2. [Public API Endpoints (OpenAI-Compatible)](#2-public-api-endpoints-openai-compatible)
3. [Gateway API Endpoints (Admin/Configuration)](#3-gateway-api-endpoints-adminconfiguration)
4. [Authentication](#4-authentication)
5. [Rate Limiting](#5-rate-limiting)
6. [Error Handling](#6-error-handling)
7. [Webhook Events](#7-webhook-events)
8. [SSE Streaming Format](#8-sse-streaming-format)

---

## 1. API Design Principles

### 1.1 OpenAI Compatibility

All public AI endpoints (`/v1/*`) follow the OpenAI API specification where applicable. Requests and responses match OpenAI's schema to ensure drop-in compatibility with existing SDKs and integrations.

- **Drop-in replacement**: Existing OpenAI client libraries work without modification for core features
- **Extended fields**: Gateway-specific fields are additive only (never modify existing OpenAI fields)
- **Ignored fields**: Unknown fields in requests are silently ignored (forward-compatible)

### 1.2 Extension Points

Gateway-specific extensions use these patterns:

| Extension Pattern | Location | Example |
|---|---|---|
| `X-Gateway-*` headers | HTTP request/response headers | `X-Gateway-Request-ID`, `X-Gateway-Provider` |
| `gateway_*` fields in request body | Top-level or inside `metadata` | `gateway_provider_hint`, `gateway_routing_strategy` |
| `gateway` object in responses | Top-level sibling to OpenAI fields | `gateway.provider_used`, `gateway.latency_ms` |
| `/api/v1/*` prefix | Admin/configuration endpoints | `/api/v1/organizations`, `/api/v1/providers` |

### 1.3 Versioning Strategy

| API Surface | Path | Versioning Method |
|---|---|---|
| Public AI API (OpenAI-compatible) | `/v1/*` | Path-based. Version bumped only on breaking OpenAI spec changes. |
| Admin/Gateway API | `/api/v1/*` | Path-based. Independent versioning from public API. |
| Future admin versions | `/api/v2/*` | Path-based. Deprecation period: 6 months with sunset headers. |

**Deprecation Headers** (sent on deprecated endpoints):

```
Deprecation: true
Sunset: Sat, 01 Nov 2025 00:00:00 GMT
Link: </api/v2/resource>; rel="successor-version"
```

### 1.4 Error Response Format

All endpoints return a consistent error envelope:

```json
{
  "error": {
    "code": "error_code_snake_case",
    "message": "Human-readable description",
    "type": "error_category",
    "param": "request_field_name_or_null",
    "status": 400,
    "request_id": "req_abc123"
  }
}
```

| Field | Type | Description |
|---|---|---|
| `error.code` | string | Machine-readable error code in snake_case |
| `error.message` | string | Human-readable description |
| `error.type` | string | Error category: `invalid_request_error`, `authentication_error`, `rate_limit_error`, `gateway_error`, `provider_error`, `not_found_error` |
| `error.param` | string \| null | Request field that caused the error, if applicable |
| `error.status` | integer | HTTP status code |
| `error.request_id` | string | Unique request ID for tracing |

### 1.5 Authentication Methods

| Method | Header | Used By |
|---|---|---|
| API Key | `Authorization: Bearer {api_key}` | Public AI API endpoints |
| Admin JWT | `Authorization: Bearer {admin_jwt_token}` | Admin/Gateway API endpoints |
| Webhook Secret | `X-Webhook-Secret: {webhook_secret}` | Webhook verification (incoming to subscriber) |

**Gateway-to-client response headers** (sent on all responses):

```
X-Gateway-Request-ID: req_abc123
X-Gateway-Version: 1.0.0
```

---

## 2. Public API Endpoints (OpenAI-Compatible)

All endpoints require `Authorization: Bearer {api_key}` header unless noted.

### 2.1 Chat Completions

#### `POST /v1/chat/completions`

Create a chat completion. Supports streaming and non-streaming responses.

**Auth Required**: Yes (API Key)

**Request Headers**:

| Header | Required | Description |
|---|---|---|
| `Authorization` | Yes | `Bearer {api_key}` |
| `Content-Type` | Yes | `application/json` |
| `Accept` | No | `text/event-stream` for streaming |
| `X-Gateway-Provider-Hint` | No | Preferred provider ID (e.g., `openai`, `anthropic`) |
| `X-Gateway-Routing-Strategy` | No | Override routing: `cost`, `latency`, `quality`, `fallback` |
| `X-Gateway-Model-Fallback` | No | Comma-separated fallback model IDs |

**Request Body Schema**:

```json
{
  "model": "string (required) -- Model ID or alias, e.g., 'gpt-4o', 'claude-3-sonnet'",
  "messages": [
    {
      "role": "system | user | assistant | tool",
      "content": "string | array of content parts",
      "name": "string (optional)",
      "tool_calls": [ ... ],
      "tool_call_id": "string"
    }
  ],
  "frequency_penalty": "number (-2.0 to 2.0, default: 0)",
  "logit_bias": "object {token_id: bias_value}",
  "logprobs": "boolean (default: false)",
  "top_logprobs": "integer (0 to 20)",
  "max_tokens": "integer",
  "max_completion_tokens": "integer",
  "n": "integer (default: 1, must be 1 for streaming)",
  "presence_penalty": "number (-2.0 to 2.0, default: 0)",
  "response_format": { "type": "text | json_object | json_schema" },
  "seed": "integer",
  "stop": "string | array of strings",
  "stream": "boolean (default: false)",
  "stream_options": { "include_usage": "boolean (default: false)" },
  "temperature": "number (0 to 2, default: 1)",
  "top_p": "number (0 to 1, default: 1)",
  "tools": [
    {
      "type": "function",
      "function": {
        "name": "string",
        "description": "string",
        "parameters": { "type": "object", "properties": {}, "required": [] }
      }
    }
  ],
  "tool_choice": "none | auto | required | {type: function, function: {name}}",
  "user": "string (end-user identifier for tracking)",
  "metadata": "object (custom key-value pairs passed through)",
  "gateway_provider_hint": "string (optional, preferred provider)",
  "gateway_routing_strategy": "string (cost | latency | quality | fallback)",
  "gateway_model_fallback": "array of strings (fallback model IDs)",
  "gateway_metadata": "object (gateway-specific key-value storage, max 16 keys, 512 bytes per value)"
}
```

**Non-Streaming Response Schema** (`200 OK`, `application/json`):

```json
{
  "id": "chatcmpl_abc123",
  "object": "chat.completion",
  "created": 1715000000,
  "model": "gpt-4o-2024-05-13",
  "choices": [
    {
      "index": 0,
      "message": {
        "role": "assistant",
        "content": "Hello! How can I help you today?",
        "tool_calls": null
      },
      "logprobs": null,
      "finish_reason": "stop"
    }
  ],
  "usage": {
    "prompt_tokens": 10,
    "completion_tokens": 20,
    "total_tokens": 30
  },
  "system_fingerprint": "fp_abc123",
  "gateway": {
    "request_id": "req_abc123",
    "provider_used": "openai",
    "model_routed_to": "gpt-4o-2024-05-13",
    "latency_ms": 450,
    "input_cost_usd": 0.00005,
    "output_cost_usd": 0.00030,
    "total_cost_usd": 0.00035,
    "cached_tokens": 0,
    "routing_strategy_applied": "quality"
  }
}
```

**Streaming Response Schema** (`200 OK`, `text/event-stream`):

SSE stream of `data:` lines. See [Section 8: SSE Streaming Format](#8-sse-streaming-format) for full details.

**Error Responses**:

| Status | Code | Description |
|---|---|---|
| 400 | `invalid_request_error` | Malformed request, invalid parameters |
| 401 | `authentication_error` | Invalid or missing API key |
| 403 | `insufficient_scope` | API key lacks permission for this model |
| 404 | `model_not_found` | Requested model not found or not available |
| 429 | `rate_limit_exceeded` | Rate limit exceeded (key, org, or global) |
| 429 | `quota_exceeded` | Organization token or budget quota exceeded |
| 500 | `gateway_error` | Internal gateway error |
| 502 | `provider_error` | Upstream provider error |
| 503 | `service_unavailable` | No healthy providers available |
| 504 | `provider_timeout` | Upstream provider timed out |

**Example cURL**:

```bash
curl -X POST https://gateway.example.com/v1/chat/completions \
  -H "Authorization: Bearer $API_KEY" \
  -H "Content-Type: application/json" \
  -d '{
    "model": "gpt-4o",
    "messages": [{"role": "user", "content": "Hello!"}],
    "temperature": 0.7
  }'
```

---

### 2.2 Embeddings

#### `POST /v1/embeddings`

Create vector embeddings for input text.

**Auth Required**: Yes (API Key)

**Request Headers**:

| Header | Required | Description |
|---|---|---|
| `Authorization` | Yes | `Bearer {api_key}` |
| `Content-Type` | Yes | `application/json` |

**Request Body Schema**:

```json
{
  "input": "string | array of strings (required, max 2048 items)",
  "model": "string (required, e.g., 'text-embedding-3-small', 'text-embedding-3-large')",
  "encoding_format": "string (float | base64, default: float)",
  "dimensions": "integer (optional, reduced embedding dimensions)",
  "user": "string (optional, end-user identifier)",
  "gateway_provider_hint": "string (optional)",
  "gateway_routing_strategy": "string (optional, cost | latency | quality)"
}
```

**Response Schema** (`200 OK`, `application/json`):

```json
{
  "object": "list",
  "data": [
    {
      "object": "embedding",
      "embedding": [0.0023064255, -0.009327292, ...],
      "index": 0
    }
  ],
  "model": "text-embedding-3-small",
  "usage": {
    "prompt_tokens": 8,
    "total_tokens": 8
  },
  "gateway": {
    "request_id": "req_abc123",
    "provider_used": "openai",
    "latency_ms": 120,
    "input_cost_usd": 0.000002,
    "total_cost_usd": 0.000002,
    "model_routed_to": "text-embedding-3-small"
  }
}
```

**Error Responses**: Same error codes as Chat Completions.

---

### 2.3 Models List

#### `GET /v1/models`

List all available models across configured providers.

**Auth Required**: Yes (API Key)

**Request Headers**:

| Header | Required | Description |
|---|---|---|
| `Authorization` | Yes | `Bearer {api_key}` |

**Query Parameters**:

| Parameter | Type | Description |
|---|---|---|
| `provider` | string | Filter by provider ID |
| `capability` | string | Filter by capability: `chat`, `embeddings`, `images`, `audio` |

**Response Schema** (`200 OK`, `application/json`):

```json
{
  "object": "list",
  "data": [
    {
      "id": "gpt-4o",
      "object": "model",
      "created": 1715000000,
      "owned_by": "openai",
      "gateway": {
        "provider_id": "openai",
        "provider_name": "OpenAI",
        "capabilities": ["chat", "vision", "json_mode", "function_calling"],
        "context_window": 128000,
        "pricing": {
          "input_per_1m_tokens": 2.50,
          "output_per_1m_tokens": 10.00,
          "currency": "USD"
        },
        "health_status": "healthy",
        "latency_p50_ms": 350,
        "aliases": ["gpt-4o-latest"]
      }
    }
  ]
}
```

#### `GET /v1/models/{model}`

Retrieve a specific model.

**Auth Required**: Yes (API Key)

**Response Schema**: Single model object (same shape as items in list response).

---

## 3. Gateway API Endpoints (Admin/Configuration)

All endpoints require `Authorization: Bearer {admin_jwt_token}` header.
Base path: `/api/v1`

### 3.1 Organizations

#### `POST /api/v1/organizations` — Create Organization

**Auth Required**: Yes (Admin JWT with `organizations:write` scope)

**Request Body**:

```json
{
  "name": "string (required, 1-128 chars)",
  "display_name": "string (optional, 1-256 chars)",
  "description": "string (optional, max 1024 chars)",
  "metadata": "object (optional, max 16 keys, 1024 bytes per value)",
  "settings": {
    "default_routing_strategy": "string (cost | latency | quality | fallback, default: quality)",
    "allowed_providers": ["string array (empty = all)"],
    "blocked_models": ["string array"],
    "token_budget": {
      "monthly_limit": "integer (max tokens per month, null = unlimited)",
      "cost_budget_usd": "number (max monthly spend, null = unlimited)",
      "alert_threshold_percent": "integer (0-100, when to fire alert)"
    }
  }
}
```

**Response** (`201 Created`):

```json
{
  "id": "org_abc123",
  "name": "acme-corp",
  "display_name": "Acme Corporation",
  "description": "Primary engineering organization",
  "metadata": {},
  "settings": {
    "default_routing_strategy": "quality",
    "allowed_providers": [],
    "blocked_models": [],
    "token_budget": {
      "monthly_limit": null,
      "cost_budget_usd": 1000.00,
      "alert_threshold_percent": 80
    }
  },
  "created_at": "2024-05-06T12:00:00Z",
  "updated_at": "2024-05-06T12:00:00Z",
  "created_by": "user_admin123",
  "status": "active"
}
```

**Error Responses**:

| Status | Code | Description |
|---|---|---|
| 400 | `invalid_request_error` | Invalid name format or settings |
| 409 | `organization_already_exists` | Name already in use |
| 403 | `insufficient_scope` | Missing `organizations:write` permission |

---

#### `GET /api/v1/organizations` — List Organizations

**Auth Required**: Yes (Admin JWT with `organizations:read` scope)

**Query Parameters**:

| Parameter | Type | Default | Description |
|---|---|---|---|
| `limit` | integer | 20 | Max items per page (1-100) |
| `offset` | integer | 0 | Pagination offset |
| `status` | string | all | Filter: `active`, `suspended`, `all` |
| `search` | string | - | Search by name or display_name |

**Response** (`200 OK`):

```json
{
  "object": "list",
  "data": [ { ...organization object... } ],
  "pagination": {
    "limit": 20,
    "offset": 0,
    "total": 150,
    "has_more": true
  }
}
```

---

#### `GET /api/v1/organizations/{id}` — Get Organization

**Auth Required**: Yes (Admin JWT with `organizations:read` scope, or member of the org)

**Response** (`200 OK`): Single organization object.

**Error Responses**:

| Status | Code | Description |
|---|---|---|
| 404 | `organization_not_found` | Organization does not exist |

---

#### `PUT /api/v1/organizations/{id}` — Update Organization

**Auth Required**: Yes (Admin JWT with `organizations:write` scope)

**Request Body**: Partial organization object. All top-level fields are optional; omitted fields are not modified. `settings` object is merged (not replaced).

**Response** (`200 OK`): Updated organization object.

---

#### `DELETE /api/v1/organizations/{id}` — Delete Organization

**Auth Required**: Yes (Admin JWT with `organizations:write` scope)

**Response** (`204 No Content`)

**Error Responses**:

| Status | Code | Description |
|---|---|---|
| 409 | `organization_has_keys` | Cannot delete org with active API keys. Revoke keys first. |
| 409 | `organization_has_users` | Cannot delete org with active users. Remove users first. |

---

### 3.2 API Keys

#### `POST /api/v1/organizations/{org_id}/keys` — Create API Key

**Auth Required**: Yes (Admin JWT with `keys:write` scope for the org)

**Request Body**:

```json
{
  "name": "string (required, 1-128 chars, descriptive name)",
  "scopes": ["string array (required, see scope list below)"],
  "allowed_models": ["string array (optional, empty = all models)"],
  "allowed_ips": ["string array (optional, CIDR notation, e.g., ['10.0.0.0/8'])"],
  "rate_limit": {
    "requests_per_minute": "integer (optional, default: org tier limit)",
    "tokens_per_minute": "integer (optional, default: org tier limit)",
    "requests_per_day": "integer (optional)"
  },
  "expires_at": "string (ISO 8601 datetime, optional, null = no expiry)",
  "metadata": "object (optional)"
}
```

**Available Scopes**:

| Scope | Description |
|---|---|
| `chat:write` | Create chat completions |
| `embeddings:write` | Create embeddings |
| `models:read` | List available models |
| `usage:read` | Read own usage data |
| `admin:read` | Read admin data (full org) |
| `admin:write` | Write admin data (full org) |

**Response** (`201 Created`):

```json
{
  "id": "key_abc123",
  "name": "Production API Key",
  "org_id": "org_abc123",
  "key_prefix": "sk-ag...x7k",
  "key_full": "sk-ag-abc123def456 (shown ONLY on creation)",
  "scopes": ["chat:write", "embeddings:write", "models:read"],
  "allowed_models": [],
  "allowed_ips": [],
  "rate_limit": {
    "requests_per_minute": 100,
    "tokens_per_minute": 100000,
    "requests_per_day": null
  },
  "expires_at": null,
  "created_at": "2024-05-06T12:00:00Z",
  "created_by": "user_admin123",
  "last_used_at": null,
  "usage_count": 0,
  "status": "active",
  "metadata": {}
}
```

> **Security Note**: `key_full` is returned **only on creation**. It cannot be retrieved later.

---

#### `GET /api/v1/organizations/{org_id}/keys` — List API Keys

**Auth Required**: Yes (Admin JWT with `keys:read` scope for the org)

**Query Parameters**:

| Parameter | Type | Default | Description |
|---|---|---|---|
| `limit` | integer | 20 | Max items per page (1-100) |
| `offset` | integer | 0 | Pagination offset |
| `status` | string | all | Filter: `active`, `revoked`, `expired`, `all` |

**Response** (`200 OK`): List of key objects (without `key_full` field).

---

#### `DELETE /api/v1/organizations/{org_id}/keys/{key_id}` — Revoke API Key

**Auth Required**: Yes (Admin JWT with `keys:write` scope for the org)

**Response** (`204 No Content`)

Revocation is immediate. The key becomes invalid within 5 seconds across all gateway instances.

**Error Responses**:

| Status | Code | Description |
|---|---|---|
| 404 | `key_not_found` | Key not found in organization |

---

### 3.3 Users

#### `POST /api/v1/users` — Create User

**Auth Required**: Yes (Admin JWT with `users:write` scope)

**Request Body**:

```json
{
  "email": "string (required, valid email)",
  "name": "string (required, 1-128 chars)",
  "role": "string (required, admin | member | viewer)",
  "organization_ids": ["string array (optional, org memberships)"],
  "password": "string (required on creation, min 12 chars)",
  "metadata": "object (optional)"
}
```

**Response** (`201 Created`):

```json
{
  "id": "user_abc123",
  "email": "admin@example.com",
  "name": "Admin User",
  "role": "admin",
  "organizations": [
    {
      "org_id": "org_abc123",
      "org_name": "acme-corp",
      "role": "admin"
    }
  ],
  "created_at": "2024-05-06T12:00:00Z",
  "updated_at": "2024-05-06T12:00:00Z",
  "last_login_at": null,
  "status": "active"
}
```

---

#### `GET /api/v1/users` — List Users

**Auth Required**: Yes (Admin JWT with `users:read` scope)

**Query Parameters**:

| Parameter | Type | Default | Description |
|---|---|---|---|
| `limit` | integer | 20 | Max items per page |
| `offset` | integer | 0 | Pagination offset |
| `role` | string | all | Filter: `admin`, `member`, `viewer` |
| `org_id` | string | - | Filter by organization membership |
| `status` | string | active | Filter: `active`, `inactive`, `all` |
| `search` | string | - | Search by email or name |

**Response** (`200 OK`): List of user objects.

---

#### `GET /api/v1/users/{id}` — Get User

**Auth Required**: Yes (Admin JWT with `users:read` scope, or own user record)

**Response** (`200 OK`): Single user object.

---

#### `PUT /api/v1/users/{id}` — Update User

**Auth Required**: Yes (Admin JWT with `users:write` scope, or own user record for limited fields)

**Request Body**: Partial user object. Users can update their own `name` and `password` only. Admins can update all fields.

**Response** (`200 OK`): Updated user object.

---

#### `DELETE /api/v1/users/{id}` — Delete User

**Auth Required**: Yes (Admin JWT with `users:write` scope)

**Response** (`204 No Content`)

---

### 3.4 Providers

#### `GET /api/v1/providers` — List Configured Providers

**Auth Required**: Yes (Admin JWT with `providers:read` scope)

**Response** (`200 OK`):

```json
{
  "object": "list",
  "data": [
    {
      "id": "openai",
      "name": "OpenAI",
      "type": "openai_compatible",
      "base_url": "https://api.openai.com/v1",
      "models": [
        {
          "id": "gpt-4o",
          "name": "GPT-4o",
          "context_window": 128000,
          "capabilities": ["chat", "vision", "json_mode", "function_calling"],
          "pricing": {
            "input_per_1m_tokens": 2.50,
            "output_per_1m_tokens": 10.00,
            "currency": "USD"
          },
          "status": "active"
        }
      ],
      "health": {
        "status": "healthy",
        "last_checked_at": "2024-05-06T12:00:00Z",
        "latency_p50_ms": 350,
        "latency_p99_ms": 1200,
        "error_rate_1h": 0.001
      },
      "routing_weight": 100,
      "priority": 1,
      "created_at": "2024-01-01T00:00:00Z",
      "updated_at": "2024-05-06T12:00:00Z"
    }
  ]
}
```

---

#### `POST /api/v1/providers` — Add Provider Configuration

**Auth Required**: Yes (Admin JWT with `providers:write` scope)

**Request Body**:

```json
{
  "id": "string (required, unique identifier, e.g., 'openai')",
  "name": "string (required, display name)",
  "type": "string (required, openai_compatible | anthropic | azure_openai | custom)",
  "base_url": "string (required, provider API base URL)",
  "api_key": "string (required, provider API key, encrypted at rest)",
  "api_key_header": "string (optional, default: Authorization)",
  "api_key_prefix": "string (optional, default: Bearer)",
  "models": [
    {
      "id": "string (provider model ID)",
      "aliases": ["string array (gateway-facing aliases)"],
      "context_window": "integer",
      "capabilities": ["chat", "vision", "embeddings", "json_mode", "function_calling"],
      "pricing": {
        "input_per_1m_tokens": "number",
        "output_per_1m_tokens": "number",
        "currency": "string (default: USD)"
      },
      "enabled": "boolean (default: true)"
    }
  ],
  "health_check": {
    "enabled": "boolean (default: true)",
    "interval_seconds": "integer (default: 60)",
    "timeout_seconds": "integer (default: 10)",
    "model": "string (model to use for health check probes)"
  },
  "routing_weight": "integer (1-1000, default: 100)",
  "priority": "integer (1-100, lower = higher priority, default: 50)",
  "request_timeout_seconds": "integer (default: 120)",
  "retry_policy": {
    "max_retries": "integer (default: 3)",
    "retry_on_status": ["integer array (default: [502, 503, 504])"],
    "backoff_ms": "integer (default: 1000)"
  },
  "transforms": {
    "request_headers": "object (header overrides)",
    "response_headers": "object (header overrides)"
  }
}
```

**Response** (`201 Created`): Provider object (with `api_key` redacted to `***`).

---

#### `GET /api/v1/providers/{id}` — Get Provider

**Auth Required**: Yes (Admin JWT with `providers:read` scope)

**Response** (`200 OK`): Single provider object.

---

#### `PUT /api/v1/providers/{id}` — Update Provider

**Auth Required**: Yes (Admin JWT with `providers:write` scope)

**Request Body**: Partial provider object. `api_key` is only updated if provided (otherwise kept unchanged).

**Response** (`200 OK`): Updated provider object.

---

#### `DELETE /api/v1/providers/{id}` — Delete Provider

**Auth Required**: Yes (Admin JWT with `providers:write` scope)

**Response** (`204 No Content`)

**Error Responses**:

| Status | Code | Description |
|---|---|---|
| 409 | `provider_in_use` | Provider is referenced by active routing rules |

---

#### `POST /api/v1/providers/{id}/health-check` — Trigger Health Check

**Auth Required**: Yes (Admin JWT with `providers:write` scope)

**Response** (`200 OK`):

```json
{
  "provider_id": "openai",
  "checked_at": "2024-05-06T12:00:00Z",
  "status": "healthy",
  "latency_ms": 345,
  "details": "All probes passed"
}
```

---

#### `GET /api/v1/providers/{id}/health-history` — Health Check History

**Auth Required**: Yes (Admin JWT with `providers:read` scope)

**Query Parameters**:

| Parameter | Type | Default | Description |
|---|---|---|---|
| `hours` | integer | 24 | Lookback period (1-168) |

**Response** (`200 OK`):

```json
{
  "object": "list",
  "data": [
    {
      "checked_at": "2024-05-06T12:00:00Z",
      "status": "healthy",
      "latency_ms": 345,
      "error": null
    },
    {
      "checked_at": "2024-05-06T11:59:00Z",
      "status": "degraded",
      "latency_ms": 2500,
      "error": "Latency above threshold"
    }
  ]
}
```

---

### 3.5 Routing Rules

#### `GET /api/v1/routing-rules` — List Routing Rules

**Auth Required**: Yes (Admin JWT with `routing:read` scope)

**Response** (`200 OK`):

```json
{
  "object": "list",
  "data": [
    {
      "id": "rule_abc123",
      "name": "Cost-Optimized Routing",
      "description": "Route to cheapest provider meeting quality threshold",
      "priority": 10,
      "enabled": true,
      "criteria": {
        "models": ["gpt-4o", "gpt-4o-mini"],
        "organizations": [],
        "request_types": ["chat.completion"]
      },
      "strategy": {
        "type": "cost",
        "fallback_enabled": true,
        "providers": [
          { "provider_id": "openai", "weight": 100 },
          { "provider_id": "azure_openai", "weight": 80 }
        ]
      },
      "constraints": {
        "max_latency_ms": 5000,
        "require_health_check": true,
        "require_capabilities": ["function_calling"]
      },
      "created_at": "2024-05-06T12:00:00Z",
      "updated_at": "2024-05-06T12:00:00Z"
    }
  ]
}
```

**Strategy Types**:

| Type | Description |
|---|---|
| `cost` | Route to lowest-cost provider meeting constraints |
| `latency` | Route to lowest-latency healthy provider |
| `quality` | Route to highest-quality provider |
| `weighted` | Distribute by configured weights |
| `priority` | Try providers in order until success |
| `fallback` | Use primary, fallback on failure |

---

#### `POST /api/v1/routing-rules` — Create Routing Rule

**Auth Required**: Yes (Admin JWT with `routing:write` scope)

**Request Body**:

```json
{
  "name": "string (required, 1-128 chars)",
  "description": "string (optional)",
  "priority": "integer (required, 1-1000, lower = evaluated first)",
  "enabled": "boolean (default: true)",
  "criteria": {
    "models": ["string array (model IDs/aliases, empty = all)"],
    "organizations": ["string array (org IDs, empty = all)"],
    "request_types": ["chat.completion | embeddings.create"]
  },
  "strategy": {
    "type": "cost | latency | quality | weighted | priority | fallback",
    "fallback_enabled": "boolean (default: true)",
    "providers": [
      {
        "provider_id": "string",
        "weight": "integer (1-1000, for weighted strategy)"
      }
    ]
  },
  "constraints": {
    "max_latency_ms": "integer (optional)",
    "require_health_check": "boolean (default: true)",
    "require_capabilities": ["string array"]
  }
}
```

**Response** (`201 Created`): Routing rule object.

---

#### `GET /api/v1/routing-rules/{id}` — Get Routing Rule

**Auth Required**: Yes (Admin JWT with `routing:read` scope)

**Response** (`200 OK`): Single routing rule object.

---

#### `PUT /api/v1/routing-rules/{id}` — Update Routing Rule

**Auth Required**: Yes (Admin JWT with `routing:write` scope)

**Request Body**: Partial routing rule object.

**Response** (`200 OK`): Updated routing rule object.

---

#### `DELETE /api/v1/routing-rules/{id}` — Delete Routing Rule

**Auth Required**: Yes (Admin JWT with `routing:write` scope)

**Response** (`204 No Content`)

---

### 3.6 Quotas / Budgets

#### `GET /api/v1/organizations/{org_id}/quotas` — List Quota Rules

**Auth Required**: Yes (Admin JWT with `quotas:read` scope for the org)

**Response** (`200 OK`):

```json
{
  "object": "list",
  "data": [
    {
      "id": "quota_abc123",
      "org_id": "org_abc123",
      "name": "Monthly Token Limit",
      "type": "token_limit",
      "scope": "organization",
      "limit": 100000000,
      "window": "1 month",
      "action": "block_with_alert",
      "usage_current": 45000000,
      "usage_remaining": 55000000,
      "alert_threshold_percent": 80,
      "alert_triggered": false,
      "created_at": "2024-05-06T12:00:00Z",
      "updated_at": "2024-05-06T12:00:00Z"
    }
  ]
}
```

**Quota Types**:

| Type | Description |
|---|---|
| `token_limit` | Maximum tokens consumed in a time window |
| `cost_limit` | Maximum spend in a time window |
| `request_limit` | Maximum number of requests in a time window |
| `rate_limit` | Maximum requests/tokens per minute |

**Window Values**: `1 minute`, `1 hour`, `1 day`, `7 days`, `1 month`

**Actions**: `block` (reject), `block_with_alert`, `alert_only` (allow but notify), `throttle` (reduce rate)

---

#### `POST /api/v1/organizations/{org_id}/quotas` — Create Quota Rule

**Auth Required**: Yes (Admin JWT with `quotas:write` scope)

**Request Body**:

```json
{
  "name": "string (required)",
  "type": "token_limit | cost_limit | request_limit | rate_limit",
  "scope": "organization | api_key | user",
  "scope_id": "string (optional, ID of the scoped entity)",
  "limit": "integer | number (required)",
  "window": "string (required, see window values above)",
  "action": "block | block_with_alert | alert_only | throttle",
  "alert_threshold_percent": "integer (0-100, default: 80)"
}
```

**Response** (`201 Created`): Quota rule object.

---

#### `GET /api/v1/organizations/{org_id}/quotas/{quota_id}` — Get Quota Rule

**Auth Required**: Yes (Admin JWT with `quotas:read` scope)

**Response** (`200 OK`): Single quota rule object.

---

#### `PUT /api/v1/organizations/{org_id}/quotas/{quota_id}` — Update Quota Rule

**Auth Required**: Yes (Admin JWT with `quotas:write` scope)

**Request Body**: Partial quota rule object.

**Response** (`200 OK`): Updated quota rule object.

---

#### `DELETE /api/v1/organizations/{org_id}/quotas/{quota_id}` — Delete Quota Rule

**Auth Required**: Yes (Admin JWT with `quotas:write` scope)

**Response** (`204 No Content`)

---

### 3.7 Usage / Analytics

#### `GET /api/v1/organizations/{org_id}/usage` — Usage Data

**Auth Required**: Yes (Admin JWT with `usage:read` scope for the org, or API key with `usage:read`)

**Query Parameters**:

| Parameter | Type | Default | Description |
|---|---|---|---|
| `start_time` | string (ISO 8601) | 7 days ago | Start of time range |
| `end_time` | string (ISO 8601) | now | End of time range |
| `granularity` | string | `1 day` | `1 minute`, `1 hour`, `1 day`, `7 days`, `1 month` |
| `group_by` | string | `day` | `minute`, `hour`, `day`, `model`, `provider`, `api_key` |
| `model` | string | all | Filter by model ID |
| `provider` | string | all | Filter by provider ID |
| `api_key_id` | string | all | Filter by API key |

**Response** (`200 OK`):

```json
{
  "object": "list",
  "granularity": "1 day",
  "start_time": "2024-05-01T00:00:00Z",
  "end_time": "2024-05-07T23:59:59Z",
  "data": [
    {
      "timestamp": "2024-05-01T00:00:00Z",
      "requests": 1523,
      "prompt_tokens": 450000,
      "completion_tokens": 890000,
      "total_tokens": 1340000,
      "input_cost_usd": 0.45,
      "output_cost_usd": 2.67,
      "total_cost_usd": 3.12,
      "avg_latency_ms": 420,
      "p99_latency_ms": 1800,
      "errors": 3,
      "model_breakdown": {
        "gpt-4o": { "requests": 800, "tokens": 700000, "cost_usd": 2.50 },
        "gpt-4o-mini": { "requests": 723, "tokens": 640000, "cost_usd": 0.62 }
      }
    }
  ],
  "summary": {
    "total_requests": 10661,
    "total_tokens": 9380000,
    "total_cost_usd": 21.84,
    "avg_latency_ms": 415,
    "total_errors": 12
  }
}
```

---

#### `GET /api/v1/organizations/{org_id}/costs` — Cost Data

**Auth Required**: Yes (Admin JWT with `usage:read` scope)

**Query Parameters**: Same as usage endpoint.

**Response** (`200 OK`):

```json
{
  "object": "list",
  "start_time": "2024-05-01T00:00:00Z",
  "end_time": "2024-05-07T23:59:59Z",
  "data": [
    {
      "timestamp": "2024-05-01T00:00:00Z",
      "input_cost_usd": 0.45,
      "output_cost_usd": 2.67,
      "total_cost_usd": 3.12,
      "by_provider": {
        "openai": { "cost_usd": 3.12, "tokens": 1340000 }
      },
      "by_model": {
        "gpt-4o": { "cost_usd": 2.50, "tokens": 700000 },
        "gpt-4o-mini": { "cost_usd": 0.62, "tokens": 640000 }
      }
    }
  ],
  "summary": {
    "total_cost_usd": 21.84,
    "budget_limit_usd": 1000.00,
    "budget_used_percent": 2.18,
    "projected_monthly_cost_usd": 93.60
  }
}
```

---

#### `GET /api/v1/organizations/{org_id}/requests` — Request Logs

**Auth Required**: Yes (Admin JWT with `usage:read` scope)

**Query Parameters**:

| Parameter | Type | Default | Description |
|---|---|---|---|
| `limit` | integer | 20 | Max items (1-100) |
| `offset` | integer | 0 | Pagination offset |
| `start_time` | string (ISO 8601) | 1 hour ago | Start of time range |
| `end_time` | string (ISO 8601) | now | End of time range |
| `model` | string | all | Filter by model |
| `provider` | string | all | Filter by provider |
| `status` | string | all | Filter: `success`, `error`, `cached` |
| `api_key_id` | string | all | Filter by API key |

**Response** (`200 OK`):

```json
{
  "object": "list",
  "data": [
    {
      "id": "req_abc123",
      "timestamp": "2024-05-06T12:00:00Z",
      "api_key_id": "key_abc123",
      "org_id": "org_abc123",
      "model": "gpt-4o",
      "provider": "openai",
      "type": "chat.completion",
      "status": "success",
      "prompt_tokens": 45,
      "completion_tokens": 128,
      "total_tokens": 173,
      "input_cost_usd": 0.00011,
      "output_cost_usd": 0.00128,
      "total_cost_usd": 0.00139,
      "latency_ms": 420,
      "streaming": false,
      "finish_reason": "stop",
      "error_code": null,
      "error_message": null,
      "gateway_request_id": "req_abc123",
      "cached": false,
      "routing_rule_id": "rule_abc123"
    }
  ],
  "pagination": {
    "limit": 20,
    "offset": 0,
    "total": 15420,
    "has_more": true
  }
}
```

---

### 3.8 Webhooks

#### `POST /api/v1/webhooks` — Register Webhook

**Auth Required**: Yes (Admin JWT with `webhooks:write` scope)

**Request Body**:

```json
{
  "url": "string (required, HTTPS URL, max 2048 chars)",
  "description": "string (optional)",
  "events": ["string array (required, see event types below)"],
  "secret": "string (optional, auto-generated if not provided, min 32 chars)",
  "active": "boolean (default: true)",
  "metadata": "object (optional)"
}
```

**Event Types** (see [Section 7](#7-webhook-events) for payload schemas):

| Event Type | Description |
|---|---|
| `request.completed` | AI request completed successfully |
| `request.failed` | AI request failed |
| `budget.threshold_reached` | Budget threshold exceeded |
| `provider.error` | Provider error detected |
| `key.created` | API key created |
| `key.revoked` | API key revoked |
| `provider.health_changed` | Provider health status changed |
| `quota.threshold_reached` | Quota threshold reached |

**Response** (`201 Created`):

```json
{
  "id": "whk_abc123",
  "url": "https://example.com/webhooks/gateway",
  "description": "Production webhook endpoint",
  "events": ["request.completed", "request.failed"],
  "secret": "whsec_xxx (shown ONLY on creation)",
  "active": true,
  "created_at": "2024-05-06T12:00:00Z",
  "updated_at": "2024-05-06T12:00:00Z",
  "last_delivered_at": null,
  "delivery_stats": {
    "total_delivered": 0,
    "total_failed": 0,
    "last_status": null
  }
}
```

---

#### `GET /api/v1/webhooks` — List Webhooks

**Auth Required**: Yes (Admin JWT with `webhooks:read` scope)

**Query Parameters**:

| Parameter | Type | Default | Description |
|---|---|---|---|
| `limit` | integer | 20 | Max items |
| `offset` | integer | 0 | Pagination offset |
| `active` | boolean | all | Filter by active status |
| `event` | string | all | Filter by event type |

**Response** (`200 OK`): List of webhook objects (without `secret` field).

---

#### `GET /api/v1/webhooks/{id}` — Get Webhook

**Auth Required**: Yes (Admin JWT with `webhooks:read` scope)

**Response** (`200 OK`): Single webhook object (without `secret`).

---

#### `PUT /api/v1/webhooks/{id}` — Update Webhook

**Auth Required**: Yes (Admin JWT with `webhooks:write` scope)

**Request Body**: Partial webhook object. `secret` can be regenerated by passing `regenerate_secret: true`.

**Response** (`200 OK`): Updated webhook object.

---

#### `DELETE /api/v1/webhooks/{id}` — Delete Webhook

**Auth Required**: Yes (Admin JWT with `webhooks:write` scope)

**Response** (`204 No Content`)

---

#### `POST /api/v1/webhooks/{id}/test` — Test Webhook

**Auth Required**: Yes (Admin JWT with `webhooks:write` scope)

Sends a test payload to the webhook URL.

**Response** (`200 OK`):

```json
{
  "success": true,
  "status_code": 200,
  "response_time_ms": 245,
  "message": "Webhook delivered successfully"
}
```

---

#### `GET /api/v1/webhooks/{id}/deliveries` — Delivery History

**Auth Required**: Yes (Admin JWT with `webhooks:read` scope)

**Query Parameters**:

| Parameter | Type | Default | Description |
|---|---|---|---|
| `limit` | integer | 20 | Max items |
| `offset` | integer | 0 | Pagination offset |
| `status` | string | all | `delivered`, `failed`, `pending` |

**Response** (`200 OK`):

```json
{
  "object": "list",
  "data": [
    {
      "id": "whd_abc123",
      "event_type": "request.completed",
      "payload_size_bytes": 512,
      "status": "delivered",
      "http_status": 200,
      "response_time_ms": 245,
      "attempts": 1,
      "delivered_at": "2024-05-06T12:00:00Z",
      "error": null
    }
  ]
}
```

---

## 4. Authentication

### 4.1 API Key Authentication (AI API)

Used for all `/v1/*` endpoints (chat completions, embeddings, models).

**Header**: `Authorization: Bearer {api_key}`

**API Key Format**: `sk-ag-{base58_encoded_24_bytes}`

Example: `sk-ag-3J98t1WpMZ1bBG2j4v9xQ7yK`

**Validation Flow**:

1. Extract token from `Authorization: Bearer <token>` header
2. Validate prefix (`sk-ag-`)
3. Look up key in cache (Redis, 30s TTL)
4. Verify key status is `active`
5. Check expiry (`expires_at` > now)
6. Verify IP allowlist (if `allowed_ips` is non-empty)
7. Check organization status is `active`
8. Enforce rate limits (see [Section 5](#5-rate-limiting))
9. Attach key metadata to request context for downstream use

**Scope/Permission Model**:

| Endpoint | Required Scope |
|---|---|
| `POST /v1/chat/completions` | `chat:write` |
| `POST /v1/embeddings` | `embeddings:write` |
| `GET /v1/models` | `models:read` |
| `GET /api/v1/organizations/{id}/usage` | `usage:read` |

Keys without the required scope receive `403 insufficient_scope`.

---

### 4.2 Session Authentication (Admin Dashboard)

JWT-based authentication for admin API and dashboard.

#### `POST /api/v1/auth/login` — Login

**Auth Required**: No

**Request Body**:

```json
{
  "email": "string (required)",
  "password": "string (required)",
  "totp_code": "string (optional, if 2FA enabled)"
}
```

**Response** (`200 OK`):

```json
{
  "access_token": "eyJhbGciOiJSUzI1NiIs...",
  "refresh_token": "eyJhbGciOiJSUzI1NiIs...",
  "token_type": "Bearer",
  "expires_in": 3600,
  "user": {
    "id": "user_abc123",
    "email": "admin@example.com",
    "name": "Admin User",
    "role": "admin"
  }
}
```

**Error Responses**:

| Status | Code | Description |
|---|---|---|
| 401 | `invalid_credentials` | Wrong email or password |
| 401 | `totp_required` | 2FA code required |
| 429 | `too_many_attempts` | Account locked after failed attempts |

---

#### `POST /api/v1/auth/refresh` — Refresh Token

**Auth Required**: No (uses refresh token)

**Request Body**:

```json
{
  "refresh_token": "string (required)"
}
```

**Response** (`200 OK`): New `access_token` and `refresh_token` pair.

**Error Responses**:

| Status | Code | Description |
|---|---|---|
| 401 | `invalid_refresh_token` | Token expired or revoked |

---

#### `POST /api/v1/auth/logout` — Logout

**Auth Required**: Yes (Bearer token)

Invalidates the current access token and refresh token.

**Response** (`204 No Content`)

---

#### `GET /api/v1/auth/me` — Current User

**Auth Required**: Yes (Bearer token)

**Response** (`200 OK`):

```json
{
  "id": "user_abc123",
  "email": "admin@example.com",
  "name": "Admin User",
  "role": "admin",
  "organizations": [
    { "org_id": "org_abc123", "org_name": "acme-corp", "role": "admin" }
  ],
  "permissions": [
    "organizations:read",
    "organizations:write",
    "keys:read",
    "keys:write",
    "providers:read",
    "providers:write",
    "routing:read",
    "routing:write",
    "quotas:read",
    "quotas:write",
    "usage:read",
    "users:read",
    "users:write",
    "webhooks:read",
    "webhooks:write"
  ],
  "session": {
    "issued_at": "2024-05-06T12:00:00Z",
    "expires_at": "2024-05-06T13:00:00Z",
    "ip_address": "10.0.0.1",
    "user_agent": "Mozilla/5.0..."
  }
}
```

---

### 4.3 JWT Token Specification

**Access Token Claims**:

| Claim | Type | Description |
|---|---|---|
| `sub` | string | User ID (`user_xxx`) |
| `iss` | string | `"gateway"` |
| `aud` | string | `"gateway-admin"` |
| `iat` | integer | Issued at (unix timestamp) |
| `exp` | integer | Expiration (unix timestamp, +1 hour) |
| `jti` | string | Unique token ID for revocation |
| `role` | string | User role: `admin`, `member`, `viewer` |
| `org_ids` | string[] | Organization IDs user belongs to |
| `perms` | string[] | Granted permission strings |

**Refresh Token Claims**:

| Claim | Type | Description |
|---|---|---|
| `sub` | string | User ID |
| `iss` | string | `"gateway"` |
| `iat` | integer | Issued at |
| `exp` | integer | Expiration (+7 days) |
| `jti` | string | Unique token ID |
| `type` | string | `"refresh"` |

---

## 5. Rate Limiting

### 5.1 Rate Limit Headers

All API responses include these headers:

| Header | Description | Example |
|---|---|---|
| `X-RateLimit-Limit` | Maximum allowed requests in the current window | `100` |
| `X-RateLimit-Remaining` | Remaining requests in current window | `87` |
| `X-RateLimit-Reset` | Unix timestamp when the limit resets | `1715004000` |
| `X-RateLimit-Policy` | The applied rate limit policy | `100;w=60;type=key` |

For streaming responses, headers are sent in the initial HTTP response (before SSE data).

### 5.2 Rate Limit Tiers

Applied hierarchically: **Global > Organization > API Key**. The most restrictive limit wins.

| Tier | Default RPM | Default TPM | Configurable |
|---|---|---|---|
| Free (fallback) | 20 | 20,000 | No |
| Starter | 60 | 60,000 | Yes |
| Growth | 300 | 300,000 | Yes |
| Enterprise | 2,000 | 2,000,000 | Yes |
| Custom | Unlimited | Unlimited | Yes |

RPM = requests per minute. TPM = tokens per minute.

### 5.3 Rate Limit Response

When limit exceeded:

```
HTTP/1.1 429 Too Many Requests
X-RateLimit-Limit: 100
X-RateLimit-Remaining: 0
X-RateLimit-Reset: 1715004000
X-RateLimit-Policy: 100;w=60;type=key
Retry-After: 45
Content-Type: application/json
```

```json
{
  "error": {
    "code": "rate_limit_exceeded",
    "message": "Rate limit exceeded: 100 requests per minute. Retry in 45 seconds.",
    "type": "rate_limit_error",
    "param": null,
    "status": 429,
    "request_id": "req_abc123"
  }
}
```

### 5.4 Rate Limit Types

The `X-RateLimit-Policy` header indicates which limit was applied:

| Policy Type | Description |
|---|---|
| `key` | Per-API-key limit |
| `org` | Per-organization limit |
| `global` | Global gateway limit |
| `model` | Per-model rate limit |

---

## 6. Error Handling

### 6.1 Error Response Format

All errors follow this structure:

```json
{
  "error": {
    "code": "error_code_snake_case",
    "message": "Human-readable description",
    "type": "error_category",
    "param": "request_field_name_or_null",
    "status": 400,
    "request_id": "req_abc123"
  }
}
```

### 6.2 Error Codes Reference

#### Client Errors (4xx)

| Code | HTTP Status | Type | Description |
|---|---|---|---|
| `invalid_request_error` | 400 | `invalid_request_error` | Malformed request, missing required field |
| `invalid_json` | 400 | `invalid_request_error` | Request body is not valid JSON |
| `invalid_parameter` | 400 | `invalid_request_error` | Invalid parameter value or type |
| `missing_required_parameter` | 400 | `invalid_request_error` | Required field is missing |
| `context_length_exceeded` | 400 | `invalid_request_error` | Input exceeds model's context window |
| `authentication_error` | 401 | `authentication_error` | Missing or invalid API key |
| `invalid_api_key` | 401 | `authentication_error` | API key format is invalid |
| `expired_api_key` | 401 | `authentication_error` | API key has expired |
| `revoked_api_key` | 401 | `authentication_error` | API key has been revoked |
| `insufficient_scope` | 403 | `authentication_error` | API key lacks required scope |
| `ip_not_allowed` | 403 | `authentication_error` | Request IP not in allowlist |
| `model_not_found` | 404 | `invalid_request_error` | Requested model does not exist |
| `organization_not_found` | 404 | `not_found_error` | Organization does not exist |
| `key_not_found` | 404 | `not_found_error` | API key does not exist |
| `user_not_found` | 404 | `not_found_error` | User does not exist |
| `provider_not_found` | 404 | `not_found_error` | Provider does not exist |
| `routing_rule_not_found` | 404 | `not_found_error` | Routing rule does not exist |
| `quota_not_found` | 404 | `not_found_error` | Quota rule does not exist |
| `webhook_not_found` | 404 | `not_found_error` | Webhook does not exist |
| `method_not_allowed` | 405 | `invalid_request_error` | HTTP method not allowed on this endpoint |
| `rate_limit_exceeded` | 429 | `rate_limit_error` | Rate limit (RPM/TPM) exceeded |
| `quota_exceeded` | 429 | `rate_limit_error` | Token/cost/request quota exceeded |
| `organization_suspended` | 403 | `authentication_error` | Organization is suspended |
| `organization_already_exists` | 409 | `invalid_request_error` | Organization name already in use |
| `provider_in_use` | 409 | `invalid_request_error` | Provider referenced by routing rules |
| `organization_has_keys` | 409 | `invalid_request_error` | Org has active API keys |
| `organization_has_users` | 409 | `invalid_request_error` | Org has active users |

#### Server Errors (5xx)

| Code | HTTP Status | Type | Description |
|---|---|---|---|
| `gateway_error` | 500 | `gateway_error` | Internal gateway error |
| `configuration_error` | 500 | `gateway_error` | Gateway configuration issue |
| `provider_error` | 502 | `provider_error` | Upstream provider returned an error |
| `provider_timeout` | 504 | `provider_error` | Upstream provider timed out |
| `service_unavailable` | 503 | `gateway_error` | No healthy providers available |
| `provider_overloaded` | 503 | `provider_error` | Provider returned 503/overloaded |
| `all_providers_failed` | 502 | `gateway_error` | All providers failed for the request |

#### Auth Errors

| Code | HTTP Status | Type | Description |
|---|---|---|---|
| `invalid_credentials` | 401 | `authentication_error` | Wrong email/password |
| `totp_required` | 401 | `authentication_error` | 2FA code required |
| `invalid_totp` | 401 | `authentication_error` | Invalid 2FA code |
| `invalid_refresh_token` | 401 | `authentication_error` | Refresh token expired or revoked |
| `token_expired` | 401 | `authentication_error` | Access token expired |
| `too_many_attempts` | 429 | `rate_limit_error` | Account temporarily locked |

---

## 7. Webhook Events

### 7.1 Delivery Format

All webhook deliveries are HTTP POST requests to the registered URL.

**Request Headers**:

| Header | Value | Description |
|---|---|---|
| `Content-Type` | `application/json` | Payload format |
| `User-Agent` | `AI-Gateway/1.0.0` | Sender identification |
| `X-Webhook-ID` | `whd_abc123` | Delivery ID |
| `X-Webhook-Event` | `request.completed` | Event type |
| `X-Webhook-Timestamp` | `1715000000` | Unix timestamp |
| `X-Webhook-Signature` | `t=1715000000,v1=hmac_sha256_hex` | HMAC-SHA256 signature |

**Signature Verification**:

```
X-Webhook-Signature: t={timestamp},v1={hex(hmac_sha256(secret, timestamp + "." + json_payload))}
```

Verification steps:
1. Extract `t` (timestamp) and `v1` (signature) from header
2. Reject if timestamp is > 5 minutes old
3. Compute `expected = hex(hmac_sha256(webhook_secret, t + "." + body))`
4. Use constant-time comparison: reject if `v1 != expected`

### 7.2 Delivery Guarantees

| Property | Guarantee |
|---|---|
| At-least-once delivery | Yes, with idempotency key |
| Ordering | Best-effort per webhook endpoint (not guaranteed across events) |
| Retry on failure | Yes, exponential backoff |
| Deduplication | Use `X-Webhook-ID` header |
| Timeout | 30 seconds per delivery attempt |

### 7.3 Retry Policy

| Attempt | Delay After | Total Delay |
|---|---|---|
| 1 (initial) | 0s | 0s |
| 2 | 1s | 1s |
| 3 | 2s | 3s |
| 4 | 4s | 7s |
| 5 | 8s | 15s |
| 6 | 16s | 31s |
| 7 (final) | 32s | 63s |

After 7 failed attempts (over ~63 seconds), the delivery is marked as `failed` and a `webhook.delivery_failed` notification is sent to admin emails.

Retry on: HTTP 408, 429, 500-599, or network timeout/error.
Do NOT retry on: HTTP 400-407, 410, 412-499 (except 429).

### 7.4 Event: `request.completed`

Fired when an AI request completes successfully.

**Payload Schema**:

```json
{
  "event": "request.completed",
  "id": "evt_abc123",
  "created_at": "2024-05-06T12:00:00Z",
  "data": {
    "request_id": "req_abc123",
    "org_id": "org_abc123",
    "api_key_id": "key_abc123",
    "type": "chat.completion | embeddings.create",
    "model": "gpt-4o",
    "provider": "openai",
    "routing_rule_id": "rule_abc123",
    "timestamp": "2024-05-06T12:00:00Z",
    "latency_ms": 420,
    "prompt_tokens": 45,
    "completion_tokens": 128,
    "total_tokens": 173,
    "input_cost_usd": 0.00011,
    "output_cost_usd": 0.00128,
    "total_cost_usd": 0.00139,
    "finish_reason": "stop",
    "cached": false,
    "streaming": false,
    "user": "end-user-id",
    "metadata": {}
  }
}
```

### 7.5 Event: `request.failed`

Fired when an AI request fails (after all retries).

**Payload Schema**:

```json
{
  "event": "request.failed",
  "id": "evt_def456",
  "created_at": "2024-05-06T12:00:00Z",
  "data": {
    "request_id": "req_def456",
    "org_id": "org_abc123",
    "api_key_id": "key_abc123",
    "type": "chat.completion",
    "model": "gpt-4o",
    "provider": "openai",
    "timestamp": "2024-05-06T12:00:00Z",
    "latency_ms": 5000,
    "error": {
      "code": "provider_timeout",
      "message": "Upstream provider timed out after 120 seconds",
      "type": "provider_error",
      "status": 504
    },
    "providers_tried": ["openai", "azure_openai"],
    "prompt_tokens": 45,
    "total_cost_usd": 0.0,
    "user": "end-user-id",
    "metadata": {}
  }
}
```

### 7.6 Event: `budget.threshold_reached`

Fired when an organization's spending crosses the configured alert threshold.

**Payload Schema**:

```json
{
  "event": "budget.threshold_reached",
  "id": "evt_ghi789",
  "created_at": "2024-05-06T12:00:00Z",
  "data": {
    "org_id": "org_abc123",
    "org_name": "acme-corp",
    "threshold_percent": 80,
    "current_spend_usd": 800.00,
    "budget_limit_usd": 1000.00,
    "remaining_usd": 200.00,
    "period": "2024-05-01 to 2024-05-31",
    "projected_spend_usd": 1200.00,
    "overage_projected_usd": 200.00
  }
}
```

### 7.7 Event: `provider.error`

Fired when a provider health check fails or a significant error rate is detected.

**Payload Schema**:

```json
{
  "event": "provider.error",
  "id": "evt_jkl012",
  "created_at": "2024-05-06T12:00:00Z",
  "data": {
    "provider_id": "openai",
    "provider_name": "OpenAI",
    "error_type": "health_check_failed | high_error_rate | latency_spike | authentication_error",
    "previous_status": "healthy",
    "current_status": "unhealthy",
    "error_details": {
      "message": "Health check probe failed: 503 Service Unavailable",
      "http_status": 503,
      "error_rate_1h": 0.45,
      "latency_p99_ms": 15000,
      "last_success_at": "2024-05-06T11:55:00Z"
    },
    "affected_models": ["gpt-4o", "gpt-4o-mini"],
    "routing_impact": "failover_to_azure"
  }
}
```

### 7.8 Event: `key.created`

Fired when a new API key is created.

**Payload Schema**:

```json
{
  "event": "key.created",
  "id": "evt_mno345",
  "created_at": "2024-05-06T12:00:00Z",
  "data": {
    "key_id": "key_abc123",
    "key_prefix": "sk-ag...x7k",
    "org_id": "org_abc123",
    "org_name": "acme-corp",
    "name": "Production API Key",
    "scopes": ["chat:write", "embeddings:write"],
    "created_by": "user_admin123",
    "expires_at": null,
    "created_at": "2024-05-06T12:00:00Z"
  }
}
```

### 7.9 Event: `key.revoked`

Fired when an API key is revoked or deleted.

**Payload Schema**:

```json
{
  "event": "key.revoked",
  "id": "evt_pqr678",
  "created_at": "2024-05-06T12:00:00Z",
  "data": {
    "key_id": "key_abc123",
    "key_prefix": "sk-ag...x7k",
    "org_id": "org_abc123",
    "org_name": "acme-corp",
    "name": "Production API Key",
    "revoked_by": "user_admin123",
    "revoked_at": "2024-05-06T12:00:00Z",
    "reason": "string (optional, if provided during revocation)"
  }
}
```

### 7.10 Event: `provider.health_changed`

Fired when a provider's health status changes.

**Payload Schema**:

```json
{
  "event": "provider.health_changed",
  "id": "evt_stu901",
  "created_at": "2024-05-06T12:00:00Z",
  "data": {
    "provider_id": "openai",
    "provider_name": "OpenAI",
    "previous_status": "healthy",
    "current_status": "degraded",
    "latency_p50_ms": 350,
    "latency_p99_ms": 8200,
    "error_rate_1h": 0.05,
    "checked_at": "2024-05-06T12:00:00Z",
    "details": "Latency p99 above 5000ms threshold"
  }
}

Health status values: `healthy`, `degraded`, `unhealthy`, `unknown`.
```

### 7.11 Event: `quota.threshold_reached`

Fired when a quota's usage crosses the alert threshold.

**Payload Schema**:

```json
{
  "event": "quota.threshold_reached",
  "id": "evt_vwx234",
  "created_at": "2024-05-06T12:00:00Z",
  "data": {
    "quota_id": "quota_abc123",
    "quota_name": "Monthly Token Limit",
    "org_id": "org_abc123",
    "type": "token_limit",
    "threshold_percent": 80,
    "current_usage": 80000000,
    "limit": 100000000,
    "remaining": 20000000,
    "window": "1 month",
    "action": "block_with_alert"
  }
}
```

---

## 8. SSE Streaming Format

### 8.1 Chat Completions Streaming

When `stream: true` is set in the request, the response is a `text/event-stream`.

**Response Headers**:

```
HTTP/1.1 200 OK
Content-Type: text/event-stream
Cache-Control: no-cache
Connection: keep-alive
X-Gateway-Request-ID: req_abc123
X-Gateway-Provider: openai
X-RateLimit-Limit: 100
X-RateLimit-Remaining: 99
```

**Event Format**:

Each event is a SSE `data:` line containing a JSON chunk. Events are separated by double newlines (`\n\n`).

```
data: {"id":"chatcmpl-abc","object":"chat.completion.chunk","created":1715000000,"model":"gpt-4o","system_fingerprint":"fp_abc","choices":[{"index":0,"delta":{"role":"assistant"},"logprobs":null,"finish_reason":null}]}

data: {"id":"chatcmpl-abc","object":"chat.completion.chunk","created":1715000000,"model":"gpt-4o","system_fingerprint":"fp_abc","choices":[{"index":0,"delta":{"content":"Hello"},"logprobs":null,"finish_reason":null}]}

data: {"id":"chatcmpl-abc","object":"chat.completion.chunk","created":1715000000,"model":"gpt-4o","system_fingerprint":"fp_abc","choices":[{"index":0,"delta":{"content":"!"},"logprobs":null,"finish_reason":null}]}

data: {"id":"chatcmpl-abc","object":"chat.completion.chunk","created":1715000000,"model":"gpt-4o","system_fingerprint":"fp_abc","choices":[{"index":0,"delta":{},"logprobs":null,"finish_reason":"stop"}]}

data: [DONE]

```

**Chunk Fields**:

| Field | Type | Description |
|---|---|---|
| `id` | string | Same ID across all chunks for the request |
| `object` | string | Always `chat.completion.chunk` |
| `created` | integer | Unix timestamp |
| `model` | string | Model ID used |
| `system_fingerprint` | string | System fingerprint |
| `choices` | array | Array of choice deltas |
| `choices[].index` | integer | Choice index |
| `choices[].delta` | object | Delta object with incremental content |
| `choices[].delta.role` | string | Present only in first chunk (`assistant`) |
| `choices[].delta.content` | string \| null | Incremental text content |
| `choices[].delta.tool_calls` | array \| null | Incremental tool call deltas |
| `choices[].logprobs` | object \| null | Log probabilities (if requested) |
| `choices[].finish_reason` | string \| null | `stop`, `length`, `tool_calls`, `content_filter`, or `null` |

**Usage Chunk** (sent when `stream_options.include_usage: true`):

Before the `[DONE]` message, an additional chunk with usage:

```
data: {"id":"chatcmpl-abc","object":"chat.completion.chunk","created":1715000000,"model":"gpt-4o","system_fingerprint":"fp_abc","choices":[],"usage":{"prompt_tokens":10,"completion_tokens":20,"total_tokens":30},"gateway":{"provider_used":"openai","latency_ms":450,"input_cost_usd":0.00005,"output_cost_usd":0.00030,"total_cost_usd":0.00035}}

```

### 8.2 Stream Termination

The stream terminates with:

```
data: [DONE]

```

After `[DONE]`, the server closes the connection. No further events are sent.

### 8.3 Error Handling Mid-Stream

If an error occurs after the stream has started, the gateway sends an error event and then terminates:

```
data: {"id":"chatcmpl-abc","object":"chat.completion.chunk","created":1715000000,"model":"gpt-4o","choices":[{"index":0,"delta":{"content":""},"finish_reason":null}],"gateway_error":{"code":"provider_timeout","message":"Upstream provider timed out","status":504}}

data: [DONE]

```

**Mid-Stream Error Event Format**:

When an error occurs mid-stream, the chunk includes a `gateway_error` object:

| Field | Type | Description |
|---|---|---|
| `gateway_error.code` | string | Error code |
| `gateway_error.message` | string | Error description |
| `gateway_error.status` | integer | HTTP-equivalent status |

The client should treat this as a failed completion. Partial content received before the error event should be discarded or marked as incomplete.

### 8.4 Connection Management

| Property | Behavior |
|---|---|
| Keep-alive | SSE connection remains open until completion or error |
| Client disconnect | Gateway detects disconnect and cancels upstream request within 2s |
| Timeout | Total stream timeout: 10 minutes (configurable per provider) |
| Idle timeout | If no chunk received from provider in 60s, gateway sends error and closes |
| Reconnection | Not supported; client must retry the full request |

---

## Appendix A: OpenAPI Schema Summary

### Common Types

```json
{
  "Pagination": {
    "type": "object",
    "properties": {
      "limit": { "type": "integer", "minimum": 1, "maximum": 100 },
      "offset": { "type": "integer", "minimum": 0 },
      "total": { "type": "integer", "minimum": 0 },
      "has_more": { "type": "boolean" }
    },
    "required": ["limit", "offset", "total", "has_more"]
  },

  "Error": {
    "type": "object",
    "properties": {
      "error": {
        "type": "object",
        "properties": {
          "code": { "type": "string" },
          "message": { "type": "string" },
          "type": { "type": "string", "enum": ["invalid_request_error", "authentication_error", "rate_limit_error", "gateway_error", "provider_error", "not_found_error"] },
          "param": { "type": ["string", "null"] },
          "status": { "type": "integer" },
          "request_id": { "type": "string" }
        },
        "required": ["code", "message", "type", "status"]
      }
    },
    "required": ["error"]
  },

  "GatewayMetadata": {
    "type": "object",
    "properties": {
      "request_id": { "type": "string" },
      "provider_used": { "type": "string" },
      "model_routed_to": { "type": "string" },
      "latency_ms": { "type": "integer" },
      "input_cost_usd": { "type": "number" },
      "output_cost_usd": { "type": "number" },
      "total_cost_usd": { "type": "number" },
      "cached_tokens": { "type": "integer" },
      "routing_strategy_applied": { "type": "string" }
    }
  }
}
```

### Endpoint Summary

| Method | Path | Auth | Description |
|---|---|---|---|
| POST | `/v1/chat/completions` | API Key | Chat completion |
| POST | `/v1/embeddings` | API Key | Create embeddings |
| GET | `/v1/models` | API Key | List models |
| GET | `/v1/models/{model}` | API Key | Get model |
| POST | `/api/v1/auth/login` | None | Login |
| POST | `/api/v1/auth/refresh` | None | Refresh token |
| POST | `/api/v1/auth/logout` | JWT | Logout |
| GET | `/api/v1/auth/me` | JWT | Current user |
| POST | `/api/v1/organizations` | JWT | Create org |
| GET | `/api/v1/organizations` | JWT | List orgs |
| GET | `/api/v1/organizations/{id}` | JWT | Get org |
| PUT | `/api/v1/organizations/{id}` | JWT | Update org |
| DELETE | `/api/v1/organizations/{id}` | JWT | Delete org |
| POST | `/api/v1/organizations/{id}/keys` | JWT | Create key |
| GET | `/api/v1/organizations/{id}/keys` | JWT | List keys |
| DELETE | `/api/v1/organizations/{id}/keys/{key}` | JWT | Revoke key |
| POST | `/api/v1/users` | JWT | Create user |
| GET | `/api/v1/users` | JWT | List users |
| GET | `/api/v1/users/{id}` | JWT | Get user |
| PUT | `/api/v1/users/{id}` | JWT | Update user |
| DELETE | `/api/v1/users/{id}` | JWT | Delete user |
| POST | `/api/v1/providers` | JWT | Add provider |
| GET | `/api/v1/providers` | JWT | List providers |
| GET | `/api/v1/providers/{id}` | JWT | Get provider |
| PUT | `/api/v1/providers/{id}` | JWT | Update provider |
| DELETE | `/api/v1/providers/{id}` | JWT | Delete provider |
| POST | `/api/v1/providers/{id}/health-check` | JWT | Trigger health check |
| GET | `/api/v1/providers/{id}/health-history` | JWT | Health history |
| POST | `/api/v1/routing-rules` | JWT | Create routing rule |
| GET | `/api/v1/routing-rules` | JWT | List routing rules |
| GET | `/api/v1/routing-rules/{id}` | JWT | Get routing rule |
| PUT | `/api/v1/routing-rules/{id}` | JWT | Update routing rule |
| DELETE | `/api/v1/routing-rules/{id}` | JWT | Delete routing rule |
| POST | `/api/v1/organizations/{id}/quotas` | JWT | Create quota |
| GET | `/api/v1/organizations/{id}/quotas` | JWT | List quotas |
| GET | `/api/v1/organizations/{id}/quotas/{qid}` | JWT | Get quota |
| PUT | `/api/v1/organizations/{id}/quotas/{qid}` | JWT | Update quota |
| DELETE | `/api/v1/organizations/{id}/quotas/{qid}` | JWT | Delete quota |
| GET | `/api/v1/organizations/{id}/usage` | JWT/API Key | Usage data |
| GET | `/api/v1/organizations/{id}/costs` | JWT | Cost data |
| GET | `/api/v1/organizations/{id}/requests` | JWT | Request logs |
| POST | `/api/v1/webhooks` | JWT | Register webhook |
| GET | `/api/v1/webhooks` | JWT | List webhooks |
| GET | `/api/v1/webhooks/{id}` | JWT | Get webhook |
| PUT | `/api/v1/webhooks/{id}` | JWT | Update webhook |
| DELETE | `/api/v1/webhooks/{id}` | JWT | Delete webhook |
| POST | `/api/v1/webhooks/{id}/test` | JWT | Test webhook |
| GET | `/api/v1/webhooks/{id}/deliveries` | JWT | Delivery history |

---

## Appendix B: Header Reference

### Request Headers (Client → Gateway)

| Header | Used On | Description |
|---|---|---|
| `Authorization` | All | `Bearer {token}` |
| `Content-Type` | POST/PUT | `application/json` |
| `Accept` | Chat | `text/event-stream` for streaming |
| `X-Gateway-Provider-Hint` | Chat, Embeddings | Preferred provider |
| `X-Gateway-Routing-Strategy` | Chat, Embeddings | Override routing |
| `X-Gateway-Model-Fallback` | Chat | Fallback model IDs |

### Response Headers (Gateway → Client)

| Header | Sent On | Description |
|---|---|---|
| `X-Gateway-Request-ID` | All | Unique request ID |
| `X-Gateway-Version` | All | Gateway version |
| `X-Gateway-Provider` | AI API | Provider that served the request |
| `X-Gateway-Model` | AI API | Actual model used |
| `X-Gateway-Latency-Ms` | AI API | Total gateway latency |
| `X-Gateway-Cost-USD` | AI API | Estimated cost |
| `X-RateLimit-Limit` | All | Rate limit cap |
| `X-RateLimit-Remaining` | All | Remaining requests |
| `X-RateLimit-Reset` | All | Limit reset timestamp |
| `X-RateLimit-Policy` | All | Applied rate limit policy |
| `Retry-After` | 429 | Seconds until retry |
| `Deprecation` | Deprecated | Deprecation flag |
| `Sunset` | Deprecated | Endpoint sunset date |
