-- Semantic cache: embedding storage with pgvector
-- Skipped entirely if pgvector extension is not available.

DO $$
BEGIN
    -- Only create the table and indexes if pgvector is installed
    IF EXISTS (SELECT 1 FROM pg_extension WHERE extname = 'vector') THEN
        CREATE TABLE IF NOT EXISTS query_embeddings (
            id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
            org_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
            model TEXT NOT NULL,
            embedding vector(1536) NOT NULL,
            response_hash TEXT NOT NULL,
            created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            expires_at TIMESTAMPTZ NOT NULL,
            last_hit_at TIMESTAMPTZ,
            UNIQUE (org_id, model, response_hash)
        );

        -- HNSW index for fast cosine similarity search
        CREATE INDEX IF NOT EXISTS idx_query_embeddings_hnsw
            ON query_embeddings
            USING hnsw (embedding vector_cosine_ops);

        -- Index for active entry lookups by org + model + expiration
        CREATE INDEX IF NOT EXISTS idx_query_embeddings_active
            ON query_embeddings (org_id, model, expires_at);

        -- Index for cleanup and LRU eviction
        CREATE INDEX IF NOT EXISTS idx_query_embeddings_expires
            ON query_embeddings (expires_at);

        CREATE INDEX IF NOT EXISTS idx_query_embeddings_org_model
            ON query_embeddings (org_id, model);
    ELSE
        RAISE NOTICE 'pgvector extension not available; skipping query_embeddings table creation';
    END IF;
END;
$$;
