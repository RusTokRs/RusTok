use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("apps/server should live under workspace root")
        .to_path_buf()
}

fn source(relative: &str) -> String {
    let path = repo_root().join(relative);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()))
}

#[test]
fn variable_payload_caches_use_weighted_capacity() {
    let weighted_caches = [
        "apps/server/src/services/field_definition_cache.rs",
        "apps/server/src/services/rbac_runtime.rs",
        "apps/server/src/middleware/locale.rs",
        "crates/rustok-seo/src/services/mod.rs",
    ];

    for relative in weighted_caches {
        let source = source(relative);
        assert!(
            source.contains(".weigher("),
            "variable-size cache must use byte-weighted capacity: {relative}"
        );
    }
}

#[test]
fn database_backed_hot_misses_are_coalesced() {
    for relative in [
        "apps/server/src/middleware/locale.rs",
        "crates/rustok-seo/src/services/redirects.rs",
    ] {
        let source = source(relative);
        assert!(
            source.contains(".try_get_with("),
            "database-backed cache miss must be coalesced: {relative}"
        );
    }
}

#[test]
fn weighted_backend_uses_cache_service_owned_redis_client() {
    let weighted = source("crates/rustok-cache/src/weighted.rs");
    assert!(
        weighted.contains("SharedClientRedisCacheBackend::new"),
        "weighted backend must reuse the CacheService-owned Redis client"
    );
    assert!(
        !weighted.contains("redis_url()"),
        "weighted backend must not reopen Redis from a URL"
    );
    assert!(
        !weighted.contains("RedisCacheBackend::with_circuit_breaker"),
        "weighted backend must not use the legacy URL constructor"
    );
}

#[test]
fn stale_refresh_does_not_duplicate_a_foreground_fill() {
    let refresh = source("crates/rustok-cache/src/refresh.rs");
    assert!(
        refresh.contains("cache.source == CacheLoadSource::Hit"),
        "background refresh must only follow an existing stale cache hit"
    );
    assert!(
        refresh.contains("foreground_stale_fill_does_not_run_loader_twice"),
        "SWR must retain regression coverage for duplicate foreground/background loads"
    );
}

#[test]
fn invalidation_stream_requires_recovery_without_a_seeded_offset() {
    let invalidation = source("crates/rustok-cache/src/invalidation.rs");
    assert!(
        invalidation.contains("UnverifiedFirst"),
        "unseeded invalidation consumers must not trust the first observed generation"
    );
    assert!(
        invalidation.contains("pub fn seed("),
        "gap tracker must support a persisted durable consumer offset"
    );
}

#[test]
fn cache_values_and_keys_have_bounded_versioned_contracts() {
    let key = source("crates/rustok-cache/src/key.rs");
    let envelope = source("crates/rustok-cache/src/envelope.rs");

    assert!(
        key.contains("MAX_CACHE_KEY_BYTES"),
        "canonical cache keys must retain an explicit maximum length"
    );
    assert!(
        envelope.contains("DEFAULT_MAX_CACHE_ENVELOPE_BYTES"),
        "typed cache envelopes must retain an explicit maximum encoded size"
    );
    assert!(
        envelope.contains("CACHE_ENVELOPE_FORMAT_VERSION"),
        "typed cache envelopes must remain wire-format versioned"
    );
}
