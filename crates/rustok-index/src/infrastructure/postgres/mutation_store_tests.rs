use std::collections::BTreeMap;

use rustok_core::MigrationSource;
use sea_orm::{
    ConnectionTrait, Database, DatabaseConnection, DbBackend, Statement, Value as SqlValue,
};
use sea_orm_migration::SchemaManager;
use uuid::Uuid;

use super::{MutationApplyOutcome, MutationDelivery, MutationStorageError, PostgresMutationStore};
use crate::{
    EntityKey, EntityName, FieldCardinality, FieldName, IndexField, IndexLink, IndexLinkValue,
    IndexModule, IndexMutation, IndexRecord, IndexSchema, IndexValue, IndexValueType,
    LinkCardinality, LinkName, LinkedEntityKey, LocaleKey, LocaleMode, ModuleName, SchemaRef,
    SchemaRegistry, SchemaVersion,
};

const TENANT_A: &str = "11111111-1111-1111-1111-111111111111";
const TENANT_B: &str = "22222222-2222-2222-2222-222222222222";

struct Fixture {
    db: DatabaseConnection,
    registry: SchemaRegistry,
    store: PostgresMutationStore,
    product: SchemaRef,
    channel: SchemaRef,
}

impl Fixture {
    async fn new(persist_schemas: bool) -> Self {
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

        let channel = schema_ref("channel");
        let product = schema_ref("product");
        let channel_schema = IndexSchema {
            reference: channel.clone(),
            locale_mode: LocaleMode::None,
            fields: vec![field("id")],
            links: Vec::new(),
        };
        let product_schema = IndexSchema {
            reference: product.clone(),
            locale_mode: LocaleMode::Required,
            fields: vec![field("id"), field("channel_id")],
            links: vec![IndexLink {
                name: LinkName::new("channel").unwrap(),
                source_fields: vec![FieldName::new("channel_id").unwrap()],
                target_schema: channel.clone(),
                target_fields: vec![FieldName::new("id").unwrap()],
                cardinality: LinkCardinality::Many,
            }],
        };
        let mut registry = SchemaRegistry::new();
        registry
            .register_batch([product_schema.clone(), channel_schema.clone()])
            .expect("schemas should register");
        if persist_schemas {
            for tenant in [TENANT_A, TENANT_B] {
                persist_schema(&db, tenant, &product_schema).await;
                persist_schema(&db, tenant, &channel_schema).await;
            }
        }
        let store = PostgresMutationStore::new(db.clone());
        Self {
            db,
            registry,
            store,
            product,
            channel,
        }
    }

    fn record(&self, tenant: &str, entity_id: Uuid, version: u64, channel_id: Uuid) -> IndexRecord {
        IndexRecord {
            key: EntityKey {
                tenant_id: Uuid::parse_str(tenant).unwrap(),
                schema: self.product.clone(),
                entity_id,
                locale: Some(LocaleKey::new("en-US").unwrap()),
            },
            source_version: version,
            fields: BTreeMap::from([
                (FieldName::new("id").unwrap(), IndexValue::Uuid(entity_id)),
                (
                    FieldName::new("channel_id").unwrap(),
                    IndexValue::Uuid(channel_id),
                ),
            ]),
            links: vec![IndexLinkValue {
                name: LinkName::new("channel").unwrap(),
                targets: vec![LinkedEntityKey {
                    schema: self.channel.clone(),
                    entity_id: channel_id,
                    locale: None,
                }],
            }],
        }
    }
}

fn schema_ref(entity: &str) -> SchemaRef {
    SchemaRef {
        module: ModuleName::new("catalog").unwrap(),
        entity: EntityName::new(entity).unwrap(),
        version: SchemaVersion::INITIAL,
    }
}

fn field(name: &str) -> IndexField {
    IndexField {
        name: FieldName::new(name).unwrap(),
        value_type: IndexValueType::Uuid,
        cardinality: FieldCardinality::One,
        nullable: false,
        selectable: true,
        filterable: true,
        sortable: false,
    }
}

async fn persist_schema(db: &DatabaseConnection, tenant: &str, schema: &IndexSchema) {
    let fingerprint = schema.fingerprint().unwrap().to_string();
    let schema_json = serde_json::to_value(schema).unwrap();
    db.execute_raw(Statement::from_sql_and_values(
        DbBackend::Sqlite,
        "INSERT INTO index_schemas (tenant_id, module_name, entity_name, schema_version, schema_fingerprint, schema_json) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        vec![
            tenant.to_owned().into(),
            schema.reference.module.as_str().to_owned().into(),
            schema.reference.entity.as_str().to_owned().into(),
            i64::from(schema.reference.version.get()).into(),
            fingerprint.into(),
            SqlValue::Json(Some(Box::new(schema_json))),
        ],
    ))
    .await
    .expect("schema row should persist");
}

fn upsert_delivery(source: &str, delivery: &str, record: IndexRecord) -> MutationDelivery {
    MutationDelivery::new(
        source,
        delivery,
        IndexMutation::Upsert {
            event_id: Uuid::new_v4(),
            record,
        },
    )
    .unwrap()
}

async fn scalar_i64(db: &DatabaseConnection, sql: &str) -> i64 {
    db.query_one_raw(Statement::from_string(DbBackend::Sqlite, sql.to_owned()))
        .await
        .expect("scalar query should execute")
        .expect("scalar query should return one row")
        .try_get("", "value")
        .expect("scalar value should be integer")
}

#[tokio::test]
async fn atomically_upserts_entity_links_and_terminal_inbox_state() {
    let fixture = Fixture::new(true).await;
    let entity_id = Uuid::new_v4();
    let first_channel = Uuid::new_v4();
    let first = upsert_delivery(
        "catalog-source",
        "delivery-1",
        fixture.record(TENANT_A, entity_id, 1, first_channel),
    );
    assert_eq!(
        fixture
            .store
            .apply(&fixture.registry, &first)
            .await
            .unwrap(),
        MutationApplyOutcome::Applied { source_version: 1 }
    );

    let second_channel = Uuid::new_v4();
    let second = upsert_delivery(
        "catalog-source",
        "delivery-2",
        fixture.record(TENANT_A, entity_id, 2, second_channel),
    );
    assert_eq!(
        fixture
            .store
            .apply(&fixture.registry, &second)
            .await
            .unwrap(),
        MutationApplyOutcome::Applied { source_version: 2 }
    );

    assert_eq!(
        scalar_i64(
            &fixture.db,
            "SELECT COUNT(*) AS value FROM index_entities WHERE is_deleted = FALSE"
        )
        .await,
        1
    );
    assert_eq!(
        scalar_i64(&fixture.db, "SELECT COUNT(*) AS value FROM index_links").await,
        1
    );
    let target: String = fixture
        .db
        .query_one_raw(Statement::from_string(
            DbBackend::Sqlite,
            "SELECT target_entity_id FROM index_links".to_owned(),
        ))
        .await
        .unwrap()
        .unwrap()
        .try_get("", "target_entity_id")
        .unwrap();
    assert_eq!(target, second_channel.to_string());
    assert_eq!(
        scalar_i64(
            &fixture.db,
            "SELECT COUNT(*) AS value FROM index_inbox WHERE state = 'applied' AND completed_at IS NOT NULL"
        )
        .await,
        2
    );
}

#[tokio::test]
async fn exact_redelivery_is_duplicate_but_payload_reuse_conflicts() {
    let fixture = Fixture::new(true).await;
    let entity_id = Uuid::new_v4();
    let record = fixture.record(TENANT_A, entity_id, 1, Uuid::new_v4());
    let event_id = Uuid::new_v4();
    let delivery = MutationDelivery::new(
        "catalog-source",
        "stable-delivery",
        IndexMutation::Upsert {
            event_id,
            record: record.clone(),
        },
    )
    .unwrap();
    fixture
        .store
        .apply(&fixture.registry, &delivery)
        .await
        .unwrap();
    assert_eq!(
        fixture
            .store
            .apply(&fixture.registry, &delivery)
            .await
            .unwrap(),
        MutationApplyOutcome::Duplicate { source_version: 1 }
    );

    let conflict = MutationDelivery::new(
        "catalog-source",
        "stable-delivery",
        IndexMutation::Upsert {
            event_id,
            record: fixture.record(TENANT_A, entity_id, 2, Uuid::new_v4()),
        },
    )
    .unwrap();
    assert_eq!(
        fixture.store.apply(&fixture.registry, &conflict).await,
        Err(MutationStorageError::DeliveryConflict)
    );
}

#[tokio::test]
async fn tombstone_and_source_version_guards_prevent_stale_resurrection() {
    let fixture = Fixture::new(true).await;
    let entity_id = Uuid::new_v4();
    let record = fixture.record(TENANT_A, entity_id, 3, Uuid::new_v4());
    fixture
        .store
        .apply(
            &fixture.registry,
            &upsert_delivery("catalog-source", "upsert-3", record.clone()),
        )
        .await
        .unwrap();

    let stale_delete = MutationDelivery::new(
        "catalog-source",
        "delete-2",
        IndexMutation::Delete {
            event_id: Uuid::new_v4(),
            key: record.key.clone(),
            source_version: 2,
        },
    )
    .unwrap();
    assert_eq!(
        fixture
            .store
            .apply(&fixture.registry, &stale_delete)
            .await
            .unwrap(),
        MutationApplyOutcome::StaleIgnored {
            incoming_source_version: 2,
            current_source_version: 3,
        }
    );

    let delete = MutationDelivery::new(
        "catalog-source",
        "delete-4",
        IndexMutation::Delete {
            event_id: Uuid::new_v4(),
            key: record.key.clone(),
            source_version: 4,
        },
    )
    .unwrap();
    fixture
        .store
        .apply(&fixture.registry, &delete)
        .await
        .unwrap();
    let resurrection = upsert_delivery(
        "catalog-source",
        "upsert-3-replayed",
        fixture.record(TENANT_A, entity_id, 3, Uuid::new_v4()),
    );
    assert_eq!(
        fixture
            .store
            .apply(&fixture.registry, &resurrection)
            .await
            .unwrap(),
        MutationApplyOutcome::StaleIgnored {
            incoming_source_version: 3,
            current_source_version: 4,
        }
    );
    assert_eq!(
        scalar_i64(
            &fixture.db,
            "SELECT COUNT(*) AS value FROM index_entities WHERE is_deleted = TRUE AND payload IS NULL"
        )
        .await,
        1
    );
    assert_eq!(
        scalar_i64(&fixture.db, "SELECT COUNT(*) AS value FROM index_links").await,
        0
    );
}

#[tokio::test]
async fn failed_entity_write_rolls_back_the_inbox_claim() {
    let fixture = Fixture::new(false).await;
    let delivery = upsert_delivery(
        "catalog-source",
        "missing-schema",
        fixture.record(TENANT_A, Uuid::new_v4(), 1, Uuid::new_v4()),
    );
    assert!(matches!(
        fixture.store.apply(&fixture.registry, &delivery).await,
        Err(MutationStorageError::Storage(_))
    ));
    assert_eq!(
        scalar_i64(&fixture.db, "SELECT COUNT(*) AS value FROM index_inbox").await,
        0
    );
    assert_eq!(
        scalar_i64(&fixture.db, "SELECT COUNT(*) AS value FROM index_entities").await,
        0
    );
}

#[tokio::test]
async fn tenant_and_locale_identity_do_not_collide() {
    let fixture = Fixture::new(true).await;
    let entity_id = Uuid::new_v4();
    let first = upsert_delivery(
        "catalog-source",
        "tenant-a",
        fixture.record(TENANT_A, entity_id, 1, Uuid::new_v4()),
    );
    let second = upsert_delivery(
        "catalog-source",
        "tenant-b",
        fixture.record(TENANT_B, entity_id, 1, Uuid::new_v4()),
    );
    let mut localized_record = fixture.record(TENANT_A, entity_id, 1, Uuid::new_v4());
    localized_record.key.locale = Some(LocaleKey::new("fr-FR").unwrap());
    let third = upsert_delivery("catalog-source", "locale-fr", localized_record);
    fixture
        .store
        .apply(&fixture.registry, &first)
        .await
        .unwrap();
    fixture
        .store
        .apply(&fixture.registry, &second)
        .await
        .unwrap();
    fixture
        .store
        .apply(&fixture.registry, &third)
        .await
        .unwrap();
    assert_eq!(
        scalar_i64(&fixture.db, "SELECT COUNT(*) AS value FROM index_entities").await,
        3
    );
}
