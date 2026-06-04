# OpenCode Integration

This gateway is fully compatible with [OpenCode](https://opencode.ai) — an open-source AI coding agent. You can use this gateway as a custom OpenAI-compatible provider in OpenCode.

## Quick Start

### 1. Get an API Key

Create an API key in the gateway dashboard or via the API:

```bash
curl -X POST http://localhost:8080/v1/api-keys \
  -H "Authorization: Bearer YOUR_ADMIN_KEY" \
  -H "Content-Type: application/json" \
  -d '{"name": "OpenCode"}'
```

Copy the returned `key` (starts with `sk_gw_`).

### 2. Configure OpenCode

#### Method A: Config File

Create or edit `~/.config/opencode/opencode.json`:

```json
{
  "$schema": "https://opencode.ai/config.json",
  "provider": {
    "gateway": {
      "npm": "@ai-sdk/openai-compatible",
      "name": "AI Gateway",
      "options": {
        "baseURL": "http://localhost:8080/v1",
        "apiKey": "{env:GATEWAY_API_KEY}"
      },
      "models": {
        "gpt-4o": {
          "name": "GPT-4o"
        },
        "claude-3-5-sonnet-20241022": {
          "name": "Claude 3.5 Sonnet"
        }
      }
    }
  },
  "model": "gpt-4o"
}
```

Then set your API key:

```bash
export GATEWAY_API_KEY="sk_gw_your_key_here"
```

#### Method B: Interactive (`/connect`)

1. Start OpenCode: `opencode`
2. Run `/connect`
3. Select **Other**
4. Enter provider ID: `gateway`
5. Paste your API key when prompted
6. Edit `opencode.json` to add the `baseURL` and `models` as shown above

### 3. Verify

```
/connect
```

You should see your gateway provider listed. Start chatting!

## Supported Endpoints

OpenCode uses the following OpenAI-compatible endpoints provided by this gateway:

| Endpoint | Description |
|----------|-------------|
| `POST /v1/chat/completions` | Chat completions (streaming + non-streaming) |
| `GET /v1/models` | List available models |
| `GET /v1/models/{id}` | Get a single model |

## Supported Parameters

All standard OpenAI chat completion parameters are supported:

- `model` — required, use any model configured in your gateway
- `messages` — required
- `stream` — optional, defaults to `false`
- `temperature` — optional, 0.0–2.0
- `top_p` — optional, 0.0–1.0
- `max_tokens` — optional
- `frequency_penalty` — optional, -2.0–2.0
- `presence_penalty` — optional, -2.0–2.0
- `stop` — optional
- `tools` / `tool_choice` — optional
- `response_format` — optional
- `seed` — optional
- `user` — optional

## Features

- **Provider routing** — OpenCode requests are routed through your configured providers (OpenAI, Anthropic, Gemini, Ollama)
- **Circuit breaker** — Automatic fallback if a provider fails
- **Caching** — Repeated identical prompts are served from cache instantly
- **Rate limiting** — Per-API-key rate limits protect your backend
- **Usage tracking** — All requests are logged for analytics

## Troubleshooting

### "Invalid API key" error

Ensure your API key starts with `sk_gw_` and is passed as:
```
Authorization: Bearer sk_gw_...
```

### "Model not found" error

The model ID in OpenCode must match a model configured in your gateway. Check available models:

```bash
curl http://localhost:8080/v1/models \
  -H "Authorization: Bearer sk_gw_your_key"
```

### Streaming not working

Ensure your gateway is running with SSE support (default). The gateway sends `data: {...}` formatted SSE chunks ending with `data: [DONE]`.

### CORS errors in browser/desktop OpenCode

The gateway already enables CORS for all origins in development. In production, configure the `allowed_origins` setting.

## Environment Variables for Docker

When running the gateway in Docker:

```yaml
services:
  gateway:
    image: gateway:latest
    ports:
      - "8080:8080"
    environment:
      - OPENAI_API_KEY=${OPENAI_API_KEY}
      - ANTHROPIC_API_KEY=${ANTHROPIC_API_KEY}
```

Then point OpenCode to `http://localhost:8080/v1`.
