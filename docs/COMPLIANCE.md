# AI Gateway - Compliance Requirements and Controls

**Document ID:** COMP-AIGW-001  
**Version:** 1.0  
**Classification:** Internal Use  
**Owner:** Compliance Lead / CISO  
**Last Updated:** 2025-01-15

---

## 1. Applicable Frameworks

### 1.1 SOC 2 Type II

| Attribute | Detail |
|-----------|--------|
| **Applicability** | Required for B2B sales to enterprise customers; serves as baseline security assurance |
| **Priority** | P1 - Critical |
| **Timeline to Readiness** | 6-9 months to Type II report |

#### Applicable Trust Criteria

| TSC | Applicability | Key Requirements | Evidence Collection |
|-----|---------------|------------------|---------------------|
| **CC6.1 (Logical Access)** | Full | Role-based access; MFA; least privilege; quarterly access reviews | IAM policies; access review logs; MFA enrollment reports |
| **CC6.2 (Access Removal)** | Full | Timely deprovisioning within 24h; automated offboarding | HR termination tickets; IAM audit logs |
| **CC6.3 (Authentication)** | Full | Strong authentication for all systems; unique user IDs | SSO configuration; password policy; MFA logs |
| **CC6.6 (Encryption)** | Full | AES-256 at rest; TLS 1.2+ in transit; key rotation | Encryption configuration screenshots; cert expiry reports |
| **CC7.1 (Security Monitoring)** | Full | SIEM/IDS; anomaly detection; 24/7 alerting | Monitoring dashboards; alert runbooks; response logs |
| **CC7.2 (Vulnerability Management)** | Full | Monthly scans; patch SLAs (Critical: 7d, High: 30d) | Scan reports; patch tickets; SLAs met/missed |
| **CC8.1 (Change Management)** | Full | All changes require approval, testing, rollback plan | Change request tickets; deployment logs; approval chains |
| **A1.1 (Availability)** | Partial | 99.9% uptime SLA; DR plan tested annually | Uptime reports; DR test results; incident post-mortems |
| **A1.2 (Incident Response)** | Partial | IR plan documented; RTO < 4h, RPO < 1h | IR plan; test results; actual recovery metrics |
| **C1.1 (Confidentiality)** | Full | Data classification; encryption; access controls | Classification policy; encryption evidence; ACL audits |

#### SOC 2 Evidence Requirements

| Evidence Type | Collection Method | Frequency | Retention |
|---------------|-------------------|-----------|-----------|
| Access control logs | Automated IAM export | Continuous | 1 year |
| Vulnerability scan reports | Trivy/Burp automated scans | Weekly | 3 years |
| Penetration test reports | Third-party vendor | Annual | 3 years |
| Policy attestations | DocuSign / manual ack | Annual | 3 years |
| Change management tickets | GitHub/Jira integration | Per change | 3 years |
| Incident response logs | PagerDuty/Opsgenie export | Per incident | 3 years |
| Backup restore tests | Automated + manual | Quarterly | 3 years |
| Employee training records | LMS completion export | Annual | 3 years |

#### SOC 2 Timeline

| Phase | Duration | Activities |
|-------|----------|------------|
| Readiness Assessment | Month 1-2 | Gap analysis; policy creation; control implementation |
| Type I Audit | Month 3-4 | Auditor engagement; evidence collection; Type I report |
| Observation Period | Month 4-9 | Continuous monitoring; evidence accumulation |
| Type II Audit | Month 9-10 | Full period audit; Type II report issued |
| Ongoing | Annual | Surveillance audits; control maintenance |

---

### 1.2 GDPR (General Data Protection Regulation)

| Attribute | Detail |
|-----------|--------|
| **Applicability** | Applies if any customer or end-user is an EU/EEA data subject |
| **Priority** | P1 - Critical |
| **Role Determination** | **Data Processor** (processing on behalf of customers); customers are Data Controllers |

#### Lawful Basis for Processing

| Processing Activity | Lawful Basis | Justification |
|---------------------|--------------|---------------|
| Proxying AI API requests | Legitimate Interest (Art. 6(1)(f)) | Necessary for service delivery; balanced against user rights |
| Request/response logging | Legitimate Interest + Contract (Art. 6(1)(b)(f)) | Required for debugging, security, SLA enforcement |
| Usage analytics | Legitimate Interest (Art. 6(1)(f)) | Service optimization; aggregated/anonymized where possible |
| Audit log generation | Legal Obligation + Legitimate Interest (Art. 6(1)(c)(f)) | SOC 2, security incident response |
| Support ticket analysis | Contract (Art. 6(1)(b)) | Necessary for support services |

#### Data Subject Rights Implementation

| Right (Art.) | Implementation | SLA | Technical Mechanism |
|--------------|----------------|-----|---------------------|
| Access (15) | Provide all personal data within 30 days | 30 days | Admin API endpoint; manual DB query |
| Rectification (16) | Correct inaccurate data | 30 days | Admin API; direct DB update |
| Erasure (17) | Delete all personal data | 30 days | `DELETE /admin/data-subjects/{id}`; cascade deletion |
| Restriction (18) | Pause processing | Immediate | Feature flag to exclude tenant from processing |
| Portability (20) | Export in machine-readable format | 30 days | `GET /admin/data-subjects/{id}/export` (JSON) |
| Objection (21) | Stop processing on legitimate interest grounds | Immediate | Tenant-level opt-out flag |
| Automated Decision-Making (22) | Not applicable - no automated profiling | N/A | N/A |

#### Cross-Border Transfer Considerations

| Scenario | Transfer Mechanism | Status |
|----------|-------------------|--------|
| EU data to US AI providers | SCCs (Standard Contractual Clauses) + DPA | Required - implement DPA with each provider |
| EU data processed on VPS | Ensure VPS provider offers EU data center regions | Select EU region (e.g., Frankfurt, Amsterdam) |
| Sub-processor transfers | Maintain sub-processor list; notify customers 30 days in advance | Publish list in DPA |
| UK data post-Brexit | UK Addendum to SCCs | Required if UK customers |

#### GDPR Implementation Checklist

| Item | Status | Owner |
|------|--------|-------|
| DPA (Data Processing Addendum) drafted | Required | Legal |
| Record of Processing Activities (ROPA) maintained | Required | DPO |
| Privacy notice published | Required | Legal |
| Data Subject Request (DSR) workflow documented | Required | Compliance |
| Sub-processor list maintained and published | Required | Legal |
| DPIA completed for high-risk processing | Required | DPO |
| EU representative appointed (if no EU presence) | Required | Legal |
| Breach notification procedure (72h to DPA) | Required | Security |
| Data retention schedules defined | Required | Compliance |

---

### 1.3 CCPA/CPRA (California Consumer Privacy Act/Regulation)

| Attribute | Detail |
|-----------|--------|
| **Applicability** | Applies if processing personal information of California residents; threshold: >100k consumers/yr or >$25M revenue |
| **Priority** | P2 - High (if CA customers exceed threshold) |
| **Business Classification** | "Service Provider" (processes on behalf of businesses) |

| CPRA Right | Implementation | SLA |
|------------|----------------|-----|
| Right to Know | Disclose categories/sources of PI collected | 45 days |
| Right to Delete | Delete PI; forward deletion requests to providers | 45 days |
| Right to Opt-Out of Sale/Sharing | No sale of PI; provide opt-out mechanism | Immediate |
| Right to Correct | Correct inaccurate PI | 45 days |
| Right to Limit Use of Sensitive PI | Minimize collection of sensitive PI | Immediate |
| Right to Non-Discrimination | No service degradation for privacy choices | N/A |

| Requirement | Status | Implementation |
|-------------|--------|----------------|
| Privacy Policy with CPRA disclosures | Required | Update privacy notice |
| "Do Not Sell or Share My PI" link | Required | Footer link + preference center |
| Contractual restrictions on downstream use | Required | DPA/Service Provider Agreement |
| Sub-processor disclosure | Required | Listed in privacy policy |
| Annual cybersecurity audit (if >$25M revenue) | Required if threshold met | Third-party assessment |
| Risk assessment for high-risk processing | Required | DPIA aligned with GDPR |

---

### 1.4 Industry-Specific Frameworks

#### HIPAA (Health Insurance Portability and Accountability Act)

| Attribute | Detail |
|-----------|--------|
| **Applicability** | Only if customers are Covered Entities or Business Associates AND process PHI through the gateway |
| **Priority** | P2 - Conditional |
| **Gateway Role** | Business Associate (if applicable) |

| HIPAA Requirement | Implementation If Applicable |
|-------------------|------------------------------|
| BAA (Business Associate Agreement) | Execute BAA with each healthcare customer |
| PHI Access Controls | Role-based; minimum necessary; audit trails |
| Encryption (Addressable) | AES-256 at rest; TLS 1.2+ in transit |
| Audit Controls | Log all PHI access; immutable logs |
| Integrity Controls | Hash verification for PHI in transit |
| Transmission Security | TLS 1.3 preferred; mutual TLS for sensitive |
| Breach Notification | Notify CE within 60 days; HHS within 60 days of discovery |
| Risk Analysis | Annual risk assessment; documented |

| Decision Point | Action |
|--------------|--------|
| Healthcare customer onboarding | Require BAA; enable HIPAA audit logging tier |
| PHI detected in requests | Flag; enforce encryption; restrict logging (no body content) |

---

#### PCI DSS (Payment Card Industry Data Security Standard)

| Attribute | Detail |
|-----------|--------|
| **Applicability** | Unlikely; only if payment card data is present in prompts/responses |
| **Priority** | P3 - Monitor |
| **Assessment** | Not a payment processor; gateway does not handle cardholder data directly |

| Consideration | Implementation |
|---------------|----------------|
| Card data in prompts | Implement content detection; block/reject if card patterns detected |
| If PCI scope confirmed | Full PCI DSS SAQ-D; network segmentation; annual QSA assessment |
| Policy | Explicitly prohibit transmission of payment card data in API requests |

---

#### FedRAMP (Federal Risk and Authorization Management Program)

| Attribute | Detail |
|-----------|--------|
| **Applicability** | Only if selling to US federal government agencies |
| **Priority** | P3 - Conditional |
| **Level Required** | Likely Moderate; High if sensitive data |

| FedRAMP Control Family | Effort | Timeline |
|------------------------|------|----------|
| Access Control (AC) | High | 12-18 months |
| Audit and Accountability (AU) | High | 12-18 months |
| Configuration Management (CM) | Medium | 12-18 months |
| Incident Response (IR) | Medium | 12-18 months |
| Risk Assessment (RA) | Medium | 12-18 months |
| System and Communications Protection (SC) | High | 12-18 months |

| Prerequisite | Implementation |
|--------------|----------------|
| CSP Supplied Package | Full SSP; POA&M; continuous monitoring |
| 3PAO Assessment | Third-party audit required |
| Agency Sponsor | Required for FedRAMP authorization |

---

### 1.5 Framework Priority Matrix

| Framework | Priority | Timeline | Cost Estimate | Trigger |
|-----------|----------|----------|---------------|---------|
| SOC 2 Type II | P1 | 6-9 months | $40-80K | First enterprise customer |
| GDPR | P1 | Immediate | $10-20K | Any EU customer |
| CCPA/CPRA | P2 | 3 months | $5-10K | CA customers > threshold |
| HIPAA | P2 | 3-6 months | $15-30K | Healthcare customer signed |
| ISO 27001 | P2 | 12 months | $30-60K | Enterprise demand |
| PCI DSS | P3 | As needed | $20-50K | If scope confirmed |
| FedRAMP | P3 | 18-24 months | $200-500K | Federal customer committed |

---

## 2. Control Mapping

### 2.1 Control Register

| Control ID | Framework | Requirement | Implementation | Evidence | Owner |
|------------|-----------|-------------|----------------|----------|-------|
| AC-001 | SOC 2 CC6.1 | Unique user IDs and authentication | Keycloak/Auth0 SSO integration; unique username enforcement | IAM user list; SSO config export | Platform Lead |
| AC-002 | SOC 2 CC6.1 | MFA enforcement | TOTP/SMS MFA required for all admin accounts; hardware keys for infra | MFA enrollment report; failed auth logs | Security Lead |
| AC-003 | SOC 2 CC6.1 | Role-based access control (RBAC) | 4 roles: Admin, Operator, Viewer, API Key (least privilege) | RBAC policy JSON; role assignment audit | Platform Lead |
| AC-004 | SOC 2 CC6.1 | Quarterly access reviews | Automated access review emails; manager attestation required | Access review completion reports | Compliance Lead |
| AC-005 | SOC 2 CC6.2 | Access removal within 24h of termination | Webhook from HRIS; automated IAM deprovisioning | Deprovisioning logs; HR termination list | Platform Lead |
| AC-006 | GDPR Art. 32 | Data access limited to authorized personnel | Row-level security in DB; tenant isolation | DB audit logs; RLS config | Security Lead |
| AC-007 | SOC 2 CC6.6 | Encryption at rest | AES-256-GCM for DB volumes; LUKS for disk encryption | Cryptographic module config; cert inventory | Platform Lead |
| AC-008 | SOC 2 CC6.6 | Encryption in transit | TLS 1.2+ mandatory; TLS 1.3 preferred; HSTS enabled | SSL Labs report; nginx config | Platform Lead |
| AC-009 | GDPR Art. 32 | Pseudonymization where possible | Tokenize user identifiers in logs; use tenant UUIDs | Log format specification; tokenization code | Security Lead |
| AU-001 | SOC 2 CC7.1 | Comprehensive audit logging | Log: auth events, data access, config changes, admin actions | Audit log sample; log schema | Security Lead |
| AU-002 | SOC 2 CC7.1 | Log integrity protection | Immutable logs via append-only storage; hash chain verification | WORM storage config; integrity check script | Security Lead |
| AU-003 | SOC 2 CC7.1 | Log monitoring and alerting | Real-time alerting on: brute force, privilege escalation, data exfiltration patterns | Alert rule definitions; SIEM dashboards | Security Lead |
| AU-004 | GDPR Art. 30 | Record processing activities | Automated ROPA generation from data flow diagrams | ROPA document; data flow diagram | DPO |
| CM-001 | SOC 2 CC8.1 | Change management process | All changes require: ticket, approval, test evidence, rollback plan | Change request tickets; deployment pipeline config | Platform Lead |
| CM-002 | SOC 2 CC8.1 | Segregation of duties | No single person can approve and deploy production changes | Approval chain config; Git branch protection | Platform Lead |
| CM-003 | SOC 2 CC7.2 | Vulnerability management | Trivy scan on every build; weekly full scans; Critical < 7 days, High < 30 days | Scan reports; Jira vuln tickets | Security Lead |
| CM-004 | SOC 2 CC7.2 | Penetration testing | Annual third-party penetration test; annual bug bounty review | Pen test report; remediation tickets | Security Lead |
| DP-001 | GDPR Art. 28 | Data Processing Agreement | Template DPA with all required Art. 28 clauses; signed before processing | DPA template; signed DPA inventory | Legal |
| DP-002 | GDPR Art. 28 | Sub-processor governance | Maintain published sub-processor list; 30-day notification for additions | Sub-processor page; notification email archive | Legal |
| DP-003 | GDPR Art. 28 | Data location control | Deploy to EU region for EU customers; data residency enforcement | Region deployment config; data flow validation | Platform Lead |
| DS-001 | GDPR Art. 12-22 | Data Subject Rights handling | DSR workflow: intake → validate → execute → confirm; 30-day SLA | DSR ticket template; completion metrics | Compliance Lead |
| DS-002 | GDPR Art. 17 | Right to erasure (right to be forgotten) | Cascade delete: tenant data, logs, backups (after retention) | Deletion script; audit trail of deletions | Platform Lead |
| DS-003 | GDPR Art. 20 | Data portability | Export API: machine-readable JSON of all tenant data | API endpoint; export sample | Platform Lead |
| IR-001 | SOC 2 CC7.3 | Incident response plan | Documented IRP with: severity levels, response procedures, communication templates | IRP document; severity matrix | Security Lead |
| IR-002 | GDPR Art. 33 | Personal data breach notification | Notify DPA within 72h; notify data subjects if high risk | Breach notification template; timer procedure | DPO |
| IR-003 | SOC 2 CC7.3 | Incident post-mortem | All incidents require post-mortem within 5 business days | Post-mortem template; completed post-mortems | Security Lead |
| RM-001 | SOC 2 CC3.2 | Risk assessment | Annual risk assessment; quarterly control testing | Risk register; control test results | Compliance Lead |
| RM-002 | SOC 2 CC3.4 | Vendor risk management | Security questionnaire for all vendors; annual reassessment | Vendor assessment forms; vendor risk scores | Compliance Lead |
| BC-001 | SOC 2 A1.1 | Business continuity | Daily automated backups; tested quarterly; RTO < 4h, RPO < 1h | Backup job config; DR test report | Platform Lead |
| BC-002 | SOC 2 A1.1 | Disaster recovery | Multi-region failover capability; documented DR runbook | DR runbook; failover test results | Platform Lead |
| TS-001 | SOC 2 CC4.1 | Security awareness training | Annual training for all employees; phishing simulations quarterly | Training completion report; phishing metrics | HR/Security |
| TS-002 | SOC 2 CC5.2 | Background checks | Background checks for all employees with system access | Background check vendor contract; completion log | HR |

### 2.2 Control Effectiveness Tracking

| Control ID | Test Frequency | Last Tested | Result | Next Test | Notes |
|------------|----------------|-------------|--------|-----------|-------|
| AC-001 | Quarterly | N/A | Pending | Next quarter post-launch | Baseline after IAM setup |
| AC-002 | Quarterly | N/A | Pending | Next quarter post-launch | MFA rollout required first |
| AC-003 | Quarterly | N/A | Pending | Next quarter post-launch | RBAC implementation needed |
| AU-001 | Continuous | N/A | Pending | Post logging infrastructure | Audit pipeline build required |
| CM-003 | Weekly | N/A | Pending | Post CI/CD setup | Trivy integration in pipeline |
| IR-001 | Semi-annually | N/A | Pending | After IRP documentation | Tabletop exercise required |
| BC-001 | Quarterly | N/A | Pending | After backup system live | Automated restore tests |

---

## 3. Data Classification

### 3.1 Data Types Flowing Through System

| Data Type | Source | In Transit | At Rest | Classification |
|-----------|--------|------------|---------|----------------|
| API request payloads | Customer applications | Yes | Cached (optional) | Customer-dependent |
| API response payloads | AI providers | Yes | Cached (optional) | Customer-dependent |
| API keys / authentication tokens | Customer-provided | Yes | Yes (encrypted) | Confidential |
| Tenant configuration | Admin UI / API | Yes | Yes | Internal |
| Usage metrics and analytics | System-generated | Yes | Yes | Internal |
| Audit logs | System-generated | Yes | Yes (immutable) | Restricted |
| Error logs | System-generated | Yes | Yes | Internal |
| Support tickets / communications | Customer-submitted | Yes | Yes | Confidential |
| Billing information | Customer-provided | Yes | Yes | Restricted |
| Employee / admin PII | HR / onboarding | Yes | Yes | Confidential |

### 3.2 Classification Levels

| Level | Definition | Examples | Handling Requirements |
|-------|------------|----------|-----------------------|
| **Public** | Approved for public disclosure | Marketing materials, public docs | None; freely distributable |
| **Internal** | Business use only; no external sharing | Analytics, configs, non-sensitive logs | Authenticated access required; no public URLs |
| **Confidential** | Sensitive business/customer data; disclosure could harm | API keys, customer data, support tickets | Encryption at rest and in transit; need-to-know access; audit logging |
| **Restricted** | Highest sensitivity; disclosure causes significant harm | Audit logs, billing data, PII, PHI (if applicable) | Encryption at rest+transit; MFA required; strict access control; immutable logs; DLP monitoring |

### 3.3 Data Handling Matrix

| Classification | Encryption At Rest | Encryption In Transit | Access Control | Audit Logging | Retention |
|----------------|-------------------|----------------------|----------------|---------------|-----------|
| Public | Optional | TLS | None | No | Per policy |
| Internal | Recommended | TLS | Authenticated | Yes | 90 days |
| Confidential | AES-256 required | TLS 1.2+ required | RBAC + need-to-know | Yes, detailed | Per DPA |
| Restricted | AES-256 required | TLS 1.3 preferred | RBAC + MFA + approval | Yes, immutable | Per DPA + legal hold |

### 3.4 Customer Data Classification (Inferred)

| Customer Type | Typical Data Sensitivity | Gateway Treatment |
|---------------|-------------------------|-------------------|
| General SaaS | Internal - Confidential | Standard encryption; no body logging |
| Healthcare | Confidential - Restricted | BAA required; no PHI in logs; enhanced audit |
| Financial Services | Confidential - Restricted | Enhanced encryption; no PII in logs; SOC 2+ required |
| Government | Restricted | FedRAMP/highest tier; air-gapped options |
| Education | Confidential | FERPA considerations; student data protection |

---

## 4. Audit Logging Requirements

### 4.1 Required Event Types

| Event Category | Event Type | Log Level | Description | Example |
|----------------|------------|-----------|-------------|---------|
| **Authentication** | Login | INFO | User login attempt | `user_login {user_id, tenant_id, ip, method, success}` |
| Authentication | Logout | INFO | User logout | `user_logout {user_id, tenant_id, ip}` |
| Authentication | MFA challenge | INFO | MFA verification | `mfa_challenge {user_id, method, success}` |
| Authentication | API key usage | INFO | API key authentication | `api_key_auth {key_id, tenant_id, ip, success}` |
| Authentication | Failed auth | WARN | Failed authentication attempt | `auth_failure {ip, username, reason, attempt_count}` |
| **Authorization** | Permission denied | WARN | Access denied | `access_denied {user_id, resource, action, reason}` |
| Authorization | Role change | INFO | Role assignment modified | `role_change {actor_id, target_id, old_role, new_role}` |
| **Data Access** | Tenant data read | INFO | Access to tenant configuration | `tenant_config_read {user_id, tenant_id}` |
| Data Access | Tenant data modify | INFO | Tenant config modified | `tenant_config_write {user_id, tenant_id, changes}` |
| Data Access | Cross-tenant access | CRITICAL | Attempted cross-tenant access | `cross_tenant_access {user_id, source_tenant, target_tenant}` |
| **Admin Action** | User created | INFO | New user provisioned | `user_created {actor_id, new_user_id, tenant_id}` |
| Admin Action | User deleted | INFO | User deprovisioned | `user_deleted {actor_id, deleted_user_id}` |
| Admin Action | API key rotated | INFO | API key rotation | `api_key_rotated {actor_id, key_id, tenant_id}` |
| Admin Action | Billing update | INFO | Billing information changed | `billing_update {actor_id, tenant_id}` |
| **System** | Configuration change | INFO | System config modified | `config_change {actor_id, key, old_value, new_value}` |
| System | Deployment | INFO | Code deployment | `deployment {version, actor_id, environment, rollback_plan}` |
| System | Backup | INFO | Backup completed/failed | `backup {status, size, duration, error}` |
| System | Alert triggered | WARN/CRITICAL | Monitoring alert | `alert {severity, rule, metric, value}` |
| **Security** | Rate limit hit | WARN | Rate limit exceeded | `rate_limit {tenant_id, ip, endpoint, limit}` |
| Security | Suspicious pattern | WARN | Anomalous traffic detected | `anomaly_detected {tenant_id, pattern, confidence}` |
| Security | DLP trigger | CRITICAL | Sensitive data detected | `dlp_trigger {tenant_id, pattern_type, action_taken}` |
| **Compliance** | DSR initiated | INFO | Data subject request | `dsr_initiated {type, subject_id, request_date}` |
| Compliance | DSR completed | INFO | DSR fulfilled | `dsr_completed {type, subject_id, completion_date}` |
| Compliance | Data deletion | INFO | Scheduled data deletion | `data_deletion {tenant_id, scope, records_deleted}` |
| Compliance | Retention policy run | INFO | Retention enforcement | `retention_run {date, records_archived, records_deleted}` |

### 4.2 Log Format Specification

| Field | Required | Description | Example |
|-------|----------|-------------|---------|
| `timestamp` | Yes | ISO 8601 UTC timestamp | `2025-01-15T10:30:00.000Z` |
| `event_id` | Yes | Unique UUID for the event | `evt_a1b2c3d4...` |
| `event_type` | Yes | Event type code | `user_login` |
| `severity` | Yes | Log level | `INFO` / `WARN` / `CRITICAL` |
| `actor_id` | Yes | ID of actor (user/service) | `user_123` / `svc_gateway` |
| `actor_type` | Yes | Type of actor | `user` / `api_key` / `system` |
| `tenant_id` | Conditional | Target tenant (if applicable) | `tenant_456` |
| `ip_address` | Yes | Source IP (hashed if PII risk) | `10.0.0.1` / `hash:abc123` |
| `user_agent` | No | Client user agent | `curl/8.0` |
| `action` | Yes | Action performed | `create` / `read` / `update` / `delete` |
| `resource` | Yes | Resource affected | `/api/v1/tenants/456/config` |
| `status` | Yes | Outcome | `success` / `failure` / `denied` |
| `duration_ms` | No | Request duration | `45` |
| `metadata` | No | JSON metadata | `{"mfa_method": "totp"}` |
| `integrity_hash` | Yes | SHA-256 hash of log entry | `sha256:abc123...` |
| `chain_hash` | Yes | Hash of previous log entry | `sha256:prev123...` |

### 4.3 Log Retention Periods

| Log Type | Retention Period | Justification | Storage |
|----------|-----------------|---------------|---------|
| Authentication logs | 1 year | SOC 2 requirement; incident investigation | Hot: 30d → Cold: 335d |
| Authorization logs | 1 year | SOC 2 requirement; access audit | Hot: 30d → Cold: 335d |
| Admin action logs | 3 years | Change tracking; accountability | Hot: 90d → Cold: remainder |
| Security event logs | 3 years | Forensic analysis; compliance evidence | Hot: 90d → Cold: remainder |
| System event logs | 90 days | Operational troubleshooting | Hot: 30d → Cold: 60d |
| Compliance audit logs | 7 years | Legal/regulatory requirement | Immutable archive |
| API access logs (no body) | 90 days | Usage analysis; rate limiting | Hot: 30d → Cold: 60d |
| Error logs | 90 days | Debugging; pattern analysis | Hot: 30d → Cold: 60d |

### 4.4 Log Integrity Protection

| Control | Implementation | Verification |
|---------|----------------|--------------|
| Immutable storage | Append-only log store; no delete/modify permissions | Quarterly integrity audit script |
| Hash chain | Each entry hashes previous entry's hash | Tamper detection script; run daily |
| Digital signature | Daily batch signatures with HSM-backed key | Signature verification on retrieval |
| Access logging | Log all access to logs themselves | Meta-audit trail |
| WORM (Write Once Read Many) | Object storage with object lock enabled | Storage config validation |

### 4.5 Log Access Controls

| Role | Log Access | Justification |
|------|------------|---------------|
| Security Lead | All logs | Security operations; incident response |
| Compliance Lead | Compliance + admin logs | Audit; regulatory response |
| Platform Lead | System + error logs | Operational troubleshooting |
| Customer Admin | Own tenant logs only | Self-service audit; data subject access |
| Auditor (external) | Anonymized samples per engagement scope | SOC 2; other audits |
| Customer Support | Own tenant logs (limited, with consent) | Support ticket resolution |

---

## 5. Data Retention Policy

### 5.1 Request/Response Data Retention

| Data Category | Retention Period | Rationale | Deletion Method |
|---------------|-----------------|-----------|-----------------|
| Request/response bodies (full logging enabled) | 7 days | Debugging; support; opt-in feature only | Automated cron; secure wipe |
| Request/response bodies (default: no logging) | Not retained | Privacy by design; not stored | N/A |
| Request metadata (timestamp, size, status) | 90 days | Usage analytics; rate limiting | Automated; standard delete |
| Cached responses | TTL-based (max 24h) | Performance optimization | Expiration-based eviction |
| Error responses | 30 days | Troubleshooting; pattern detection | Automated cron |
| Streaming response chunks | Not retained | Real-time only; not persisted | N/A |

### 5.2 Audit Log Retention

| Log Category | Retention | Storage Tier | Notes |
|--------------|-----------|--------------|-------|
| Real-time operational logs | 30 days hot | SSD/performance | Immediate query access |
| Recent historical logs | 90 days warm | Standard storage | Available for analysis |
| Long-term compliance logs | 1-3 years cold | Archive storage | Retrieve within 24h |
| Permanent compliance records | 7 years | Immutable archive | Legal/regulatory hold |

### 5.3 Backup Retention

| Backup Type | Frequency | Retention | Location | Encryption |
|-------------|-----------|-----------|----------|------------|
| Database full backup | Daily | 30 days | Object storage (same region) | AES-256 |
| Database incremental | Hourly | 7 days | Object storage | AES-256 |
| Configuration backup | On change | 90 days | Version control + object storage | AES-256 |
| Cross-region replica | Real-time | N/A | Secondary region | AES-256 |
| Annual archive | Yearly | 7 years | Cold archive | AES-256 + split key |

### 5.4 Deletion Procedures

| Deletion Type | Trigger | Procedure | Verification |
|---------------|---------|-----------|------------|
| **Tenant-initiated deletion** | Customer request | 1. Validate request 2. Export data (if requested) 3. Delete tenant record + cascade 4. Purge from caches 5. Schedule log redaction 6. Confirm deletion | Deletion report; DB row count zero |
| **Automated retention expiry** | Retention period reached | 1. Identify expired records 2. Archive if required 3. Cryptographic erase 4. Log deletion event | Retention run report |
| **Right to erasure (GDPR Art. 17)** | Valid DSR request | 1. Authenticate request 2. Identify all data locations 3. Delete from primary DB 4. Queue log redaction 5. Notify backup system for next cycle 6. Confirm within 30 days | DSR completion certificate |
| **Account termination** | Contract end / non-payment | 1. 30-day grace period 2. Soft delete (data inaccessible) 3. 90-day purge window 4. Hard delete with cryptographic erase 5. Final confirmation | Deletion audit trail |
| **Provider data deletion** | Sub-processor termination | 1. Notify provider per DPA 2. Confirm deletion within SLA 3. Document confirmation | Provider deletion certificate |

### 5.5 Data Residency Controls

| Customer Region | Deployment Region | Data Transfer | Mechanism |
|-----------------|-------------------|---------------|-----------|
| EU/EEA | EU region (Frankfurt/Amsterdam) | Intra-EU | N/A |
| UK | EU region + UK Addendum | EU→UK | UK Addendum to SCCs |
| US | US region (Virginia/Oregon) | Intra-US | N/A |
| Global / No preference | Closest to customer | Per routing | Standard DPA |

---

## 6. Incident Response

### 6.1 Response Procedures

#### Incident Classification

| Severity | Definition | Examples | Response Time | Escalation |
|----------|------------|----------|---------------|------------|
| **Critical (P1)** | Service unusable; data breach; active attack | RCE exploited; DB exposed; ransomware | 15 min | CEO + Legal + Security immediately |
| **High (P2)** | Major feature degraded; security vulnerability | Primary API down; auth bypass; major provider outage | 1 hour | Engineering Lead + Security within 1h |
| **Medium (P3)** | Partial degradation; non-critical vuln | Rate limiting issues; minor provider issues; low-sev vuln | 4 hours | Engineering Lead within 4h |
| **Low (P4)** | Cosmetic; feature request; informational | UI glitch; doc typo; info disclosure with no impact | 1 business day | Track in backlog |

#### Response Workflow

| Phase | Timeline | Actions | Owner |
|-------|----------|---------|-------|
| **Detection** | 0-15 min | Automated alert or manual report received; on-call paged | Monitoring / Reporter |
| **Triage** | 15-30 min | Validate incident; classify severity; assign IR lead | On-call Engineer |
| **Containment** | 30 min - 2h | Short-term: stop bleeding (block IPs, rotate keys, disable feature) | IR Lead |
| **Investigation** | 2h - 24h | Determine scope, root cause, affected tenants; preserve evidence | IR Lead + Security |
| **Eradication** | 1h - 48h | Remove threat; patch vulnerability; fix root cause | Engineering |
| **Recovery** | 1h - 72h | Restore service; verify integrity; monitor for recurrence | Engineering |
| **Post-Incident** | 5 business days | Post-mortem; action items; communication | IR Lead |
| **Closure** | 10 business days | All action items assigned; incident closed; lessons learned | Compliance Lead |

### 6.2 Notification Requirements

#### Regulatory Notification Timeline

| Regulation | Trigger | Timeline | Recipients | Content Required |
|------------|---------|----------|------------|------------------|
| **GDPR Art. 33** | Personal data breach likely to result in risk to rights | 72 hours to DPA | Supervisory Authority | Nature, categories, approximate numbers, likely consequences, measures taken |
| **GDPR Art. 34** | High risk to rights | Without undue delay | Affected data subjects | Clear language; nature; measures; DPO contact |
| **CCPA/CPRA** | Unauthorized access to unencrypted personal information | Without undue delay | Affected CA residents | Nature; types of PI; steps taken |
| **HIPAA Breach Rule** | Breach of unsecured PHI | 60 days to HHS; 60 days to individuals | HHS; affected individuals; media (if >500) | Description; types of PHI; steps; contact |
| **State breach laws** | Varies by state | Varies (typically 72h - 60 days) | Affected individuals; AGs | Varies |

#### Customer Notification

| Severity | Internal Notification | Customer Notification | Public Notification |
|----------|----------------------|----------------------|---------------------|
| Critical | Immediate (15 min) | Within 4 hours | If widespread impact |
| High | Within 1 hour | Within 24 hours | If customer-impacting |
| Medium | Within 4 hours | Status page update | Status page only |
| Low | Next business day | None / release notes | None |

#### Notification Content Template

| Section | Required Content |
|---------|------------------|
| Incident summary | What happened, when, duration |
| Impact scope | Which customers, services, data types affected |
| Root cause | Technical explanation (post-investigation) |
| Data impact | Whether data was accessed, modified, or exfiltrated |
| Actions taken | Containment, eradication, recovery steps |
| Customer actions | Any required customer response (key rotation, etc.) |
| Prevention | Measures to prevent recurrence |
| Contact | IR team contact; escalation path |

### 6.3 Documentation Requirements

| Document | Timing | Owner | Retention |
|----------|--------|-------|-----------|
| Incident ticket | Upon detection | On-call | 7 years |
| Evidence collection log | During investigation | IR Lead | 7 years |
| Timeline of events | During investigation | IR Lead | 7 years |
| Communication log | Throughout | IR Lead | 7 years |
| Post-mortem report | Within 5 business days | IR Lead | 7 years |
| Action items / remediation plan | Within 5 business days | IR Lead | 7 years |
| Regulatory notification copies | Upon submission | DPO / Legal | 7 years |
| Customer notification copies | Upon send | IR Lead | 7 years |
| Root cause analysis | Within 10 business days | Engineering | 7 years |

### 6.4 Incident Response Roles

| Role | Responsibility | Primary | Backup |
|------|---------------|---------|--------|
| Incident Commander | Overall IR coordination; decision authority | Security Lead | CTO |
| Technical Lead | Technical investigation; containment; recovery | Platform Lead | Senior Engineer |
| Communications Lead | Internal + external communications | Compliance Lead | CEO |
| Legal Advisor | Regulatory notification; legal risk assessment | Legal Counsel | External counsel |
| Customer Liaison | Customer communication; DSR coordination | Customer Success Lead | Support Lead |
| Forensics Lead | Evidence preservation; forensic analysis | Security Lead | External forensics |

---

## Appendix A: Compliance Glossary

| Term | Definition |
|------|------------|
| **DPA** | Data Processing Addendum/Agreement - contract governing processor-controller relationship |
| **DSR** | Data Subject Request - request from individual to exercise privacy rights |
| **ROPA** | Record of Processing Activities - GDPR Art. 30 documentation |
| **SCC** | Standard Contractual Clauses - EU-approved data transfer mechanism |
| **TSC** | Trust Services Criteria - SOC 2 control categories |
| **PII** | Personally Identifiable Information - data that identifies an individual |
| **PHI** | Protected Health Information - health data under HIPAA |
| **BAA** | Business Associate Agreement - HIPAA-specific data handling contract |
| **IRP** | Incident Response Plan - documented incident handling procedures |
| **RTO** | Recovery Time Objective - maximum acceptable downtime |
| **RPO** | Recovery Point Objective - maximum acceptable data loss |
| **WORM** | Write Once Read Many - immutable storage technology |
| **DPIA** | Data Protection Impact Assessment - GDPR risk assessment for high-risk processing |
| **DPO** | Data Protection Officer - GDPR-mandated privacy officer |

## Appendix B: Compliance Checklist Summary

### Pre-Launch (Must-Have)

| Item | Priority | Status | Owner |
|------|----------|--------|-------|
| [ ] Privacy notice published | P1 | | Legal |
| [ ] DPA template ready | P1 | | Legal |
| [ ] Encryption at rest + in transit | P1 | | Platform Lead |
| [ ] Authentication and authorization | P1 | | Platform Lead |
| [ ] Basic audit logging | P1 | | Security Lead |
| [ ] Access controls implemented | P1 | | Platform Lead |
| [ ] Sub-processor list published | P1 | | Legal |
| [ ] Incident response plan documented | P1 | | Security Lead |
| [ ] Data retention policy defined | P1 | | Compliance Lead |
| [ ] Secure key management | P1 | | Security Lead |
| [ ] TLS 1.2+ enforced | P1 | | Platform Lead |
| [ ] Input validation and sanitization | P1 | | Platform Lead |
| [ ] Rate limiting implemented | P1 | | Platform Lead |
| [ ] Error handling (no sensitive data in errors) | P1 | | Platform Lead |

### Post-Launch (Near-Term)

| Item | Priority | Target | Owner |
|------|----------|--------|-------|
| [ ] SOC 2 Type I | P1 | Month 3-4 | Compliance Lead |
| [ ] Automated vulnerability scanning | P1 | Month 1-2 | Security Lead |
| [ ] Penetration test | P1 | Month 2-3 | Security Lead |
| [ ] Log integrity protection (hash chain) | P1 | Month 2 | Security Lead |
| [ ] DSR workflow operational | P1 | Month 1 | Compliance Lead |
| [ ] Quarterly access reviews process | P2 | Month 3 | Compliance Lead |
| [ ] Security awareness training | P2 | Month 3 | HR/Security |
| [ ] Disaster recovery tested | P2 | Month 4 | Platform Lead |
| [ ] Data residency controls | P2 | Month 3 | Platform Lead |
| [ ] SOC 2 Type II | P1 | Month 9-10 | Compliance Lead |

### Ongoing

| Item | Frequency | Owner |
|------|-----------|-------|
| Access reviews | Quarterly | Compliance Lead |
| Vulnerability scans | Weekly | Security Lead |
| Penetration tests | Annual | Security Lead |
| Policy reviews | Annual | Compliance Lead |
| Security training | Annual | HR/Security |
| DR tests | Quarterly | Platform Lead |
| Risk assessment | Annual | Compliance Lead |
| Vendor assessments | Annual | Compliance Lead |
| ROPA updates | As needed | DPO |
| Sub-processor list updates | As needed | Legal |
