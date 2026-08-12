use std::{
    collections::{BTreeMap, VecDeque},
    env,
    error::Error,
    sync::{Arc, Mutex},
};

use async_trait::async_trait;
use rustok_core::{MigrationSource, ModuleRuntimeExtensions};
use rustok_index::{
    EntityKey, EntityName, FieldCardinality, FieldName, IndexDriftDependencyFailureKind,
    IndexDriftDigestRequest, IndexDriftEntityState, IndexDriftSnapshotReader, IndexField,
    IndexModule, IndexMutation, IndexRecord, IndexSchema, IndexSource, IndexSourceFailure,
    IndexSourceLoadBatch, IndexSourceLoadRequest, IndexSourcePage, IndexSourceScanRequest,
    IndexValue, IndexValueType, LocaleMode, ModuleName, MutationDelivery, PostgresMutationStore,
    PostgresSchemaRegistrationStore, SchemaRef, SchemaVersion, materialize_index_schema_registry,
    materialize_index_source_registry, materialize_postgres_index_drift_snapshot_reader,
    register_index_schema_source, register_index_source,
};
use sea_orm::{ConnectOptions, ConnectionTrait, Database, DatabaseConnection};
use sea_orm_migration::SchemaManager;
use uuid::Uuid;

const DATABASE_ENV: &str = "RUSTOK_INDEX_TEST_DATABASE_URL";
const SOURCE_NAME: &str = "snapshot-test-primary";

type TestResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

#[derive(Clone)]
struct SequencedSource {
    responses: Arc<Mutex<VecDeque<Vec<IndexMutation>>>>,
}

impl SequencedSource {
    fn new(responses: impl IntoIterator<Item = Vec<IndexMutation>>) -> Self {
        Self {
            responses: Arc::new(Mutex::new(responses.into_iter().collect())),
        }
    }
}

#[async_trait]
impl IndexSource for SequencedSource {
    async fn scan(
        &self,
        request: IndexSourceScanRequest,
    ) -> Result<IndexSourcePage, IndexSourceFailure> {
        IndexSourcePage::new(&request, Vec::new(), None)
            .map_err(|_| permanent("snapshot_test_page_invalid"))
    }

    async fn load(
        &self,
        request: IndexSourceLoadRequest,
    ) -> Result<IndexSourceLoadBatch, IndexSourceFailure> {
        let mutations = self
            .responses
            .lock()
            .map_err(|_| permanent("snapshot_test_source_poisoned"))?
            .pop_front()
            .unwrap_or_default();
        IndexSourceLoadBatch::new(&request, mutations)
            .map_err(|_| permanent("snapshot_test_batch_invalid"))
    }
}

struct TestDatabase {
    control: DatabaseConnection,
    db: DatabaseConnection,
    schema_name: String,
    tenant_id: Uuid,
}

impl TestDatabase {
    async fn setup() -> TestResult<Option<Self>> {
        let Some(database_url) = database_url() else {
            eprintln!(
                "{DATABASE_ENV} is not set to a PostgreSQL URL; skipping drift snapshot reader harness"
            );
            return Ok(None);
        };
        let control = connect(&database_url).await?;
        let schema_name = format!("rustok_index_drift_snapshot_{}", Uuid::new_v4().simple());
        control
            .execute_unprepared(&format!(r#"CREATE SCHEMA "{schema_name}""#))
            .await?;
        let db = scoped_connection(&database_url, &schema_name).await?;
        db.execute_unprepared("CREATE TABLE tenants (id UUID NOT NULL PRIMARY KEY)")
            .await?;
        let tenant_id = Uuid::new_v4();
        db.execute_unprepared(&format!("INSERT INTO tenants (id) VALUES ('{tenant_id}')"))
            .await?;
        let manager = SchemaManager::new(&db);
        for migration in IndexModule.migrations() {
            migration.up(&manager).await?;
        }
        Ok(Some(Self {
            control,
            db,
            schema_name,
            tenant_id,
        }))
    }

    async fn cleanup(self) -> TestResult<()> {
        self.control
            .execute_unprepared(&format!(
                r#"DROP SCHEMA IF EXISTS "{}" CASCADE"#,
                self.schema_name
            ))
            .await?;
        Ok(())
    }
}

#[tokio::test]
async fn source_version_fence_captures_and_rejects_unstable_postgres_snapshots() -> TestResult<()> {
    let Some(database) = TestDatabase::setup().await? else {
        return Ok(());
    };
    let schema = snapshot_schema();
    PostgresSchemaRegistrationStore::new(database.db.clone())
        .register(database.tenant_id, &schema)
        .await?;

    let stable_key = entity_key(database.tenant_id, &schema, Uuid::from_u128(11));
    let stable_materialized = upsert(&stable_key, 1, "materialized", Uuid::from_u128(101));
    let stable_source = upsert(&stable_key, 2, "source", Uuid::from_u128(102));
    let (stable_reader, stable_schemas) = reader(
        database.db.clone(),
        schema.clone(),
        SequencedSource::new([vec![stable_source.clone()], vec![stable_source.clone()]]),
    )?;
    PostgresMutationStore::new(database.db.clone())
        .apply(
            stable_schemas.registry(),
            &MutationDelivery::from_event(SOURCE_NAME, stable_materialized)?,
        )
        .await?;

    let pair = stable_reader
        .capture_entity_snapshot(&IndexDriftDigestRequest::new(stable_key.clone())?)
        .await?;
    assert!(pair.boundary().as_str().starts_with("pg:"));
    assert_eq!(pair.boundary().as_str().len(), 67);
    assert_upsert(pair.source(), &stable_key, 2, "source");
    assert_upsert(pair.materialized(), &stable_key, 1, "materialized");

    let changing_key = entity_key(database.tenant_id, &schema, Uuid::from_u128(12));
    let changing_materialized = upsert(&changing_key, 1, "materialized", Uuid::from_u128(201));
    let changing_first = upsert(&changing_key, 2, "source-before", Uuid::from_u128(202));
    let changing_second = upsert(&changing_key, 3, "source-after", Uuid::from_u128(203));
    let (changing_reader, changing_schemas) = reader(
        database.db.clone(),
        schema.clone(),
        SequencedSource::new([vec![changing_first], vec![changing_second]]),
    )?;
    PostgresMutationStore::new(database.db.clone())
        .apply(
            changing_schemas.registry(),
            &MutationDelivery::from_event(SOURCE_NAME, changing_materialized)?,
        )
        .await?;
    let changed = changing_reader
        .capture_entity_snapshot(&IndexDriftDigestRequest::new(changing_key)?)
        .await
        .expect_err("source version change must reject the pair");
    assert_eq!(changed.kind(), IndexDriftDependencyFailureKind::Retryable);
    assert_eq!(changed.code(), "index_drift_source_changed_during_capture");

    let missing_key = entity_key(database.tenant_id, &schema, Uuid::from_u128(13));
    let (missing_reader, _) = reader(
        database.db.clone(),
        schema,
        SequencedSource::new([Vec::new()]),
    )?;
    let missing = missing_reader
        .capture_entity_snapshot(&IndexDriftDigestRequest::new(missing_key)?)
        .await
        .expect_err("unwatermarked source absence must fail closed");
    assert_eq!(missing.kind(), IndexDriftDependencyFailureKind::Permanent);
    assert_eq!(missing.code(), "index_drift_source_watermark_missing");

    database.cleanup().await
}

fn reader(
    db: DatabaseConnection,
    schema: IndexSchema,
    source: SequencedSource,
) -> TestResult<(
    rustok_index::PostgresIndexDriftSnapshotReader,
    rustok_index::SharedIndexSchemaRegistry,
)> {
    let mut extensions = ModuleRuntimeExtensions::default();
    register_index_schema_source(&mut extensions, "snapshot_test", schema.clone())?;
    register_index_source(
        &mut extensions,
        "snapshot_test",
        SOURCE_NAME,
        [schema.reference.clone()],
        source,
    )?;
    let schemas = materialize_index_schema_registry(&extensions)?
        .ok_or_else(|| std::io::Error::other("snapshot schema registry is missing"))?;
    let sources = materialize_index_source_registry(&extensions)?
        .ok_or_else(|| std::io::Error::other("snapshot source registry is missing"))?;
    extensions.insert(schemas.clone());
    extensions.insert(sources);
    let reader = materialize_postgres_index_drift_snapshot_reader(&extensions, db)?
        .ok_or_else(|| std::io::Error::other("snapshot reader was not materialized"))?;
    Ok((reader, schemas))
}

fn snapshot_schema() -> IndexSchema {
    IndexSchema {
        reference: SchemaRef {
            module: ModuleName::new("snapshot-test").unwrap(),
            entity: EntityName::new("item").unwrap(),
            version: SchemaVersion::INITIAL,
        },
        locale_mode: LocaleMode::None,
        fields: vec![
            IndexField {
                name: FieldName::new("id").unwrap(),
                value_type: IndexValueType::Uuid,
                cardinality: FieldCardinality::One,
                nullable: false,
                selectable: true,
                filterable: true,
                sortable: true,
            },
            IndexField {
                name: FieldName::new("name").unwrap(),
                value_type: IndexValueType::String,
                cardinality: FieldCardinality::One,
                nullable: false,
                selectable: true,
                filterable: true,
                sortable: true,
            },
        ],
        links: Vec::new(),
    }
}

fn entity_key(tenant_id: Uuid, schema: &IndexSchema, entity_id: Uuid) -> EntityKey {
    EntityKey {
        tenant_id,
        schema: schema.reference.clone(),
        entity_id,
        locale: None,
    }
}

fn upsert(key: &EntityKey, source_version: u64, name: &str, event_id: Uuid) -> IndexMutation {
    IndexMutation::Upsert {
        event_id,
        record: IndexRecord {
            key: key.clone(),
            source_version,
            fields: BTreeMap::from([
                (
                    FieldName::new("id").unwrap(),
                    IndexValue::Uuid(key.entity_id),
                ),
                (
                    FieldName::new("name").unwrap(),
                    IndexValue::String(name.to_owned()),
                ),
            ]),
            links: Vec::new(),
        },
    }
}

fn assert_upsert(
    state: &IndexDriftEntityState,
    expected_key: &EntityKey,
    expected_version: u64,
    expected_name: &str,
) {
    let IndexDriftEntityState::Upsert { record } = state else {
        panic!("expected upsert state, got {state:?}");
    };
    assert_eq!(&record.key, expected_key);
    assert_eq!(record.source_version, expected_version);
    assert_eq!(
        record.fields.get(&FieldName::new("name").unwrap()),
        Some(&IndexValue::String(expected_name.to_owned()))
    );
}

fn permanent(code: &'static str) -> IndexSourceFailure {
    IndexSourceFailure::permanent(code).expect("static snapshot test failure code is valid")
}

fn database_url() -> Option<String> {
    env::var(DATABASE_ENV)
        .or_else(|_| env::var("DATABASE_URL"))
        .ok()
        .filter(|url| url.starts_with("postgres://") || url.starts_with("postgresql://"))
}

async fn connect(database_url: &str) -> TestResult<DatabaseConnection> {
    let mut options = ConnectOptions::new(database_url.to_owned());
    options
        .max_connections(1)
        .min_connections(1)
        .sqlx_logging(false);
    Ok(Database::connect(options).await?)
}

async fn scoped_connection(
    database_url: &str,
    schema_name: &str,
) -> TestResult<DatabaseConnection> {
    let db = connect(database_url).await?;
    db.execute_unprepared(&format!(r#"SET search_path TO "{schema_name}""#))
        .await?;
    Ok(db)
}
