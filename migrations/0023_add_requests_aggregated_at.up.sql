-- Add aggregated_at column to requests table for usage aggregation tracking

ALTER TABLE requests ADD COLUMN IF NOT EXISTS aggregated_at TIMESTAMPTZ;

CREATE INDEX IF NOT EXISTS idx_requests_aggregated_at ON requests(aggregated_at) WHERE aggregated_at IS NULL;
