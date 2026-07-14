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
fn rbac_listener_starts_before_superadmin_role_reconciliation() {
    let bootstrap = source("apps/server/src/services/server_bootstrap.rs");
    let cache_init = bootstrap
        .find("let cache = ensure_cache_service(runtime_ctx);")
        .expect("server bootstrap must initialize the shared cache service");
    let listener = bootstrap
        .find("start_rbac_cache_invalidation_listener(runtime_ctx, cache).await?;")
        .expect("server bootstrap must start RBAC invalidation");
    let superadmin = bootstrap
        .find("ensure_default_superadmin(runtime_ctx).await")
        .expect("server bootstrap must reconcile the default superadmin");

    assert!(cache_init < listener);
    assert!(listener < superadmin);
}

#[test]
fn full_runtime_reuses_the_early_cache_service() {
    let runtime = source("apps/server/src/services/app_runtime.rs");
    let cache_runtime = source("apps/server/src/services/cache_runtime.rs");

    assert!(runtime.contains("let cache_service = ensure_cache_service(&runtime_ctx);"));
    assert!(!runtime.contains("CacheService::from_url(settings.cache.redis_url.as_deref())"));
    assert!(cache_runtime.contains("shared_insert_if_absent(candidate.clone())"));
    assert!(cache_runtime.contains("ctx.shared_get::<CacheService>().unwrap_or(candidate)"));
}

#[test]
fn rate_limit_cleanup_and_existing_bootstrap_tests_remain_present() {
    let runtime = source("apps/server/src/services/app_runtime.rs");

    for required in [
        "cleanup_task(limiter_for_cleanup).await;",
        "compiled_surface_contract_rejects_missing_embedded_admin",
        "compiled_surface_contract_rejects_missing_embedded_storefront",
        "compiled_surface_contract_accepts_matching_features",
        "compiled_surface_contract_allows_headless_profile_without_embedded_ui_features",
        "bootstrap_registry_only_runtime_forces_headless_surfaces",
    ] {
        assert!(runtime.contains(required), "app runtime must retain {required}");
    }
}
