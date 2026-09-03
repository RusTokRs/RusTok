use async_graphql::{EmptySubscription, Request, Schema, Variables};
use chrono::{Duration, Utc};
use rustok_core::ModuleRegistry;
use rustok_modules::{
    ConflictFenceSet, GlobalSecurityEpoch, ModuleTransitionCheckpoint, ModuleTransitionState,
    RetentionHoldKind, RetentionHoldRecord, RetentionHoldStore, RetentionTarget,
    TransitionCheckpointStore,
};
use rustok_server::graphql::mutations::RootMutation;
use rustok_server::graphql::queries::RootQuery;
use sea_orm::{ConnectOptions, Database, DatabaseConnection};
use sea_orm_migration::SchemaManager;
use uuid::Uuid;

async fn setup_test_db() -> DatabaseConnection {
    let url = format!(
        "sqlite:file:parity_test_{}?mode=memory&cache=shared",
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
async fn test_module_graphql_schema_contract_parity() {
    let db = setup_test_db().await;
    let schema = Schema::build(RootQuery, RootMutation, EmptySubscription)
        .data(db.clone())
        .data(ModuleRegistry::new())
        .finish();

    // 1. Verify moduleTransitionCheckpoint query structure
    let op_id = Uuid::new_v4();
    let checkpoint = ModuleTransitionCheckpoint {
        operation_id: op_id,
        revision: 1,
        module_slug: "commerce".to_string(),
        tenant_id: None,
        predecessor_digest: Some("sha256:predecessor".to_string()),
        candidate_digest: "sha256:candidate".to_string(),
        state: ModuleTransitionState::Observing {
            timeout_at: Utc::now() + Duration::minutes(5),
        },
        security_epoch: GlobalSecurityEpoch(1),
        fences: ConflictFenceSet::new(vec![]),
        recovery_attempt_count: 0,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };
    TransitionCheckpointStore::save_checkpoint(&db, &checkpoint)
        .await
        .expect("save checkpoint");

    let query = r#"
        query CheckpointParity($opId: UUID!) {
            moduleTransitionCheckpoint(operationId: $opId) {
                operationId
                moduleSlug
                predecessorDigest
                candidateDigest
                state
                securityEpoch
                recoveryAttemptCount
            }
        }
    "#;
    let request = Request::new(query).variables(Variables::from_json(serde_json::json!({
        "opId": op_id.to_string()
    })));
    let response = schema.execute(request).await;
    assert!(response.errors.is_empty(), "GraphQL errors: {:?}", response.errors);

    let data = response.data.into_json().expect("valid JSON");
    let cp = &data["moduleTransitionCheckpoint"];
    assert_eq!(cp["operationId"], op_id.to_string());
    assert_eq!(cp["moduleSlug"], "commerce");
    assert_eq!(cp["predecessorDigest"], "sha256:predecessor");
    assert_eq!(cp["candidateDigest"], "sha256:candidate");
    assert_eq!(cp["state"], "OBSERVING");
    assert_eq!(cp["securityEpoch"], 1);
    assert_eq!(cp["recoveryAttemptCount"], 0);

    // 2. Verify triggerModuleRecovery mutation
    let mutation = r#"
        mutation TriggerRecovery($opId: UUID!, $reason: String!) {
            triggerModuleRecovery(operationId: $opId, reason: $reason) {
                operationId
                state
                recoveryAttemptCount
            }
        }
    "#;
    let request = Request::new(mutation).variables(Variables::from_json(serde_json::json!({
        "opId": op_id.to_string(),
        "reason": "Synthetic regression detected in canary"
    })));
    let response = schema.execute(request).await;
    assert!(response.errors.is_empty(), "GraphQL errors: {:?}", response.errors);

    let data = response.data.into_json().expect("valid JSON");
    let recovered = &data["triggerModuleRecovery"];
    assert_eq!(recovered["state"], "RECOVERED_TO_PREDECESSOR");
    assert_eq!(recovered["recoveryAttemptCount"], 1);

    // 3. Verify finalizeModuleTransition mutation
    let finalize_mutation = r#"
        mutation FinalizeTransition($opId: UUID!) {
            finalizeModuleTransition(operationId: $opId) {
                operationId
                state
            }
        }
    "#;
    let request = Request::new(finalize_mutation).variables(Variables::from_json(serde_json::json!({
        "opId": op_id.to_string()
    })));
    let response = schema.execute(request).await;
    assert!(response.errors.is_empty(), "GraphQL errors: {:?}", response.errors);

    let data = response.data.into_json().expect("valid JSON");
    assert_eq!(data["finalizeModuleTransition"]["state"], "CONVERGED");
}

#[tokio::test]
async fn test_retention_holds_parity() {
    let db = setup_test_db().await;
    let hold_id = Uuid::new_v4();

    let record = RetentionHoldRecord {
        hold_id,
        target: RetentionTarget::SourceCasBlob {
            digest: "sha256:test_hold_digest".to_string(),
        },
        kind: RetentionHoldKind::ActiveRolloutWindow {
            operation_id: Uuid::new_v4(),
            expires_at: Utc::now() + Duration::hours(1),
        },
        created_at: Utc::now(),
    };
    RetentionHoldStore::insert_hold(&db, &record)
        .await
        .expect("insert hold");

    let schema = Schema::build(RootQuery, RootMutation, EmptySubscription)
        .data(db)
        .finish();

    let query = r#"
        query {
            moduleRetentionHolds {
                holdId
                targetType
                targetIdentity
                kind
            }
        }
    "#;
    let response = schema.execute(Request::new(query)).await;
    assert!(response.errors.is_empty(), "GraphQL errors: {:?}", response.errors);

    let data = response.data.into_json().expect("valid JSON");
    let holds = data["moduleRetentionHolds"].as_array().expect("array");
    assert_eq!(holds.len(), 1);
    assert_eq!(holds[0]["holdId"], hold_id.to_string());
    assert_eq!(holds[0]["targetType"], "source_cas");
    assert_eq!(holds[0]["targetIdentity"], "sha256:test_hold_digest");
}

#[tokio::test]
async fn test_active_module_transitions_query() {
    let db = setup_test_db().await;
    let schema = Schema::build(RootQuery, RootMutation, EmptySubscription)
        .data(db.clone())
        .data(ModuleRegistry::new())
        .finish();

    // 1. Seed active observing checkpoint
    let active_op_id = Uuid::new_v4();
    let active_cp = ModuleTransitionCheckpoint {
        operation_id: active_op_id,
        revision: 1,
        module_slug: "analytics".to_string(),
        tenant_id: None,
        predecessor_digest: None,
        candidate_digest: "sha256:analytics_v1".to_string(),
        state: ModuleTransitionState::Observing {
            timeout_at: Utc::now() + Duration::minutes(15),
        },
        security_epoch: GlobalSecurityEpoch(1),
        fences: ConflictFenceSet::new(vec![]),
        recovery_attempt_count: 0,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };
    TransitionCheckpointStore::save_checkpoint(&db, &active_cp)
        .await
        .expect("save active checkpoint");

    // 2. Seed terminal converged checkpoint (MUST NOT be in active transitions)
    let terminal_op_id = Uuid::new_v4();
    let terminal_cp = ModuleTransitionCheckpoint {
        operation_id: terminal_op_id,
        revision: 2,
        module_slug: "search".to_string(),
        tenant_id: None,
        predecessor_digest: None,
        candidate_digest: "sha256:search_v1".to_string(),
        state: ModuleTransitionState::Converged {
            finalized_at: Utc::now(),
        },
        security_epoch: GlobalSecurityEpoch(1),
        fences: ConflictFenceSet::new(vec![]),
        recovery_attempt_count: 0,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };
    TransitionCheckpointStore::save_checkpoint(&db, &terminal_cp)
        .await
        .expect("save terminal checkpoint");

    let query = r#"
        query {
            activeModuleTransitions {
                operationId
                moduleSlug
                state
            }
        }
    "#;
    let response = schema.execute(Request::new(query)).await;
    assert!(response.errors.is_empty(), "GraphQL errors: {:?}", response.errors);

    let data = response.data.into_json().expect("valid JSON");
    let active_list = data["activeModuleTransitions"].as_array().expect("array");
    assert_eq!(active_list.len(), 1);
    assert_eq!(active_list[0]["operationId"], active_op_id.to_string());
    assert_eq!(active_list[0]["moduleSlug"], "analytics");
    assert_eq!(active_list[0]["state"], "OBSERVING");
}
