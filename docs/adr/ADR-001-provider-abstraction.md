# ADR-001: Provider Abstraction

## Status
Accepted

## Context
The AI Gateway must communicate with multiple LLM providers (OpenAI, Anthropic, Google Gemini, Ollama, and custom OpenAI-compatible endpoints). Each provider has a distinct API shape, authentication scheme, request/response format, and streaming behavior. The gateway must present a single uniform interface to consumers while internally managing provider-specific complexity.

Key forces:
- Consumers expect an OpenAI-compatible API (`/v1/chat/completions`, `/v1/embeddings`, `/v1/models`) as the de facto standard.
- Provider APIs diverge in message format, parameter names, authentication headers, and streaming protocols.
- New providers emerge frequently; adding one must not require changes to the core request pipeline.
- The gateway must route, transform, retry, and fallback across providers transparently.
- Module boundaries are treated as API boundaries per architectural principle 1.6.

## Decision
All LLM providers are abstracted behind a unified `Provider` trait defined in the `gateway-providers` crate. Every provider implements this trait: OpenAI, Anthropic, Gemini, Ollama, and Custom (OpenAI-compatible).

**The canonical request/response format is OpenAI-compatible.** The gateway-core crate defines `ChatCompletionRequest` and `ChatCompletionResponse` as OpenAI-shaped structs. Each provider adapter transforms from this canonical form to its native format on the outbound leg, and back to the canonical form on the inbound leg.

**Provider trait interface:**
```rust
#[async_trait]
pub trait Provider: Send + Sync {
    fn name(&self) -> &str;
    fn supported_models(&self) -> Vec<&str>;
    async fn chat_completion(&self, request: ChatCompletionRequest) -> Result<ChatCompletionResponse, ProviderError>;
    async fn chat_completion_stream(&self, request: ChatCompletionRequest) -> Result<SseStream, ProviderError>;
    async fn embeddings(&self, request: EmbeddingRequest) -> Result<EmbeddingResponse, ProviderError>;
    async fn health_check(&self) -> HealthStatus;
}
```

**Adding a new provider** requires only:
1. Create a new adapter struct implementing the `Provider` trait.
2. Add provider-specific request/response transformation functions.
3. Register the provider in the factory function `create_provider(config: ProviderConfig)`.
4. Add the provider configuration to the database (`provider_configs` table).
No changes to gateway-core, gateway-api, or gateway-cache are required.

**Provider configuration model:**
```rust
pub struct ProviderConfig {
    pub provider_id: String,
    pub base_url: String,
    pub api_key_ref: String,
    pub default_model: String,
    pub rate_limit: RateLimitConfig,
    pub timeout_ms: u64,
    pub retry_policy: RetryPolicy,
}
```

## Alternatives Considered

### Alternative 1: Direct Proxy (Pass-Through)
- Description: Forward requests directly to providers without transformation, requiring clients to use each provider's native API.
- Why rejected: Forces clients to manage multiple API formats, authentication schemes, and endpoints. Eliminates the gateway's value proposition of a single unified interface. Breaks caching, quota tracking, and fallback routing which depend on canonical request parsing.

### Alternative 2: Plugin System with Dynamic Loading
- Description: Providers loaded as dynamic libraries (`.so`/`.dll`) at runtime via a plugin registry.
- Why rejected: Adds significant operational complexity (versioning, ABI compatibility, security sandboxing). Violates the "Prefer Boring Technology" principle. Rust's dynamic loading story is immature. A trait-based compile-time system is simpler and sufficient.

### Alternative 3: Per-Provider REST Routes
- Description: Expose separate routes for each provider (`/anthropic/chat`, `/openai/chat`, etc.) with native request bodies.
- Why rejected: Leaks provider-specific concerns to the consumer. Makes provider failover and routing impossible at the gateway level. Forces clients to change code when switching providers.

### Alternative 4: Anthropic API as Canonical Format
- Description: Use Anthropic's message format as the canonical shape instead of OpenAI's.
- Why rejected: OpenAI's API is the dominant standard in the ecosystem. Most tools, SDKs, and integrations target the OpenAI format. Choosing Anthropic would force every consumer to adapt.

## Tradeoffs

### What We Gain
- **Single integration effort for consumers:** Write once to OpenAI format; work with any provider.
- **Provider swaps without client changes:** Change the backend provider in the dashboard; zero client code changes.
- **Isolated provider complexity:** Bugs in one adapter (e.g., Gemini system prompt handling) are contained within that adapter.
- **Testability:** Mock providers implement the same trait for unit testing the request pipeline.
- **Fast provider onboarding:** New provider = new trait implementation; no core changes.

### What We Give Up
- **Lossless transformation:** Some provider-specific features (e.g., Anthropic's `thinking` blocks, Gemini's multimodal capabilities) may be lost or flattened in the OpenAI-shaped canonical format.
- **Adapter maintenance burden:** Every upstream API change requires updating the corresponding adapter.
- **Slight latency overhead:** Request/response transformation adds ~1-2ms per request (JSON deserialization + reserialization).
- **Feature lowest-common-denominator:** The canonical format can only represent features that all providers support; cutting-edge features on one provider may not be expressible.

## Consequences
- All provider adapters must map to and from the OpenAI-compatible `ChatCompletionRequest`/`ChatCompletionResponse` types.
- The `gateway-providers` crate is the sole location for provider-specific code; no other crate references provider-native types.
- Provider selection logic in `gateway-core::Router` operates on trait objects (`Box<dyn Provider>`), making it agnostic to the concrete provider list.
- Streaming responses must be transformed chunk-by-chunk for providers that use non-SSE streaming formats (e.g., Gemini's bidirectional stream).
- Ollama's local-only deployment pattern requires special handling for timeouts and base URL resolution (see ADR-008).

## Related Decisions
- ADR-007: Fallback Strategy — provider failover depends on the unified trait interface for interchangeable provider calls.
- ADR-008: Ollama Support — local provider deployment pattern differs from cloud providers.

## Notes
- The `Provider` trait is intentionally minimal (6 methods). Resist expanding it; provider-specific features should be handled via adapter configuration, not new trait methods.
- Future work: Consider a WASM-based provider adapter for user-defined transformations without recompilation.
- Health check implementation varies by provider: OpenAI uses `GET /models`, Anthropic uses `GET /v1/health`, Ollama uses `GET /api/tags`. Each adapter defines its own health check endpoint.
