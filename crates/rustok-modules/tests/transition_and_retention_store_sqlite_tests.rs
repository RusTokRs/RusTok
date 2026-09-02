use chrono::{Duration, Utc};
use rustok_modules::{
    ConflictFenceSet, GlobalSecurityEpoch, ModuleTransitionCheckpoint, ModuleTransitionState,
    RetentionHoldKind, RetentionHoldRecord, RetentionHoldStore, RetentionTarget,
    TransitionCheckpointStore,
};
use sea_orm::{ConnectOptions, Database, DatabaseConnection};
use sea_orm_migration::SchemaManager;
use uuid::Uuid;

async fn setup_db() -> DatabaseConnection {
    let url = format!(
        "sqlite:file:transition_store_test_{}?mode=memory&cache=shared",
        Uuid::new_v4()
    );
    let mut options = ConnectOptions::new(url);
    options
        .max_connections(1)
        .min_connections(1)
        .sqlx_logging(false);
    let db = Database::connect(options)
        .await
        .expect("sqlite in-memory database connection");

    let manager = SchemaManager::new(&db);
    for migration in rustok_modules::migrations::migrations() {
        migration
            .up(&manager)
            .await
            .expect("migration up should succeed");
    }

    db
}

#[tokio::test]
async fn test_transition_checkpoint_persistence_and_update() {
    let db = setup_db().await;
    let operation_id = Uuid::new_v4();
    let tenant_id = Some(Uuid::new_v4());
    let fences = ConflictFenceSet::derive_module_update_fences(
        "orders",
        tenant_id,
        &["node-alpha".to_string(), "node-beta".to_string()],
    );

    let initial_checkpoint = ModuleTransitionCheckpoint {
        operation_id,
        revision: 1,
        module_slug: "orders".to_string(),
        tenant_id,
        predecessor_digest: Some("sha256:orders_v1".to_string()),
        candidate_digest: "sha256:orders_v2".to_string(),
        state: ModuleTransitionState::Fenced,
        security_epoch: GlobalSecurityEpoch(42),
        fences: fences.clone(),
        recovery_attempt_count: 0,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };

    // 1. Save initial checkpoint
    TransitionCheckpointStore::save_checkpoint(&db, &initial_checkpoint)
        .await
        .expect("save_checkpoint should succeed");

    // 2. Load checkpoint
    let loaded = TransitionCheckpointStore::load_checkpoint(&db, operation_id)
        .await
        .expect("load_checkpoint should succeed")
        .expect("checkpoint must exist");

    assert_eq!(loaded.operation_id, operation_id);
    assert_eq!(loaded.revision, 1);
    assert_eq!(loaded.module_slug, "orders");
    assert_eq!(loaded.tenant_id, tenant_id);
    assert_eq!(
        loaded.predecessor_digest,
        Some("sha256:orders_v1".to_string())
    );
    assert_eq!(loaded.candidate_digest, "sha256:orders_v2");
    assert_eq!(loaded.state, ModuleTransitionState::Fenced);
    assert_eq!(loaded.security_epoch.value(), 42);
    assert_eq!(loaded.fences, fences);
    assert_eq!(loaded.recovery_attempt_count, 0);

    // 3. Advance state and update in DB
    let timeout_at = Utc::now() + Duration::minutes(5);
    let updated_checkpoint = ModuleTransitionCheckpoint {
        operation_id,
        revision: 2,
        module_slug: "orders".to_string(),
        tenant_id,
        predecessor_digest: Some("sha256:orders_v1".to_string()),
        candidate_digest: "sha256:orders_v2".to_string(),
        state: ModuleTransitionState::Observing { timeout_at },
        security_epoch: GlobalSecurityEpoch(42),
        fences,
        recovery_attempt_count: 1,
        created_at: initial_checkpoint.created_at,
        updated_at: Utc::now(),
    };

    TransitionCheckpointStore::save_checkpoint(&db, &updated_checkpoint)
        .await
        .expect("update save_checkpoint should succeed");

    // 4. Reload and verify updated fields
    let reloaded = TransitionCheckpointStore::load_checkpoint(&db, operation_id)
        .await
        .expect("load_checkpoint should succeed")
        .expect("checkpoint must exist");

    assert!(matches!(
        reloaded.state,
        ModuleTransitionState::Observing { .. }
    ));
    assert_eq!(reloaded.recovery_attempt_count, 1);
    assert_eq!(reloaded.revision, 2);
}

#[tokio::test]
async fn test_retention_holds_persistence_and_ledger_reconstruction() {
    let db = setup_db().await;

    let payload_target = RetentionTarget::AdmittedPayloadCas {
        digest: "sha256:wasm_module_payload".to_string(),
    };
    let recovery_point_target = RetentionTarget::RecoveryPoint {
        snapshot_id: Uuid::new_v4(),
    };

    let hold1_id = Uuid::new_v4();
    let hold1 = RetentionHoldRecord {
        hold_id: hold1_id,
        target: payload_target.clone(),
        kind: RetentionHoldKind::ActiveRolloutWindow {
            operation_id: Uuid::new_v4(),
            expires_at: Utc::now() + Duration::hours(1),
        },
        created_at: Utc::now(),
    };

    let hold2_id = Uuid::new_v4();
    let hold2 = RetentionHoldRecord {
        hold_id: hold2_id,
        target: recovery_point_target.clone(),
        kind: RetentionHoldKind::IncidentInvestigation {
            incident_id: Uuid::new_v4(),
            reason: "Investigation trace".to_string(),
        },
        created_at: Utc::now(),
    };

    // 1. Insert holds into database
    RetentionHoldStore::insert_hold(&db, &hold1)
        .await
        .expect("insert_hold 1 should succeed");
    RetentionHoldStore::insert_hold(&db, &hold2)
        .await
        .expect("insert_hold 2 should succeed");

    // 2. Reconstruct ledger from database
    let ledger = RetentionHoldStore::load_active_ledger(&db)
        .await
        .expect("load_active_ledger should succeed");

    assert!(!ledger.is_collection_allowed(&payload_target));
    assert!(!ledger.is_collection_allowed(&recovery_point_target));

    // 3. Delete hold1 from DB (finalization)
    let deleted = RetentionHoldStore::delete_hold(&db, hold1_id)
        .await
        .expect("delete_hold should succeed");
    assert!(deleted);

    // 4. Reload ledger from DB
    let reloaded_ledger = RetentionHoldStore::load_active_ledger(&db)
        .await
        .expect("reloaded load_active_ledger should succeed");

    // Payload is now collectable, recovery point still held
    assert!(reloaded_ledger.is_collection_allowed(&payload_target));
    assert!(!reloaded_ledger.is_collection_allowed(&recovery_point_target));
}
