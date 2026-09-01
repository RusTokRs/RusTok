use rustok_modules::{
    ConflictFenceSet, ConflictKey, ConflictKeyKind, GlobalSecurityEpoch,
    SecurityEpochConflictError, SecurityEpochRegistry,
};
use uuid::Uuid;

#[test]
fn test_deadlock_free_sorting_across_all_scopes() {
    let tenant_id = Uuid::new_v4();

    let topo = ConflictKey::topology("node-42");
    let ns = ConflictKey::namespace(tenant_id, "storefront");
    let data_owner = ConflictKey::data_migration_owner("storefront");
    let release_unit = ConflictKey::release_unit("storefront");

    let fence_set = ConflictFenceSet::new(vec![
        topo.clone(),
        ns.clone(),
        data_owner.clone(),
        release_unit.clone(),
    ]);

    let keys = fence_set.keys();
    assert_eq!(keys.len(), 4);
    assert_eq!(keys[0].kind, ConflictKeyKind::ReleaseUnit);
    assert_eq!(keys[1].kind, ConflictKeyKind::DataMigrationOwner);
    assert_eq!(keys[2].kind, ConflictKeyKind::Namespace);
    assert_eq!(keys[3].kind, ConflictKeyKind::Topology);
}

#[test]
fn test_multi_scope_conflict_contention() {
    let tenant_1 = Uuid::new_v4();
    let tenant_2 = Uuid::new_v4();

    let node_list = vec!["node-1".to_string(), "node-2".to_string()];

    // Global rollout of dynamic 'checkout' module
    let checkout_rollout =
        ConflictFenceSet::derive_module_update_fences("checkout", None, &node_list);

    // Tenant 1 updating 'checkout' settings
    let tenant_1_checkout =
        ConflictFenceSet::derive_module_update_fences("checkout", Some(tenant_1), &[]);

    // Tenant 2 purging 'orders' module
    let tenant_2_orders_purge = ConflictFenceSet::derive_tenant_purge_fences("orders", tenant_2);

    // Static distribution rollout on node-1
    let static_rollout = ConflictFenceSet::derive_static_distribution_rollout_fences(
        Uuid::new_v4(),
        &["node-1".to_string()],
    );

    // 1. Checkout rollout conflicts with Tenant 1 checkout on release_unit / data_owner
    assert!(checkout_rollout.has_conflict_with(&tenant_1_checkout));

    // 2. Checkout rollout conflicts with static rollout on 'node-1' topology fence
    assert!(checkout_rollout.has_conflict_with(&static_rollout));

    // 3. Tenant 1 checkout does NOT conflict with Tenant 2 orders purge
    assert!(!tenant_1_checkout.has_conflict_with(&tenant_2_orders_purge));
}

#[test]
fn test_security_epoch_preemption_and_fail_closed_containment() {
    let mut registry = SecurityEpochRegistry::new();
    assert_eq!(registry.current_epoch(), GlobalSecurityEpoch::INITIAL);

    // Worker 1 and Worker 2 start in-flight jobs under Epoch 1
    let job_1_epoch = registry.current_epoch();
    let job_2_epoch = registry.current_epoch();

    // Critical security incident occurs (compromised dependency detected)
    let epoch_after_quarantine = registry
        .advance_epoch("Quarantined 'payment-gateway' due to suspicious outbound connection");
    assert_eq!(epoch_after_quarantine, GlobalSecurityEpoch(2));

    // Job 1 tries to commit with stale Epoch 1 -> MUST FAIL CLOSED!
    let commit_result_1 = registry.validate_epoch(job_1_epoch);
    assert!(matches!(
        commit_result_1,
        Err(SecurityEpochConflictError::EpochStale {
            expected: GlobalSecurityEpoch(1),
            current: GlobalSecurityEpoch(2),
            ..
        })
    ));

    // Job 2 also fails closed
    let commit_result_2 = registry.validate_epoch(job_2_epoch);
    assert!(commit_result_2.is_err());

    // Subsequent operation started after quarantine commits successfully under Epoch 2
    let post_quarantine_job_epoch = registry.current_epoch();
    assert!(registry.validate_epoch(post_quarantine_job_epoch).is_ok());
}
