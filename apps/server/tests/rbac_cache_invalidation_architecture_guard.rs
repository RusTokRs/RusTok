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
fn rbac_invalidation_startup_is_serialized_and_committed_after_recovery() {
    let rbac = source("apps/server/src/services/rbac_cache_invalidation.rs");

    for required in [
        "RbacCacheInvalidationListenerStartLock",
        "shared_insert_if_absent(RbacCacheInvalidationListenerStartLock::default())",
        "let _start_guard = start_lock.0.lock().await;",
        "listener.recover_generation_and_clear().await?;",
        "RBAC_INVALIDATION_CACHE_SERVICE",
        "ctx.shared_insert(RbacCacheInvalidationListenerHandle);",
    ] {
        assert!(rbac.contains(required), "RBAC startup must retain {required}");
    }

    let recovery = rbac
        .find("listener.recover_generation_and_clear().await?;")
        .expect("RBAC listener must recover before becoming publishable");
    let publisher_commit = rbac
        .rfind("*RBAC_INVALIDATION_CACHE_SERVICE")
        .expect("RBAC publisher must be installed after startup succeeds");
    let handle_commit = rbac
        .rfind("ctx.shared_insert(RbacCacheInvalidationListenerHandle);")
        .expect("RBAC listener handle must be committed after startup succeeds");

    assert!(recovery < publisher_commit);
    assert!(publisher_commit < handle_commit);
}

#[test]
fn rbac_invalidation_recovers_missed_publications_and_superseded_offsets() {
    let rbac = source("apps/server/src/services/rbac_cache_invalidation.rs");
    let runtime = source("apps/server/src/services/rbac_runtime.rs");
    let tracker = source("crates/rustok-cache/src/bounded_invalidation.rs");

    for required in [
        "RBAC_PERMISSION_RECONCILE_INTERVAL",
        "reconcile_generation_if_advanced",
        "MissedTickBehavior::Skip",
        "periodic_reconciliation",
        "redis_publish_deferred",
        "local_publish_deferred",
        "RBAC invalidation publication deferred to generation reconciliation",
        "Local RBAC invalidation delivery deferred to generation reconciliation",
        "must not be retried blindly",
        "CacheInvalidationPayloadError::OffsetRegressed",
        "superseded_rbac_acknowledgements_are_safe_noops",
        "invalidate_all_user_permissions_cache().await",
    ] {
        assert!(
            rbac.contains(required),
            "RBAC invalidation recovery must retain {required}"
        );
    }

    assert!(rbac.contains(
        "        });\n    }\n\n    let reconcile_listener = listener.clone();"
    ));
    assert!(!rbac.contains(
        "RBAC permission cache generation advanced but Redis publish failed"
    ));
    assert!(!rbac.contains(
        "RBAC permission cache generation advanced without a local subscriber"
    ));
    assert!(!rbac.contains("users::Entity::find()"));
    assert!(runtime.contains("pub(crate) async fn invalidate_all_user_permissions_cache()"));
    assert!(runtime.contains("USER_PERMISSION_CACHE.invalidate_all();"));
    assert!(runtime.contains("full_permission_cache_invalidation_removes_unknown_user_entries"));
    assert!(tracker.contains("if proposed < current"));
    assert!(tracker.contains(
        "CacheInvalidationPayloadError::OffsetRegressed { current, proposed }"
    ));
    assert!(tracker.contains(
        "applied_acknowledgement_rejects_unseeded_skipped_or_regressed_offsets"
    ));
}

#[test]
fn rbac_permission_cache_rejects_fills_superseded_by_invalidation() {
    let core = source("crates/rustok-rbac/src/services/relation_permission_resolver.rs");
    let exports = source("crates/rustok-rbac/src/lib.rs");
    let runtime = source("apps/server/src/services/rbac_runtime.rs");

    for required in [
        "pub struct PermissionCacheLookup",
        "async fn lookup(",
        "async fn insert_if_current(",
        "cache.lookup(tenant_id, user_id).await.into_parts()",
        "resolver_uses_generation_checked_cache_publication",
    ] {
        assert!(core.contains(required), "RBAC cache core must retain {required}");
    }
    assert!(exports.contains("PermissionCacheLookup"));

    for required in [
        "RBAC_PERMISSION_CACHE_EPOCH",
        "CachedPermissionSnapshot",
        "RBAC_PERMISSION_CACHE_LOOKUP_ATTEMPTS",
        "current_permission_cache_epoch() != token",
        "advance_permission_cache_epoch();",
        "stale_permission_fill_is_rejected_after_invalidation",
    ] {
        assert!(
            runtime.contains(required),
            "RBAC Moka adapter must retain {required}"
        );
    }

    let invalidate_impl = runtime
        .find("async fn invalidate(&self, tenant_id: &uuid::Uuid, user_id: &uuid::Uuid)")
        .expect("Moka adapter must implement invalidation");
    let epoch_advance = runtime[invalidate_impl..]
        .find("advance_permission_cache_epoch();")
        .expect("every adapter invalidation must advance the epoch");
    let physical_invalidation = runtime[invalidate_impl..]
        .find("USER_PERMISSION_CACHE")
        .expect("adapter invalidation must remove the physical entry");
    assert!(epoch_advance < physical_invalidation);
}
