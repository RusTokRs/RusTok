use std::{
    sync::{
        Arc,
        atomic::AtomicUsize,
    },
    time::Duration,
};

use rustok_index::{
    IndexReconciliationRunRequest, IndexSchemaSourceCatalog, IndexSourceCatalog,
    PostgresIndexReconciliationRunner, SchemaRegistry,
};
use sea_orm::DatabaseConnection;
use uuid::Uuid;

use super::{
    schema::{schema, schema_ref},
    source::CountedSource,
};

pub const SOURCE_NAME: &str = "stored-job-admission-primary";

pub fn runner(
    db: DatabaseConnection,
    calls: Arc<AtomicUsize>,
) -> PostgresIndexReconciliationRunner {
    let schema = schema();
    let mut schema_catalog = IndexSchemaSourceCatalog::new();
    schema_catalog
        .register("stored-job-admission-harness", schema.clone())
        .expect("fixture schema source must register");

    let mut source_catalog = IndexSourceCatalog::new();
    source_catalog
        .register(
            "stored-job-admission-harness",
            SOURCE_NAME,
            [schema.reference.clone()],
            CountedSource::new(calls),
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

pub fn request(
    tenant_id: Uuid,
    worker_id: &str,
    max_pages: usize,
) -> IndexReconciliationRunRequest {
    IndexReconciliationRunRequest::new(
        tenant_id,
        schema_ref(),
        worker_id,
        1,
        max_pages,
        1,
        1,
        Duration::from_secs(3_600),
    )
    .expect("fixture reconciliation request must be valid")
}
