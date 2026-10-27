use chrono::{Duration, Utc};
use rustok_core::MigrationSource;
use rustok_modules::{
    ConflictFenceSet, ModuleTransitionCheckpoint, ModuleTransitionState, RetentionHoldKind,
    RetentionHoldRecord, RetentionHoldStore, RetentionTarget, SeaOrmSecurityEpochStore,
    TransitionCheckpointStore, evaluate_transition_watchdog, reap_expired_retention_holds,
};
use sea_orm::{ConnectOptions, Database, DatabaseConnection};
use sea_orm_migration::SchemaManager;
use uuid::Uuid;

async fn setup_test_db() -> DatabaseConnection {
    let url = format!(
        "sqlite:file:watchdog_test_{}?mode=memory&cache=shared",
        Uuid::new_v4()
    );
    let mut options = ConnectOptions::new(url);
    options
        .max_connections(1)
        .min_connections(1)
        .sqlx_logging(false);
    let db = Database::connect(options)
        .await
        .expect("sqlite in-memory connection");

    let manager = SchemaManager::new(&db);
    for migration in rustok_outbox::OutboxModule.migrations() {
        migration
            .up(&manager)
            .await
            .expect("outbox migration should apply");
    }
    for migration in rustok_modules::migrations::migrations() {
        migration
            .up(&manager)
            .await
            .expect("migration should apply");
    }

    db
}

#[tokio::test]
async fn test_watchdog_expired_window_requires_intervention_and_expires_rollout_hold() {
    let db = setup_test_db().await;
    let current_epoch = SeaOrmSecurityEpochStore::current(&db)
        .await
        .expect("durable epoch seeded by migration");
    let operation_id = Uuid::new_v4();

    // 1. Seed expired observation window (10 seconds in the past)
    let expired_timeout = Utc::now() - Duration::seconds(10);
    let checkpoint = ModuleTransitionCheckpoint {
        operation_id,
        revision: 1,
        module_slug: "checkout".to_string(),
        tenant_id: None,
        predecessor_digest: Some("sha256:v1".to_string()),
        candidate_digest: "sha256:v2".to_string(),
        state: ModuleTransitionState::Observing {
            timeout_at: expired_timeout,
        },
        security_epoch: current_epoch,
        fences: ConflictFenceSet::new(vec![]),
        recovery_attempt_count: 0,
        created_at: Utc::now() - Duration::minutes(10),
        updated_at: Utc::now() - Duration::minutes(5),
    };
    TransitionCheckpointStore::save_checkpoint(&db, &checkpoint)
        .await
        .expect("save checkpoint");

    // 2. Seed a rollout retention hold tied to this operation whose window
    // has elapsed. It must stop counting as active and be deleted by the
    // bounded reaper; otherwise every unattended transition would pin the
    // predecessor bytes forever.
    let rollout_hold_id = Uuid::new_v4();
    let rollout_hold = RetentionHoldRecord {
        hold_id: rollout_hold_id,
        target: RetentionTarget::SourceCasBlob {
            digest: "sha256:v1_source".to_string(),
        },
        kind: RetentionHoldKind::ActiveRolloutWindow {
            operation_id,
            expires_at: expired_timeout,
        },
        created_at: Utc::now() - Duration::minutes(10),
    };
    RetentionHoldStore::insert_hold(&db, &rollout_hold)
        .await
        .expect("insert rollout hold");

    // 3. Seed an unrelated audit hold that must also remain untouched.
    let audit_hold_id = Uuid::new_v4();
    let audit_hold = RetentionHoldRecord {
        hold_id: audit_hold_id,
        target: RetentionTarget::DiagnosticLog { operation_id },
        kind: RetentionHoldKind::AuditTrail {
            compliance_id: "COMPLIANCE-2026-A".to_string(),
        },
        created_at: Utc::now() - Duration::minutes(10),
    };
    RetentionHoldStore::insert_hold(&db, &audit_hold)
        .await
        .expect("insert audit hold");

    // 4. Run transition watchdog evaluator. An elapsed window is not health
    // evidence, so automatic action must stop for operator intervention
    // instead of converging.
    let updated = evaluate_transition_watchdog(&db)
        .await
        .expect("evaluate watchdog");

    assert_eq!(updated.len(), 1);
    assert_eq!(updated[0].operation_id, operation_id);
    match &updated[0].state {
        ModuleTransitionState::InterventionRequired { failure_reason } => {
            assert!(
                failure_reason.contains("explicit finalize attestation"),
                "unexpected failure reason: {failure_reason}"
            );
            assert!(
                failure_reason.contains("serving selection was not changed"),
                "unexpected failure reason: {failure_reason}"
            );
        }
        other => panic!("expected InterventionRequired, got {other:?}"),
    }

    // 5. Verify the intervention-required state is persisted.
    let reloaded = TransitionCheckpointStore::load_checkpoint(&db, operation_id)
        .await
        .expect("load checkpoint")
        .expect("checkpoint exists");
    assert!(matches!(
        reloaded.state,
        ModuleTransitionState::InterventionRequired { .. }
    ));

    // 6. The elapsed rollout hold is no longer active, while the indefinite
    // audit hold remains active. The reaper then deletes only the elapsed
    // rollout row.
    let active_holds = RetentionHoldStore::list_active_holds(&db)
        .await
        .expect("list holds");
    assert_eq!(active_holds.len(), 1);
    assert_eq!(active_holds[0].hold_id, audit_hold_id);
    assert!(
        !active_holds
            .iter()
            .any(|hold| hold.hold_id == rollout_hold_id)
    );

    let report = reap_expired_retention_holds(&db)
        .await
        .expect("reap expired retention holds");
    assert_eq!(report.scanned, 2);
    assert_eq!(report.expired, 1);
    assert_eq!(report.deleted, 1);
    assert!(report.has_changes());
    let active_holds = RetentionHoldStore::list_active_holds(&db)
        .await
        .expect("list holds after reap");
    assert_eq!(active_holds.len(), 1);
    assert_eq!(active_holds[0].hold_id, audit_hold_id);
}

#[tokio::test]
async fn test_watchdog_active_window_not_prematurely_converged() {
    let db = setup_test_db().await;
    let current_epoch = SeaOrmSecurityEpochStore::current(&db)
        .await
        .expect("durable epoch seeded by migration");
    let operation_id = Uuid::new_v4();

    // Observation window still active (10 minutes in the future)
    let active_timeout = Utc::now() + Duration::minutes(10);
    let checkpoint = ModuleTransitionCheckpoint {
        operation_id,
        revision: 1,
        module_slug: "payments".to_string(),
        tenant_id: None,
        predecessor_digest: Some("sha256:p1".to_string()),
        candidate_digest: "sha256:p2".to_string(),
        state: ModuleTransitionState::Observing {
            timeout_at: active_timeout,
        },
        security_epoch: current_epoch,
        fences: ConflictFenceSet::new(vec![]),
        recovery_attempt_count: 0,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };
    TransitionCheckpointStore::save_checkpoint(&db, &checkpoint)
        .await
        .expect("save checkpoint");

    let updated = evaluate_transition_watchdog(&db)
        .await
        .expect("evaluate watchdog");

    // Nothing should have changed
    assert!(updated.is_empty());

    let reloaded = TransitionCheckpointStore::load_checkpoint(&db, operation_id)
        .await
        .expect("load checkpoint")
        .expect("checkpoint exists");
    assert!(matches!(
        reloaded.state,
        ModuleTransitionState::Observing { .. }
    ));
}

#[tokio::test]
async fn test_watchdog_epoch_preemption_requires_intervention_without_claiming_recovery() {
    let db = setup_test_db().await;
    // The checkpoint starts under the epoch durable at activation time...
    let stale_epoch = SeaOrmSecurityEpochStore::current(&db)
        .await
        .expect("durable epoch seeded by migration");

    // ...then a quarantine anywhere in the platform advances the durable
    // epoch, invalidating every in-flight transition from the old epoch.
    let advanced = SeaOrmSecurityEpochStore::advance(
        &db,
        "Global security quarantine: supply-chain threat detected",
        Utc::now(),
    )
    .await
    .expect("advance durable epoch");
    assert!(advanced > stale_epoch);

    let operation_id = Uuid::new_v4();
    let checkpoint = ModuleTransitionCheckpoint {
        operation_id,
        revision: 1,
        module_slug: "auth".to_string(),
        tenant_id: None,
        predecessor_digest: Some("sha256:safe_predecessor".to_string()),
        candidate_digest: "sha256:compromised_candidate".to_string(),
        state: ModuleTransitionState::Observing {
            timeout_at: Utc::now() + Duration::minutes(5),
        },
        security_epoch: stale_epoch, // Stale!
        fences: ConflictFenceSet::new(vec![]),
        recovery_attempt_count: 0,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };
    TransitionCheckpointStore::save_checkpoint(&db, &checkpoint)
        .await
        .expect("save checkpoint");

    let updated = evaluate_transition_watchdog(&db)
        .await
        .expect("evaluate watchdog");

    assert_eq!(updated.len(), 1);
    assert_eq!(updated[0].operation_id, operation_id);
    match &updated[0].state {
        ModuleTransitionState::InterventionRequired { failure_reason } => {
            assert!(
                failure_reason.contains("Security epoch preemption"),
                "unexpected failure reason: {failure_reason}"
            );
            assert!(
                failure_reason.contains("serving selection was not changed"),
                "unexpected failure reason: {failure_reason}"
            );
        }
        other => panic!("expected InterventionRequired, got {other:?}"),
    }
    assert_eq!(updated[0].recovery_attempt_count, 0);
}
