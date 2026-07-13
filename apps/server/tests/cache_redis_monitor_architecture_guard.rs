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
fn monitor_handle_prevents_duplicate_workers() {
    let monitor = source("apps/server/src/services/cache_redis_status_monitor.rs");
    assert!(monitor.contains("shared_get::<CacheRedisStatusMonitorHandle>()"));
    assert!(monitor.contains("shared_insert(CacheRedisStatusMonitorHandle)"));
}
