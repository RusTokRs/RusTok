use chrono::{Duration, Utc};
use rustok_modules::{
    ConflictFenceSet, ModuleTransitionCheckpoint, ModuleTransitionState, RetentionHoldKind,
    RetentionHoldRecord, RetentionHoldStore, RetentionTarget, SecurityEpochRegistry,
    TransitionCheckpointStore, evaluate_transition_watchdog,
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
    for migration in rustok_modules::migrations::migrations() {
        migration
            .up(&manager)
            .await
            .expect("migration should apply");
    }

    db
}

#[tokio::test]
async fn test_watchdog_automatic_convergence_and_hold_release() {
    let db = setup_test_db().await;
    let registry = SecurityEpochRegistry::new();
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
        security_epoch: registry.current_epoch(),
        fences: ConflictFenceSet::new(vec![]),
        recovery_attempt_count: 0,
        created_at: Utc::now() - Duration::minutes(10),
        updated_at: Utc::now() - Duration::minutes(5),
    };
    TransitionCheckpointStore::save_checkpoint(&db, &checkpoint)
        .await
        .expect("save checkpoint");

    // 2. Seed active rollout retention hold tied to this operation
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

    // 3. Seed an unrelated audit hold that MUST NOT be released
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

    // 4. Run transition watchdog evaluator
    let updated = evaluate_transition_watchdog(&db, &registry)
        .await
        .expect("evaluate watchdog");

    assert_eq!(updated.len(), 1);
    assert_eq!(updated[0].operation_id, operation_id);
    assert!(matches!(
        updated[0].state,
        ModuleTransitionState::Converged { .. }
    ));

    // 5. Verify persisted checkpoint is converged
    let reloaded = TransitionCheckpointStore::load_checkpoint(&db, operation_id)
        .await
        .expect("load checkpoint")
        .expect("checkpoint exists");
    assert!(matches!(
        reloaded.state,
        ModuleTransitionState::Converged { .. }
    ));

    // 6. Verify rollout hold was released while audit hold remains
    let active_holds = RetentionHoldStore::list_active_holds(&db)
        .await
        .expect("list holds");
    assert_eq!(active_holds.len(), 1);
    assert_eq!(active_holds[0].hold_id, audit_hold_id);
}

#[tokio::test]
async fn test_watchdog_active_window_not_prematurely_converged() {
    let db = setup_test_db().await;
    let registry = SecurityEpochRegistry::new();
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
        security_epoch: registry.current_epoch(),
        fences: ConflictFenceSet::new(vec![]),
        recovery_attempt_count: 0,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };
    TransitionCheckpointStore::save_checkpoint(&db, &checkpoint)
        .await
        .expect("save checkpoint");

    let updated = evaluate_transition_watchdog(&db, &registry)
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
async fn test_watchdog_epoch_preemption_triggers_single_attempt_recovery() {
    let db = setup_test_db().await;
    let mut registry = SecurityEpochRegistry::new();
    let stale_epoch = registry.current_epoch();

    // Advance registry epoch to invalidate stale transitions
    registry.advance_epoch("Global security quarantine: supply-chain threat detected");

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

    let updated = evaluate_transition_watchdog(&db, &registry)
        .await
        .expect("evaluate watchdog");

    assert_eq!(updated.len(), 1);
    assert_eq!(updated[0].operation_id, operation_id);
    assert!(matches!(
        updated[0].state,
        ModuleTransitionState::RecoveredToPredecessor { .. }
    ));
    assert_eq!(updated[0].recovery_attempt_count, 1);
}
