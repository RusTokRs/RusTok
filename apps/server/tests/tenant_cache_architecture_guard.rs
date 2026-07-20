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
fn tenant_cache_uses_weighted_shared_client_backends() {
    let tenant = source("apps/server/src/middleware/tenant.rs");
    assert!(tenant.contains(".backend_weighted("));
    assert!(!tenant.contains(".backend(\n"));
    assert!(tenant.contains("TENANT_CACHE_MAX_WEIGHT_BYTES"));
    assert!(tenant.contains("TENANT_NEGATIVE_CACHE_MAX_WEIGHT_BYTES"));
}

#[test]
fn tenant_keys_are_canonical_and_schema_versioned() {
    let tenant = source("apps/server/src/middleware/tenant.rs");
    assert!(tenant.contains("CanonicalCacheKeyBuilder::new"));
    assert!(tenant.contains("TENANT_CACHE_VERSION: &str = \"v2\""));
    assert!(tenant.contains("TENANT_CONTEXT_SCHEMA_VERSION"));
    assert!(tenant.contains("TENANT_NEGATIVE_SCHEMA_VERSION"));
}

#[test]
fn tenant_payloads_use_typed_envelopes_and_explicit_negative_policy() {
    let tenant = source("apps/server/src/middleware/tenant.rs");
    assert!(tenant.contains("load_enveloped_or_fill("));
    assert!(tenant.contains("CacheEnvelope::new"));
    assert!(tenant.contains("NegativeCachePolicy::deterministic_jittered"));
    assert!(tenant.contains("get_negative::<CachedTenantMiss>"));
    assert!(tenant.contains("store_negative("));
    assert!(!tenant.contains("serde_json::from_slice::<TenantContext>"));
    assert!(!tenant.contains("serde_json::to_vec(&context)"));
}

#[test]
fn tenant_loader_and_direct_redis_metrics_are_bounded() {
    let tenant = source("apps/server/src/middleware/tenant.rs");
    assert!(tenant.contains("TENANT_CACHE_LOADER_TIMEOUT"));
    assert!(tenant.contains("CacheTtlPolicy::deterministic_jitter"));
    assert!(tenant.contains("tenant_cache_redis_timeout("));
    assert!(tenant.contains("redis::cmd(\"INCR\")"));
    assert!(tenant.contains("redis::cmd(\"GET\")"));
}

#[test]
fn tenant_invalidation_payload_parser_rejects_extra_parts() {
    let tenant = source("apps/server/src/middleware/tenant.rs");
    assert!(tenant.contains("parts.next().is_some()"));
}

#[test]
fn weighted_factories_apply_generation_before_instrumentation() {
    let weighted = source("crates/rustok-cache/src/weighted.rs");
    let wrap = weighted
        .find("self.wrap_generation_aware_backend(prefix, backend).await")
        .expect("weighted backend must be generation-aware");
    let instrument = weighted
        .find("InstrumentedWeightedCacheBackend::new(prefix, backend)")
        .expect("weighted backend instrumentation must remain present");
    assert!(
        wrap < instrument,
        "generation mapping must occur before metrics instrumentation"
    );
}

#[test]
fn tenant_generation_matches_and_aliases_both_physical_backend_prefixes() {
    let tenant = source("apps/server/src/middleware/tenant.rs");
    let generation = source("apps/server/src/services/tenant_cache_generation.rs");
    let backend_generation = source("crates/rustok-cache/src/backend_generation.rs");

    assert!(tenant.contains("tenant-cache:{}:data"));
    assert!(tenant.contains("tenant-cache:{}:negative"));
    assert!(
        generation.contains("TENANT_CACHE_DATA_BACKEND_PREFIX: &str = \"tenant-cache:v2:data\"")
    );
    assert!(
        generation
            .contains("TENANT_CACHE_NEGATIVE_BACKEND_PREFIX: &str = \"tenant-cache:v2:negative\"")
    );
    assert!(generation.contains("bind_cache_backend_generation_aliases("));
    assert!(generation.contains("bind_tenant_backend_generations()?"));
    assert!(backend_generation.contains("pub fn bind_cache_backend_generation_aliases"));
    assert!(backend_generation.contains("AliasAlreadyBound"));
}

#[test]
fn outbox_keeps_transactional_transport_and_rotates_relay_target() {
    let factory = source("apps/server/src/services/event_transport_factory.rs");
    assert!(factory.contains("transport: outbox_transport"));
    assert!(factory.contains("TenantCacheGenerationTransport::new(relay_target, cache.clone())"));
    assert!(factory.contains("start_tenant_cache_generation_listener(ctx, cache.clone()).await?"));
}

#[test]
fn tenant_generation_closes_subscribe_gap_and_rotates_before_delivery() {
    let generation = source("apps/server/src/services/tenant_cache_generation.rs");
    assert!(generation.contains("consume_subscription_with_ready("));
    assert!(generation.contains("redis_ready_recovery"));

    let bump = generation
        .find(".bump_cache_backend_generation(TENANT_CACHE_BACKEND_PREFIX)")
        .expect("tenant events must bump a durable generation");
    let invalidation = generation
        .find(".publish_durable(&record)")
        .expect("tenant events must publish the versioned invalidation");
    let downstream = generation
        .find("self.inner.publish(envelope).await")
        .expect("wrapped event must still reach its downstream transport");
    assert!(bump < invalidation && invalidation < downstream);
}

#[test]
fn tenant_generation_dedupe_is_bounded_serialized_two_phase_and_retry_safe() {
    let generation = source("apps/server/src/services/tenant_cache_generation.rs");
    let dedupe = source("crates/rustok-cache/src/event_dedupe.rs");

    for required in [
        "DEFAULT_MAX_CACHE_EVENT_DEDUPE_ENTRIES",
        "DEFAULT_CACHE_EVENT_DEDUPE_TTL",
        "CACHE_EVENT_DEDUPE_LOCK_STRIPES",
        "capacity_eviction_total",
        "probe_does_not_precommit_failed_work",
        "same_event_serialization_closes_the_concurrent_probe_race",
    ] {
        assert!(
            dedupe.contains(required),
            "event dedupe must retain {required}"
        );
    }

    let serialize = generation
        .find("successful_rotations.serialize_event(envelope.id)")
        .expect("tenant rotation must serialize concurrent retries for a stable event ID");
    let probe = generation
        .find("successful_rotations.is_duplicate(envelope.id)")
        .expect("tenant rotation must probe stable event IDs before work");
    let bump = generation
        .find(".bump_cache_backend_generation(TENANT_CACHE_BACKEND_PREFIX)")
        .expect("tenant rotation must advance the generation");
    let publish = generation
        .find(".publish_durable(&record)")
        .expect("tenant rotation must publish durable invalidation");
    let commit = generation
        .find("successful_rotations.observe(envelope.id)")
        .expect("successful rotation must commit the event ID");
    let downstream = generation
        .find("self.inner.publish(envelope).await")
        .expect("event retry must continue to downstream delivery");

    assert!(
        serialize < probe
            && probe < bump
            && bump < publish
            && publish < commit
            && commit < downstream
    );
    assert!(generation.contains("retry_delivers_downstream_without_rotating_generation_twice"));
}

#[test]
fn tenant_generation_health_is_context_scoped_and_component_aware() {
    let generation = source("apps/server/src/services/tenant_cache_generation.rs");
    let status = source("apps/server/src/services/tenant_cache_generation_status.rs");

    for required in [
        "GENERATION_RECONCILE_INTERVAL",
        "MissedTickBehavior::Skip",
        "periodic_reconciliation",
        "state: Arc<TenantCacheGenerationListenerState>",
        "tenant_cache_generation_listener_snapshot",
        ".mark_subscriber_ready_after_recovery()",
        ".mark_subscriber_activity_healthy()",
        ".mark_reconciliation_healthy()",
        ".mark_subscriber_degraded(",
        ".mark_reconciliation_degraded(",
        ".mark_local_degraded(",
    ] {
        assert!(
            generation.contains(required),
            "tenant generation runtime must retain {required}"
        );
    }

    for required in [
        "subscriber_ready && reconciliation_healthy",
        "MAX_TENANT_GENERATION_LISTENER_ERROR_BYTES",
        "redis_health_requires_subscriber_and_reconciliation",
        "subscriber_activity_does_not_hide_reconciliation_failure",
        "reconciliation_success_does_not_hide_subscriber_failure",
        "independent_runtime_states_do_not_overwrite_each_other",
        "record_tenant_generation_listener_metrics",
    ] {
        assert!(
            status.contains(required),
            "tenant generation status must retain {required}"
        );
    }
    assert!(
        !status.contains("OnceLock"),
        "tenant generation listener state must be owned by ServerRuntimeContext"
    );
}

#[test]
fn tenant_generation_metrics_are_label_free_and_registered_once() {
    let observability = source("crates/rustok-cache/src/tenant_generation_observability.rs");

    for required in [
        "rustok_cache_tenant_generation_listener_status",
        "rustok_cache_tenant_generation_local_ready",
        "rustok_cache_tenant_generation_subscriber_ready",
        "rustok_cache_tenant_generation_reconciliation_healthy",
        "register_runtime_collector",
        "metrics_are_label_free_and_component_specific",
    ] {
        assert!(
            observability.contains(required),
            "tenant generation observability must retain {required}"
        );
    }
    assert!(
        observability.contains("assert!(!payload.contains('{'))"),
        "tenant generation lifecycle metrics must remain label-free"
    );
}
