#[test]
fn root_cache_api_uses_atomic_local_backend() {
    let lib = include_str!("../src/lib.rs");
    let atomic = include_str!("../src/cache_atomic.rs");

    assert!(lib.contains("mod cache_atomic;"));
    assert!(lib.contains(
        "pub use cache_atomic::{FallbackCacheBackend, InMemoryCacheBackend};"
    ));
    assert!(!lib.contains(
        "pub use cache::{CacheStats, FallbackCacheBackend, InMemoryCacheBackend};"
    ));

    assert!(atomic.contains("moka::ops::compute::{CompResult, Op}"));
    assert!(atomic.contains(".and_compute_with(move |current|"));
    assert!(atomic.contains("CompResult::ReplacedWith(_) | CompResult::Removed(_)"));
    assert!(atomic.contains("CompResult::Unchanged(_) | CompResult::StillNone(_)"));
    assert!(!atomic.contains("let current = self.cache.get(key).await;"));
}
