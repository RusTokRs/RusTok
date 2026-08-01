use prometheus::Registry;
use rustok_telemetry::rbac_invalidation_metrics;

#[test]
fn rbac_invalidation_metrics_register_and_expose_bounded_families() {
    let registry = Registry::new();
    rbac_invalidation_metrics::register(&registry).unwrap();

    rbac_invalidation_metrics::observe_generations(11, Some(9));
    rbac_invalidation_metrics::set_worker_running("durable_generation_watchdog", true);
    rbac_invalidation_metrics::record_worker_restart(
        "durable_generation_watchdog",
        "panic",
    );
    rbac_invalidation_metrics::record_recovery("watchdog_catch_up");
    rbac_invalidation_metrics::record_full_clear("watchdog_catch_up");

    let names = registry
        .gather()
        .into_iter()
        .map(|family| family.name().to_string())
        .collect::<Vec<_>>();

    for required in [
        "rustok_rbac_invalidation_database_generation",
        "rustok_rbac_invalidation_applied_generation",
        "rustok_rbac_invalidation_generation_lag",
        "rustok_rbac_invalidation_worker_running",
        "rustok_rbac_invalidation_worker_restarts_total",
        "rustok_rbac_invalidation_recoveries_total",
        "rustok_rbac_invalidation_full_clears_total",
    ] {
        assert!(names.iter().any(|name| name == required));
    }
}
