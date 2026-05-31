
DROP TRIGGER IF EXISTS trg_sessions_updated_at ON sessions;
DROP TRIGGER IF EXISTS trg_cache_metadata_updated_at ON cache_metadata;
DROP TRIGGER IF EXISTS trg_webhook_deliveries_updated_at ON webhook_deliveries;
DROP TRIGGER IF EXISTS trg_webhooks_updated_at ON webhooks;
DROP TRIGGER IF EXISTS trg_quota_usage_updated_at ON quota_usage;
DROP TRIGGER IF EXISTS trg_quotas_updated_at ON quotas;
DROP TRIGGER IF EXISTS trg_usage_records_updated_at ON usage_records;
DROP TRIGGER IF EXISTS trg_responses_updated_at ON responses;
DROP TRIGGER IF EXISTS trg_requests_updated_at ON requests;
DROP TRIGGER IF EXISTS trg_routing_rules_updated_at ON routing_rules;
DROP TRIGGER IF EXISTS trg_provider_models_updated_at ON provider_models;
DROP TRIGGER IF EXISTS trg_provider_configs_updated_at ON provider_configs;
DROP TRIGGER IF EXISTS trg_api_keys_updated_at ON api_keys;
DROP TRIGGER IF EXISTS trg_users_updated_at ON users;
DROP TRIGGER IF EXISTS trg_organizations_updated_at ON organizations;
DROP FUNCTION IF EXISTS fn_update_timestamp();

