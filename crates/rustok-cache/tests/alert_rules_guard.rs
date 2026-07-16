#[test]
fn cache_alerts_reference_metrics_exported_by_the_capability() {
    let alerts = include_str!("../../../ops/prometheus/alert_rules.yml");
    let redis = include_str!("../src/redis_status.rs");
    let generation_and_refresh = include_str!("../src/observability.rs");
    let cas = include_str!("../src/cas_observability.rs");
    let service = include_str!("../src/service.rs");
    let telemetry = include_str!("../../rustok-telemetry/src/metrics.rs");

    for (alert, metric, source) in [
        (
            "CacheRedisDegraded",
            "rustok_cache_redis_degraded",
            redis,
        ),
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
        assert!(alerts.contains(alert), "missing cache/runtime alert {alert}");
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
    assert!(alerts.contains(
        "increase(rustok_event_bus_errors_total{error_type=~\".*restart.*\"}[10m]) > 3"
    ));
    assert!(!alerts.contains("rustok_cache_cas_mismatch_total[5m]) > 0"));

    assert_eq!(alerts.matches("- alert: VerySlowRequestLatency").count(), 1);
    assert_eq!(alerts.matches("- alert: VerySlowDatabaseQueries").count(), 1);
}
