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
fn rbac_invalidation_startup_is_serialized_supervised_and_publishable_after_recovery() {
    let rbac = source("apps/server/src/services/rbac_cache_invalidation.rs");
    let event_runtime = source("apps/server/src/services/event_transport_factory.rs");

    for required in [
        "RbacCacheInvalidationListenerStartLock",
        "shared_insert_if_absent(RbacCacheInvalidationListenerStartLock::default())",
        "let _start_guard = start_lock.0.lock().await;",
        "AbortOnDropInvalidationTask",
        "RbacCacheInvalidationRuntime",
        "self.task.abort();",
        "existing.is_running()",
        "existing.abort();",
        "if let Err(error) = listener.recover_generation_and_clear().await",
        "startup_recovery_deferred",
        "RBAC_INVALIDATION_CACHE_SERVICE",
        "spawn_supervised_rbac_invalidation_worker",
        "AssertUnwindSafe(worker_factory()).catch_unwind().await",
        "local_worker_restart",
        "redis_worker_restart",
        "reconcile_worker_restart",
        "listener_handle_reports_terminal_workers",
        "invalidation_worker_supervisor_restarts_after_panic",
        "let runtime = RbacCacheInvalidationListenerHandle::new(",
        "ctx.shared_insert(runtime);",
    ] {
        assert!(
            rbac.contains(required),
            "RBAC startup must retain {required}"
        );
    }

    let early_subscription = rbac
        .find("let initial_local = cache")
        .expect("RBAC listener must subscribe locally before startup recovery");
    let recovery = rbac
        .find("if let Err(error) = listener.recover_generation_and_clear().await")
        .expect("RBAC listener must attempt recovery before becoming publishable");
    let runtime_commit = rbac
        .rfind("ctx.shared_insert(runtime);")
        .expect("supervised RBAC runtime must be installed");
    let publisher_commit = rbac
        .rfind("*RBAC_INVALIDATION_CACHE_SERVICE")
        .expect("RBAC publisher must be installed after the runtime");

    assert!(early_subscription < recovery);
    assert!(recovery < runtime_commit);
    assert!(runtime_commit < publisher_commit);
    assert!(
        event_runtime
            .contains("start_rbac_cache_invalidation_listener(ctx, cache.clone()).await?;")
    );
    assert!(event_runtime.contains("CacheService must be initialized before the event runtime"));
}

#[test]
fn rbac_invalidation_uses_one_transactionally_reserved_generation_sequence() {
    let rbac = source("apps/server/src/services/rbac_cache_invalidation.rs");
    let generation = source("apps/server/src/services/rbac_invalidation_generation.rs");
    let generation_store = source("crates/rustok-rbac/src/invalidation_generation.rs");
    let exports = source("crates/rustok-rbac/src/lib.rs");
    let committed = source("apps/server/src/services/rbac_committed_mutations.rs");
    let admin = source("apps/server/src/services/auth_admin_mutation_provider/user_admin.rs");
    let repair = source("apps/server/src/services/rbac_repair.rs");
    let runtime = source("apps/server/src/services/rbac_runtime.rs");
    let tracker = source("crates/rustok-cache/src/bounded_invalidation.rs");

    for required in [
        "read_rbac_invalidation_generation",
        "RbacInvalidationGenerationState",
        "durable_state.observe_applied(generation)",
        "RBAC_PERMISSION_RECONCILE_INTERVAL",
        "RBAC_PERMISSION_INVALIDATE_ALL_KEY",
        "RbacInvalidationTarget::All",
        "publish_all_rbac_invalidation(generation: u64)",
        "reconcile_generation_if_advanced",
        "MissedTickBehavior::Skip",
        "periodic_reconciliation",
        "fanout_deferred",
        "redis_publish_deferred",
        "local_publish_deferred",
        "durable generation reconciliation",
        "CacheInvalidationPayloadError::OffsetRegressed",
        "superseded_rbac_acknowledgements_are_safe_noops",
        "invalidate_all_user_permissions_cache().await",
    ] {
        assert!(
            rbac.contains(required),
            "RBAC invalidation recovery must retain {required}"
        );
    }

    for required in [
        "rustok_rbac::reserve_permission_invalidation_generation(db)",
        "rustok_rbac::read_permission_invalidation_generation(db)",
        "RbacInvalidationGenerationWatchdogStartLock",
        "RbacInvalidationGenerationWatchdogHandle::new(task)",
        "supervise_rbac_invalidation_generation_watchdog",
        "applied_generation_state_is_monotonic",
    ] {
        assert!(
            generation.contains(required),
            "server generation adapter must retain {required}"
        );
    }
    for required in [
        "pub async fn reserve_permission_invalidation_generation(",
        "db: &DatabaseTransaction",
        "UPDATE rbac_invalidation_state",
        "read_permission_invalidation_generation(db).await",
        "reservation_is_rolled_back_with_the_owner_transaction",
    ] {
        assert!(
            generation_store.contains(required),
            "shared generation store must retain {required}"
        );
    }
    assert!(exports.contains("reserve_permission_invalidation_generation"));
    assert!(exports.contains("read_permission_invalidation_generation"));

    assert!(!rbac.contains("bump_cache_backend_generation"));
    assert!(!rbac.contains("RBAC_PERMISSION_GENERATION_PREFIX"));
    assert!(!rbac.contains("must not be retried blindly"));
    assert!(rbac.contains("DurableCacheInvalidationRecord::new("));
    assert!(rbac.contains("            generation,"));

    for mutation in [&committed, &admin, &repair] {
        let reserve = mutation
            .find("reserve_rbac_invalidation_generation(&tx)")
            .expect("committed RBAC mutation must reserve durable generation");
        let commit = mutation[reserve..]
            .find("tx.commit()")
            .map(|offset| reserve + offset)
            .expect("generation owner must commit after reservation");
        assert!(reserve < commit);
        assert!(
            mutation.contains("durable_generation reconciliation will recover")
                || mutation.contains("durable generation reconciliation will recover")
        );
    }

    assert!(
        committed
            .contains("publish_user_rbac_invalidation(tenant_id, user_id, durable_generation)")
    );
    assert!(
        admin.contains("publish_user_rbac_invalidation(&tenant_id, &user_id, durable_generation)")
    );
    assert!(repair.contains("publish_all_rbac_invalidation(durable_generation)"));
    assert!(!repair.contains("publish_user_rbac_invalidation"));

    assert!(!rbac.contains("users::Entity::find()"));
    assert!(runtime.contains("pub(crate) async fn invalidate_all_user_permissions_cache()"));
    assert!(runtime.contains("USER_PERMISSION_CACHE.invalidate_all();"));
    assert!(runtime.contains("full_permission_cache_invalidation_removes_unknown_user_entries"));
    assert!(tracker.contains("if proposed < current"));
    assert!(
        tracker.contains("CacheInvalidationPayloadError::OffsetRegressed { current, proposed }")
    );
    assert!(
        tracker.contains("applied_acknowledgement_rejects_unseeded_skipped_or_regressed_offsets")
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
        assert!(
            core.contains(required),
            "RBAC cache core must retain {required}"
        );
    }
    assert!(exports.contains("PermissionCacheLookup"));

    for required in [
        "RBAC_PERMISSION_CACHE_GLOBAL_EPOCH",
        "RBAC_PERMISSION_CACHE_KEY_EPOCHS",
        "RBAC_PERMISSION_CACHE_EPOCH_STRIPES",
        "CachedPermissionSnapshot",
        "RBAC_PERMISSION_CACHE_LOOKUP_ATTEMPTS",
        "current_permission_cache_token(&key)",
        "advance_permission_cache_key_epoch(&key);",
        "advance_permission_cache_global_epoch();",
        "return false;",
        "stale_permission_fill_is_rejected_after_invalidation",
        "targeted_invalidation_preserves_an_unrelated_epoch_stripe",
        "assert!(!published);",
        "invalidate_current_rbac_request_scope(tenant_id, user_id)",
        "database_rejects_cross_tenant_role_links_and_loader_keeps_local_role",
    ] {
        assert!(
            runtime.contains(required),
            "RBAC Moka adapter must retain {required}"
        );
    }
    assert!(!runtime.contains("static RBAC_PERMISSION_CACHE_EPOCH: AtomicU64"));

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
    let key_epoch_advance = runtime[invalidate_impl..]
        .find("advance_permission_cache_key_epoch(&key);")
        .expect("targeted invalidation must advance its bounded key epoch");
    let physical_invalidation = runtime[invalidate_impl..]
        .find("USER_PERMISSION_CACHE.invalidate(&key).await;")
        .expect("adapter invalidation must remove the physical entry");
    assert!(request_scope_expiry < key_epoch_advance);
    assert!(key_epoch_advance < physical_invalidation);

    let conditional_insert = runtime
        .find("async fn insert_if_current(")
        .expect("Moka adapter must implement conditional insert");
    let conditional_insert = &runtime[conditional_insert..];
    assert!(conditional_insert.contains(") -> bool {"));
    assert!(conditional_insert.contains("current_permission_cache_token(&key) != Some(token)"));
    assert!(conditional_insert.contains("return false;"));
    assert!(conditional_insert.contains("true\n    }"));
}
