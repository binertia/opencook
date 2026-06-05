# Task Index

Master index of all implementation tasks for the AI Gateway project.

## Summary

| Metric | Count |
|--------|-------|
| Total Tasks | 100 |
| Epic-01: Project Bootstrap | 5 |
| Epic-02: Database Foundation | 6 |
| Epic-03: Authentication & Authorization | 8 |
| Epic-04: Provider Abstraction Layer | 6 |
| Epic-05: Request Proxy & OpenAI-Compatible API | 5 |
| Epic-06: Routing Engine | 5 |
| Epic-07: Caching Layer | 5 |
| Epic-08: Quota, Rate Limiting & Cost Tracking | 5 |
| Epic-09: Admin Dashboard — Foundation | 6 |
| Epic-10: Provider Management UI | 4 |
| Epic-11: API Key Management UI | 4 |
| Epic-12: Usage Analytics & Cost Dashboards | 4 |
| Epic-13: Fallback, Health Checks & Circuit Breaker | 4 |
| Epic-14: Semantic Caching | 4 |
| Epic-15: Webhooks & Alerts | 5 |
| Epic-16: Observability & Monitoring | 5 |
| Epic-17: Security Hardening | 5 |
| Epic-18: Production Deployment | 5 |
| Epic-19: Smart Routing | 3 |
| Epic-20: Team Collaboration & SSO | 4 |
| Cross-Cutting: Integration & Polish | 4 |

## Task Registry

| Task | Title | Epic | Priority | Effort | Dependencies | Status |
|------|-------|------|----------|--------|--------------|--------|
| TASK-0001 | Initialize Rust Workspace with Root Cargo.toml | Epic-01 | Critical | 0.5d | — | done |
| TASK-0002 | Set Up Docker Compose Development Environment | Epic-01 | Critical | 1d | TASK-0001 | done |
| TASK-0003 | Set Up CI/CD Pipeline with GitHub Actions | Epic-01 | Critical | 1d | TASK-0001 | todo |
| TASK-0004 | Configure Linting, Formatting, and Pre-Commit Hooks | Epic-01 | Critical | 1d | TASK-0001, TASK-0003 | todo |
| TASK-0005 | Create README.md and GitHub Issue Templates | Epic-01 | High | 0.5d | TASK-0001, TASK-0002 | todo |
| TASK-0006 | Create Database Migration Framework and Connection Pool | Epic-02 | Critical | 1d | TASK-0001, TASK-0002 | done |
| TASK-0007 | Create Organizations and Users Migrations (0001-0003) | Epic-02 | Critical | 1d | TASK-0006 | done |
| TASK-0008 | Create API Keys, Provider Configs, and Models Migrations (0004-0006) | Epic-02 | Critical | 1d | TASK-0007 | done |
| TASK-0009 | Create Routing Rules, Requests, and Responses Migrations (0007-0009) | Epic-02 | Critical | 1d | TASK-0008 | done |
| TASK-0010 | Create Usage, Quota, Webhook, and Audit Migrations (0010-0017) | Epic-02 | Critical | 1.5d | TASK-0009 | done |
| TASK-0011 | Create Indexes, Triggers, RLS Policies, and Seed Data (0018-0022) | Epic-02 | Critical | 1.5d | TASK-0010 | done |
| TASK-0012 | Implement Password Hashing and User Registration | Epic-03 | Critical | 1d | TASK-0007 | done |
| TASK-0013 | Implement JWT Session Authentication | Epic-03 | Critical | 1.5d | TASK-0012, TASK-0011 | done |
| TASK-0014 | Implement API Key Generation and Storage | Epic-03 | Critical | 1d | TASK-0008, TASK-0012 | done |
| TASK-0015 | Implement API Key Validation Middleware | Epic-03 | Critical | 1.5d | TASK-0014, TASK-0006 | done |
| TASK-0016 | Implement RBAC Permission System | Epic-03 | Critical | 1d | TASK-0013, TASK-0015 | done |
| TASK-0017 | Implement Account Lockout and Password Reset | Epic-03 | High | 1d | TASK-0013, TASK-0012 | done |
| TASK-0018 | Implement Key Revocation and Cache Invalidation | Epic-03 | High | 1d | TASK-0015, TASK-0016 | done |
| TASK-0019 | Implement Tenant Isolation Enforcement | Epic-03 | Critical | 1d | TASK-0015, TASK-0016, TASK-0006 | done |
| TASK-0020 | Define Provider Trait and Canonical Request/Response Types | Epic-04 | Critical | 1d | TASK-0001 | done |
| TASK-0021 | Implement OpenAI Provider Adapter | Epic-04 | Critical | 1.5d | TASK-0020, TASK-0008 | done |
| TASK-0022 | Implement Anthropic, Gemini, and Ollama Adapters | Epic-04 | Critical | 2d | TASK-0021, TASK-0020 | done |
| TASK-0023 | Implement Provider Config Encryption and Storage | Epic-04 | Critical | 1d | TASK-0008, TASK-0019 | done |
| TASK-0024 | Implement Model Registry with Pricing and Capabilities | Epic-04 | Critical | 1d | TASK-0023, TASK-0006 | done |
| TASK-0025 | Implement Provider Health Check Framework | Epic-04 | High | 1d | TASK-0021, TASK-0022, TASK-0006 | done |
| TASK-0026 | Set Up Axum HTTP Server with Middleware Stack | Epic-05 | Critical | 1d | TASK-0015, TASK-0013, TASK-0006 | done |
| TASK-0027 | Implement POST /v1/chat/completions (Non-Streaming) | Epic-05 | Critical | 1.5d | TASK-0026, TASK-0021, TASK-0024, TASK-0015 | done |
| TASK-0028 | Implement SSE Streaming for /v1/chat/completions | Epic-05 | Critical | 1.5d | TASK-0027, TASK-0021 | done |
| TASK-0029 | Implement Request Logging and Response Metadata | Epic-05 | Critical | 1d | TASK-0027, TASK-0009, TASK-0024 | done |
| TASK-0030 | Implement GET /v1/models and Health/Ready Endpoints | Epic-05 | Critical | 1d | TASK-0026, TASK-0024, TASK-0025 | done |
| TASK-0031 | Implement Routing Rule Data Model and Repository | Epic-06 | Critical | 1d | TASK-0009, TASK-0019, TASK-0016 | done |
| TASK-0032 | Implement Rule Evaluation Engine | Epic-06 | Critical | 1d | TASK-0031, TASK-0025, TASK-0020 | done |
| TASK-0033 | Integrate Routing Engine into Request Orchestrator | Epic-06 | Critical | 1d | TASK-0032, TASK-0027, TASK-0021 | done |
| TASK-0034 | Implement Weighted and Conditional Routing Strategies | Epic-06 | High | 1d | TASK-0032, TASK-0033 | done |
| TASK-0035 | Implement Routing Admin API and Cache Invalidation | Epic-06 | High | 1d | TASK-0031, TASK-0034, TASK-0016 | done |
| TASK-0036 | Implement Cache Key Builder and Cacheability Rules | Epic-07 | Critical | 1d | TASK-0020, TASK-0006 | done |
| TASK-0037 | Implement L1 In-Process Cache with moka | Epic-07 | Critical | 1d | TASK-0036, TASK-0001 | done |
| TASK-0038 | Implement L2 Redis Cache and Two-Tier Integration | Epic-07 | Critical | 1d | TASK-0037, TASK-0006 | done |
| TASK-0039 | Integrate Cache into Request Orchestrator | Epic-07 | Critical | 1d | TASK-0038, TASK-0027, TASK-0036 | done |
| TASK-0040 | Implement Cache Metrics and Analytics | Epic-07 | Medium | 1d | TASK-0039, TASK-0010 | done |
| TASK-0041 | Implement Sliding Window Rate Limiter with Redis Lua | Epic-08 | Critical | 1d | TASK-0006, TASK-0015 | done |
| TASK-0042 | Implement Quota Engine and Budget Caps | Epic-08 | Critical | 1d | TASK-0010, TASK-0019 | done |
| TASK-0043 | Implement Usage Aggregation Pipeline | Epic-08 | High | 1d | TASK-0010, TASK-0029 | done |
| TASK-0044 | Integrate Rate Limiting and Quota into Request Orchestrator | Epic-08 | Critical | 1d | TASK-0041, TASK-0042, TASK-0039 | done |
| TASK-0045 | Implement Quota and Budget Admin API | Epic-08 | High | 1d | TASK-0042, TASK-0043, TASK-0016 | done |
| TASK-0046 | Initialize React Dashboard with Vite, TypeScript, and shadcn/ui | Epic-09 | Critical | 1d | TASK-0001 | done |
| TASK-0047 | Implement API Client, Auth Hooks, and Login Page | Epic-09 | Critical | 1d | TASK-0046, TASK-0013 | done |
| TASK-0048 | Implement Dashboard Layout with Sidebar Navigation | Epic-09 | Critical | 1d | TASK-0046, TASK-0047 | done |
| TASK-0049 | Implement Organization Settings Page | Epic-09 | High | 1d | TASK-0048, TASK-0047 | done |
| TASK-0050 | Implement User Management and Invitation Flow | Epic-09 | High | 1.5d | TASK-0048, TASK-0016 | done |
| TASK-0051 | Implement Dashboard Overview Page with KPI Cards | Epic-09 | High | 1d | TASK-0048, TASK-0045, TASK-0047 | done |
| TASK-0052 | Implement Provider List Page with Health Status | Epic-10 | High | 1d | TASK-0048, TASK-0025, TASK-0047 | done |
| TASK-0053 | Implement Add/Edit Provider Wizard | Epic-10 | High | 1.5d | TASK-0052, TASK-0023, TASK-0047 | done |
| TASK-0054 | Implement Provider Detail Page with Model Management | Epic-10 | Medium | 1d | TASK-0053, TASK-0047 | done |
| TASK-0055 | Serve Dashboard as Static Files from Gateway Container | Epic-09 | Critical | 0.5d | TASK-0026, TASK-0046 | done |
| TASK-0056 | Implement API Key List Page with Status and Usage | Epic-11 | High | 1d | TASK-0048, TASK-0014, TASK-0047 | done |
| TASK-0057 | Implement API Key Creation with One-Time Display | Epic-11 | High | 1d | TASK-0056, TASK-0047 | done |
| TASK-0058 | Implement API Key Revocation and Edit | Epic-11 | High | 1d | TASK-0056, TASK-0018 | done |
| TASK-0059 | Implement Request Logs Viewer with Filtering | Epic-09 | Medium | 1.5d | TASK-0048, TASK-0029, TASK-0047 | done |
| TASK-0060 | Implement Cost Dashboard with Charts and KPIs | Epic-12 | High | 1.5d | TASK-0048, TASK-0045, TASK-0047 | done |
| TASK-0061 | Implement Token Usage and Cache Analytics Pages | Epic-12 | Medium | 1d | TASK-0060, TASK-0040 | done |
| TASK-0062 | Implement API Key Usage Table with Drill-Down | Epic-12 | Medium | 1d | TASK-0060, TASK-0045 | done |
| TASK-0063 | Implement Budget Configuration and Alert UI | Epic-12 | Medium | 1d | TASK-0049, TASK-0060 | done |
| TASK-0064 | Implement Retry Logic with Exponential Backoff | Epic-13 | Critical | 1d | TASK-0033, TASK-0021 | done |
| TASK-0065 | Implement Circuit Breaker Pattern | Epic-13 | Critical | 1d | TASK-0064, TASK-0032, TASK-0006 | done |
| TASK-0066 | Implement Request Cancellation and Fallback Chain | Epic-13 | Critical | 1d | TASK-0064, TASK-0065, TASK-0033 | done |
| TASK-0067 | Implement Health Check Background Worker | Epic-13 | High | 1d | TASK-0065, TASK-0025, TASK-0006 | done |
| TASK-0068 | Implement Vector Similarity Cache with pgvector | Epic-14 | Medium | 1.5d | TASK-0038, TASK-0021, TASK-0007 | todo |
| TASK-0069 | Integrate Semantic Cache into Request Orchestrator | Epic-14 | Medium | 1d | TASK-0068, TASK-0039 | todo |
| TASK-0070 | Implement Semantic Cache Background Maintenance | Epic-14 | Medium | 1d | TASK-0068, TASK-0069 | todo |
| TASK-0071 | Add Semantic Cache Configuration UI | Epic-14 | Low | 1d | TASK-0068, TASK-0049 | todo |
| TASK-0072 | Implement Webhook CRUD and Delivery System | Epic-15 | Medium | 1.5d | TASK-0013, TASK-0023, TASK-0016 | done |
| TASK-0073 | Implement Webhook Event Publisher and Retry Logic | Epic-15 | Medium | 1d | TASK-0072, TASK-0006 | done |
| TASK-0074 | Implement Quota and Budget Alert Webhooks | Epic-15 | Medium | 1d | TASK-0073, TASK-0042, TASK-0067 | done |
| TASK-0075 | Implement Webhook Management UI | Epic-15 | Medium | 1d | TASK-0072, TASK-0047 | done |
| TASK-0076 | Implement Webhook Delivery Retry UI | Epic-15 | Low | 1d | TASK-0075 | done |
| TASK-0077 | Implement Structured Request Logging with tracing | Epic-16 | Critical | 1d | TASK-0029, TASK-0001 | done |
| TASK-0078 | Implement Prometheus Metrics and /metrics Endpoint | Epic-16 | Critical | 1d | TASK-0026, TASK-0029 | done |
| TASK-0079 | Implement PII Redaction in Request/Response Logging | Epic-16 | Critical | 1d | TASK-0077, TASK-0026 | done |
| TASK-0080 | Build Admin Dashboard Grafana Export | Epic-16 | Medium | 1d | TASK-0078 | done |
| TASK-0081 | Implement Request Timing Middleware and Error Reporting | Epic-16 | Critical | 1d | TASK-0026, TASK-0077 | done |
| TASK-0082 | Implement Input Validation and Injection Protection | Epic-17 | Critical | 1d | TASK-0026, TASK-0041 | done |
| TASK-0083 | Implement TLS/HTTPS Configuration and Security Headers | Epic-17 | Critical | 1d | TASK-0026 | done |
| TASK-0084 | Implement Audit Log System | Epic-17 | Critical | 1d | TASK-0010, TASK-0016 | done |
| TASK-0085 | Implement Secrets Rotation and Master Key Management | Epic-17 | High | 1d | TASK-0013, TASK-0023 | done |
| TASK-0086 | Implement CORS, CSRF Protection, and Security Audit | Epic-17 | High | 1d | TASK-0082, TASK-0083 | done |
| TASK-0087 | Create Production Dockerfile and Docker Compose | Epic-18 | Critical | 1d | TASK-0001, TASK-0055 | done |
| TASK-0088 | Create Kubernetes Deployment Manifests | Epic-18 | Critical | 1d | TASK-0087 | done |
| TASK-0089 | Implement Zero-Downtime Deployment with Signal Handling | Epic-18 | Critical | 1d | TASK-0087 | done |
| TASK-0090 | Create Terraform/Helm Infrastructure Definition | Epic-18 | Medium | 1.5d | TASK-0088 | done |
| TASK-0091 | Create Database Backup and Migration Strategy | Epic-18 | High | 1d | TASK-0011 | done |
| TASK-0092 | Implement Per-Model Pricing and Cost-Optimized Routing | Epic-19 | Medium | 1d | TASK-0034, TASK-0024, TASK-0049 | done |
| TASK-0093 | Implement Provider Latency Tracking and Latency-Based Routing | Epic-19 | Medium | 1d | TASK-0034, TASK-0078 | done |
| TASK-0094 | Implement Quality and Balanced Routing Strategies | Epic-19 | Low | 1d | TASK-0092, TASK-0093 | done |
| TASK-0095 | Implement Multi-Organization Support and Org Switching | Epic-20 | Medium | 1.5d | TASK-0007, TASK-0013, TASK-0019 | done |
| TASK-0096 | Implement SAML 2.0 and OIDC SSO Integration | Epic-20 | Medium | 2d | TASK-0095, TASK-0013 | todo |
| TASK-0097 | Implement SCIM 2.0 User Provisioning | Epic-20 | Low | 1.5d | TASK-0096, TASK-0084 | todo |
| TASK-0098 | Implement Audit Log Dashboard and Admin Actions Log | Epic-20 | Medium | 1d | TASK-0084, TASK-0047, TASK-0048 | todo |
| TASK-0099 | Implement End-to-End Integration Tests | Cross-Cutting | Critical | 2d | All backend tasks | todo |
| TASK-0100 | Final Documentation and Release Checklist | Cross-Cutting | High | 1.5d | All tasks | todo |
| TASK-0101 | Implement Dual-Database Support (PostgreSQL + SQLite) | Cross-Cutting | Critical | 2d | TASK-0006, TASK-0011 | done |
| TASK-0102 | Implement SOLO Mode Binary (gateway-solo) | Cross-Cutting | High | 2d | TASK-0101, TASK-0027, TASK-0045 | done |

## Dependency Graph

```
Epic-01: Project Bootstrap
  TASK-0001 (Workspace)
    --> TASK-0002 (Docker)
    --> TASK-0003 (CI/CD)
    --> TASK-0004 (Linting)
    --> TASK-0005 (README)

Epic-02: Database Foundation
  TASK-0006 (Migration Framework)
    --> TASK-0007 (Migrations 0001-0003)
      --> TASK-0008 (Migrations 0004-0006)
        --> TASK-0009 (Migrations 0007-0009)
          --> TASK-0010 (Migrations 0010-0017)
            --> TASK-0011 (Migrations 0018-0022 + Seed)

Epic-03: Authentication
  TASK-0012 (Password Hashing + Register)
    --> TASK-0013 (JWT Sessions)
      --> TASK-0016 (RBAC)
      --> TASK-0017 (Lockout + Reset)
    --> TASK-0014 (API Key Generation)
      --> TASK-0015 (API Key Validation)
        --> TASK-0016 (RBAC)
        --> TASK-0018 (Key Revocation)
        --> TASK-0019 (Tenant Isolation)
  TASK-0011 (DB Migrations complete)
    --> TASK-0013 (JWT needs sessions table)

Epic-04: Provider Abstraction
  TASK-0020 (Provider Trait + Types)
    --> TASK-0021 (OpenAI Adapter)
      --> TASK-0022 (Anthropic/Gemini/Ollama)
      --> TASK-0025 (Health Checks)
    --> TASK-0023 (Provider Encryption + Config Repo)
      --> TASK-0024 (Model Registry)
        --> TASK-0025 (Health Checks)

Epic-05: Request Proxy
  TASK-0026 (Axum Server)
    --> TASK-0027 (Chat Completions)
      --> TASK-0028 (Streaming)
      --> TASK-0029 (Request Logging)
    --> TASK-0030 (Models + Health Endpoints)

Epic-06: Routing Engine
  TASK-0031 (Routing Rule Model)
    --> TASK-0032 (Rule Evaluation Engine)
      --> TASK-0033 (Orchestrator Integration)
        --> TASK-0034 (Weighted + Conditional Strategies)
          --> TASK-0035 (Routing Admin API)

Epic-07: Caching Layer
  TASK-0036 (Cache Key Builder)
    --> TASK-0037 (L1 In-Process Cache)
      --> TASK-0038 (L2 Redis + Two-Tier)
        --> TASK-0039 (Cache Integration)
          --> TASK-0040 (Cache Metrics + Analytics)

Epic-08: Quota + Billing
  TASK-0041 (Rate Limiter)
    --> TASK-0044 (Integration)
  TASK-0042 (Quota Engine)
    --> TASK-0044 (Integration)
    --> TASK-0045 (Quota Admin API)
  TASK-0043 (Usage Aggregation)
    --> TASK-0045 (Admin API)

Epic-09: Dashboard Foundation
  TASK-0046 (React Setup)
    --> TASK-0047 (API Client + Login)
      --> TASK-0048 (Dashboard Layout)
        --> TASK-0049 (Org Settings)
        --> TASK-0050 (User Management)
        --> TASK-0051 (Overview KPIs)

Epic-10: Provider UI
  TASK-0052 (Provider List)
    --> TASK-0053 (Add/Edit Wizard)
      --> TASK-0054 (Provider Detail)
  TASK-0055 (Static File Serving)

Epic-11: API Key UI
  TASK-0056 (Key List)
    --> TASK-0057 (Key Creation)
    --> TASK-0058 (Key Revocation + Edit)

Epic-12: Analytics
  TASK-0060 (Cost Dashboard)
    --> TASK-0061 (Token + Cache Analytics)
    --> TASK-0062 (Key Usage Drill-Down)
    --> TASK-0063 (Budget Config)

Epic-13: Fallback + Circuit Breaker
  TASK-0064 (Retry Logic)
    --> TASK-0065 (Circuit Breaker)
      --> TASK-0066 (Cancellation + Fallback)
    --> TASK-0067 (Health Worker)

Epic-14: Semantic Caching
  TASK-0068 (Vector Cache)
    --> TASK-0069 (Semantic Integration)
      --> TASK-0070 (Cache Maintenance)
    --> TASK-0071 (Semantic Cache UI)

Epic-15: Webhooks
  TASK-0072 (Webhook CRUD)
    --> TASK-0073 (Event Publisher)
      --> TASK-0074 (Alert Webhooks)
    --> TASK-0075 (Webhook UI)
      --> TASK-0076 (Delivery Retry UI)

Epic-16: Observability
  TASK-0077 (Tracing + Logs)
    --> TASK-0079 (PII Redaction)
    --> TASK-0081 (Timing Middleware)
  TASK-0078 (Prometheus Metrics)
    --> TASK-0080 (Grafana Dashboard)

Epic-17: Security
  TASK-0082 (Input Validation)
    --> TASK-0086 (CORS + CSRF + Audit)
  TASK-0083 (TLS + Headers)
    --> TASK-0086
  TASK-0084 (Audit Log)
    --> TASK-0098 (Audit Log UI)
  TASK-0085 (Secrets Rotation)

Epic-18: Deployment
  TASK-0087 (Production Dockerfile)
    --> TASK-0088 (K8s Manifests)
      --> TASK-0090 (Terraform + Helm)
    --> TASK-0089 (Zero-Downtime Deploy)
  TASK-0091 (Backup + Migration Strategy)

Epic-19: Smart Routing
  TASK-0092 (Cost Routing)
  TASK-0093 (Latency Routing)
    --> TASK-0094 (Quality + Balanced)

Epic-20: Team + SSO
  TASK-0095 (Multi-Org)
    --> TASK-0096 (SSO)
      --> TASK-0097 (SCIM)
    --> TASK-0098 (Audit Log Dashboard)

Cross-Cutting
  All Backend Tasks --> TASK-0099 (E2E Tests)
  All Tasks --> TASK-0100 (Documentation + Release)
  TASK-0006 (Migration Framework) --> TASK-0101 (Dual-Database)
    --> TASK-0102 (SOLO Mode)
```

## Critical Path

The following tasks form the critical path for a minimum viable product:

```
TASK-0001 → TASK-0006 → TASK-0007 → TASK-0008 → TASK-0009 → TASK-0010 → TASK-0011
    → TASK-0012 → TASK-0013 → TASK-0014 → TASK-0015 → TASK-0019
    → TASK-0020 → TASK-0021 → TASK-0023 → TASK-0024
    → TASK-0026 → TASK-0027 → TASK-0029 → TASK-0030
    → TASK-0041 → TASK-0042 → TASK-0044
    → TASK-0046 → TASK-0047 → TASK-0048 → TASK-0051 → TASK-0055
    → TASK-0099 → TASK-0100
```

## Suggested Execution Order

### Phase 1: Foundation (Weeks 1-2)
- Epic-01: TASK-0001 through TASK-0005
- Epic-02: TASK-0006 through TASK-0011

### Phase 2: Core Backend (Weeks 3-5)
- Epic-03: TASK-0012 through TASK-0019
- Epic-04: TASK-0020 through TASK-0025
- Epic-05: TASK-0026 through TASK-0030

### Phase 3: Routing + Cache + Quota (Weeks 6-7)
- Epic-06: TASK-0031 through TASK-0035
- Epic-07: TASK-0036 through TASK-0040
- Epic-08: TASK-0041 through TASK-0045

### Phase 4: Dashboard (Weeks 8-10)
- Epic-09: TASK-0046 through TASK-0051
- Epic-10: TASK-0052 through TASK-0055
- Epic-11: TASK-0056 through TASK-0059

### Phase 5: Reliability + Advanced Features (Weeks 11-13)
- Epic-13: TASK-0064 through TASK-0067
- Epic-12: TASK-0060 through TASK-0063
- Epic-14: TASK-0068 through TASK-0071
- Epic-15: TASK-0072 through TASK-0076

### Phase 6: Observability + Security + Deploy (Weeks 14-15)
- Epic-16: TASK-0077 through TASK-0081
- Epic-17: TASK-0082 through TASK-0086
- Epic-18: TASK-0087 through TASK-0091

### Phase 7: Enterprise Features (Weeks 16-17)
- Epic-19: TASK-0092 through TASK-0094
- Epic-20: TASK-0095 through TASK-0098

### Phase 8: Final Polish (Week 18)
- TASK-0099: E2E Tests
- TASK-0100: Documentation + Release
- TASK-0101: Dual-Database Support
- TASK-0102: SOLO Mode Binary

## Parallelization Opportunities

The following epic groups can be developed in parallel once their dependencies are met:

1. **Frontend (Epic-09 through Epic-12)** can proceed in parallel with backend work
   - Requires: backend API endpoints from Epic-03, Epic-05, Epic-08

2. **Observability (Epic-16)** can be added incrementally
   - Requires: request logging (TASK-0029) and metrics (TASK-0078)

3. **Security hardening (Epic-17)** can run parallel to feature development
   - Requires: Axum server (TASK-0026) and auth (Epic-03)

4. **Deployment (Epic-18)** can be prepared while features are in development
   - Requires: production Dockerfile (TASK-0087) built last

5. **Advanced routing (Epic-19)** extends existing routing infrastructure
   - Requires: Epic-06 complete

6. **Enterprise features (Epic-20)** are largely independent
   - Requires: Epic-03 and basic dashboard (Epic-09)
