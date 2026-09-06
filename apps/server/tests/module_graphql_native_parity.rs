use async_graphql::{EmptySubscription, Schema};
use rustok_core::ModuleRegistry;
use rustok_server::graphql::mutations::RootMutation;
use rustok_server::graphql::queries::RootQuery;
use sea_orm::{ConnectOptions, Database};
use uuid::Uuid;

#[tokio::test]
async fn transition_graphql_contract_matches_the_owner_command_shape() {
    let url = format!(
        "sqlite:file:module_transition_schema_{}?mode=memory&cache=shared",
        Uuid::new_v4()
    );
    let mut options = ConnectOptions::new(url);
    options
        .max_connections(1)
        .min_connections(1)
        .sqlx_logging(false);
    let db = Database::connect(options).await.expect("test database");
    let schema = Schema::build(RootQuery, RootMutation, EmptySubscription)
        .data(db)
        .data(ModuleRegistry::new())
        .finish();
    let contract = schema.sdl();

    assert!(contract.contains("moduleTransitionCheckpoint(operationId: UUID!)"));
    assert!(contract.contains("activeModuleTransitions"));
    assert!(contract.contains("moduleRetentionHolds"));
    assert!(contract.contains("revision: Int!"));
    assert!(contract.contains("finalizeModuleTransition"));
    assert!(contract.contains("expectedRevision: Int!"));
    assert!(contract.contains("idempotencyKey: UUID!"));
    assert!(!contract.contains("triggerModuleRecovery"));
}
