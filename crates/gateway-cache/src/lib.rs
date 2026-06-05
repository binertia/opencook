//! Gateway Cache — L1 (moka) + L2 (Redis) two-tier caching + semantic cache.

pub mod analytics;
pub mod l1;
pub mod l2;
pub mod key_builder;
pub mod metrics;
pub mod types;
pub mod cacheable;
pub mod l1_cache;
pub mod l2_cache;
pub mod two_tier;
pub mod semantic;
pub mod semantic_cache;
pub mod semantic_pg;

pub use types::{CacheKey, CachedResponse};
pub use key_builder::build_cache_key;
pub use cacheable::is_cacheable;
pub use l1_cache::{L1Cache, CacheStats};
pub use l2_cache::L2Cache;
pub use two_tier::TwoTierCache;
pub use semantic::{cosine_similarity, EmbeddingClient, SemanticEntry};
pub use semantic_cache::SemanticCache;
pub use semantic_pg::{PgvectorSemanticCache, SemanticCacheStats};
