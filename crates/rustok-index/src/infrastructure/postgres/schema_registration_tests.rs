use rustok_core::MigrationSource;
use sea_orm::{ConnectionTrait, Database, DatabaseConnection, DbBackend, Statement};
use sea_orm_migration::SchemaManager;
use uuid::Uuid;

use super::{
    PersistedSchemaRegistrationOutcome, PostgresSchemaRegistrationStore, SchemaRegistrationError,
};
use crate::{
    EntityName, FieldCardinality, FieldName, IndexField, IndexModule, IndexSchema, IndexValueType,
    LocaleMode, ModuleName, SchemaRef, SchemaVersion,
};

const TENANT_A: &str = "11111111-1111-1111-1111-111111111111";
const TENANT_B: &str = "22222222-2222-2222-2222-222222222222";

async fn fixture() -> (DatabaseConnection, PostgresSchemaRegistrationStore) {
    let db = Database::connect("sqlite::memory:")
        .await
        .expect("in-memory sqlite should connect");
    db.execute_unprepared("PRAGMA foreign_keys = ON")
        .await
        .expect("foreign keys should be enabled");
    db.execute_unprepared("CREATE TABLE tenants (id TEXT PRIMARY KEY)")
        .await
        .expect("tenant fixture should be created");
    db.execute_unprepared(&format!(
        "INSERT INTO tenants (id) VALUES ('{TENANT_A}'), ('{TENANT_B}')"
    ))
    .await
    .expect("tenant fixtures should be inserted");
    let manager = SchemaManager::new(&db);
    for migration in IndexModule.migrations() {
        migration
            .up(&manager)
            .await
            .unwrap_or_else(|error| panic!("{} should apply: {error}", migration.name()));
    }
    let store = PostgresSchemaRegistrationStore::new(db.clone());
    (db, store)
}

fn schema(version: u32) -> IndexSchema {
    IndexSchema {
        reference: SchemaRef {
            module: ModuleName::new("social-graph").unwrap(),
            entity: EntityName::new("relation").unwrap(),
            version: SchemaVersion::new(version),
        },
        locale_mode: LocaleMode::None,
        fields: vec![IndexField {
            name: FieldName::new("source_user_id").unwrap(),
            value_type: IndexValueType::Uuid,
            cardinality: FieldCardinality::One,
            nullable: false,
            selectable: true,
            filterable: true,
            sortable: false,
        }],
        links: Vec::new(),
    }
}

fn tenant(value: &str) -> Uuid {
    Uuid::parse_str(value).unwrap()
}

async fn schema_count(db: &DatabaseConnection) -> i64 {
    db.query_one(Statement::from_string(
        DbBackend::Sqlite,
        "SELECT COUNT(*) AS value FROM index_schemas".to_owned(),
    ))
    .await
    .unwrap()
    .unwrap()
    .try_get("", "value")
    .unwrap()
}

#[tokio::test]
async fn registration_is_tenant_scoped_and_exactly_idempotent() {
    let (db, store) = fixture().await;
    let contract = schema(1);
    assert!(matches!(
        store.register(tenant(TENANT_A), &contract).await.unwrap(),
        PersistedSchemaRegistrationOutcome::Inserted { .. }
    ));
    assert!(matches!(
        store.register(tenant(TENANT_A), &contract).await.unwrap(),
        PersistedSchemaRegistrationOutcome::Unchanged { .. }
    ));
    assert!(matches!(
        store.register(tenant(TENANT_B), &contract).await.unwrap(),
        PersistedSchemaRegistrationOutcome::Inserted { .. }
    ));
    assert_eq!(schema_count(&db).await, 2);
}

#[tokio::test]
async fn same_version_contract_reuse_fails_closed() {
    let (_, store) = fixture().await;
    let first = schema(1);
    store.register(tenant(TENANT_A), &first).await.unwrap();
    let mut changed = first.clone();
    changed.fields[0].filterable = false;
    assert!(matches!(
        store.register(tenant(TENANT_A), &changed).await,
        Err(SchemaRegistrationError::VersionConflict { .. })
    ));
}

#[tokio::test]
async fn unregistered_lower_version_is_rejected_after_newer_version() {
    let (_, store) = fixture().await;
    store
        .register(tenant(TENANT_A), &schema(2))
        .await
        .unwrap();
    assert!(matches!(
        store.register(tenant(TENANT_A), &schema(1)).await,
        Err(SchemaRegistrationError::NonMonotonicVersion { .. })
    ));
}

#[tokio::test]
async fn retired_schema_cannot_be_reactivated_by_registration() {
    let (db, store) = fixture().await;
    let contract = schema(1);
    store.register(tenant(TENANT_A), &contract).await.unwrap();
    db.execute(Statement::from_string(
        DbBackend::Sqlite,
        format!(
            "UPDATE index_schemas SET status = 'retired' WHERE tenant_id = '{TENANT_A}'"
        ),
    ))
    .await
    .unwrap();
    assert!(matches!(
        store.register(tenant(TENANT_A), &contract).await,
        Err(SchemaRegistrationError::SchemaRetired(_))
    ));
}

#[tokio::test]
async fn nil_tenant_fails_before_storage() {
    let (_, store) = fixture().await;
    assert_eq!(
        store.register(Uuid::nil(), &schema(1)).await,
        Err(SchemaRegistrationError::NilTenantId)
    );
}
