-- Enable required PostgreSQL extensions

CREATE EXTENSION IF NOT EXISTS "pgcrypto";

-- pgvector may not be available in all distributions; semantic caching will be unavailable if missing
DO $$
BEGIN
    CREATE EXTENSION IF NOT EXISTS "vector";
EXCEPTION WHEN OTHERS THEN
    RAISE NOTICE 'pgvector extension not available, semantic caching disabled';
END;
$$;

-- pg_uuidv7 may not be available in all distributions; if missing, we fall back to gen_random_uuid()
DO $$
BEGIN
    CREATE EXTENSION IF NOT EXISTS "pg_uuidv7";
EXCEPTION WHEN OTHERS THEN
    RAISE NOTICE 'pg_uuidv7 extension not available, using gen_random_uuid()';
END;
$$;


