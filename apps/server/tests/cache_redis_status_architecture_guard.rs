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
fn redis_status_separates_configuration_client_and_connectivity() {
    let status = source("crates/rustok-cache/src/redis_status.rs");
    for field in [
        "pub url_present: bool",
        "pub client_initialized: bool",
        "pub connectivity_healthy: bool",
        "pub last_error: Option<String>",
    ] {
        assert!(status.contains(field), "missing Redis status field {field}");
    }
    assert!(status.contains("redis_configuration_present"));
    assert!(status.contains("redis_client_initialized"));
    assert!(status.contains("Redis URL is configured but the client could not be initialized"));
}

#[test]
fn legacy_health_report_is_derived_from_exact_redis_status() {
    let service = source("crates/rustok-cache/src/service.rs");
    let regression = source("crates/rustok-cache/tests/redis_health_regression.rs");

    assert!(service.contains("let status = self.redis_status().await;"));
    assert!(service.contains("redis_configured: status.url_present"));
    assert!(service.contains("redis_healthy: status.connectivity_healthy"));
    assert!(service.contains("redis_error: status.last_error"));
    assert!(!service.contains("redis_configured: self.has_redis()"));
    assert!(
        regression.contains("configured_invalid_redis_url_is_degraded_in_legacy_health_report")
    );
}

#[test]
fn exact_redis_gauges_use_the_shared_telemetry_registry() {
    let status = source("crates/rustok-cache/src/redis_status.rs");
    let telemetry = source("crates/rustok-telemetry/src/lib.rs");
    for metric in [
        "rustok_cache_redis_url_present",
        "rustok_cache_redis_client_initialized",
        "rustok_cache_redis_connectivity_healthy",
        "rustok_cache_redis_degraded",
    ] {
        assert!(
            status.contains(metric),
            "missing exact Redis gauge {metric}"
        );
    }
    assert!(status.contains("rustok_telemetry::register_runtime_collector"));
    assert!(telemetry.contains("pub fn register_runtime_collector"));
    assert!(telemetry.contains("REGISTRY.get()"));
}

#[test]
fn cache_module_logs_lifecycle_without_redis_url() {
    let cache_module = source("crates/rustok-cache/src/lib.rs");
    assert!(cache_module.contains("service.redis_configuration_present()"));
    assert!(cache_module.contains("service.redis_client_initialized()"));
    assert!(cache_module.contains("self.service.redis_status().await"));
    assert!(!cache_module.contains("url = ?service.redis_url()"));
    assert!(!cache_module.contains("url = %service.redis_url()"));
}

#[test]
fn invalid_config_cannot_become_a_local_shared_generation() {
    let generation = source("crates/rustok-cache/src/backend_generation.rs");
    assert!(generation.contains("self.redis_configuration_present()"));
    assert!(generation.contains("!self.redis_client_initialized()"));
    assert!(generation.contains("CacheBackendGenerationError::RedisClientUnavailable"));
    assert!(generation.contains("using isolated boot namespace"));
}

#[test]
fn tenant_generation_uses_configuration_presence_not_only_client_presence() {
    let tenant = source("apps/server/src/services/tenant_cache_generation.rs");
    assert!(tenant.contains("self.cache.redis_configuration_present()"));
    assert!(tenant.contains("!self.cache.redis_client_initialized()"));
    assert!(tenant.contains("isolated boot namespace remains active"));
}
