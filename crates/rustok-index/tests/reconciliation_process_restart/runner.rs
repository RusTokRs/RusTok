use std::{sync::Arc, time::Duration};

use rustok_index::{
    IndexReconciliationRunRequest, IndexSchemaSourceCatalog, IndexSourceCatalog,
    PostgresIndexReconciliationRunner, SchemaRegistry,
};
use sea_orm::DatabaseConnection;
use uuid::Uuid;

use super::{
    schema::{SOURCE_NAME, schema, schema_ref},
    source::ProcessRestartSource,
};

pub fn runner(db: DatabaseConnection) -> PostgresIndexReconciliationRunner {
    let schema = schema();
    let mut schema_catalog = IndexSchemaSourceCatalog::new();
    schema_catalog
        .register("process-restart-harness", schema.clone())
        .expect("fixture schema source must register");
    let mut source_catalog = IndexSourceCatalog::new();
    source_catalog
        .register(
            "process-restart-harness",
            SOURCE_NAME,
            [schema.reference.clone()],
            ProcessRestartSource,
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
        1,
        1,
        1,
        Duration::from_secs(60),
    )
    .expect("fixture request must be valid")
}
