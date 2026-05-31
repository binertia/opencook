-- Remove aggregated_at column from requests table

DROP INDEX IF EXISTS idx_requests_aggregated_at;
ALTER TABLE requests DROP COLUMN IF EXISTS aggregated_at;
