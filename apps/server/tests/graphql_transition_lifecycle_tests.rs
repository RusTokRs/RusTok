use async_graphql::{EmptySubscription, Request, Schema, Variables};
use chrono::{Duration, Utc};
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

async fn setup_db() -> DatabaseConnection {
    let url = format!(
        "sqlite:file:graphql_transition_test_{}?mode=memory&cache=shared",
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
async fn test_graphql_transition_lifecycle_queries_and_mutations() {
    let db = setup_db().await;
    let operation_id = Uuid::new_v4();

    // 1. Seed active transition checkpoint in observing state
    let initial_checkpoint = ModuleTransitionCheckpoint {
        operation_id,
        revision: 1,
        module_slug: "orders".to_string(),
        tenant_id: None,
        predecessor_digest: Some("sha256:v1".to_string()),
        candidate_digest: "sha256:v2".to_string(),
        state: ModuleTransitionState::Observing {
            timeout_at: Utc::now() + Duration::minutes(10),
        },
        security_epoch: GlobalSecurityEpoch(10),
        fences: ConflictFenceSet::new(vec![]),
        recovery_attempt_count: 0,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };
    TransitionCheckpointStore::save_checkpoint(&db, &initial_checkpoint)
        .await
        .expect("save_checkpoint should succeed");

    // 2. Build GraphQL schema
    let schema = Schema::build(RootQuery, RootMutation, EmptySubscription)
        .data(db.clone())
        .finish();

    // 3. Query moduleTransitionCheckpoint
    let query = r#"
        query GetCheckpoint($opId: UUID!) {
            moduleTransitionCheckpoint(operationId: $opId) {
                operationId
                moduleSlug
                state
                recoveryAttemptCount
            }
        }
    "#;
    let request = Request::new(query).variables(Variables::from_json(serde_json::json!({
        "opId": operation_id.to_string(),
    })));
    let response = schema.execute(request).await;
    assert!(
        response.errors.is_empty(),
        "query errors: {:?}",
        response.errors
    );

    let data = response.data.into_json().expect("valid JSON data");
    assert_eq!(data["moduleTransitionCheckpoint"]["moduleSlug"], "orders");
    assert_eq!(data["moduleTransitionCheckpoint"]["state"], "OBSERVING");
    assert_eq!(
        data["moduleTransitionCheckpoint"]["recoveryAttemptCount"],
        0
    );

    // 4. Trigger recovery mutation (First attempt)
    let mutation = r#"
        mutation Recover($opId: UUID!, $reason: String!) {
            triggerModuleRecovery(operationId: $opId, reason: $reason) {
                operationId
                state
                recoveryAttemptCount
            }
        }
    "#;
    let request = Request::new(mutation).variables(Variables::from_json(serde_json::json!({
        "opId": operation_id.to_string(),
        "reason": "Watchdog: Candidate memory leak detected",
    })));
    let response = schema.execute(request).await;
    assert!(
        response.errors.is_empty(),
        "mutation errors: {:?}",
        response.errors
    );

    let data = response.data.into_json().expect("valid JSON data");
    assert_eq!(
        data["triggerModuleRecovery"]["state"],
        "RECOVERED_TO_PREDECESSOR"
    );
    assert_eq!(data["triggerModuleRecovery"]["recoveryAttemptCount"], 1);

    // 5. Trigger recovery mutation a second time -> MUST be rejected (Zero-Flapping Invariant)
    let request_second =
        Request::new(mutation).variables(Variables::from_json(serde_json::json!({
            "opId": operation_id.to_string(),
            "reason": "Second failure signal",
        })));
    let response_second = schema.execute(request_second).await;
    assert!(
        !response_second.errors.is_empty(),
        "second recovery must return GraphQL error"
    );
}

#[tokio::test]
async fn test_graphql_retention_holds_query() {
    let db = setup_db().await;
    let hold_id = Uuid::new_v4();

    // Seed a retention hold
    let record = RetentionHoldRecord {
        hold_id,
        target: RetentionTarget::SourceCasBlob {
            digest: "sha256:source_archive_held".to_string(),
        },
        kind: RetentionHoldKind::AuditTrail {
            compliance_id: "AUDIT-2026-X".to_string(),
        },
        created_at: Utc::now(),
    };
    RetentionHoldStore::insert_hold(&db, &record)
        .await
        .expect("insert_hold should succeed");

    let schema = Schema::build(RootQuery, RootMutation, EmptySubscription)
        .data(db)
        .finish();

    let query = r#"
        query {
            moduleRetentionHolds {
                holdId
                targetType
                targetIdentity
            }
        }
    "#;
    let response = schema.execute(Request::new(query)).await;
    assert!(
        response.errors.is_empty(),
        "query errors: {:?}",
        response.errors
    );

    let data = response.data.into_json().expect("valid JSON data");
    let holds = data["moduleRetentionHolds"]
        .as_array()
        .expect("array of holds");
    assert_eq!(holds.len(), 1);
    assert_eq!(holds[0]["holdId"], hold_id.to_string());
    assert_eq!(holds[0]["targetType"], "source_cas");
    assert_eq!(holds[0]["targetIdentity"], "sha256:source_archive_held");
}
