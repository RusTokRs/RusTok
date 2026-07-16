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
fn field_definition_cache_runtime_is_atomic_owned_and_restartable() {
    let wrapper = source("apps/server/src/services/field_definition_cache.rs");
    let cache = source("apps/server/src/services/field_definition_cache_base.rs");
    assert!(wrapper.contains("#[path = \"field_definition_cache_base.rs\"]"));
    assert!(wrapper.contains("#[path = \"field_definition_cache_reconciliation.rs\"]"));

    for required in [
        "FieldDefinitionCacheInvalidationRuntime",
        "self.task.abort();",
        "FieldDefinitionCacheStartLock",
        "shared_insert_if_absent(FieldDefinitionCacheStartLock::default())",
        "let _start_guard = start_lock",
        "Some(existing) if existing.is_running()",
        "existing.abort();",
        "shared_take::<FieldDefinitionCacheInvalidationHandle>()",
        "spawn_field_definition_cache_invalidation_consumer",
        "let first_receiver = Arc::new(Mutex::new(Some(bus.subscribe())))",
        "supervise_field_definition_cache_invalidation",
        "std::panic::catch_unwind(AssertUnwindSafe(&mut worker_factory))",
        "AssertUnwindSafe(worker).catch_unwind().await",
        "worker_panicked",
        "worker_exited",
        "invalidation_handle_reports_terminal_worker",
        "invalidation_supervisor_restarts_after_panic",
    ] {
        assert!(
            cache.contains(required),
            "field definition cache runtime must retain {required}"
        );
    }
    assert!(!cache.contains("_handle: JoinHandle<()>"));
}

#[test]
fn field_definition_cache_retains_weighted_lag_safe_behavior() {
    let cache = source("apps/server/src/services/field_definition_cache_base.rs");
    for required in [
        ".weigher(field_definition_entry_weight)",
        "FIELD_DEFINITION_CACHE_MAX_WEIGHT_BYTES",
        "cache.invalidate_all();",
        "consumer_runtime.lagged(skipped);",
        "oversized_schema_is_not_retained_beyond_weight_budget",
        "invalidate_all_clears_every_tenant_schema",
    ] {
        assert!(
            cache.contains(required),
            "field definition cache must retain {required}"
        );
    }
}
