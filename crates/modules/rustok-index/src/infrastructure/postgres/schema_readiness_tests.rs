use rustok_core::MigrationSource;
use sea_orm::{ConnectionTrait, Database, DatabaseConnection, DbBackend, Statement};
use sea_orm_migration::SchemaManager;
use uuid::Uuid;

use super::{
    IndexSchemaReadinessError, IndexSchemaReadinessRequest, MAX_INDEX_SCHEMA_READINESS_SCHEMAS,
    PostgresIndexSchemaReadinessStore, PostgresSchemaRegistrationStore,
};
use crate::{
    EntityName, FieldCardinality, FieldName, IndexField, IndexModule, IndexSchema, IndexValueType,
    LocaleMode, ModuleName, PersistedSchemaReadinessFailure, SchemaRef, SchemaRegistry,
    SchemaVersion,
};

const TENANT: &str = "11111111-1111-1111-1111-111111111111";

async fn fixture() -> (
    DatabaseConnection,
    PostgresSchemaRegistrationStore,
    PostgresIndexSchemaReadinessStore,
) {
    let db = Database::connect("sqlite::memory:")
        .await
        .expect("in-memory sqlite should connect");
    db.execute_unprepared("PRAGMA foreign_keys = ON")
        .await
        .expect("foreign keys should be enabled");
    db.execute_unprepared("CREATE TABLE tenants (id TEXT PRIMARY KEY)")
        .await
        .expect("tenant fixture should be created");
    db.execute_unprepared(&format!("INSERT INTO tenants (id) VALUES ('{TENANT}')"))
        .await
        .expect("tenant fixture should be inserted");
    let manager = SchemaManager::new(&db);
    for migration in IndexModule.migrations() {
        migration
            .up(&manager)
            .await
            .unwrap_or_else(|error| panic!("{} should apply: {error}", migration.name()));
    }

    (
        db.clone(),
        PostgresSchemaRegistrationStore::new(db.clone()),
        PostgresIndexSchemaReadinessStore::new(db),
    )
}

fn tenant() -> Uuid {
    Uuid::parse_str(TENANT).unwrap()
}

fn schema(module: &str, entity: &str, version: u32) -> IndexSchema {
    IndexSchema {
        reference: SchemaRef {
            module: ModuleName::new(module).unwrap(),
            entity: EntityName::new(entity).unwrap(),
            version: SchemaVersion::new(version),
        },
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

fn selected_schemas() -> Vec<IndexSchema> {
    vec![
        schema("rustok-product", "product", 2),
        schema("rustok-product", "product_variant", 2),
        schema("rustok-channel", "sales_channel", 1),
    ]
}

fn registry(schemas: &[IndexSchema]) -> SchemaRegistry {
    let mut registry = SchemaRegistry::new();
    registry.register_batch(schemas.iter().cloned()).unwrap();
    registry
}

fn request(schemas: &[IndexSchema]) -> IndexSchemaReadinessRequest {
    IndexSchemaReadinessRequest::new(
        tenant(),
        schemas.iter().map(|schema| schema.reference.clone()),
    )
    .unwrap()
}

#[tokio::test]
async fn readiness_requires_the_complete_exact_tenant_schema_set() {
    let (_, registration, readiness) = fixture().await;
    let schemas = selected_schemas();
    let registry = registry(&schemas);
    for schema in &schemas {
        registration.register(tenant(), schema).await.unwrap();
    }

    let receipt = readiness
        .require(&request(&schemas), &registry)
        .await
        .unwrap();
    assert_eq!(receipt.tenant_id(), tenant());
    assert_eq!(receipt.schemas().len(), schemas.len());
    assert_eq!(
        receipt
            .schemas()
            .iter()
            .map(|entry| entry.reference.clone())
            .collect::<Vec<_>>(),
        request(&schemas).schemas().to_vec()
    );
}

#[tokio::test]
async fn readiness_reports_a_missing_exact_schema_without_partial_success() {
    let (_, registration, readiness) = fixture().await;
    let schemas = selected_schemas();
    let registry = registry(&schemas);
    for schema in &schemas[..2] {
        registration.register(tenant(), schema).await.unwrap();
    }

    let error = readiness
        .require(&request(&schemas), &registry)
        .await
        .expect_err("missing SalesChannel must block readiness");
    match error {
        IndexSchemaReadinessError::NotReady { failures } => {
            assert_eq!(failures.len(), 1);
            assert_eq!(failures[0].reference, schemas[2].reference);
            assert_eq!(failures[0].reason, PersistedSchemaReadinessFailure::Missing);
        }
        error => panic!("unexpected error: {error}"),
    }
}

#[tokio::test]
async fn readiness_rejects_inactive_or_contract_drifted_rows() {
    let (db, registration, readiness) = fixture().await;
    let schemas = selected_schemas();
    let registry = registry(&schemas);
    for schema in &schemas {
        registration.register(tenant(), schema).await.unwrap();
    }

    db.execute_raw(Statement::from_string(
        DbBackend::Sqlite,
        format!(
            "UPDATE index_schemas SET status = 'retired' WHERE tenant_id = '{TENANT}' AND module_name = 'rustok-product' AND entity_name = 'product' AND schema_version = 2"
        ),
    ))
    .await
    .unwrap();
    db.execute_raw(Statement::from_string(
        DbBackend::Sqlite,
        format!(
            "UPDATE index_schemas SET schema_fingerprint = '{}' WHERE tenant_id = '{TENANT}' AND module_name = 'rustok-channel' AND entity_name = 'sales_channel' AND schema_version = 1",
            "0".repeat(64)
        ),
    ))
    .await
    .unwrap();

    let error = readiness
        .require(&request(&schemas), &registry)
        .await
        .unwrap_err();
    match error {
        IndexSchemaReadinessError::NotReady { failures } => {
            assert_eq!(failures.len(), 2);
            assert!(failures.iter().any(|failure| {
                failure.reference == schemas[0].reference
                    && failure.reason == PersistedSchemaReadinessFailure::Inactive
            }));
            assert!(failures.iter().any(|failure| {
                failure.reference == schemas[2].reference
                    && failure.reason == PersistedSchemaReadinessFailure::FingerprintMismatch
            }));
        }
        error => panic!("unexpected error: {error}"),
    }
}

#[tokio::test]
async fn readiness_rejects_schema_json_drift_even_with_the_expected_fingerprint() {
    let (db, registration, readiness) = fixture().await;
    let schemas = selected_schemas();
    let registry = registry(&schemas);
    for schema in &schemas {
        registration.register(tenant(), schema).await.unwrap();
    }

    db.execute_raw(Statement::from_string(
        DbBackend::Sqlite,
        format!(
            "UPDATE index_schemas SET schema_json = json_set(schema_json, '$.locale_mode', 'required') WHERE tenant_id = '{TENANT}' AND module_name = 'rustok-product' AND entity_name = 'product' AND schema_version = 2"
        ),
    ))
    .await
    .unwrap();

    let error = readiness
        .require(&request(&schemas), &registry)
        .await
        .unwrap_err();
    match error {
        IndexSchemaReadinessError::NotReady { failures } => {
            assert_eq!(failures.len(), 1);
            assert_eq!(failures[0].reference, schemas[0].reference);
            assert_eq!(
                failures[0].reason,
                PersistedSchemaReadinessFailure::ContractMismatch
            );
        }
        error => panic!("unexpected error: {error}"),
    }
}

#[test]
fn readiness_request_is_bounded_and_unambiguous() {
    let reference = schema("rustok-product", "product", 2).reference;
    assert_eq!(
        IndexSchemaReadinessRequest::new(Uuid::nil(), [reference.clone()]),
        Err(IndexSchemaReadinessError::NilTenantId)
    );
    assert_eq!(
        IndexSchemaReadinessRequest::new(tenant(), Vec::<SchemaRef>::new()),
        Err(IndexSchemaReadinessError::EmptySchemaSet)
    );
    assert_eq!(
        IndexSchemaReadinessRequest::new(tenant(), [reference.clone(), reference.clone()]),
        Err(IndexSchemaReadinessError::DuplicateSchema(reference))
    );

    let too_many = (0..=MAX_INDEX_SCHEMA_READINESS_SCHEMAS)
        .map(|index| schema("readiness", &format!("entity_{index}"), 1).reference)
        .collect::<Vec<_>>();
    assert!(matches!(
        IndexSchemaReadinessRequest::new(tenant(), too_many),
        Err(IndexSchemaReadinessError::TooManySchemas { .. })
    ));
}

#[tokio::test]
async fn readiness_rejects_refs_absent_from_the_runtime_registry_before_storage() {
    let db = Database::connect("sqlite::memory:").await.unwrap();
    let schemas = selected_schemas();
    let registry = SchemaRegistry::new();
    let readiness = PostgresIndexSchemaReadinessStore::new(db);

    let error = readiness
        .require(&request(&schemas), &registry)
        .await
        .unwrap_err();
    assert_eq!(
        error,
        IndexSchemaReadinessError::SchemaNotInRegistry(schemas[2].reference.clone())
    );
}
