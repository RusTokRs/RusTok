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
    let event_runtime = source("apps/server/src/services/event_transport_factory.rs");

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
    assert!(event_runtime.contains(
        "start_rbac_cache_invalidation_listener(ctx, cache.clone()).await?;"
    ));
    assert!(event_runtime.contains(
        "CacheService must be initialized before the event runtime"
    ));
}

#[test]
fn rbac_invalidation_recovers_missed_publications_and_superseded_offsets() {
    let rbac = source("apps/server/src/services/rbac_cache_invalidation.rs");
    let repair = source("apps/server/src/services/rbac_repair.rs");
    let runtime = source("apps/server/src/services/rbac_runtime.rs");
    let tracker = source("crates/rustok-cache/src/bounded_invalidation.rs");

    for required in [
        "RBAC_PERMISSION_RECONCILE_INTERVAL",
        "RBAC_PERMISSION_INVALIDATE_ALL_KEY",
        "RbacInvalidationTarget::All",
        "publish_all_rbac_invalidation",
        "namespace_wide_invalidation_key_is_explicit",
        "reconcile_generation_if_advanced",
        "MissedTickBehavior::Skip",
        "periodic_reconciliation",
        "fanout_deferred",
        "redis_publish_deferred",
        "local_publish_deferred",
        "RBAC invalidation fan-out deferred to generation reconciliation",
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

    let publish_start = rbac
        .find("pub async fn publish_user_rbac_invalidation")
        .expect("RBAC invalidation publisher must exist");
    let publish_end = rbac[publish_start..]
        .find("pub async fn start_rbac_cache_invalidation_listener")
        .map(|offset| publish_start + offset)
        .expect("RBAC listener startup must follow the publisher");
    let publish = &rbac[publish_start..publish_end];
    assert!(publish.contains("let fanout: Result<CacheInvalidationOutcome> = async"));
    assert!(publish.contains("let outcome = match fanout"));
    assert!(publish.contains("Err(error) =>"));
    assert!(publish.contains("return Ok(());"));
    assert_eq!(
        publish.matches("must not be retried blindly").count(),
        1,
        "only generation advance may remain a hard post-commit error"
    );

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

    assert!(repair.contains(
        "use super::rbac_cache_invalidation::publish_all_rbac_invalidation;"
    ));
    assert!(!repair.contains("publish_user_rbac_invalidation"));
    assert!(repair.contains("if !effective_affected_users.is_empty()"));
    assert_eq!(
        repair.matches("publish_all_rbac_invalidation().await?;").count(),
        1,
        "role repair must publish exactly one namespace-wide invalidation"
    );
    let affected_loop = repair
        .find("for affected in report.affected_users.drain(..)")
        .expect("role repair must filter and clear affected users locally");
    let namespace_publish = repair
        .find("publish_all_rbac_invalidation().await?;")
        .expect("role repair must publish the namespace-wide invalidation");
    assert!(
        affected_loop < namespace_publish,
        "namespace-wide publication must occur after the affected-user loop"
    );
}

#[test]
fn rbac_permission_cache_rejects_fills_superseded_by_invalidation() {
    let core = source("crates/rustok-rbac/src/services/relation_permission_resolver.rs");
    let exports = source("crates/rustok-rbac/src/lib.rs");
    let runtime = source("apps/server/src/services/rbac_runtime.rs");
    let request_scope = source("apps/server/src/services/rbac_request_scope.rs");

    for required in [
        "pub struct PermissionCacheLookup",
        "MAX_PERMISSION_CACHE_RESOLUTION_ATTEMPTS",
        "async fn lookup(",
        "async fn insert_if_current(",
        ") -> bool {",
        "cache.lookup(tenant_id, user_id).await.into_parts()",
        "resolver_retries_generation_checked_cache_publication",
        "continuous_invalidation_fails_closed",
        "permissions: Vec::new()",
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
        "return false;",
        "stale_permission_fill_is_rejected_after_invalidation",
        "assert!(!published);",
        "invalidate_current_rbac_request_scope(tenant_id, user_id)",
        "database_rejects_cross_tenant_role_links_and_loader_keeps_local_role",
    ] {
        assert!(
            runtime.contains(required),
            "RBAC Moka adapter must retain {required}"
        );
    }

    for required in [
        "struct RbacRequestScopeState",
        "valid: AtomicBool",
        "invalidate_current_rbac_request_scope",
        "exact_actor_invalidation_expires_the_request_snapshot",
        "unrelated_actor_invalidation_preserves_the_request_snapshot",
    ] {
        assert!(
            request_scope.contains(required),
            "RBAC request scope must retain {required}"
        );
    }

    let invalidate_impl = runtime
        .find("async fn invalidate(&self, tenant_id: &uuid::Uuid, user_id: &uuid::Uuid)")
        .expect("Moka adapter must implement invalidation");
    let request_scope_expiry = runtime[invalidate_impl..]
        .find("invalidate_current_rbac_request_scope(tenant_id, user_id)")
        .expect("adapter invalidation must expire the matching request scope");
    let epoch_advance = runtime[invalidate_impl..]
        .find("advance_permission_cache_epoch();")
        .expect("every adapter invalidation must advance the epoch");
    let physical_invalidation = runtime[invalidate_impl..]
        .find("USER_PERMISSION_CACHE")
        .expect("adapter invalidation must remove the physical entry");
    assert!(request_scope_expiry < epoch_advance);
    assert!(epoch_advance < physical_invalidation);

    let conditional_insert = runtime
        .find("async fn insert_if_current(")
        .expect("Moka adapter must implement conditional insert");
    let conditional_insert = &runtime[conditional_insert..];
    assert!(conditional_insert.contains(") -> bool {"));
    assert!(conditional_insert.contains("return false;"));
    assert!(conditional_insert.contains("true\n    }"));
}
