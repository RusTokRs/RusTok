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
fn invalidation_recovery_is_two_phase_and_monotonic() {
    let invalidation = source("crates/rustok-cache/src/invalidation.rs");
    assert!(
        invalidation.contains("UnverifiedFirst"),
        "unseeded invalidation consumers must not trust the first observed generation"
    );
    assert!(
        invalidation.contains("pub fn acknowledge_recovery("),
        "gap recovery must be acknowledged only after clear/rebuild succeeds"
    );
    assert!(
        invalidation.contains("OffsetRegressed"),
        "durable invalidation offsets must never move backwards"
    );
    assert!(
        invalidation.contains("gap_does_not_advance_until_recovery_is_acknowledged"),
        "gap tracking must retain regression coverage for failed recovery"
    );
}

#[test]
fn generation_fallback_is_trusted_and_monotonic() {
    let generation = source("crates/rustok-cache/src/generation.rs");
    assert!(
        generation.contains("NoLocalSnapshot"),
        "Redis generation failure without a trusted local snapshot must fail closed"
    );
    assert!(
        generation.contains("GenerationRegressed"),
        "shared generation loss must not lower a locally observed generation"
    );
}

#[test]
fn typed_loading_invalidates_raced_incompatible_values() {
    let typed = source("crates/rustok-cache/src/typed.rs");
    assert!(
        typed.contains("incompatible_value_racing_after_initial_probe_is_invalidated"),
        "typed loading must retain race regression coverage"
    );
    assert!(
        typed.contains("backend.invalidate(&key).await?"),
        "typed validation failures must propagate shared invalidation failures"
    );
}

#[test]
fn distributed_lease_deadline_is_usable_after_confirmation() {
    let lease = source("crates/rustok-cache/src/lease.rs");
    assert!(
        lease.contains("OperationTimeoutNotLessThanTtl"),
        "lease operation timeout must remain strictly below lease TTL"
    );
    assert!(
        lease.contains("ExpiredBeforeConfirmation"),
        "a lease confirmed after its deadline must not be returned as acquired"
    );
    assert!(
        lease.contains("MAX_LEASE_CACHE_KEY_BYTES"),
        "lease source keys must be bounded before hashing"
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
