use chrono::{Duration, Utc};
use rustok_api::{
    ModuleRetentionHoldView, ModuleTransitionCheckpointView, ModuleTransitionStateView,
};
use rustok_core::MigrationSource;
use rustok_modules::{
    ConflictFenceSet, ControlPlaneInfrastructure, GlobalSecurityEpoch, ModuleCommandContext,
    ModuleControlPlane, ModuleTransitionCheckpoint, ModuleTransitionFinalizeCommand,
    ModuleTransitionServiceError, ModuleTransitionState, ModulesModule, RetentionHoldKind,
    RetentionHoldRecord, RetentionHoldStore, RetentionTarget, TransitionCheckpointStore,
    TransitionStoreError,
};
use sea_orm::{ConnectOptions, Database, DatabaseConnection};
use sea_orm_migration::SchemaManager;
use uuid::Uuid;

async fn setup_db() -> DatabaseConnection {
    let url = format!(
        "sqlite:file:transition_service_{}?mode=memory&cache=shared",
        Uuid::new_v4()
    );
    let mut options = ConnectOptions::new(url);
    options
        .max_connections(1)
        .min_connections(1)
        .sqlx_logging(false);
    let db = Database::connect(options).await.expect("test database");
    let manager = SchemaManager::new(&db);
    for migration in rustok_outbox::OutboxModule.migrations() {
        migration.up(&manager).await.expect("outbox migration");
    }
    for migration in ModulesModule.migrations() {
        migration.up(&manager).await.expect("module migration");
    }
    db
}

fn context(idempotency_key: Uuid) -> ModuleCommandContext {
    ModuleCommandContext {
        actor_id: Uuid::new_v4(),
        tenant_id: None,
        trace_id: "transition-service-test".to_string(),
        correlation_id: idempotency_key,
        idempotency_key,
    }
}

fn checkpoint(operation_id: Uuid) -> ModuleTransitionCheckpoint {
    let now = Utc::now();
    ModuleTransitionCheckpoint {
        operation_id,
        revision: 1,
        module_slug: "checkout".to_string(),
        tenant_id: None,
        predecessor_digest: Some("sha256:predecessor".to_string()),
        candidate_digest: "sha256:candidate".to_string(),
        state: ModuleTransitionState::Observing {
            timeout_at: now + Duration::minutes(5),
        },
        security_epoch: GlobalSecurityEpoch::INITIAL,
        fences: ConflictFenceSet::new(Vec::new()),
        recovery_attempt_count: 0,
        created_at: now,
        updated_at: now,
    }
}

#[test]
fn transition_owner_projects_the_canonical_browser_safe_contract() {
    let operation_id = Uuid::new_v4();
    let checkpoint = checkpoint(operation_id);
    let view = ModuleTransitionCheckpointView::from(checkpoint.clone());

    assert_eq!(view.operation_id, operation_id.to_string());
    assert_eq!(view.module_slug, checkpoint.module_slug);
    assert_eq!(view.revision, 1);
    assert_eq!(view.state, ModuleTransitionStateView::Observing);
    assert!(
        view.state_details
            .as_deref()
            .is_some_and(|details| details.starts_with("Timeout at "))
    );
    assert_eq!(
        serde_json::to_value(&view).expect("view serialization")["operationId"],
        serde_json::Value::String(operation_id.to_string())
    );

    let hold_id = Uuid::new_v4();
    let hold = ModuleRetentionHoldView::from(RetentionHoldRecord {
        hold_id,
        target: RetentionTarget::AdmittedPayloadCas {
            digest: "sha256:predecessor".to_string(),
        },
        kind: RetentionHoldKind::ActiveRolloutWindow {
            operation_id,
            expires_at: Utc::now() + Duration::minutes(5),
        },
        created_at: Utc::now(),
    });
    assert_eq!(hold.hold_id, hold_id.to_string());
    assert_eq!(hold.target_type, "payload_cas");
    assert_eq!(hold.target_identity, "sha256:predecessor");
}

#[tokio::test]
async fn finalize_is_revision_guarded_idempotent_and_releases_holds_atomically() {
    let db = setup_db().await;
    let operation_id = Uuid::new_v4();
    let checkpoint = checkpoint(operation_id);
    TransitionCheckpointStore::save_checkpoint(&db, &checkpoint)
        .await
        .expect("checkpoint");
    RetentionHoldStore::insert_hold(
        &db,
        &RetentionHoldRecord {
            hold_id: Uuid::new_v4(),
            target: RetentionTarget::AdmittedPayloadCas {
                digest: "sha256:predecessor".to_string(),
            },
            kind: RetentionHoldKind::ActiveRolloutWindow {
                operation_id,
                expires_at: Utc::now() + Duration::minutes(5),
            },
            created_at: Utc::now(),
        },
    )
    .await
    .expect("hold");

    let idempotency_key = Uuid::new_v4();
    let command = ModuleTransitionFinalizeCommand {
        operation_id,
        expected_revision: 1,
        context: context(idempotency_key),
        actor_can_manage_modules: true,
    };
    let service = ModuleControlPlane::with_infrastructure(
        db.clone(),
        ControlPlaneInfrastructure::for_database(db.clone()),
    )
    .transitions();
    let first = service.finalize(command.clone()).await.expect("finalize");
    assert!(first.created);
    assert_eq!(first.checkpoint.revision, 2);
    assert!(matches!(
        first.checkpoint.state,
        ModuleTransitionState::Converged { .. }
    ));
    assert_eq!(first.released_holds, 1);

    let replay = service
        .finalize(command.clone())
        .await
        .expect("idempotent replay");
    assert!(!replay.created);
    assert_eq!(replay.checkpoint, first.checkpoint);

    let mut conflicting_replay = command.clone();
    conflicting_replay.expected_revision = 2;
    assert!(matches!(
        service
            .finalize(conflicting_replay)
            .await
            .expect_err("an idempotency key cannot identify a different payload"),
        ModuleTransitionServiceError::IdempotencyConflict
    ));

    let stale_command = ModuleTransitionFinalizeCommand {
        operation_id,
        expected_revision: 1,
        context: context(Uuid::new_v4()),
        actor_can_manage_modules: true,
    };
    assert!(matches!(
        service
            .finalize(stale_command)
            .await
            .expect_err("a stale revision cannot overwrite the terminal checkpoint"),
        ModuleTransitionServiceError::RevisionConflict {
            expected: 1,
            current: 2
        }
    ));
    assert!(
        RetentionHoldStore::list_active_holds(&db)
            .await
            .expect("holds")
            .is_empty()
    );
}

#[tokio::test]
async fn finalize_denies_unauthorized_commands_without_mutation() {
    let db = setup_db().await;
    let operation_id = Uuid::new_v4();
    TransitionCheckpointStore::save_checkpoint(&db, &checkpoint(operation_id))
        .await
        .expect("checkpoint");
    let command = ModuleTransitionFinalizeCommand {
        operation_id,
        expected_revision: 1,
        context: context(Uuid::new_v4()),
        actor_can_manage_modules: false,
    };
    let error = ModuleControlPlane::new(db.clone())
        .transitions()
        .finalize(command)
        .await
        .expect_err("authorization must fail");
    assert!(matches!(
        error,
        ModuleTransitionServiceError::AuthorizationDenied
    ));
    assert_eq!(
        TransitionCheckpointStore::load_checkpoint(&db, operation_id)
            .await
            .expect("load")
            .expect("checkpoint")
            .revision,
        1
    );
}

#[tokio::test]
async fn transition_queries_are_exactly_tenant_scoped() {
    let db = setup_db().await;
    let tenant_id = Uuid::new_v4();
    let mut tenant_checkpoint = checkpoint(Uuid::new_v4());
    tenant_checkpoint.tenant_id = Some(tenant_id);
    TransitionCheckpointStore::save_checkpoint(&db, &tenant_checkpoint)
        .await
        .expect("tenant checkpoint");

    let service = ModuleControlPlane::new(db).transitions();
    assert_eq!(
        service
            .active_checkpoint_for_module("checkout", Some(tenant_id))
            .await
            .expect("tenant query")
            .expect("tenant checkpoint")
            .operation_id,
        tenant_checkpoint.operation_id
    );
    assert!(
        service
            .active_checkpoint_for_module("checkout", Some(Uuid::new_v4()))
            .await
            .expect("other tenant query")
            .is_none()
    );
    assert!(
        service
            .active_checkpoint_for_module("checkout", None)
            .await
            .expect("platform query")
            .is_none()
    );
}

#[tokio::test]
async fn checkpoint_store_rejects_values_outside_the_database_contract() {
    let db = setup_db().await;
    let mut invalid = checkpoint(Uuid::new_v4());
    invalid.revision = u64::MAX;

    assert!(matches!(
        TransitionCheckpointStore::save_checkpoint(&db, &invalid)
            .await
            .expect_err("an overflowing revision must fail before persistence"),
        TransitionStoreError::CorruptData(_)
    ));
    assert!(
        TransitionCheckpointStore::load_checkpoint(&db, invalid.operation_id)
            .await
            .expect("checkpoint lookup")
            .is_none()
    );
}
