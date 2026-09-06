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

#[test]
fn shared_redis_backend_connects_lazily_and_recovers_generation_through_monitor() {
    let cache_lib = include_str!("../../rustok-cache/src/lib.rs");
    let shared = include_str!("../../rustok-cache/src/shared_backend.rs");
    let weighted = include_str!("../../rustok-cache/src/weighted.rs");
    let generation_recovery = include_str!("../../rustok-cache/src/backend_generation_recovery.rs");
    let redis_status = include_str!("../../rustok-cache/src/redis_status.rs");
    let recovery_test = include_str!("../../rustok-cache/src/startup_recovery_tests.rs");

    assert!(shared.contains("manager: AsyncMutex<Option<redis::aio::ConnectionManager>>"));
    assert!(shared.contains("async fn connection_manager(&self)"));
    assert!(shared.contains("let mut manager = self.connection_manager().await?;"));
    assert!(!shared.contains("client.get_connection_manager()\n                .await?"));
    assert!(
        shared.contains("configured_redis_outage_remains_visible_and_local_writes_stay_bounded")
    );

    assert!(cache_lib.contains("include!(\"backend_generation_recovery.rs\");"));
    assert!(shared.contains("self.wrap_generation_recovery_health(prefix, backend)"));
    assert!(weighted.contains("self.wrap_generation_recovery_health(prefix, backend)"));
    assert!(generation_recovery.contains("MAX_GENERATION_RECOVERIES_PER_PROBE"));
    assert!(generation_recovery.contains("generation_store_identity()"));
    assert!(generation_recovery.contains("CacheGenerationSource::SharedRedis"));
    assert!(generation_recovery.contains("self.ensure_owner()?;"));
    assert!(
        generation_recovery.contains("different_services_cannot_claim_the_same_generation_state")
    );
    assert!(generation_recovery.contains("aliased_untrusted_generation_requires_domain_recovery"));
    assert!(redis_status.contains("self.recover_registered_backend_generations().await"));

    assert!(cache_lib.contains("#[cfg(all(test, feature = \"redis-cache\"))]"));
    assert!(cache_lib.contains("mod startup_recovery_tests;"));
    assert!(
        recovery_test.contains("backend_created_during_startup_outage_recovers_shared_generation")
    );
    assert!(recovery_test.contains("RUSTOK_CACHE_REDIS_SERVER_BIN"));
    assert!(recovery_test.contains("service.redis_status().await.is_healthy()"));
    assert!(
        recovery_test
            .contains("Redis status monitor path did not recover shared generation after startup")
    );
    assert!(recovery_test.contains("format!(\"{prefix}:g-0:shared\")"));
}

#[test]
fn memory_only_cache_feature_matrix_remains_enforced() {
    let cache_lib = include_str!("../../rustok-cache/src/lib.rs");
    let cache_manifest = include_str!("../../rustok-cache/Cargo.toml");
    let shared = include_str!("../../rustok-cache/src/shared_backend.rs");
    let weighted = include_str!("../../rustok-cache/src/weighted.rs");
    let workflow = include_str!("../../../.github/workflows/cache-feature-matrix.yml");

    assert!(cache_lib.contains("#[cfg(feature = \"redis-cache\")]\nmod fallback;"));
    assert!(cache_lib.contains("mod shared_backend;"));
    assert!(!cache_lib.contains("include!(\"shared_backend.rs\")"));
    assert!(shared.contains(
        "#[cfg(feature = \"redis-cache\")]\nuse crate::fallback::DegradationAwareFallbackBackend;"
    ));
    assert!(weighted.contains(
        "#[cfg(feature = \"redis-cache\")]\nuse crate::fallback::DegradationAwareFallbackBackend;"
    ));
    assert!(
        shared
            .contains("#[cfg(not(feature = \"redis-cache\"))]\n        let _ = (prefix, options);")
    );
    assert!(
        weighted
            .contains("#[cfg(not(feature = \"redis-cache\"))]\n        let _ = (prefix, options);")
    );
    for target in ["fallback_cas_live", "real_redis_hardening"] {
        assert!(cache_manifest.contains(&format!("name = \"{target}\"")));
    }
    assert_eq!(
        cache_manifest
            .matches("required-features = [\"redis-cache\"]")
            .count(),
        2
    );
    assert!(workflow.contains("cargo check -p rustok-cache --all-targets --no-default-features"));
    assert!(workflow.contains("cargo test -p rustok-cache --no-default-features"));
    assert!(workflow.contains(
        "cargo clippy -p rustok-cache --all-targets --no-default-features -- -D warnings"
    ));
}
