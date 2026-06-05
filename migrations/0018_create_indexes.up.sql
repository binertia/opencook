-- Create any remaining indexes not defined in table migrations

-- Add migration script here

-- Additional composite indexes for hot-path queries
CREATE INDEX IF NOT EXISTS idx_requests_org_status ON requests(org_id, status, created_at DESC) WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_requests_org_model_requested ON requests(org_id, model_requested, created_at DESC) WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_responses_org_status ON responses(org_id, status_code, created_at DESC) WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_usage_records_org_cost ON usage_records(org_id, total_cost) WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_cache_metadata_org_expires ON cache_metadata(org_id, expires_at) WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_webhooks_org_events ON webhooks(org_id, events) WHERE deleted_at IS NULL;


