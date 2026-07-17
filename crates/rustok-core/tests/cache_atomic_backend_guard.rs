#[test]
fn root_cache_api_exposes_only_the_atomic_local_backend() {
    let lib = include_str!("../src/lib.rs");
    let atomic = include_str!("../src/cache_atomic.rs");

    assert!(lib.contains("mod cache;"));
    assert!(!lib.contains("pub mod cache;"));
    assert!(lib.contains("mod cache_atomic;"));
    assert!(lib.contains("pub use cache_atomic::InMemoryCacheBackend;"));
    assert!(!lib.contains("pub use cache::RedisCacheBackend;"));
    assert!(!lib.contains("pub use cache_atomic::{FallbackCacheBackend"));
    assert!(!lib.contains("pub use crate::RedisCacheBackend;"));
    assert!(!lib.contains("CacheStats, FallbackCacheBackend,"));

    assert!(atomic.contains("moka::ops::compute::{CompResult, Op}"));
    assert!(atomic.contains(".and_compute_with(move |current|"));
    assert!(atomic.contains("CompResult::ReplacedWith(_) | CompResult::Removed(_)"));
    assert!(atomic.contains("CompResult::Unchanged(_) | CompResult::StillNone(_)"));
    assert!(!atomic.contains("let current = self.cache.get(key).await;"));
}

#[test]
fn internal_compatibility_fallback_preserves_bounded_degradation_contract() {
    let atomic = include_str!("../src/cache_atomic.rs");

    assert!(atomic.contains("if self.has_degraded_write(key).await"));
    assert!(atomic.contains("self.warm_fallback(key, value.clone()).await;"));
    assert!(atomic.contains("Healthy primary miss could not clear stale local mirror"));
    assert!(atomic.contains("Primary cache unhealthy; bounded fallback reads remain available"));
    assert!(atomic.contains("fallback_health_preserves_primary_degradation"));
    assert!(atomic.contains("successful_primary_read_warms_fallback_for_a_later_outage"));
    assert!(atomic.contains("healthy_primary_miss_clears_local_mirror_before_later_outage"));
    assert!(atomic.contains("degraded_write_wins_over_stale_primary_only_until_marker_expiry"));
}
