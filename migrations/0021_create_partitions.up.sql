-- Create initial monthly partitions for partitioned tables (current + next month)

-- Add migration script here

-- usage_records partitions
DO $$
DECLARE
    this_month DATE := date_trunc('month', CURRENT_DATE);
    next_month DATE := date_trunc('month', CURRENT_DATE + interval '1 month');
    next2_month DATE := date_trunc('month', CURRENT_DATE + interval '2 months');
    part_name TEXT;
BEGIN
    part_name := 'usage_records_y' || to_char(this_month, 'YYYY') || 'm' || to_char(this_month, 'MM');
    EXECUTE format('CREATE TABLE IF NOT EXISTS IF NOT EXISTS %I PARTITION OF usage_records FOR VALUES FROM (%L) TO (%L)',
        part_name, this_month, next_month);

    part_name := 'usage_records_y' || to_char(next_month, 'YYYY') || 'm' || to_char(next_month, 'MM');
    EXECUTE format('CREATE TABLE IF NOT EXISTS IF NOT EXISTS %I PARTITION OF usage_records FOR VALUES FROM (%L) TO (%L)',
        part_name, next_month, next2_month);
END;
$$;

-- audit_log partitions
DO $$
DECLARE
    this_month DATE := date_trunc('month', CURRENT_DATE);
    next_month DATE := date_trunc('month', CURRENT_DATE + interval '1 month');
    next2_month DATE := date_trunc('month', CURRENT_DATE + interval '2 months');
    part_name TEXT;
BEGIN
    part_name := 'audit_log_y' || to_char(this_month, 'YYYY') || 'm' || to_char(this_month, 'MM');
    EXECUTE format('CREATE TABLE IF NOT EXISTS IF NOT EXISTS %I PARTITION OF audit_log FOR VALUES FROM (%L) TO (%L)',
        part_name, this_month, next_month);

    part_name := 'audit_log_y' || to_char(next_month, 'YYYY') || 'm' || to_char(next_month, 'MM');
    EXECUTE format('CREATE TABLE IF NOT EXISTS IF NOT EXISTS %I PARTITION OF audit_log FOR VALUES FROM (%L) TO (%L)',
        part_name, next_month, next2_month);
END;
$$;


