-- Semantic cache: embedding storage with pgvector

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

-- Partial index: only active (non-expired) entries
CREATE INDEX IF NOT EXISTS idx_query_embeddings_active
    ON query_embeddings (org_id, model, expires_at)
    WHERE expires_at > NOW();

-- Index for cleanup and LRU eviction
CREATE INDEX IF NOT EXISTS idx_query_embeddings_expires
    ON query_embeddings (expires_at);

CREATE INDEX IF NOT EXISTS idx_query_embeddings_org_model
    ON query_embeddings (org_id, model);
