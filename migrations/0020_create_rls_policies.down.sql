
DROP POLICY IF EXISTS tenant_isolation_organizations ON organizations;
DROP POLICY IF EXISTS tenant_isolation_sessions ON sessions;
DROP POLICY IF EXISTS tenant_isolation_audit_log ON audit_log;
DROP POLICY IF EXISTS tenant_isolation_cache_metadata ON cache_metadata;
DROP POLICY IF EXISTS tenant_isolation_webhook_deliveries ON webhook_deliveries;
DROP POLICY IF EXISTS tenant_isolation_webhooks ON webhooks;
DROP POLICY IF EXISTS tenant_isolation_quota_usage ON quota_usage;
DROP POLICY IF EXISTS tenant_isolation_quotas ON quotas;
DROP POLICY IF EXISTS tenant_isolation_usage_records ON usage_records;
DROP POLICY IF EXISTS tenant_isolation_responses ON responses;
DROP POLICY IF EXISTS tenant_isolation_requests ON requests;
DROP POLICY IF EXISTS tenant_isolation_routing_rules ON routing_rules;
DROP POLICY IF EXISTS tenant_isolation_provider_models ON provider_models;
DROP POLICY IF EXISTS tenant_isolation_provider_configs ON provider_configs;
DROP POLICY IF EXISTS tenant_isolation_api_keys ON api_keys;
DROP POLICY IF EXISTS tenant_isolation_users ON users;

ALTER TABLE organizations DISABLE ROW LEVEL SECURITY;
ALTER TABLE sessions DISABLE ROW LEVEL SECURITY;
ALTER TABLE audit_log DISABLE ROW LEVEL SECURITY;
ALTER TABLE cache_metadata DISABLE ROW LEVEL SECURITY;
ALTER TABLE webhook_deliveries DISABLE ROW LEVEL SECURITY;
ALTER TABLE webhooks DISABLE ROW LEVEL SECURITY;
ALTER TABLE quota_usage DISABLE ROW LEVEL SECURITY;
ALTER TABLE quotas DISABLE ROW LEVEL SECURITY;
ALTER TABLE usage_records DISABLE ROW LEVEL SECURITY;
ALTER TABLE responses DISABLE ROW LEVEL SECURITY;
ALTER TABLE requests DISABLE ROW LEVEL SECURITY;
ALTER TABLE routing_rules DISABLE ROW LEVEL SECURITY;
ALTER TABLE provider_models DISABLE ROW LEVEL SECURITY;
ALTER TABLE provider_configs DISABLE ROW LEVEL SECURITY;
ALTER TABLE api_keys DISABLE ROW LEVEL SECURITY;
ALTER TABLE users DISABLE ROW LEVEL SECURITY;

