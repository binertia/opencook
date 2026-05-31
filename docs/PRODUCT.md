# PRODUCT DEFINITION: AI Gateway for SMEs

## 1. Product Overview

An open-core AI Gateway that deploys in under 10 minutes on a single VPS via Docker Compose, providing intelligent request routing, semantic caching, and hard budget caps that reduce AI API spend by 30-70%. Built for cost-conscious SMEs (20-500 employees) who need multi-provider AI infrastructure without Kubernetes, DevOps teams, or enterprise budgets.

**North Star Metric:** Monthly AI Spend Avoided (dollar value of provider costs saved by all active deployments).

---

## 2. Target Customer Definition

### 2.1 Primary Persona: "The Cost-Conscious CTO"

| Attribute | Detail |
|-----------|--------|
| **Role** | CTO / VP Engineering / Lead Developer |
| **Company** | 20-100 person tech company (SaaS startup, digital agency, tech-enabled SMB) |
| **AI Spend** | $500-$10,000/month |
| **Team** | 2-10 developers touching AI features |
| **Technical Level** | High; knows Docker, doesn't want to manage Kubernetes |
| **Budget Authority** | Up to $500/month autonomously |
| **Primary Pain** | Unpredictable AI bills; juggling multiple provider API keys; no per-team cost visibility |
| **Buying Trigger** | Surprise $5K+ API bill; board asks for AI spend accountability; provider outage |
| **Evidence** | "I am juggling OpenAI, Claude, Gemini, and Grok in different projects... managing multiple API keys across projects is getting on my nerve" [CUSTOMER_RESEARCH]; 80% of enterprises miss AI forecasts by >25% [MARKET] |

### 2.2 Secondary Persona: "The Agency Technical Lead"

| Attribute | Detail |
|-----------|--------|
| **Role** | Technical Lead / Managing Partner at dev shop or agency |
| **Company** | 10-50 person agency managing AI for 5-20 clients |
| **AI Spend** | $1,000-$10,000/month (aggregated across clients) |
| **Primary Pain** | Per-client cost attribution; managing separate API keys and billing per client; client data residency requirements |
| **Buying Trigger** | Client demands data residency; need per-client billing accuracy; client requires provider switching |
| **Budget Authority** | Up to $200/month per managed client |
| **Evidence** | ResultantAI Gateway explicitly targets agencies needing per-client billing [COMPETITORS]; "Each model has its own pricing, billing structure, and SDK... fragmented visibility, unpredictable costs" [CUSTOMER_RESEARCH] |

### 2.3 Explicit Non-Customers

| Segment | Why Excluded | Risk of Including |
|---------|-------------|-------------------|
| **Fortune 500 enterprises** | Require SAML, multi-region, dedicated support, custom SLAs, SOC 2 pre-certification | Scope creep into features used by <1% of revenue potential |
| **Companies with dedicated DevOps teams of 5+** | Already have Kubernetes; have 20+ better options (Kong, Gloo, Ambassador) | Diverts engineering from simplicity promise |
| **Teams wanting prompt engineering platforms** | Different product category; LangChain/Portkey/Braintrust own this | Scope creep, feature bloat |
| **Teams wanting RAG/vector database** | Different product category; Pinecone, Weaviate, pgvector exist | Competes with data platforms, not gateways |
| **Teams wanting model hosting/fine-tuning** | Different product category; Replicate, Together AI exist | Competes with ML infrastructure, not API gateways |
| **Teams wanting workflow automation/agent frameworks** | Different product category; n8n, CrewAI, LangChain exist | Gateway becomes bloated platform instead of focused tool |
| **Monthly AI spend < $100** | Cost reduction value doesn't justify any gateway cost | Low-value support burden, no conversion path |

**Disqualification criteria for a prospect:** Requires SOC 2 Type II before purchase; wants managed model hosting; has 5+ person DevOps team with existing K8s cluster; wants prompt management, RAG, or fine-tuning as core features.

---

## 3. Core Value Propositions (Ranked by Customer Impact)

### VP1: Cut AI Spend 30-70% with Zero Markup
- **Claim:** Built-in smart routing + semantic caching reduces AI API bills by 30-70% with zero platform markup on inference
- **Evidence:** Semantic caching delivers 40-80% cost reduction in support/Q&A workloads [CUSTOMER_RESEARCH]; teams report 34-67% cost reduction with caching [CUSTOMER_RESEARCH]; 84% of enterprises report gross margin erosion from AI costs [MARKET]
- **Metric:** Monthly AI Spend Avoided (target: 40% average reduction across active deployments)
- **Competitive context:** OpenRouter adds 5% markup; no competitor offers combined smart routing + semantic caching + zero markup at affordable tiers

### VP2: Deploy in Under 10 Minutes on a $20 VPS
- **Claim:** Single-command Docker Compose deployment on any VPS; no Kubernetes, no DevOps, no external dependencies beyond Docker
- **Evidence:** LiteLLM requires 20-40 hours setup [COMPETITORS]; LiteLLM TCO ~$2,100/month including DevOps labor [COMPETITORS]; Kubernetes ecosystem has 2,500+ tools creating decision paralysis [CUSTOMER_RESEARCH]
- **Metric:** Time to first deployment (target: <10 minutes; measured from `git clone` to first proxied request)
- **Competitive context:** LiteLLM needs K8s for production; Helicone requires 4 containers; Portkey requires K8s; no competitor achieves sub-10-minute single-VPS deployment

### VP3: Hard Budget Caps That Actually Stop Spending
- **Claim:** Automatic request cutoff when budget limit reached; no surprise bills, no overage charges, no runaway agent loops
- **Evidence:** 80% of enterprises miss AI forecasts by >25% [MARKET]; "A runaway loop stopped at 100% of budget causes a fraction of the damage it would cause running overnight unchecked" [CUSTOMER_RESEARCH]; startups report daily bill spikes of 20-30% when features go viral [CUSTOMER_RESEARCH]
- **Metric:** Budget enforcement accuracy (target: 100% — zero overspend events when budget cap configured)
- **Competitive context:** Helicone has alerts but no hard caps at Pro tier; Portkey has alerts but no automatic cutoff at lower tiers; Braintrust alerts at 80/90/100% but no enforcement; this is a validated gap

### VP4: Complete Cost Visibility Without External Tools
- **Claim:** Built-in analytics dashboard showing per-request, per-team, per-project cost breakdown; no need for Datadog, Langfuse, or separate observability stack
- **Evidence:** LiteLLM requires external tools (Langfuse, Datadog) for basic dashboards [COMPETITORS]; "No visibility into which team/project is spending what" is universal complaint [CUSTOMER_RESEARCH]
- **Metric:** Dashboard usage (target: 90% of active deployments view dashboard weekly)
- **Competitive context:** LiteLLM has no built-in observability UI; Cloudflare has basic analytics; Helicone has excellent observability but in maintenance mode [COMPETITORS]

### VP5: Your Data Stays on Your Infrastructure
- **Claim:** Self-hosted on your VPS; all request/response data, API keys, and logs remain on your infrastructure; zero data residency concerns
- **Evidence:** 67% of enterprises actively planning to repatriate AI workloads [MARKET]; OpenRouter is SaaS-only (all traffic routes through their infrastructure); Cloudflare captures all request/response data [COMPETITORS]; financial services cite "inadvertent data sharing could have serious compliance implications" [CUSTOMER_RESEARCH]
- **Metric:** Self-hosted deployment ratio (target: >80% of active deployments are self-hosted)
- **Competitive context:** OpenRouter SaaS-only; Cloudflare platform-locked; managed SaaS competitors create vendor lock-in; self-hosting is our core differentiator

---

## 4. Feature Specification

### 4.1 P0 — Must Have (Ship Without = Fail)

These are table stakes. Every competitor has these. Missing any one means the product is not a viable AI Gateway.

| # | Feature | Description | Rationale | Customer Evidence | Competitive Context |
|---|---------|-------------|-----------|-------------------|---------------------|
| P0-1 | **OpenAI-Compatible Unified API** | Single `/v1/chat/completions` endpoint; drop-in replacement for OpenAI SDK by changing base URL only | Table stakes for gateway category. All 6 competitors offer this. | "One API key, unified interface" [CUSTOMER_RESEARCH]; teams expect zero code changes beyond base URL | All competitors (OpenRouter, LiteLLM, Cloudflare, Helicone, Portkey, Braintrust) provide this |
| P0-2 | **Multi-Provider Routing** | Route requests to OpenAI, Anthropic, Gemini, Ollama (local), and Groq with provider-agnostic config | Core gateway function. Customers use 3-5+ providers simultaneously. | "I'm juggling OpenAI, Claude, Gemini, and Grok in different projects" [CUSTOMER_RESEARCH]; teams use 3-5+ providers [CUSTOMER_RESEARCH] | LiteLLM: 100+ providers; OpenRouter: 500+ models; Cloudflare: 20+ native; target 10+ at launch |
| P0-3 | **Request/Response Logging** | Log every request with timestamp, provider, model, tokens, latency, cost, status | Minimum observability. Every competitor has this. Production debugging is impossible without it. | "Real-time token-level monitoring" required [CUSTOMER_RESEARCH]; "1M+ logs in DB needed for production" [CUSTOMER_RESEARCH] | Helicone excels here; LiteLLM has no built-in UI for this; Cloudflare has functional logging |
| P0-4 | **Per-Request Cost Tracking** | Calculate and store exact cost per request based on provider pricing tables | Core value proposition. Cost visibility is the #2 pain point. Without this, 30-70% reduction claim is unprovable. | "Real-time token-level monitoring and attribution" [CUSTOMER_RESEARCH]; per-team/per-project breakdown required [CUSTOMER_RESEARCH] | Helicone has automatic cost tracking; Portkey tracks 40+ data points per request; table stakes |
| P0-5 | **Streaming Support** | Proxy SSE streams from providers without buffering; transparent streaming pass-through | Expected by modern LLM applications. Non-negotiable for chat interfaces. | All competitors support streaming; expected by all modern AI apps | Universal across competitors; absence = immediate disqualification |
| P0-6 | **Virtual API Keys** | Create and manage gateway-specific API keys with per-key rate limits and model restrictions | Enables team usage patterns; security isolation. LiteLLM and Portkey both have this. | "Running 3 agents... realized recently they all share keys" [CUSTOMER_RESEARCH]; API key sprawl is medium-high severity pain | LiteLLM: virtual keys with budget limits; Portkey: virtual keys with restrictions; table stakes |
| P0-7 | **Automatic Fallback / Retry** | On provider error or rate limit, automatically retry with fallback provider; configurable retry count and backoff | Reliability at production scale. All major competitors offer this. | "When a primary provider hits rate limits or returns errors, reroutes" [CUSTOMER_RESEARCH]; provider outages are common buying trigger | OpenRouter: auto provider switch; LiteLLM: configurable fallbacks; Helicone: health-aware routing |
| P0-8 | **Rate Limiting (Request + Token)** | Per-key rate limits: requests per minute, tokens per minute, configurable windows | Prevent runaway costs; protect providers from overload. Cloudflare, LiteLLM, Helicone all have this. | "Token-based and request-based rate limits" [CUSTOMER_RESEARCH]; free tiers "fragile with timeouts" [CUSTOMER_RESEARCH] | Universal across competitors; minimum for production use |
| P0-9 | **Exact-Match Caching** | Cache request/response pairs by exact content hash; Redis-backed; configurable TTL | Minimum cost reduction feature. Cloudflare and LiteLLM both offer. Without caching, 30-70% reduction is unachievable. | "Semantic caching delivers 40-80% cost reduction" [CUSTOMER_RESEARCH]; "large percentage of LLM requests are repetitive" [CUSTOMER_RESEARCH] | Cloudflare: built-in; LiteLLM: Redis-based; Portkey: simple + semantic; table stakes |
| P0-10 | **React Admin Dashboard** | Web UI for viewing request logs, cost metrics, managing API keys, configuring providers; reads from PostgreSQL | LiteLLM's #1 pain point is requiring external tools for dashboards. This is a key differentiator. | LiteLLM requires Datadog/Langfuse for basic visibility [COMPETITORS]; "built-in admin dashboard with real-time stats" [CUSTOMER_RESEARCH] | LiteLLM has no built-in UI; Helicone has excellent dashboard; this is our differentiator vs LiteLLM |
| P0-11 | **PostgreSQL + Redis in Docker Compose** | Single `docker-compose up` spins up PostgreSQL (state), Redis (cache), and gateway; all services in one stack | Enables single-VPS deployment. Zero external dependencies. Core to <10-minute promise. | "Most solutions required complex setup" [CUSTOMER_RESEARCH]; Kubernetes has 2,500+ tools [CUSTOMER_RESEARCH] | No competitor bundles everything this simply; Helicone needs 4 containers; LiteLLM needs K8s |
| P0-12 | **Provider Health Checks** | Periodic health checks to each configured provider; mark providers up/down; circuit breaker pattern | Required for reliable fallback. Helicone has health-aware routing; we need parity. | Provider outages are common buying trigger [CUSTOMER_RESEARCH]; "Multi-model setups provide redundancy if one provider throttles or goes down" [CUSTOMER_RESEARCH] | Helicone: health-aware routing with circuit breaking; Portkey: auto fallbacks; required for production |

### 4.2 P1 — Differentiator (Why Customers Choose Us)

These are why customers choose this product over OpenRouter, LiteLLM, and Cloudflare. These earn the sale.

| # | Feature | Description | Rationale | Customer Evidence | Competitive Context |
|---|---------|-------------|-----------|-------------------|---------------------|
| P1-1 | **Semantic Caching** | Cache based on embedding similarity; semantically equivalent prompts return cached response; configurable similarity threshold | Portkey has this but it's enterprise-only. Major cost reduction lever (up to 95% savings in support workloads). | "Semantic caching delivers 40-80% cost reduction in support/Q&A workloads" [CUSTOMER_RESEARCH]; "Real traffic frequently contains multiple variations of the same question" [CUSTOMER_RESEARCH]; 47% spend reduction with budget-aware routing + semantic caching [CUSTOMER_RESEARCH] | Portkey: semantic cache (Pro+); Cloudflare: no semantic cache; Helicone: Redis cache only; LiteLLM: no semantic caching |
| P1-2 | **Hard Budget Caps with Auto-Cutoff** | When configured spend limit reached, gateway automatically rejects subsequent requests with clear error; not just alerts — stops spending | **Validated gap: No competitor offers this well at affordable tier.** This is the single strongest feature for cost-conscious buyers. | "A runaway loop stopped at 100% of budget causes a fraction of the damage it would cause running overnight unchecked" [CUSTOMER_RESEARCH]; 80% miss AI forecasts by >25% [MARKET]; hard limits requested universally [CUSTOMER_RESEARCH] | Helicone: alerts only at Pro; Portkey: no hard caps at lower tiers; Braintrust: alerts at 80/90/100% only; **NO competitor has affordable hard cutoff** |
| P1-3 | **Smart Cost-Aware Routing** | Route requests to cheapest capable model based on complexity heuristics; simple queries → cheaper models; complex queries → premium models | Directly delivers 30-70% cost reduction. No competitor offers this out-of-the-box without complex configuration. | "Route queries to different models based on complexity, cost, or performance" [CUSTOMER_RESEARCH]; "This is where model routing starts making financial sense" [CUSTOMER_RESEARCH]; 75% cost reduction potential [CUSTOMER_RESEARCH] | LiteLLM: latency/cost/weighted routing (manual config); Portkey: basic routing; **no competitor has automatic complexity-based routing** |
| P1-4 | **Budget Alerts (Email + Webhook)** | Configurable thresholds (50%, 75%, 90%, 100%); email notifications and webhook POSTs to Slack/Discord/custom endpoints | Gives teams visibility before cutoff. Braintrust and Helicone have basic versions; we do better with more channels. | Soft alert at 75%, hard limit at 100% requested [CUSTOMER_RESEARCH]; webhook integrations for team notifications | Braintrust: 80/90/100% alerts; Helicone: basic alerts; Portkey: spend alerts; we differentiate via channels + flexibility |
| P1-5 | **Team / Multi-User Support** | Multiple users per gateway instance; basic RBAC (admin, viewer); per-team cost attribution | Required for SME segment. Portkey and LiteLLM have this; our CE must include it to be "genuinely useful." | Per-team cost breakdown is critical pain [CUSTOMER_RESEARCH]; team-level usage quotas needed [MARKET] | LiteLLM: team budgets (OSS); Portkey: workspace isolation; Helicone: 5 orgs on Team; we include basic teams in CE |
| P1-6 | **One-Command Docker Compose** | `docker-compose up -d` after `git clone`. Gateway, PostgreSQL, Redis, and dashboard in one command. | **Core differentiator.** No competitor matches this simplicity. LiteLLM needs 20-40 hours; Helicone needs 4 containers. | "Deploy in under one hour guides are popular" [CUSTOMER_RESEARCH]; deployment complexity is #1 adoption barrier [CUSTOMER_RESEARCH] | LiteLLM: 20-40h setup; Helicone: 30+ min, 4 containers; Portkey: hours-days; **No competitor has single-command deployment** |
| P1-7 | **Configuration via UI (not YAML)** | Common settings editable through web dashboard; provider setup, rate limits, caching rules, budget caps — all UI-managed | LiteLLM's YAML-heavy configuration creates steep learning curve. UI config is major DX differentiator. | LiteLLM has steep learning curve due to YAML config [COMPETITORS]; environment variables + simple UI reduces friction [COMPETITORS] | LiteLLM: YAML config; Cloudflare: dashboard (but limited); our dashboard-first config is differentiator |
| P1-8 | **Usage Quotas per Team/Project** | Assign token/request/budget quotas to teams, projects, or virtual keys; enforce at gateway level | LiteLLM and Portkey both have team-level budgets; needed for SME governance without enterprise complexity. | Team-level quotas requested [CUSTOMER_RESEARCH]; "need for per-client cost tracking and billing" [MARKET] | LiteLLM: per key/user/team quotas; Portkey: virtual keys with budgets; feature parity at simpler UX |

### 4.3 P2 — Growth (Post-PMF Expansion)

Features that expand revenue per customer and open adjacent use cases. Build only after P0/P1 validate product-market fit.

| # | Feature | Description | Rationale | Customer Evidence | Competitive Context |
|---|---------|-------------|-----------|-------------------|---------------------|
| P2-1 | **Request Transformation / Middleware** | Modify requests before sending to provider (add headers, transform body); basic middleware hooks | Cloudflare offers this via Workers; enables advanced use cases without separate infrastructure. | Cloudflare: request transformation via Workers [COMPETITORS]; enables pre/post-processing | Cloudflare Workers integration is powerful but locked to CF stack; we offer open alternative |
| P2-2 | **Webhook Integrations** | Slack, Discord, Microsoft Teams, generic webhook for budget events, provider health changes, rate limit alerts | Teams need notifications in their existing channels; reduces need for separate monitoring. | Webhook notifications for budget events [CUSTOMER_RESEARCH]; Slack/Discord integration expected | Portkey: webhook support; Helicone: basic webhooks; we add more channels |
| P2-3 | **Import/Export Configurations** | Export gateway config as JSON; import from LiteLLM config format; migration tooling | Enables migration from LiteLLM and Helicone (especially Helicone maintenance mode exodus). | 16,000 organizations used Helicone and need migration paths [COMPETITORS]; LiteLLM users may switch if migration is easy | LiteLLM config is YAML-based; migration tooling captures Helicone exodus market |
| P2-4 | **Advanced Analytics** | Cost trends over time, per-model efficiency analysis, team comparison, cost prediction, anomaly detection | Operational teams need deeper analytics for FinOps practices. Premium tier driver. | FinOps teams managing AI spend doubled from 31% to 63% [MARKET]; "identify top cost contributors" [CUSTOMER_RESEARCH] | Braintrust: advanced analytics (at $249/mo); Helicone: cost analytics; Portkey: 40+ data points/req |
| P2-5 | **SAML 2.0 SSO** | Enterprise SSO via SAML 2.0; SCIM provisioning for user management | Required by mid-market (100-500 employees) for procurement. Enterprise tier gate. | Portkey and Braintrust lead on compliance [COMPETITORS]; SOC 2, GDPR alignment needed for regulated industries [CUSTOMER_RESEARCH] | Portkey: SOC-2, ISO 27001; Braintrust: SOC-2 Type II; needed for mid-market expansion |
| P2-6 | **Audit Logging** | Immutable audit trail of all gateway actions: config changes, key creation, access events; tamper-evident storage | Compliance requirement (SOC 2, ISO 27001). Enterprise tier driver. | "Audit trail, tracking what data was being sent externally, which provider received it, and who sent it" [CUSTOMER_RESEARCH] | Portkey: enterprise audit; Braintrust: compliance features; mid-market requirement |
| P2-7 | **Prompt Compression** | Automatically compress prompts before sending to provider; reduce token count by 20-40% | Additional cost reduction lever beyond routing and caching. Differentiator in cost-sensitive market. | "200-token verbose prompt becomes 120-token compressed" [CUSTOMER_RESEARCH]; 37% token reduction potential [CUSTOMER_RESEARCH] | Not offered by any major competitor; unique cost-reduction feature |
| P2-8 | **Managed SaaS Hosting** | We host and manage the gateway; customer brings their provider API keys; zero infrastructure burden | ~70% gross margins; removes primary objection to self-hosted infrastructure. Revenue stream. | Managed hosting has ~70% gross margins [MONETIZATION]; "we'll run it for you" is compelling for time-constrained teams [MONETIZATION] | OpenRouter is SaaS-only; Helicone: cloud-hosted; Portkey: managed; we offer choice |

### 4.4 P3 — Future (Nice to Have, Revisit Later)

| # | Feature | Description | Rationale | When to Revisit |
|---|---------|-------------|-----------|-----------------|
| P3-1 | **Multi-Modal Routing** | Route image generation, audio transcription, and video requests alongside text | Adds complexity; text-only covers 80%+ of SME use cases | After 1,000 active text deployments |
| P3-2 | **Prompt Versioning & A/B Testing** | Version control for prompt templates; route % of traffic to different prompt versions | Enterprise feature; adds UI complexity; Braintrust owns this at $249/mo | After Enterprise tier launches |
| P3-3 | **Content Guardrails** | Basic PII detection, content filtering, request validation | Portkey has 50+ guardrails but this is enterprise/regulatory territory; adds massive complexity | After mid-market PMF validated |
| P3-4 | **Custom Provider Integration SDK** | SDK for adding custom/local providers not in default set | LiteLLM supports 100+ providers via Python handlers; we target top 10-15 initially | After 10+ built-in providers shipped |
| P3-5 | **Model Recommendation Engine** | "For this type of query, use X model on Y provider for best cost/quality ratio" | Advanced ML feature; high value but complex to implement well | After semantic caching and smart routing proven |
| P3-6 | **EU/US Data Residency Selection** | Choose data storage region for GDPR compliance | Only relevant at mid-market scale; most SMEs don't need this initially | After first EU enterprise customer |

### 4.5 Explicitly Excluded (What We Will NOT Build)

| Feature | Why Excluded | Evidence |
|---------|-------------|----------|
| **ChatGPT clone / chatbot UI** | Different product category. Our product is infrastructure, not end-user application. Distracts from core gateway. | Customer research: "prompt playground" is "don't care" category [CUSTOMER_RESEARCH]; many standalone tools exist |
| **Workflow automation / agent framework** | Competes with LangChain, CrewAI, n8n. Gateway should route requests, not execute workflows. Scope creep into platform. | Customer research: "workflow automation" not requested by target segment [CUSTOMER_RESEARCH] |
| **RAG platform / vector database** | Competes with Pinecone, Weaviate, pgvector. Different product category entirely. Not a gateway concern. | Customer research: "RAG/vector database" explicitly excluded [MARKET]; "teams wanting RAG/vector database are out of scope" [MARKET] |
| **Prompt engineering platform** | Braintrust and Portkey own this. Adds massive UI complexity. "Evaluation is a separate concern" [COMPETITORS]. | Customer research: "collaborative prompt editing" is "don't care" [CUSTOMER_RESEARCH]; Braintrust evals at $249/mo |
| **Model training / fine-tuning** | Competes with Replicate, Together AI, Baseten. Gateway routes to providers, not trains models. | Customer research: "fine-tuning capabilities" is "don't care" [CUSTOMER_RESEARCH]; "teams wanting model training are out of scope" [MARKET] |
| **Kubernetes-native deployment** | Our differentiator is NOT requiring Kubernetes. K8s shops have 20+ better options (Kong, Gloo, Ambassador). | Customer research: "Kubernetes-native" is "don't care" [CUSTOMER_RESEARCH]; "no Kubernetes required" is explicit differentiator [VISION] |
| **Graph databases** | PostgreSQL handles all relational + analytics needs. Graph DBs add operational complexity with no clear benefit for gateway use case. | Architecture constraint: PostgreSQL + Redis only [VISION]; no query pattern requires graph traversal |
| **Event sourcing / CQRS** | Over-engineered for gateway. Monolith + PostgreSQL is sufficient. New engineer understands system in <1 day. | Architecture constraint: understood by new engineer in <1 day [VISION]; event sourcing takes weeks to understand |
| **Kafka / message queue infrastructure** | PostgreSQL can handle queue patterns for target scale. Kafka adds operational complexity incompatible with single-VPS deployment. | Architecture constraint: single VPS [VISION]; Redis pub/sub sufficient for async needs |
| **Multi-region cloud deployment** | Enterprise feature (Fortune 500). SME target doesn't need this. Diverts engineering from core simplicity. | Market: "multi-region cloud" is enterprise requirement [MARKET]; not needed for 20-500 employee companies |
| **MCP (Model Context Protocol) gateway** | Emerging standard with limited adoption in 2026 [COMPETITORS]. True adoption unclear. Wait for market validation. | Competitor analysis: "limited adoption in 2026" [COMPETITORS]; Portkey marketing MCP but actual adoption low |
| **Per-seat pricing model** | Doesn't correlate with value. AI usage scales with volume, not headcount. Braintrust proves unlimited-seat model works. | Customer research: "per-seat pricing (disliked)" [CUSTOMER_RESEARCH]; flat-rate preferred [MARKET] |
| **Air-gapped / offline enterprise deployment** | Enterprise-only requirement; delays launch by months. Handle at Enterprise Plus tier only. | Market: "air-gapped deployment" is enterprise requirement [MARKET]; delays core product |
| **Service mesh integration** | Istio/Linkerd integration is enterprise Kubernetes territory. Out of scope for SME gateway. | Architecture constraint: no service mesh [anti-goals]; K8s users have better options |

---

## 5. Feature Moat Analysis

### Primary Moat Feature: Semantic Caching + Cost Data Accumulation

**Which feature creates the strongest defensibility:** The combination of semantic caching + historical cost/routing data accumulated across all requests.

**Why it is hard to copy:**

1. **Data compounding:** Every request processed improves the caching model and routing decisions. Cost baselines per model/provider/temperature are refined over time. A new competitor starts with zero data; we improve with every request.

2. **Semantic cache warm-up cost:** Building an effective semantic cache requires significant traffic volume to learn which prompts are similar. This creates a cold-start barrier for competitors. Our deployments accumulate cache entries; new entrants have empty caches.

3. **Provider pricing intelligence:** We maintain accurate per-provider pricing tables and update them as providers change pricing. This is operational overhead that compounds: the more providers we support, the more accurate our cost routing becomes.

4. **Rust performance barrier:** Semantic caching requires fast embedding computation. Our Rust backend achieves <5ms latency overhead. A Python-based competitor (like LiteLLM) cannot match this without significant re-architecture.

**How it compounds over time:**

| Timeframe | Cache Hit Rate | Cost Reduction | Competitive Barrier |
|-----------|---------------|----------------|---------------------|
| Month 1 (new deployment) | 5-10% | 5-10% | Low — any competitor can match |
| Month 3 (established) | 15-25% | 15-25% | Medium — cache warmed, routing tuned |
| Month 6 (mature) | 25-40% | 25-45% | High — rich cost data, semantic patterns learned |
| Month 12+ (deep) | 40-60% | 35-70% | Very High — switching means losing all optimization data |

**Secondary moat:** The single-command deployment experience (P1-6). This is a product design moat, not a data moat. It compounds through: (a) content marketing flywheel (every "deployed in 10 minutes" story is free marketing), (b) word-of-mouth in SME communities, (c) GitHub stars attracting contributors, (d) deployment simplicity becoming a brand promise that competitors cannot easily replicate without architectural overhaul.

---

## 6. Release Phases

### MVP — Month 1-3: "It Routes, It Caches, It Shows Costs"

**Goal:** A developer can `git clone && docker-compose up` and route traffic through the gateway with cost tracking in under 10 minutes. Validate deployment promise and basic functionality.

**P0 Features Included:**
- P0-1: OpenAI-compatible unified API
- P0-2: Multi-provider routing (OpenAI, Anthropic, Gemini, Ollama minimum)
- P0-3: Request/response logging
- P0-4: Per-request cost tracking
- P0-5: Streaming support
- P0-6: Virtual API keys
- P0-7: Automatic fallback/retry
- P0-8: Rate limiting (request + token)
- P0-9: Exact-match caching
- P0-10: React admin dashboard (logs, costs, key management)
- P0-11: PostgreSQL + Redis in Docker Compose
- P0-12: Provider health checks

**MVP Success Criteria:**
- Time to deployment: <10 minutes (measured)
- Gateway latency overhead: <5ms (measured)
- Cache hit rate: >5% (baseline)
- 100+ GitHub stars within 90 days of public release

### V1 — Month 4-6: "It Cuts Your AI Bill"

**Goal:** The cost reduction promise is measurable and provable. Product has clear ROI narrative. Launch paid Professional tier.

**P1 Features Included:**
- P1-1: Semantic caching
- P1-2: Hard budget caps with auto-cutoff
- P1-3: Smart cost-aware routing
- P1-4: Budget alerts (email + webhook)
- P1-5: Team / multi-user support
- P1-6: One-command Docker Compose (polished, documented)
- P1-7: Configuration via UI
- P1-8: Usage quotas per team/project

**V1 Success Criteria:**
- Average cost reduction: >30% (measured across beta deployments)
- Budget enforcement: 100% accuracy (zero overspend when cap configured)
- First 10 paying Professional customers
- Semantic cache hit rate: >15%
- Product-Market Fit survey: 40%+ of CE users would be "very disappointed" if product disappeared

### V2 — Month 7-12: "It Grows With Your Organization"

**Goal:** Expand into mid-market (100-500 employees). Capture Helicone migration users. Launch Enterprise tier and managed hosting.

**P2 Features Included:**
- P2-1: Request transformation / middleware
- P2-2: Webhook integrations (Slack, Discord, Teams)
- P2-3: Import/export configurations (LiteLLM migration tool)
- P2-4: Advanced analytics (trends, predictions, anomaly detection)
- P2-5: SAML 2.0 SSO
- P2-6: Audit logging
- P2-7: Prompt compression
- P2-8: Managed SaaS hosting option

**V2 Success Criteria:**
- 50+ paying customers across all tiers
- $10K MRR
- 1,000+ active self-hosted deployments of Community Edition
- Cache hit rate: >25% (average across deployments)
- Appears in at least 3 "AI Gateway comparison" articles alongside OpenRouter, LiteLLM, Cloudflare

### Intentionally Deferred Beyond Year 1

| Feature | Deferred To | Rationale |
|---------|-------------|-----------|
| Multi-modal routing (images, audio, video) | Year 2 | Text covers 80%+ of SME use cases; adds significant API complexity |
| Prompt versioning & A/B testing | Year 2+ | Enterprise feature; adds massive UI complexity; not requested by target segment |
| Content guardrails / PII detection | Year 2+ | Enterprise/regulatory feature; adds massive complexity; not a purchase driver for SMEs |
| MCP gateway support | Year 2+ | Emerging standard with limited adoption; wait for market validation |
| Model recommendation engine | Year 2+ | Requires significant ML investment; build after core routing is mature |
| EU/US data residency | Year 2+ | Only relevant at mid-market scale; most SMEs don't need this |
| Air-gapped deployment | Enterprise Plus only | Enterprise-only; would delay launch by months |
| White-label / OEM | Year 3 | Agency market expansion; not core to initial product-market fit |

---

## 7. Success Metrics

### 7.1 North Star Metric

| Metric | Definition | Target |
|--------|-----------|--------|
| **Monthly AI Spend Avoided** | Sum of (routing savings + cache hit savings) across all active deployments, in USD | Month 3: $1K; Month 6: $10K; Month 12: $100K |

### 7.2 Feature-to-Metric Mapping

| Feature | Primary Metric | Target | Measurement Method |
|---------|---------------|--------|-------------------|
| P1-1 Semantic Caching | Cache hit rate | >20% avg | Cached requests / total requests |
| P1-2 Hard Budget Caps | Budget enforcement accuracy | 100% | Zero overspend events when cap set |
| P1-3 Smart Routing | Cost reduction per request | >30% avg | (Direct cost - Routed cost) / Direct cost |
| P0-4 Cost Tracking | Dashboard engagement | 90% weekly | % deployments with dashboard view in past 7 days |
| P0-11 Docker Compose | Time to deployment | <10 min | Measured from `git clone` to first proxied request |
| P0-7 Fallback/Retry | Uptime improvement | >99.5% | Gateway availability when primary provider fails |
| P0-8 Rate Limiting | Rate limit errors prevented | >95% | Rejected requests that would have exceeded limits |
| P1-5 Team Support | Team adoption | 60% of paid | % paid customers with >1 active user |
| P1-4 Budget Alerts | Alert delivery rate | >99% | Alerts sent / alerts triggered |

### 7.3 Phase-Level Outcomes

| Phase | Timeline | Key Outcome | Metric Target |
|-------|----------|-------------|---------------|
| **MVP** | Month 1-3 | Validate deployment promise | <10 min deploy; 100+ GitHub stars; latency <5ms |
| **V1** | Month 4-6 | Validate cost reduction promise | >30% avg cost reduction; 10 paying customers; PMF signal |
| **V2** | Month 7-12 | Validate market expansion | $10K MRR; 1,000 CE deployments; 50 paying customers |
| **Scale** | Month 12-18 | Category recognition | Featured in 3+ comparison articles; 40%+ would be "very disappointed" without product |

### 7.4 Business Metrics

| Metric | Month 3 | Month 6 | Month 12 |
|--------|---------|---------|----------|
| Active self-hosted CE deployments | 50 | 300 | 1,000+ |
| Paying customers | 0 | 10 | 50+ |
| MRR | $0 | $500 | $10,000 |
| Avg cost reduction per deployment | 10% | 30% | 40% |
| Monthly AI Spend Avoided | $1,000 | $10,000 | $100,000 |
| GitHub stars | 100 | 500 | 2,000+ |
| Churn (monthly) | N/A | <5% | <3.5% |
| Free-to-paid conversion rate | N/A | 2% | 3-5% |

### 7.5 Anti-Metrics (Things We Track to Avoid Bad Outcomes)

| Metric | Warning Threshold | Action Triggered |
|--------|-------------------|------------------|
| Avg deployment time >15 min | Month 3 | Simplify Docker Compose; improve docs |
| Cache hit rate <10% | Month 6 | Review caching heuristics; add tuning guidance |
| Budget overspend events >0 | Always | P0 bug; hard cap must be 100% reliable |
| Support tickets >20/month (solo founder) | Month 6 | Add FAQ; improve docs; consider community support hire |
| Free-to-paid conversion <2% | Month 6 | Review pricing; improve in-app upgrade prompts |
| Churn >5%/month | Month 6 | Interview churned customers; review product-market fit |

---

## Appendix A: Competitive Differentiation Checklist

### vs. OpenRouter (SaaS-only, 5% markup)
- [x] Self-hosted (data stays on your server) — THEY CANNOT MATCH THIS
- [x] Zero markup on inference — they ADD 5% cost
- [x] Hard budget caps — they have prepaid credits but no overspend protection
- [x] Deploy on any VPS — they require routing through their infrastructure
- [ ] Model breadth — they have 500+ models; we target 10-15 at launch

### vs. LiteLLM (open source, complex self-host)
- [x] Deploy in 10 minutes — they require 20-40 hours
- [x] No Kubernetes — they require K8s for production
- [x] Built-in dashboard — they require external tools
- [x] No DevOps required — they need 0.375 FTE DevOps ($93K/yr)
- [x] Rust performance — Python has import speed and latency issues
- [ ] Provider count — they support 100+; we target 10-15 at launch

### vs. Cloudflare AI Gateway (platform-locked)
- [x] Infrastructure-agnostic — they require Cloudflare stack
- [x] Full data control — they capture all traffic
- [x] Cost optimization routing — they have basic caching only
- [x] Hard budget caps — they have rate limiting only
- [ ] Edge latency — they have 300+ PoP globally; we run on single VPS

### vs. Helicone (maintenance mode)
- [x] Actively developed — they are in maintenance mode post-Mintlify acquisition
- [x] Gateway-first design — they are observability-first
- [x] Single VPS deployment — they need 4 containers
- [x] Hard budget caps — they have alerts only
- [ ] Observability depth — they have best-in-class logging

### vs. Portkey (enterprise-focused)
- [x] 10x simpler — they have steep learning curve from feature breadth
- [x] No Kubernetes — they recommend K8s
- [x] Predictable pricing — they have log-based unpredictability
- [x] Focused scope — they bundle guardrails, prompt management, evals
- [ ] Feature breadth — they have 1,600+ models and 50+ guardrails

---

## Appendix B: Architecture Constraints (Non-Negotiable)

| Constraint | Rationale | Enforcement |
|-----------|-----------|-------------|
| Deploy in <10 min on single VPS | Core product promise | Docker Compose only; no external service dependencies |
| Reduce AI spend 30-70% | Core value proposition | Must include P0-9 (caching) + P1-1 (semantic) + P1-3 (smart routing) |
| Operable by non-DevOps teams | Target market doesn't have DevOps | UI-first configuration; no YAML required for common tasks |
| No Kubernetes required | Explicit differentiator | Docker Compose only; K8s manifests not provided |
| Architecture understood by new engineer in <1 day | Solo founder constraint | Monolith; PostgreSQL + Redis only; no distributed systems |
| Maintainable by <5 engineers | Team size constraint | No microservices; no event sourcing; no CQRS; no service mesh |
| Open-core: CE genuinely useful | Business model viability | CE must include full gateway + routing + caching + basic observability |
| Rust backend, React+TS frontend, PostgreSQL, Redis | Tech stack lock | No Python, Node, MongoDB, ClickHouse, or other data stores |

---

*Document version: 1.0*
*Last updated: Product Definition Phase*
*Derived from: VISION.md, MARKET.md, COMPETITORS.md, MONETIZATION.md, CUSTOMER_RESEARCH.md*
