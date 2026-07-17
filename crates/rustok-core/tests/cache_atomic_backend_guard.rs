#[test]
fn root_cache_api_exposes_only_the_atomic_local_backend() {
    let lib = include_str!("../src/lib.rs");
    let cache = include_str!("../src/cache.rs");
    let atomic = include_str!("../src/cache_atomic.rs");

    assert!(lib.contains("mod cache;"));
    assert!(!lib.contains("pub mod cache;"));
    assert!(lib.contains("mod cache_atomic;"));
    assert!(lib.contains("pub use cache_atomic::InMemoryCacheBackend;"));
    assert!(!lib.contains("RedisCacheBackend"));
    assert!(!lib.contains("FallbackCacheBackend"));

    assert!(cache.contains("pub struct CacheStats"));
    assert!(!cache.contains("RedisCacheBackend"));
    assert!(!cache.contains("FallbackCacheBackend"));
    assert!(!cache.contains("redis::"));

    assert!(atomic.contains("moka::ops::compute::{CompResult, Op}"));
    assert!(atomic.contains(".and_compute_with(move |current|"));
    assert!(atomic.contains("CompResult::ReplacedWith(_) | CompResult::Removed(_)"));
    assert!(atomic.contains("CompResult::Unchanged(_) | CompResult::StillNone(_)"));
    assert!(!atomic.contains("FallbackCacheBackend"));
    assert!(!atomic.contains("let current = self.cache.get(key).await;"));
}

#[test]
fn atomic_local_backend_keeps_capacity_and_cas_regressions() {
    let atomic = include_str!("../src/cache_atomic.rs");

    assert!(atomic.contains("entry_weight_accounts_for_key_payload_and_value_metadata"));
    assert!(atomic.contains("weighted_cache_does_not_retain_entry_larger_than_its_budget"));
    assert!(atomic.contains("compare_and_set_does_not_insert_a_missing_or_expired_entry"));
    assert!(atomic.contains("compare_and_set_replaces_or_removes_only_a_matching_entry"));
}

#[test]
fn redis_feature_is_compatibility_only_in_core_and_owned_by_cache() {
    let core_manifest = include_str!("../Cargo.toml");
    let cache_manifest = include_str!("../../rustok-cache/Cargo.toml");

    assert!(core_manifest.contains("redis-cache = []"));
    assert!(!core_manifest.contains("\nredis ="));
    assert!(cache_manifest.contains(
        "redis-cache = [\"rustok-core/redis-cache\", \"dep:redis\", \"dep:futures-util\"]"
    ));
    assert!(cache_manifest.contains("\nredis = {"));
}
