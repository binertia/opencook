-- Enable Row-Level Security and create tenant isolation policies

-- Add migration script here

-- Enable RLS on all tenant tables
ALTER TABLE users ENABLE ROW LEVEL SECURITY;
ALTER TABLE api_keys ENABLE ROW LEVEL SECURITY;
ALTER TABLE provider_configs ENABLE ROW LEVEL SECURITY;
ALTER TABLE provider_models ENABLE ROW LEVEL SECURITY;
ALTER TABLE routing_rules ENABLE ROW LEVEL SECURITY;
ALTER TABLE requests ENABLE ROW LEVEL SECURITY;
ALTER TABLE responses ENABLE ROW LEVEL SECURITY;
ALTER TABLE usage_records ENABLE ROW LEVEL SECURITY;
ALTER TABLE quotas ENABLE ROW LEVEL SECURITY;
ALTER TABLE quota_usage ENABLE ROW LEVEL SECURITY;
ALTER TABLE webhooks ENABLE ROW LEVEL SECURITY;
ALTER TABLE webhook_deliveries ENABLE ROW LEVEL SECURITY;
ALTER TABLE cache_metadata ENABLE ROW LEVEL SECURITY;
ALTER TABLE audit_log ENABLE ROW LEVEL SECURITY;
ALTER TABLE sessions ENABLE ROW LEVEL SECURITY;

-- Create tenant isolation policies (defense-in-depth)
CREATE POLICY tenant_isolation_users ON users
    USING (org_id = current_setting('app.org_id')::UUID);

CREATE POLICY tenant_isolation_api_keys ON api_keys
    USING (org_id = current_setting('app.org_id')::UUID);

CREATE POLICY tenant_isolation_provider_configs ON provider_configs
    USING (org_id = current_setting('app.org_id')::UUID);

CREATE POLICY tenant_isolation_provider_models ON provider_models
    USING (org_id = current_setting('app.org_id')::UUID);

CREATE POLICY tenant_isolation_routing_rules ON routing_rules
    USING (org_id = current_setting('app.org_id')::UUID);

CREATE POLICY tenant_isolation_requests ON requests
    USING (org_id = current_setting('app.org_id')::UUID);

CREATE POLICY tenant_isolation_responses ON responses
    USING (org_id = current_setting('app.org_id')::UUID);

CREATE POLICY tenant_isolation_usage_records ON usage_records
    USING (org_id = current_setting('app.org_id')::UUID);

CREATE POLICY tenant_isolation_quotas ON quotas
    USING (org_id = current_setting('app.org_id')::UUID);

CREATE POLICY tenant_isolation_quota_usage ON quota_usage
    USING (org_id = current_setting('app.org_id')::UUID);

CREATE POLICY tenant_isolation_webhooks ON webhooks
    USING (org_id = current_setting('app.org_id')::UUID);

CREATE POLICY tenant_isolation_webhook_deliveries ON webhook_deliveries
    USING (org_id = current_setting('app.org_id')::UUID);

CREATE POLICY tenant_isolation_cache_metadata ON cache_metadata
    USING (org_id = current_setting('app.org_id')::UUID);

CREATE POLICY tenant_isolation_audit_log ON audit_log
    USING (org_id = current_setting('app.org_id')::UUID);

CREATE POLICY tenant_isolation_sessions ON sessions
    USING (org_id = current_setting('app.org_id')::UUID);

-- Organizations table: users can only see their own org (or system org)
CREATE POLICY tenant_isolation_organizations ON organizations
    USING (id = current_setting('app.org_id')::UUID OR id = '00000000-0000-0000-0000-000000000001');


