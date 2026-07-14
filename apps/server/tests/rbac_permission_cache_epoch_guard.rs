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
fn targeted_permission_invalidations_use_bounded_key_epochs() {
    let runtime = source("apps/server/src/services/rbac_runtime.rs");

    for required in [
        "RBAC_PERMISSION_CACHE_GLOBAL_EPOCH",
        "RBAC_PERMISSION_CACHE_KEY_EPOCHS",
        "RBAC_PERMISSION_CACHE_EPOCH_STRIPES",
        "permission_cache_epoch_stripe",
        "permission_cache_token",
        "current_permission_cache_token",
        "advance_permission_cache_key_epoch",
        "advance_permission_cache_global_epoch",
        "targeted_invalidation_preserves_an_unrelated_epoch_stripe",
    ] {
        assert!(
            runtime.contains(required),
            "permission cache epoch isolation must retain {required}"
        );
    }

    assert!(runtime.contains("const RBAC_PERMISSION_CACHE_EPOCH_STRIPES: usize = 64;"));
    assert!(runtime.contains("(u64::from(global_epoch) << 32) | u64::from(key_epoch)"));
    assert!(!runtime.contains("static RBAC_PERMISSION_CACHE_EPOCH: AtomicU64"));

    let targeted = runtime
        .find("async fn invalidate(&self, tenant_id: &uuid::Uuid, user_id: &uuid::Uuid)")
        .expect("Moka adapter must implement targeted invalidation");
    let targeted = &runtime[targeted..];
    let key_epoch = targeted
        .find("advance_permission_cache_key_epoch(&key);")
        .expect("targeted invalidation must advance only its bounded key stripe");
    let physical = targeted
        .find("USER_PERMISSION_CACHE.invalidate(&key).await;")
        .expect("targeted invalidation must remove the physical key");
    assert!(key_epoch < physical);
    assert!(!targeted[..physical].contains("advance_permission_cache_global_epoch"));

    let full = runtime
        .find("pub(crate) async fn invalidate_all_user_permissions_cache()")
        .expect("full permission invalidation must exist");
    let full = &runtime[full..];
    let global_epoch = full
        .find("advance_permission_cache_global_epoch();")
        .expect("full invalidation must advance the global epoch");
    let invalidate_all = full
        .find("USER_PERMISSION_CACHE.invalidate_all();")
        .expect("full invalidation must clear physical entries");
    assert!(global_epoch < invalidate_all);
}

#[test]
fn permission_epoch_exhaustion_is_fail_closed() {
    let runtime = source("apps/server/src/services/rbac_runtime.rs");
    let core = source("crates/rustok-rbac/src/services/relation_permission_resolver.rs");

    for required in [
        "RBAC_PERMISSION_CACHE_EPOCH_EXHAUSTED",
        "current.checked_add(1)",
        "RBAC_PERMISSION_CACHE_EPOCH_EXHAUSTED.store(true, Ordering::Release)",
        "return PermissionCacheLookup::new(None, 0);",
        "current_permission_cache_token(&key) != Some(token)",
        "return false;",
    ] {
        assert!(
            runtime.contains(required),
            "permission epoch exhaustion must retain {required}"
        );
    }

    assert!(core.contains("MAX_PERMISSION_CACHE_RESOLUTION_ATTEMPTS"));
    assert!(core.contains("continuous_invalidation_fails_closed"));
    assert!(core.contains("permissions: Vec::new()"));
}
