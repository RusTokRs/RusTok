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
fn durable_rbac_generation_exports_lag_recovery_and_worker_state() {
    let telemetry = source("crates/rustok-telemetry/src/rbac_invalidation_metrics.rs");
    let telemetry_root = source("crates/rustok-telemetry/src/lib.rs");
    let watchdog = source("apps/server/src/services/rbac_invalidation_generation.rs");

    for required in [
        "rustok_rbac_invalidation_database_generation",
        "rustok_rbac_invalidation_applied_generation",
        "rustok_rbac_invalidation_generation_lag",
        "rustok_rbac_invalidation_worker_running",
        "rustok_rbac_invalidation_worker_restarts_total",
        "rustok_rbac_invalidation_recoveries_total",
        "rustok_rbac_invalidation_full_clears_total",
        "database_generation.saturating_sub(applied_generation)",
        "i64::try_from(generation).unwrap_or(i64::MAX)",
    ] {
        assert!(telemetry.contains(required), "telemetry must retain {required}");
    }

    assert!(telemetry_root.contains("pub mod rbac_invalidation_metrics;"));
    assert!(telemetry_root.contains("rbac_invalidation_metrics::register(registry)?;"));

    for required in [
        "rbac_invalidation_metrics as invalidation_metrics",
        "invalidation_metrics::observe_generations(generation, current)",
        "invalidation_metrics::record_recovery(\"watchdog_catch_up\")",
        "invalidation_metrics::record_full_clear(\"generation_regressed\")",
        "RbacInvalidationWorkerMetricGuard",
        "invalidation_metrics::record_worker_restart(",
        "invalidation_metrics::set_worker_running(worker, true)",
        "invalidation_metrics::set_worker_running(self.worker, false)",
    ] {
        assert!(watchdog.contains(required), "watchdog must retain {required}");
    }
}
