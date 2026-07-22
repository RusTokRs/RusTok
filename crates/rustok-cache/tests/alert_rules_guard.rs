#[test]
fn cache_alerts_reference_metrics_exported_by_the_capability() {
    let alerts = include_str!("../../../ops/prometheus/alert_rules.yml");
    let redis = include_str!("../src/redis_status.rs");
    let generation_and_refresh = include_str!("../src/observability.rs");
    let cas = include_str!("../src/cas_observability.rs");
    let service = include_str!("../src/service.rs");
    let telemetry = include_str!("../../rustok-telemetry/src/metrics.rs");

    for (alert, metric, source) in [
        ("CacheRedisDegraded", "rustok_cache_redis_degraded", redis),
        (
            "CacheGenerationBumpFailure",
            "rustok_cache_generation_bump_failures_total",
            generation_and_refresh,
        ),
        (
            "CacheInvalidationPublishFailures",
            "rustok_cache_invalidation_redis_publish_failure_total",
            service,
        ),
        (
            "CacheRefreshSaturated",
            "rustok_cache_refresh_saturated_total",
            generation_and_refresh,
        ),
        (
            "CacheCompareAndSetFailures",
            "rustok_cache_cas_failed_total",
            cas,
        ),
        (
            "EventConsumerSkippedMessages",
            "rustok_event_consumer_lagged_total",
            telemetry,
        ),
        (
            "RepeatedRuntimeWorkerRestarts",
            "rustok_event_bus_errors_total",
            telemetry,
        ),
    ] {
        assert!(
            alerts.contains(alert),
            "missing cache/runtime alert {alert}"
        );
        assert!(
            alerts.contains(metric),
            "alert {alert} must query canonical metric {metric}"
        );
        assert!(
            source.contains(metric),
            "canonical metric {metric} must remain exported"
        );
    }

    assert!(alerts.contains("increase(rustok_cache_generation_bump_failures_total[5m]) > 0"));
    assert!(alerts.contains("increase(rustok_cache_cas_failed_total[5m]) > 0"));
    assert!(alerts.contains("increase(rustok_event_consumer_lagged_total[5m]) > 0"));
    assert!(
        alerts.contains(
            "increase(rustok_event_bus_errors_total{error_type=~\".*restart.*\"}[10m]) > 3"
        )
    );
    assert!(!alerts.contains("rustok_cache_cas_mismatch_total[5m]) > 0"));

    assert_eq!(alerts.matches("- alert: VerySlowRequestLatency").count(), 1);
    assert_eq!(
        alerts.matches("- alert: VerySlowDatabaseQueries").count(),
        1
    );
}

#[test]
fn live_redis_hardening_retains_latency_circuit_and_restart_recovery() {
    let evidence = include_str!("real_redis_hardening.rs");
    let shared_backend = include_str!("../src/shared_backend.rs");

    for required in [
        "shared_backend_times_out_opens_circuit_and_recovers_after_latency",
        "CLIENT",
        "PAUSE",
        ".arg(\"ALL\")",
        "failure_threshold: 1",
        "success_threshold: 1",
        "timeout: Duration::from_millis(200)",
        "shared Redis cache operation timed out after 2000 ms",
        "Redis unavailable (circuit breaker open)",
        "rejected_at.elapsed() < Duration::from_millis(250)",
        "half-open Redis probe should recover after CLIENT PAUSE expires",
        "shared_backend_recovers_across_two_redis_restarts",
        "RUSTOK_CACHE_REDIS_SERVER_BIN",
        "for cycle in 1_u8..=2",
        "stop_redis(&mut redis_process).await;",
        "redis_process = spawn_redis(binary.as_str(), port).await;",
        "wait_for_backend_health(backend.as_ref()).await;",
        "recovered backend should accept writes",
        "recovered backend should accept reads",
    ] {
        assert!(
            evidence.contains(required),
            "live Redis resilience evidence must retain {required}"
        );
    }

    for required in [
        "SHARED_REDIS_OPERATION_TIMEOUT: Duration = Duration::from_secs(2)",
        "self.circuit_breaker",
        "shared_redis_timeout(timeout",
        "Redis unavailable (circuit breaker open)",
    ] {
        assert!(
            shared_backend.contains(required),
            "shared Redis backend must retain {required}"
        );
    }
}

#[test]
fn permanent_gate_executes_expiry_eviction_and_concurrent_local_cas() {
    let cas = include_str!("atomic_cas.rs");
    let workflow = include_str!("../../../.github/workflows/cache-hardening.yml");

    for required in [
        "concurrent_local_compare_and_set_has_exactly_one_winner",
        "expired_local_entry_cannot_be_revived_by_compare_and_set",
        "evicted_local_entry_cannot_be_revived_by_compare_and_set",
        "concurrent_local_invalidation_cannot_be_lost_to_compare_and_set",
        "while backend.stats().entries > 1",
        "capacity-one cache retained both entries",
        "for iteration in 0..128",
    ] {
        assert!(
            cas.contains(required),
            "local CAS execution evidence must retain {required}"
        );
    }

    assert!(workflow.contains("cargo test -p rustok-cache --test atomic_cas\n"));
    assert!(!workflow.contains(
        "cargo test -p rustok-cache --test atomic_cas concurrent_local_compare_and_set_has_exactly_one_winner"
    ));
}

#[test]
fn live_fallback_cas_outage_evidence_remains_wired() {
    let evidence = include_str!("fallback_cas_live.rs");
    let fallback = include_str!("../src/fallback.rs");
    let workflow = include_str!("../../../.github/workflows/cache-hardening.yml");

    for required in [
        "fallback_cas_fails_closed_during_redis_outage_and_recovers",
        "CacheService::from_url_with_options(Some(&url), fast_recovery_options())",
        "CAS must fail while the shared primary is unavailable",
        "failed shared CAS must not mutate the local mirror",
        "bounded degraded write should remain locally available",
        "CAS must reject unsynchronized local and shared state",
        "local and shared state are unsynchronized",
        "wait_for_backend_health(backend.as_ref()).await",
        "CacheCompareAndSetOutcome::Applied",
        "recovered CAS value should be readable",
    ] {
        assert!(
            evidence.contains(required),
            "live fallback CAS evidence must retain {required}"
        );
    }

    for required in [
        "if self.has_unsynchronized_mutation(key).await",
        "cache compare-and-set rejected while local and shared state are unsynchronized",
        ".primary\n            .compare_and_set(key, expected, value.clone(), ttl)",
        "self.mirror_primary_cas(key, value, ttl).await",
        "Failed to discard local mirror after CAS mismatch",
    ] {
        assert!(
            fallback.contains(required),
            "fallback CAS implementation must retain {required}"
        );
    }

    assert!(workflow.contains(
        "cargo test -p rustok-cache --test fallback_cas_live -- --ignored --nocapture --test-threads=1"
    ));
}
