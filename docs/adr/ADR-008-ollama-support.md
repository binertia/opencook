# ADR-008: Ollama Support

## Status
Accepted

## Context
While cloud LLM APIs (OpenAI, Anthropic, Gemini) provide broad capability, they introduce dependencies on external services, network latency, ongoing costs, and data privacy concerns. Many customers — especially those in regulated industries, those with sensitive data, or those wanting to reduce API spend — need the ability to run models locally.

Ollama is the dominant local LLM runtime: it downloads, manages, and serves open-source models (Llama, Mistral, Qwen, etc.) via a simple HTTP API. It runs on consumer hardware (Apple Silicon, NVIDIA GPUs, CPU-only) and is operationally trivial (`ollama run llama3`).

Key forces:
- Data privacy: some customers cannot send data to cloud APIs
- Cost reduction: local inference eliminates per-token API charges
- Latency: localhost calls are faster than cross-network API calls
- Offline operation: works without internet connectivity
- Capability gap: local models are generally less capable than frontier models (GPT-4o, Claude 3.5 Sonnet)
- Resource constraints: local models require significant RAM/VRAM and GPU resources

## Decision
We will integrate Ollama as a first-class provider in the gateway, with the following design:

**Ollama as a Provider:**
- Ollama implements the same `Provider` trait as cloud providers (see ADR-001).
- The Ollama adapter converts canonical `ChatCompletionRequest` to Ollama's `/api/chat` format and responses back to OpenAI-compatible format.
- Base URL is configurable (default: `http://localhost:11434`); can point to a LAN-accessible Ollama instance.
- Model names are mapped from gateway model IDs to Ollama model tags (e.g., `llama3.1` → `llama3.1:latest`).

**Key Differences from Cloud Providers:**

| Aspect | Cloud Providers | Ollama |
|--------|----------------|--------|
| Base URL | HTTPS API endpoint | HTTP localhost/LAN |
| Authentication | API key in header | None (local trust) |
| Timeout | 120s | 300s (local models can be slower) |
| Connection pool | 100 concurrent | 20 concurrent (local resource limits) |
| Caching | Responses cached | Responses NOT cached (local inference is "free") |
| Cost tracking | Per-token cost | Zero direct cost (hardware amortized) |
| Streaming | SSE from provider | SSE from Ollama (native support) |

**Resource Implications:**
- Ollama runs outside the gateway (separate process, potentially on separate hardware).
- The gateway does not manage Ollama lifecycle (start, stop, model download). It assumes Ollama is running and the requested model is available.
- Gateway health checks verify Ollama availability via `GET /api/tags`; if Ollama is unreachable, the provider is marked unhealthy and excluded from routing.
- No caching of Ollama responses: local inference has no API cost, so caching provides no cost savings while consuming Redis memory.
- Cost tracking records zero cost for Ollama responses but still tracks token usage for quota enforcement.

**Deployment Pattern for Local + Cloud Hybrid:**
- Customers configure a fallback chain with Ollama as a low-priority or last-resort option.
- Common patterns:
  - **Cost optimization:** Route simple requests to Ollama, complex requests to cloud providers.
  - **Privacy mode:** Route sensitive data to Ollama, general queries to cloud.
  - **Offline fallback:** Use Ollama when all cloud providers are unavailable.
  - **Development:** Use Ollama for dev/test (free), cloud for production.
- Routing rules can match on model name: requests for `llama3.1` go to Ollama; requests for `gpt-4o` go to OpenAI.
- The admin dashboard shows Ollama status, available models (from `/api/tags`), and resource usage (if Ollama exposes metrics).

## Alternatives Considered

### Alternative 1: No Local Model Support
- **Description:** Support only cloud LLM APIs; require customers to use external providers for all inference.
- **Why rejected:** Eliminates a significant customer segment (privacy-sensitive, cost-conscious, regulated industries). Local models are increasingly capable and are a key differentiator for AI gateways. Many customers explicitly request on-premise inference capability.

### Alternative 2: Direct Ollama Integration (No Provider Abstraction)
- **Description:** Build Ollama-specific routes (`/ollama/chat`) and handlers separate from the unified provider pipeline.
- **Why rejected:** Fragments the API surface. Clients would need separate code paths for local vs. cloud models. Breaks the gateway's core value proposition of a single unified interface. Fallback between Ollama and cloud providers would be impossible.

### Alternative 3: Self-Hosted vLLM / TGI Instead of Ollama
- **Description:** Integrate with vLLM or Text Generation Inference (Hugging Face) as the local inference engine.
- **Why rejected:** vLLM and TGI are designed for data center deployment with GPUs; they are significantly more complex to operate than Ollama. Ollama's installation is one command; vLLM requires CUDA setup, model weight management, and careful GPU memory tuning. Ollama is the de facto standard for local LLM inference and has broader model ecosystem support.

### Alternative 4: Gateway Manages Ollama Lifecycle
- **Description:** The gateway starts, stops, and manages Ollama processes and model downloads.
- **Why rejected:** Adds significant complexity: process management, GPU scheduling, model download progress tracking, disk space management. Violates the Unix philosophy of "do one thing well." Ollama is better managed by systemd/Docker Compose on the host. The gateway should be a client of Ollama, not its operator.

## Tradeoffs

### What We Gain
- **Data privacy:** Sensitive data never leaves the local network.
- **Zero API costs for local inference:** Eliminates per-token charges for workloads that can use open-source models.
- **Offline operation:** Works without internet connectivity once models are downloaded.
- **Hybrid flexibility:** Customers can mix local and cloud models based on cost, privacy, and capability needs.
- **Provider abstraction uniformity:** Ollama benefits from the same caching (disabled), rate limiting, quota tracking, and fallback logic as cloud providers.

### What We Give Up
- **Capability gap:** Local models (Llama 3, Mistral) are generally less capable than frontier models (GPT-4o, Claude 3.5 Sonnet) for complex reasoning, coding, and multi-modal tasks.
- **Hardware requirements:** Local inference requires significant RAM (8GB+ for 7B models, 32GB+ for 70B models) and ideally GPU acceleration. Not all VPS instances can run meaningful local models.
- **Operational complexity:** Customers must install, configure, and maintain Ollama separately from the gateway. Model downloads, updates, and hardware tuning are customer responsibilities.
- **Higher latency for large models:** CPU inference or swapping on limited VRAM can be 10-100x slower than cloud APIs. The 300s timeout reflects this reality.
- **No automatic scaling:** Local inference capacity is fixed by hardware; cannot scale up during traffic spikes.

## Consequences
- The Ollama adapter implements the `Provider` trait with Ollama-specific request/response transformation (`/api/chat` format).
- Ollama base URL is configurable per-organization via `provider_configs` table; default is `http://localhost:11434`.
- No authentication header is sent to Ollama (local trust boundary).
- Health checks use `GET /api/tags` to verify Ollama is running and list available models.
- Ollama responses are not cached (deliberate decision; no cost savings from caching free inference).
- Token usage is still tracked for Ollama responses for quota enforcement, but cost is recorded as $0.00.
- The default fallback chain does not include Ollama; it must be explicitly added by the customer.
- Connection pool to Ollama is limited to 20 concurrent requests to prevent overwhelming local hardware.
- Request timeout for Ollama is 300 seconds (vs. 120 seconds for cloud providers) to accommodate slower local inference.
- The admin dashboard displays Ollama status, available models, and a configuration panel for the Ollama endpoint URL.

## Related Decisions
- **ADR-001 (Provider Abstraction):** Ollama implements the same `Provider` trait, making it interchangeable with cloud providers in the request pipeline.
- **ADR-002 (Cache Strategy):** Ollama responses are explicitly excluded from caching; this is a provider-level configuration.
- **ADR-007 (Fallback Strategy):** Ollama is typically configured as a low-priority fallback or a routing target for specific models, not a primary provider for most workloads.

## Notes
- Ollama's API is a subset of OpenAI's: it supports chat completions and embeddings but not all parameters (e.g., `logit_bias`, `seed` are ignored). Unsupported parameters are dropped with a warning logged.
- For organizations with multiple Ollama instances (e.g., one per department), each instance is configured as a separate "provider" with a unique name and base URL.
- The gateway does not verify that the requested model is available in Ollama before routing; if the model is missing, Ollama returns a 404 which is handled by the standard error path. Future work: pre-flight model availability check.
- GPU utilization metrics from Ollama are not currently exposed in the dashboard; this requires Ollama to export metrics (future Ollama feature).
- Security: Ollama's default configuration binds to localhost only. If binding to a network interface, Ollama should be behind a firewall; the gateway does not add authentication to Ollama requests.
