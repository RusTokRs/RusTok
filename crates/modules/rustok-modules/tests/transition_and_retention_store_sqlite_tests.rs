use chrono::{Duration, Utc};
use rustok_modules::{
    ConflictFenceSet, GlobalSecurityEpoch, MAX_EXPIRED_RETENTION_HOLDS_PER_REAP,
    ModuleCommandContext, ModuleInstallationScope, ModuleOperationJournal,
    ModuleOperationRequest, ModuleOperationStatus, ModuleTransitionCheckpoint,
    ModuleTransitionState, ReleaseAdmissionIntentJournal, ReleaseAdmissionJournalError,
    RetentionHoldKind, RetentionHoldRecord, RetentionHoldStore, RetentionTarget,
    TransitionCheckpointStore,
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

    // `module_operations` carries a foreign key to the core `tenants`
    // table, whose schema is owned outside this crate's migration set and
    // therefore does not exist in this isolated database. The pool is
    // pinned to a single connection above, so disabling FK enforcement
    // here covers every statement in the test.
    db.execute_raw(sea_orm::Statement::from_string(
        sea_orm::DbBackend::Sqlite,
        "PRAGMA foreign_keys = OFF".to_string(),
    ))
    .await
    .expect("disable sqlite foreign keys for the isolated crate schema");

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
async fn active_checkpoint_listing_discovers_intervention_required() {
    let db = setup_db().await;
    let tenant_id = Some(Uuid::new_v4());
    let now = Utc::now();
    let checkpoint = |operation_id: Uuid, state: ModuleTransitionState| {
        ModuleTransitionCheckpoint {
            operation_id,
            revision: 1,
            module_slug: "orders".to_string(),
            tenant_id,
            predecessor_digest: Some("sha256:orders_v1".to_string()),
            candidate_digest: "sha256:orders_v2".to_string(),
            state,
            security_epoch: GlobalSecurityEpoch(42),
            fences: ConflictFenceSet::new(vec![]),
            recovery_attempt_count: 0,
            created_at: now,
            updated_at: now,
        }
    };

    let observing_operation_id = Uuid::new_v4();
    let intervention_operation_id = Uuid::new_v4();
    let converged_operation_id = Uuid::new_v4();
    let recovered_operation_id = Uuid::new_v4();
    for checkpoint in [
        checkpoint(
            observing_operation_id,
            ModuleTransitionState::Observing {
                timeout_at: now + Duration::minutes(5),
            },
        ),
        checkpoint(
            intervention_operation_id,
            ModuleTransitionState::InterventionRequired {
                failure_reason: "operator decision required".to_string(),
            },
        ),
        checkpoint(
            converged_operation_id,
            ModuleTransitionState::Converged { finalized_at: now },
        ),
        checkpoint(
            recovered_operation_id,
            ModuleTransitionState::RecoveredToPredecessor {
                failure_reason: "predecessor restored".to_string(),
                recovered_at: now,
            },
        ),
    ] {
        TransitionCheckpointStore::save_checkpoint(&db, &checkpoint)
            .await
            .expect("seed checkpoint");
    }

    let unresolved =
        TransitionCheckpointStore::list_active_checkpoints_for_tenant(&db, tenant_id)
            .await
            .expect("list unresolved checkpoints");
    let unresolved_ids = unresolved
        .iter()
        .map(|checkpoint| checkpoint.operation_id)
        .collect::<std::collections::HashSet<_>>();

    assert_eq!(unresolved_ids.len(), 2);
    assert!(unresolved_ids.contains(&observing_operation_id));
    assert!(unresolved_ids.contains(&intervention_operation_id));
    assert!(!unresolved_ids.contains(&converged_operation_id));
    assert!(!unresolved_ids.contains(&recovered_operation_id));
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
async fn expired_rollout_holds_are_inactive_and_bounded_reaping_preserves_other_holds() {
    let db = setup_db().await;
    let now = Utc::now();
    let target = RetentionTarget::AdmittedPayloadCas {
        digest: "sha256:retention_expiry".to_string(),
    };
    let expired_id = Uuid::new_v4();
    let active_id = Uuid::new_v4();
    let audit_id = Uuid::new_v4();

    for record in [
        RetentionHoldRecord {
            hold_id: expired_id,
            target: target.clone(),
            kind: RetentionHoldKind::ActiveRolloutWindow {
                operation_id: Uuid::new_v4(),
                expires_at: now - Duration::seconds(1),
            },
            created_at: now - Duration::minutes(10),
        },
        RetentionHoldRecord {
            hold_id: active_id,
            target: target.clone(),
            kind: RetentionHoldKind::ActiveRolloutWindow {
                operation_id: Uuid::new_v4(),
                expires_at: now + Duration::hours(1),
            },
            created_at: now,
        },
        RetentionHoldRecord {
            hold_id: audit_id,
            target: target.clone(),
            kind: RetentionHoldKind::AuditTrail {
                compliance_id: "COMPLIANCE-2026-A".to_string(),
            },
            created_at: now,
        },
    ] {
        RetentionHoldStore::insert_hold(&db, &record)
            .await
            .expect("retention hold insert");
    }

    let active = RetentionHoldStore::list_active_holds(&db)
        .await
        .expect("active retention holds");
    assert_eq!(active.len(), 2);
    assert!(active.iter().any(|hold| hold.hold_id == active_id));
    assert!(active.iter().any(|hold| hold.hold_id == audit_id));
    assert!(!active.iter().any(|hold| hold.hold_id == expired_id));

    let report = RetentionHoldStore::reap_expired_holds(
        &db,
        now,
        MAX_EXPIRED_RETENTION_HOLDS_PER_REAP,
    )
    .await
    .expect("expired retention hold reap");
    assert_eq!(report.scanned, 3);
    assert_eq!(report.expired, 1);
    assert_eq!(report.deleted, 1);

    let ledger = RetentionHoldStore::load_active_ledger(&db)
        .await
        .expect("active retention ledger");
    let active_records = ledger.get_active_holds_at(&target, now);
    assert_eq!(active_records.len(), 2);
    assert!(
        active_records
            .iter()
            .any(|hold| hold.hold_id == active_id)
    );
    assert!(active_records.iter().any(|hold| hold.hold_id == audit_id));
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
    let intent =
        ReleaseAdmissionIntentJournal::record_staging_intent(&db, &scope, &context, request_digest)
            .await
            .expect("recording initial staging intent must succeed");

    assert_eq!(intent.actor_id, actor_id);
    assert_eq!(intent.idempotency_key, idempotency_key);
    assert_eq!(intent.request_digest, request_digest);
    assert_eq!(intent.installation_id, None);

    // 2. Exact idempotent retry returns the same record
    let retry =
        ReleaseAdmissionIntentJournal::record_staging_intent(&db, &scope, &context, request_digest)
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
    let stale_intents =
        ReleaseAdmissionIntentJournal::scan_stale_unfinished_intents(&db, Duration::zero())
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
        &scope,
        &context,
        installation_id,
    )
    .await
    .expect("binding committed installation should succeed");
    assert!(bound);

    // 6. Once committed, the intent is no longer reported as unfinished
    let active_unfinished =
        ReleaseAdmissionIntentJournal::scan_stale_unfinished_intents(&db, Duration::zero())
            .await
            .expect("scanning unfinished intents should succeed");
    assert!(active_unfinished.is_empty());
}

#[tokio::test]
async fn resolve_post_hook_failure_accepts_typed_and_legacy_shapes_only() {
    let db = setup_db().await;
    let tenant_id = Uuid::new_v4();
    let request = || ModuleOperationRequest {
        tenant_id,
        module_slug: "orders".to_string(),
        requested_enabled: true,
        previous_effective_enabled: false,
        requested_by: Some("operator".to_string()),
        trace_id: None,
        correlation_id: Uuid::new_v4().to_string(),
        idempotency_key: None,
        expected_revision: Some(1),
    };

    // Typed shape: Validated -> Running -> CommittedHookPending ->
    // CommittedHookFailed resolves to Committed.
    let typed = ModuleOperationJournal::record(&db, request())
        .await
        .expect("record typed operation");
    ModuleOperationJournal::mark_running(&db, typed.id)
        .await
        .expect("running");
    ModuleOperationJournal::mark_committed_hook_pending(&db, typed.id)
        .await
        .expect("hook pending");
    ModuleOperationJournal::mark_committed_hook_failed(&db, typed.id, "hook timeout")
        .await
        .expect("hook failed");
    assert!(
        ModuleOperationJournal::resolve_post_hook_failure(&db, typed.id)
            .await
            .expect("resolve typed")
    );
    assert_eq!(
        ModuleOperationJournal::find(&db, typed.id)
            .await
            .expect("find typed")
            .expect("typed row")
            .status,
        ModuleOperationStatus::Committed
    );

    // Legacy shape: Failed with the pre-split `post-hook:` message prefix.
    let legacy = ModuleOperationJournal::record(&db, request())
        .await
        .expect("record legacy operation");
    ModuleOperationJournal::mark_running(&db, legacy.id)
        .await
        .expect("legacy running");
    ModuleOperationJournal::mark_failed(&db, legacy.id, "post-hook: timeout")
        .await
        .expect("legacy failed");
    assert!(
        ModuleOperationJournal::resolve_post_hook_failure(&db, legacy.id)
            .await
            .expect("resolve legacy")
    );

    // A plain failure never committed its state and must not resolve.
    let plain = ModuleOperationJournal::record(&db, request())
        .await
        .expect("record plain operation");
    ModuleOperationJournal::mark_running(&db, plain.id)
        .await
        .expect("plain running");
    ModuleOperationJournal::mark_failed(&db, plain.id, "pre-hook: denied")
        .await
        .expect("plain failed");
    assert!(
        !ModuleOperationJournal::resolve_post_hook_failure(&db, plain.id)
            .await
            .expect("resolve plain")
    );
    assert_eq!(
        ModuleOperationJournal::find(&db, plain.id)
            .await
            .expect("find plain")
            .expect("plain row")
            .status,
        ModuleOperationStatus::Failed
    );
}

#[tokio::test]
async fn committed_result_snapshot_roundtrips_and_legacy_rows_have_none() {
    let db = setup_db().await;
    let tenant_id = Uuid::new_v4();
    let request = || ModuleOperationRequest {
        tenant_id,
        module_slug: "orders".to_string(),
        requested_enabled: true,
        previous_effective_enabled: false,
        requested_by: Some("operator".to_string()),
        trace_id: None,
        correlation_id: Uuid::new_v4().to_string(),
        idempotency_key: None,
        expected_revision: Some(1),
    };

    let with_result = ModuleOperationJournal::record(&db, request())
        .await
        .expect("record operation with result");
    ModuleOperationJournal::mark_running(&db, with_result.id)
        .await
        .expect("running");
    ModuleOperationJournal::mark_committed_with_result(
        &db,
        with_result.id,
        "{\"revision\":2}",
    )
    .await
    .expect("commit with result");
    assert_eq!(
        ModuleOperationJournal::find(&db, with_result.id)
            .await
            .expect("find committed")
            .expect("committed row")
            .status,
        ModuleOperationStatus::Committed
    );
    assert_eq!(
        ModuleOperationJournal::result_snapshot(&db, with_result.id)
            .await
            .expect("read result snapshot"),
        Some("{\"revision\":2}".to_string())
    );

    // A terminal row cannot be recommitted with a different result.
    assert!(
        ModuleOperationJournal::mark_committed_with_result(
            &db,
            with_result.id,
            "{\"revision\":3}",
        )
        .await
        .is_err()
    );
    assert_eq!(
        ModuleOperationJournal::result_snapshot(&db, with_result.id)
            .await
            .expect("result snapshot unchanged"),
        Some("{\"revision\":2}".to_string())
    );

    // Plain commits (the legacy/recovery shape) store no snapshot.
    let plain = ModuleOperationJournal::record(&db, request())
        .await
        .expect("record plain operation");
    ModuleOperationJournal::mark_committed(&db, plain.id)
        .await
        .expect("plain commit from validated");
    assert_eq!(
        ModuleOperationJournal::result_snapshot(&db, plain.id)
            .await
            .expect("read missing result snapshot"),
        None
    );
}
