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
fn tenant_readiness_and_metrics_use_canonical_generation_listener() {
    let middleware = source("apps/server/src/middleware/mod.rs");
    let health = source("apps/server/src/controllers/health.rs");
    let metrics = source("apps/server/src/controllers/metrics.rs");

    for required in [
        "#[path = \"tenant.rs\"]",
        "mod tenant_legacy;",
        "pub use super::tenant_legacy::*;",
        "TenantCacheGenerationListenerSnapshot as TenantInvalidationListenerSnapshot",
        "TenantCacheGenerationListenerStatus as TenantInvalidationListenerStatus",
        "tenant_cache_generation_listener_snapshot(ctx)",
        "super::tenant_legacy::tenant_cache_stats(ctx).await",
        "stats.invalidation_listener_status = listener.status.metric_value()",
        "pub async fn init_tenant_cache_infrastructure(",
        "super::tenant_legacy::init_tenant_cache_infrastructure(ctx, cache_service).await",
        "shared_map::<tokio::task::JoinHandle<()>, _>",
        "task.abort()",
        "pub async fn invalidate_tenant_cache_by_host",
        "pub async fn invalidate_tenant_cache_by_slug",
        "pub async fn invalidate_tenant_cache_by_uuid",
        "bump_cache_backend_generation(TENANT_CACHE_BACKEND_PREFIX)",
        ".publish_durable(&record)",
    ] {
        assert!(
            middleware.contains(required),
            "tenant readiness wrapper must retain {required}"
        );
    }

    let bump = middleware
        .find("bump_cache_backend_generation(TENANT_CACHE_BACKEND_PREFIX)")
        .expect("manual invalidation must advance the canonical generation");
    let publish = middleware
        .find(".publish_durable(&record)")
        .expect("manual invalidation must publish the durable generation");
    assert!(bump < publish);

    assert!(
        health.contains("tenant_invalidation_listener_snapshot, TenantInvalidationListenerStatus")
    );
    assert!(health.contains("check_tenant_invalidation_listener"));
    assert!(metrics.contains("tenant_cache_stats, TenantCacheStats"));
    assert!(metrics.contains("rustok_tenant_invalidation_listener_status"));
    assert!(
        !middleware.contains("pub mod tenant;"),
        "the legacy file must not bypass the canonical readiness wrapper"
    );
}
