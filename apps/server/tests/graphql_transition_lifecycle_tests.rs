use async_graphql::{EmptySubscription, Request, Schema, Variables};
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
    options.max_connections(1).min_connections(1).sqlx_logging(false);
    let db = Database::connect(options).await.expect("sqlite connection");
    let manager = SchemaManager::new(&db);
    for migration in rustok_modules::migrations::migrations() {
        migration.up(&manager).await.expect("module migration");
    }
    db
}

#[tokio::test]
async fn transition_transport_requires_authentication_and_exposes_only_real_commands() {
    let db = setup_db().await;
    let schema = Schema::build(RootQuery, RootMutation, EmptySubscription)
        .data(db)
        .finish();
    let operation_id = Uuid::new_v4();
    let query = Request::new(
        r#"query Checkpoint($operationId: UUID!) {
            moduleTransitionCheckpoint(operationId: $operationId) { operationId revision }
        }"#,
    )
    .variables(Variables::from_json(serde_json::json!({
        "operationId": operation_id,
    })));
    assert!(!schema.execute(query).await.errors.is_empty());

    let mutation = Request::new(
        r#"mutation Finalize($operationId: UUID!, $expectedRevision: Int!, $idempotencyKey: UUID!) {
            finalizeModuleTransition(
                operationId: $operationId,
                expectedRevision: $expectedRevision,
                idempotencyKey: $idempotencyKey
            ) { operationId revision state }
        }"#,
    )
    .variables(Variables::from_json(serde_json::json!({
        "operationId": operation_id,
        "expectedRevision": 1,
        "idempotencyKey": Uuid::new_v4(),
    })));
    assert!(!schema.execute(mutation).await.errors.is_empty());

    let contract = schema.sdl();
    assert!(contract.contains("finalizeModuleTransition"));
    assert!(contract.contains("expectedRevision: Int!"));
    assert!(contract.contains("idempotencyKey: UUID!"));
    assert!(!contract.contains("triggerModuleRecovery"));
}

#[tokio::test]
async fn retention_hold_transport_requires_authentication() {
    let db = setup_db().await;
    let schema = Schema::build(RootQuery, RootMutation, EmptySubscription)
        .data(db)
        .finish();
    let response = schema
        .execute(Request::new(
            "query { moduleRetentionHolds { holdId targetType targetIdentity } }",
        ))
        .await;
    assert!(!response.errors.is_empty());
}
