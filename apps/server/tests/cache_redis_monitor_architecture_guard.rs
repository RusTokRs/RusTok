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
fn monitor_probes_immediately_and_refreshes_on_a_bounded_interval() {
    let monitor = source("apps/server/src/services/cache_redis_status_monitor.rs");
    assert!(
        monitor.contains("const CACHE_REDIS_STATUS_INTERVAL: Duration = Duration::from_secs(10)")
    );
    assert!(monitor.contains("let initial = cache.redis_status().await"));
    assert!(monitor.contains("tokio::time::interval(CACHE_REDIS_STATUS_INTERVAL)"));
    assert!(monitor.contains("let current = cache.redis_status().await"));
}

#[test]
fn monitor_is_started_before_cache_event_runtime() {
    let factory = source("apps/server/src/services/event_transport_factory.rs");
    let monitor = factory
        .find("start_cache_redis_status_monitor(ctx, cache.clone()).await")
        .expect("Redis status monitor must start during cache runtime bootstrap");
    let tenant_listener = factory
        .find("start_tenant_cache_generation_listener(ctx, cache.clone()).await?")
        .expect("tenant generation listener must still start");
    assert!(monitor < tenant_listener);
}

#[test]
fn monitor_logs_only_bounded_state_not_error_text_or_urls() {
    let monitor = source("apps/server/src/services/cache_redis_status_monitor.rs");
    assert!(monitor.contains("redis_url_present = current.url_present"));
    assert!(monitor.contains("redis_client_initialized = current.client_initialized"));
    assert!(monitor.contains("redis_connectivity_healthy = current.connectivity_healthy"));
    assert!(!monitor.contains("last_error ="));
    assert!(!monitor.contains("redis_url()"));
}

#[test]
fn monitor_is_owned_supervised_and_restartable() {
    let monitor = source("apps/server/src/services/cache_redis_status_monitor.rs");
    for required in [
        "CacheRedisStatusMonitorRuntime",
        "task: Option<JoinHandle<()>>",
        "task.abort();",
        "CacheRedisStatusMonitorStartLock",
        "shared_insert_if_absent(CacheRedisStatusMonitorStartLock::default())",
        "let _start_guard = start_lock.0.lock().await;",
        "existing.is_running()",
        "existing.abort();",
        "supervise_cache_redis_status_worker",
        "std::panic::catch_unwind(AssertUnwindSafe(&mut worker_factory))",
        "AssertUnwindSafe(worker).catch_unwind().await",
        "factory_panicked",
        "worker_panicked",
        "worker_exited",
        "monitor_handle_reports_terminal_tasks",
        "monitor_supervisor_restarts_after_worker_panic",
        "monitor_supervisor_restarts_after_factory_panic",
        "shared_insert(CacheRedisStatusMonitorHandle::new(task))",
    ] {
        assert!(
            monitor.contains(required),
            "Redis status monitor must retain {required}"
        );
    }
    assert!(!monitor.contains("pub struct CacheRedisStatusMonitorHandle;"));
}
