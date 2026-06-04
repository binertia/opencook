//! Semantic cache backed by Redis.
//!
//! Stores embedding vectors alongside cached responses and performs
//! approximate nearest-neighbor lookup via linear scan with cosine similarity.

use std::time::Duration;

use redis::aio::ConnectionManager;
use tracing::{debug, warn};

use crate::semantic::{cosine_similarity, EmbeddingClient, SemanticEntry};
use crate::types::CachedResponse;
use uuid::Uuid;

/// Redis-backed semantic cache.
#[derive(Clone)]
pub struct SemanticCache {
    redis: ConnectionManager,
    embedding: EmbeddingClient,
    similarity_threshold: f32,
    max_scan_keys: usize,
}

impl SemanticCache {
    pub fn new(
        redis: ConnectionManager,
        embedding: EmbeddingClient,
        similarity_threshold: f32,
    ) -> Self {
        Self {
            redis,
            embedding,
            similarity_threshold: similarity_threshold.clamp(0.0, 1.0),
            max_scan_keys: 200,
        }
    }

    /// Build a Redis key for a semantic entry.
    fn key(&self, org_id: Uuid, model: &str, prompt_hash: &str) -> String {
        format!("semantic:{org_id}:{model}:{prompt_hash}")
    }

    /// Build a Redis pattern for scanning entries of an org+model.
    fn scan_pattern(&self, org_id: Uuid, model: &str) -> String {
        format!("semantic:{org_id}:{model}:*")
    }

    /// Extract the prompt text from a chat completion request for embedding.
    pub fn extract_prompt_text(request: &gateway_core::types::ChatCompletionRequest) -> String {
        request
            .messages
            .iter()
            .filter_map(|m| m.content.as_ref())
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Lookup a semantically similar cached response.
    pub async fn get(
        &self,
        request: &gateway_core::types::ChatCompletionRequest,
        org_id: Uuid,
    ) -> Option<CachedResponse> {
        let prompt_text = Self::extract_prompt_text(request);
        if prompt_text.is_empty() {
            return None;
        }

        let embedding = match self.embedding.embed(&prompt_text).await {
            Ok(e) => e,
            Err(err) => {
                warn!(error = %err, "Semantic cache embedding failed");
                return None;
            }
        };

        let pattern = self.scan_pattern(org_id, &request.model);
        let keys = match self.scan_keys(&pattern).await {
            Ok(k) => k,
            Err(e) => {
                warn!(error = %e, "Semantic cache scan failed");
                return None;
            }
        };

        if keys.is_empty() {
            return None;
        }

        let mut best_similarity = 0.0f32;
        let mut best_response: Option<CachedResponse> = None;

        for key in keys {
            let entry_json: Option<String> = match redis::cmd("GET")
                .arg(&key)
                .query_async::<_, Option<String>>(&mut self.redis.clone())
                .await
            {
                Ok(v) => v,
                Err(e) => {
                    warn!(key = %key, error = %e, "Semantic cache GET failed");
                    continue;
                }
            };

            let entry_json = match entry_json {
                Some(j) => j,
                None => continue,
            };

            let entry: SemanticEntry = match serde_json::from_str(&entry_json) {
                Ok(e) => e,
                Err(e) => {
                    warn!(key = %key, error = %e, "Semantic cache deserialization failed");
                    continue;
                }
            };

            if entry.embedding.len() != embedding.len() {
                continue;
            }

            let sim = cosine_similarity(&embedding, &entry.embedding);
            if sim > best_similarity {
                best_similarity = sim;
                best_response = Some(entry.response);
            }
        }

        if best_similarity >= self.similarity_threshold {
            debug!(
                similarity = best_similarity,
                threshold = self.similarity_threshold,
                "Semantic cache hit"
            );
            best_response
        } else {
            debug!(
                best_similarity = best_similarity,
                threshold = self.similarity_threshold,
                "Semantic cache miss (below threshold)"
            );
            None
        }
    }

    /// Store a response with its embedding.
    pub async fn insert(
        &self,
        request: &gateway_core::types::ChatCompletionRequest,
        org_id: Uuid,
        response: CachedResponse,
        ttl: Duration,
    ) {
        let prompt_text = Self::extract_prompt_text(request);
        if prompt_text.is_empty() {
            return;
        }

        let embedding = match self.embedding.embed(&prompt_text).await {
            Ok(e) => e,
            Err(err) => {
                warn!(error = %err, "Semantic cache embedding failed during insert");
                return;
            }
        };

        let prompt_hash = format!("{:x}", md5::compute(&prompt_text));
        let key = self.key(org_id, &request.model, &prompt_hash);
        let entry = SemanticEntry {
            embedding,
            response,
        };

        let json = match serde_json::to_string(&entry) {
            Ok(j) => j,
            Err(e) => {
                warn!(error = %e, "Semantic cache serialization failed");
                return;
            }
        };

        match redis::cmd("SETEX")
            .arg(&key)
            .arg(ttl.as_secs() as i64)
            .arg(json)
            .query_async::<_, ()>(&mut self.redis.clone())
            .await
        {
            Ok(()) => {
                debug!(key = %key, ttl = ttl.as_secs(), "Semantic cache insert");
            }
            Err(e) => {
                warn!(key = %key, error = %e, "Semantic cache insert failed");
            }
        }
    }

    /// Scan Redis for keys matching a pattern (bounded).
    async fn scan_keys(&self, pattern: &str) -> Result<Vec<String>, redis::RedisError> {
        let mut conn = self.redis.clone();
        let mut cursor = 0u64;
        let mut keys = Vec::new();

        loop {
            let (next_cursor, batch): (u64, Vec<String>) = redis::cmd("SCAN")
                .arg(cursor)
                .arg("MATCH")
                .arg(pattern)
                .arg("COUNT")
                .arg(50)
                .query_async(&mut conn)
                .await?;

            keys.extend(batch);
            if keys.len() >= self.max_scan_keys {
                keys.truncate(self.max_scan_keys);
                break;
            }

            cursor = next_cursor;
            if cursor == 0 {
                break;
            }
        }

        Ok(keys)
    }
}
