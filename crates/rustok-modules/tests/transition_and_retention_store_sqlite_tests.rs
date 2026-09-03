use chrono::{Duration, Utc};
use rustok_modules::{
    ConflictFenceSet, GlobalSecurityEpoch, ModuleCommandContext, ModuleInstallationScope,
    ModuleTransitionCheckpoint, ModuleTransitionState, ReleaseAdmissionIntentJournal,
    ReleaseAdmissionJournalError, RetentionHoldKind, RetentionHoldRecord, RetentionHoldStore,
    RetentionTarget, TransitionCheckpointStore,
};
use sea_orm::{ConnectOptions, ConnectionTrait, Database, DatabaseConnection};
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

#[tokio::test]
async fn test_release_admission_intent_journal_lifecycle_sqlite() {
    let db = setup_db().await;
    let actor_id = Uuid::new_v4();
    let idempotency_key = Uuid::new_v4();
    let correlation_id = Uuid::new_v4();
    let tenant_id = Uuid::new_v4();
    let scope = ModuleInstallationScope::Tenant { tenant_id };
    let context = ModuleCommandContext {
        actor_id,
        tenant_id: Some(tenant_id),
        idempotency_key,
        trace_id: "trace-admission-001".to_string(),
        correlation_id,
    };
    let request_digest = "sha256:payload_staged_digest_001";

    // 1. Initial reservation before staging / CAS publication
    let intent = ReleaseAdmissionIntentJournal::record_staging_intent(
        &db,
        &scope,
        &context,
        request_digest,
    )
    .await
    .expect("recording initial staging intent must succeed");

    assert_eq!(intent.actor_id, actor_id);
    assert_eq!(intent.idempotency_key, idempotency_key);
    assert_eq!(intent.request_digest, request_digest);
    assert_eq!(intent.installation_id, None);

    // 2. Exact idempotent retry returns the same record
    let retry = ReleaseAdmissionIntentJournal::record_staging_intent(
        &db,
        &scope,
        &context,
        request_digest,
    )
    .await
    .expect("idempotent retry must succeed");
    assert_eq!(retry.idempotency_key, idempotency_key);
    assert_eq!(retry.installation_id, None);

    // 3. Conflicting request digest on the same idempotency key fails closed
    let conflict = ReleaseAdmissionIntentJournal::record_staging_intent(
        &db,
        &scope,
        &context,
        "sha256:different_conflicting_digest",
    )
    .await;
    assert!(matches!(
        conflict,
        Err(ReleaseAdmissionJournalError::Conflict(key, _)) if key == idempotency_key
    ));

    // 4. Stale/unfinished scan detects the intent (using zero duration to find in-flight intents)
    let stale_intents = ReleaseAdmissionIntentJournal::scan_stale_unfinished_intents(
        &db,
        Duration::zero(),
    )
    .await
    .expect("scanning unfinished intents should succeed");
    assert_eq!(stale_intents.len(), 1);
    assert_eq!(stale_intents[0].idempotency_key, idempotency_key);

    // 5. Create admitted installation record to satisfy foreign key constraint
    let installation_id = Uuid::new_v4();
    db.execute_unprepared(&format!(
        "INSERT INTO module_artifact_installations (\
            installation_id, scope_kind, tenant_id, registry, repository, manifest_digest, \
            slug, version, payload_kind, runtime_abi, payload_digest, entrypoint, descriptor, \
            data_owner_id, settings_instance_id, dependency_graph_revision, dependency_graph_digest, \
            dependency_lock, installed_at\
         ) VALUES ('{installation_id}', 'tenant', '{tenant_id}', 'local', 'orders', 'sha256:manifest', \
            'orders', '1.0.0', 'wasm', 'wasi_p2', 'sha256:payload', 'run', '{{}}', \
            '{actor_id}', '{actor_id}', 1, 'sha256:lock', '{{}}', '2026-09-03T00:00:00Z')"
    ))
    .await
    .expect("inserting installation record must succeed");

    let bound = ReleaseAdmissionIntentJournal::bind_committed_installation(
        &db,
        idempotency_key,
        installation_id,
    )
    .await
    .expect("binding committed installation should succeed");
    assert!(bound);

    // 6. Once committed, the intent is no longer reported as unfinished
    let active_unfinished = ReleaseAdmissionIntentJournal::scan_stale_unfinished_intents(
        &db,
        Duration::zero(),
    )
    .await
    .expect("scanning unfinished intents should succeed");
    assert!(active_unfinished.is_empty());
}

