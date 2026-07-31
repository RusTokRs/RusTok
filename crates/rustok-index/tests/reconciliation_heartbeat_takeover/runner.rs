use std::{sync::Arc, time::Duration};

use rustok_index::{
    IndexReconciliationRunRequest, IndexSchemaSourceCatalog, IndexSourceCatalog,
    PostgresIndexReconciliationRunner, SchemaRegistry,
};
use sea_orm::DatabaseConnection;
use uuid::Uuid;

use super::{schema::{schema, schema_ref}, source::HeartbeatSource};

pub const SOURCE_NAME: &str = "heartbeat-takeover-primary";

pub fn runner(
    db: DatabaseConnection,
    source: HeartbeatSource,
) -> PostgresIndexReconciliationRunner {
    let schema = schema();
    let mut schema_catalog = IndexSchemaSourceCatalog::new();
    schema_catalog
        .register("heartbeat-takeover-harness", schema.clone())
        .expect("fixture schema source must register");

    let mut source_catalog = IndexSourceCatalog::new();
    source_catalog
        .register(
            "heartbeat-takeover-harness",
            SOURCE_NAME,
            [schema.reference.clone()],
            source,
        )
        .expect("fixture source must register");
    let sources = source_catalog
        .materialize(&schema_catalog)
        .expect("fixture source registry must materialize");

    let mut registry = SchemaRegistry::new();
    registry
        .register(schema)
        .expect("fixture schema registry must materialize");

    PostgresIndexReconciliationRunner::new(db, sources, Arc::new(registry))
}

pub fn request(tenant_id: Uuid, worker_id: &str) -> IndexReconciliationRunRequest {
    IndexReconciliationRunRequest::new(
        tenant_id,
        schema_ref(),
        worker_id,
        1,
        2,
        1,
        1,
        Duration::from_secs(3_600),
    )
    .expect("fixture reconciliation request must be valid")
}
