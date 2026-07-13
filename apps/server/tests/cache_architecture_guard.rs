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
        "apps/server/src/middleware/channel.rs",
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
        "apps/server/src/middleware/channel.rs",
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
fn channel_cache_bounds_request_keys_and_negative_results() {
    let channel = source("apps/server/src/middleware/channel.rs");
    assert!(
        channel.contains("bounded_cache_component"),
        "request-controlled channel selectors must not be stored verbatim in cache keys"
    );
    assert!(
        channel.contains("Sha256::digest"),
        "channel selector cache components must remain cryptographically bounded"
    );
    assert!(
        channel.contains("CHANNEL_NEGATIVE_CACHE_TTL"),
        "missing channel resolutions must keep an independent short negative TTL"
    );
    assert!(
        channel.contains("ChannelCacheExpiry"),
        "positive and negative channel resolutions must retain separate expirations"
    );
}

#[test]
fn redis_rate_limit_operations_are_bounded_and_redacted() {
    let rate_limit = source("apps/server/src/middleware/rate_limit.rs");
    assert!(
        rate_limit.contains("RATE_LIMIT_REDIS_OPERATION_TIMEOUT"),
        "Redis rate-limit connection, Lua and health operations must retain a deadline"
    );
    assert!(
        rate_limit.contains("redis_with_timeout("),
        "Redis rate-limit operations must use the shared timeout wrapper"
    );
    assert!(
        rate_limit.contains("redis_rate_limit_key"),
        "Redis rate-limit identities must use a bounded canonical key helper"
    );
    assert!(
        rate_limit.contains("Sha256::digest(identity.as_bytes())"),
        "Redis rate-limit keys must not contain raw IP, tenant or OAuth identity"
    );
    assert!(
        rate_limit.contains("bounded_redis_window_seconds"),
        "Redis EXPIRE arguments must not overflow when converting to i64"
    );
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
fn default_backend_factory_uses_the_service_owned_redis_client() {
    let service = source("crates/rustok-cache/src/service.rs");

    assert!(
        service.contains("self.backend_shared_client(prefix, ttl, max_capacity)"),
        "the default backend factory must delegate to the service-owned client path"
    );
    assert!(
        service.contains("self.backend_shared_client_with_options(prefix, ttl, max_capacity, options)"),
        "the per-call backend factory must delegate to the service-owned client path"
    );
    assert!(
        !service.contains("async fn raw_backend("),
        "the default factory must not retain a second URL-based Redis construction path"
    );
    assert!(
        !service.contains("RedisCacheBackend::with_circuit_breaker"),
        "the default factory must not reopen Redis from the stored URL"
    );
}

#[test]
fn shared_fallback_health_does_not_mask_primary_degradation() {
    let fallback = source("crates/rustok-cache/src/fallback.rs");
    let weighted = source("crates/rustok-cache/src/weighted.rs");
    let shared = source("crates/rustok-cache/src/shared_backend.rs");

    assert!(
        fallback.contains("self.primary.health().await"),
        "fallback health must report the shared primary state"
    );
    assert!(
        weighted.contains("DegradationAwareFallbackBackend::new"),
        "weighted Redis backends must use degradation-aware fallback"
    );
    assert!(
        shared.contains("DegradationAwareFallbackBackend::new"),
        "entry-count shared Redis backends must use degradation-aware fallback"
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

    for required in [
        "MAX_CACHE_KEY_BYTES",
        "MAX_CACHE_IDENTITY_BYTES",
        "MAX_CACHE_KEY_INPUT_BYTES",
        "MAX_CACHE_KEY_DYNAMIC_COMPONENTS",
        "IdentityTooLong",
        "TooManyDynamicComponents",
    ] {
        assert!(
            key.contains(required),
            "canonical cache key contract must retain {required}"
        );
    }
    assert!(
        envelope.contains("DEFAULT_MAX_CACHE_ENVELOPE_BYTES"),
        "typed cache envelopes must retain an explicit maximum encoded size"
    );
    assert!(
        envelope.contains("CACHE_ENVELOPE_FORMAT_VERSION"),
        "typed cache envelopes must remain wire-format versioned"
    );
    assert!(
        envelope.contains("ser_flavors::Size::default()"),
        "cache envelopes must be measured before output allocation"
    );
    assert!(
        envelope.contains("BoundedEnvelopeWriter"),
        "cache envelope output must remain physically bounded during serialization"
    );
    assert!(
        !envelope.contains("postcard::to_stdvec(self)"),
        "cache envelope limits must not be checked only after allocating the complete output"
    );
}
