use std::sync::Arc;

use rustok_index::{
    EntityName, FieldCardinality, FieldName, IndexField, IndexSchema, IndexSchemaSourceCatalog,
    IndexSourceCatalog, IndexValueType, LocaleMode, ModuleName, PostgresIndexReconciliationRunner,
    SchemaRef, SchemaRegistry, SchemaVersion,
};
use sea_orm::DatabaseConnection;

use super::source::BlockingSource;

pub fn schema_ref() -> SchemaRef {
    SchemaRef {
        module: ModuleName::new("running-cancel-harness").unwrap(),
        entity: EntityName::new("item").unwrap(),
        version: SchemaVersion::INITIAL,
    }
}

pub fn schema() -> IndexSchema {
    IndexSchema {
        reference: schema_ref(),
        locale_mode: LocaleMode::None,
        fields: vec![IndexField {
            name: FieldName::new("id").unwrap(),
            value_type: IndexValueType::Uuid,
            cardinality: FieldCardinality::One,
            nullable: false,
            selectable: true,
            filterable: true,
            sortable: true,
        }],
        links: Vec::new(),
    }
}

pub fn runner(
    db: DatabaseConnection,
    source: BlockingSource,
) -> PostgresIndexReconciliationRunner {
    let schema = schema();
    let mut schema_catalog = IndexSchemaSourceCatalog::new();
    schema_catalog
        .register("running-cancel-harness", schema.clone())
        .expect("fixture schema source must register");
    let mut source_catalog = IndexSourceCatalog::new();
    source_catalog
        .register(
            "running-cancel-harness",
            "running-cancel-harness-primary",
            [schema.reference.clone()],
            source,
        )
        .expect("fixture source must register");
    let sources = source_catalog
        .materialize(&schema_catalog)
        .expect("fixture source registry must materialize");
    let mut registry = SchemaRegistry::new();
    registry.register(schema).expect("fixture schema must register");
    PostgresIndexReconciliationRunner::new(db, sources, Arc::new(registry))
}
