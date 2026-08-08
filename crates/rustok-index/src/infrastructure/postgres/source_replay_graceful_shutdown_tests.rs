use std::{
    collections::BTreeMap,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

use async_trait::async_trait;
use rustok_core::MigrationSource;
use sea_orm::{
    ConnectionTrait, Database, DatabaseConnection, DbBackend, Statement, Value as SqlValue,
};
use sea_orm_migration::SchemaManager;
use uuid::Uuid;

use super::{IndexReplayRunRequest, IndexReplayRunStatus, PostgresIndexReplayRunner};
use crate::{
    EntityKey, EntityName, FieldCardinality, FieldName, IndexField, IndexModule, IndexMutation,
    IndexRecord, IndexSchema, IndexSchemaSourceCatalog, IndexSource, IndexSourceCatalog,
    IndexSourceFailure, IndexSourceLoadBatch, IndexSourceLoadRequest, IndexSourcePage,
    IndexSourceScanRequest, IndexValue, IndexValueType, LocaleMode, ModuleName, SchemaRef,
    SchemaRegistry, SchemaVersion,
};

const TENANT: &str = "11111111-1111-1111-1111-111111111111";
const ENTITY_ID: Uuid = Uuid::from_u128(101);
const EVENT_ID: Uuid = Uuid::from_u128(1001);

struct StableSinglePageSource {
    calls: Arc<AtomicUsize>,
}

#[async_trait]
impl IndexSource for StableSinglePageSource {
    async fn scan(
        &self,
        request: IndexSourceScanRequest,
    ) -> Result<IndexSourcePage, IndexSourceFailure> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        let mutation = IndexMutation::Upsert {
            event_id: EVENT_ID,
            record: IndexRecord {
                key: EntityKey {
                    tenant_id: request.tenant_id(),
                    schema: schema_ref(),
                    entity_id: ENTITY_ID,
                    locale: None,
                },
                source_version: 1,
                fields: BTreeMap::from([(
                    FieldName::new("id").expect("field name"),
                    IndexValue::Uuid(ENTITY_ID),
                )]),
                links: Vec::new(),
            },
        };
        IndexSourcePage::new(&request, vec![mutation], None)
    }

    async fn load(
        &self,
        request: IndexSourceLoadRequest,
    ) -> Result<IndexSourceLoadBatch, IndexSourceFailure> {
        IndexSourceLoadBatch::new(&request, Vec::new())
    }
}

struct Fixture {
    db: DatabaseConnection,
    runner: PostgresIndexReplayRunner,
    source_calls: Arc<AtomicUsize>,
}

impl Fixture {
    async fn new() -> Self {
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

        let schema = schema();
        let fingerprint = schema.fingerprint().expect("schema fingerprint").to_string();
        let schema_json = serde_json::to_value(&schema).expect("schema json");
        db.execute(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "INSERT INTO index_schemas (tenant_id, module_name, entity_name, schema_version, schema_fingerprint, schema_json, status) VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'active')",
            vec![
                TENANT.to_owned().into(),
                schema.reference.module.as_str().to_owned().into(),
                schema.reference.entity.as_str().to_owned().into(),
                i64::from(schema.reference.version.get()).into(),
                fingerprint.into(),
                SqlValue::Json(Some(Box::new(schema_json))),
            ],
        ))
        .await
        .expect("schema fixture should persist");

        let mut schema_catalog = IndexSchemaSourceCatalog::new();
        schema_catalog
            .register("shutdown-owner", schema.clone())
            .expect("schema source registration");
        let source_calls = Arc::new(AtomicUsize::new(0));
        let mut source_catalog = IndexSourceCatalog::new();
        source_catalog
            .register(
                "shutdown-owner",
                "shutdown-owner-primary",
                [schema.reference.clone()],
                StableSinglePageSource {
                    calls: source_calls.clone(),
                },
            )
            .expect("source registration");
        let sources = source_catalog
            .materialize(&schema_catalog)
            .expect("source registry");
        let mut registry = SchemaRegistry::new();
        registry.register(schema).expect("schema registry");
        let runner = PostgresIndexReplayRunner::new(db.clone(), sources, Arc::new(registry));

        Self {
            db,
            runner,
            source_calls,
        }
    }

    fn request(&self, worker_id: &str) -> IndexReplayRunRequest {
        IndexReplayRunRequest::new(
            tenant_id(),
            schema_ref(),
            worker_id,
            10,
            1,
            1,
            Duration::from_secs(60),
        )
        .expect("bounded replay request")
    }
}

fn tenant_id() -> Uuid {
    Uuid::parse_str(TENANT).expect("tenant UUID")
}

fn schema_ref() -> SchemaRef {
    SchemaRef {
        module: ModuleName::new("shutdown-owner").expect("module name"),
        entity: EntityName::new("item").expect("entity name"),
        version: SchemaVersion::INITIAL,
    }
}

fn schema() -> IndexSchema {
    IndexSchema {
        reference: schema_ref(),
        locale_mode: LocaleMode::None,
        fields: vec![IndexField {
            name: FieldName::new("id").expect("field name"),
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

async fn scalar_i64(db: &DatabaseConnection, sql: &str) -> i64 {
    db.query_one(Statement::from_string(DbBackend::Sqlite, sql.to_owned()))
        .await
        .expect("scalar query should execute")
        .expect("scalar query should return one row")
        .try_get("", "value")
        .expect("scalar value should be integer")
}

async fn scalar_string(db: &DatabaseConnection, sql: &str) -> String {
    db.query_one(Statement::from_string(DbBackend::Sqlite, sql.to_owned()))
        .await
        .expect("scalar query should execute")
        .expect("scalar query should return one row")
        .try_get("", "value")
        .expect("scalar value should be text")
}

#[tokio::test]
async fn host_stop_before_scan_yields_pending_and_restart_completes_with_new_attempt() {
    let fixture = Fixture::new().await;
    let first = fixture
        .runner
        .run_interruptible(fixture.request("worker-a"), || true)
        .await
        .expect("host stop should yield replay");

    assert_eq!(first.status(), IndexReplayRunStatus::Yielded);
    assert_eq!(first.attempt_count(), Some(1));
    assert_eq!(first.pages_processed(), 0);
    assert_eq!(fixture.source_calls.load(Ordering::SeqCst), 0);
    assert_eq!(
        scalar_i64(
            &fixture.db,
            "SELECT COUNT(*) AS value FROM index_jobs WHERE kind = 'rebuild' AND state = 'pending' AND cancel_requested = FALSE AND lease_owner IS NULL AND lease_expires_at IS NULL AND completed_at IS NULL",
        )
        .await,
        1,
    );
    assert_eq!(
        scalar_i64(
            &fixture.db,
            "SELECT COUNT(*) AS value FROM index_checkpoints WHERE checkpoint_kind = 'rebuild'",
        )
        .await,
        0,
    );

    let resumed = fixture
        .runner
        .run(fixture.request("worker-b"))
        .await
        .expect("restart should resume pending replay");
    assert_eq!(resumed.status(), IndexReplayRunStatus::Complete);
    assert_eq!(resumed.job_id(), first.job_id());
    assert_eq!(resumed.attempt_count(), Some(2));
    assert_eq!(resumed.applied_count(), 1);
    assert_eq!(resumed.duplicate_count(), 0);
    assert_eq!(fixture.source_calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn host_stop_after_durable_mutation_before_checkpoint_replays_as_duplicate_on_restart() {
    let fixture = Fixture::new().await;
    let probe_calls = AtomicUsize::new(0);
    let first = fixture
        .runner
        .run_interruptible(fixture.request("worker-a"), || {
            probe_calls.fetch_add(1, Ordering::SeqCst) + 1 >= 3
        })
        .await
        .expect("host stop before checkpoint commit should yield replay");

    // Safe points for one mutation are: before scan, before mutation, before checkpoint commit.
    assert_eq!(probe_calls.load(Ordering::SeqCst), 3);
    assert_eq!(first.status(), IndexReplayRunStatus::Yielded);
    assert_eq!(first.attempt_count(), Some(1));
    assert_eq!(first.pages_processed(), 0);
    assert_eq!(fixture.source_calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        scalar_i64(
            &fixture.db,
            "SELECT COUNT(*) AS value FROM index_entities WHERE tenant_id = '11111111-1111-1111-1111-111111111111' AND module_name = 'shutdown-owner' AND entity_name = 'item' AND schema_version = 1 AND entity_id = '00000000-0000-0000-0000-000000000065' AND is_deleted = 0",
        )
        .await,
        1,
    );
    assert_eq!(
        scalar_i64(
            &fixture.db,
            "SELECT COUNT(*) AS value FROM index_inbox WHERE tenant_id = '11111111-1111-1111-1111-111111111111' AND source_name = 'shutdown-owner-primary' AND delivery_id = '00000000-0000-0000-0000-0000000003e9' AND state = 'applied'",
        )
        .await,
        1,
    );
    assert_eq!(
        scalar_i64(
            &fixture.db,
            "SELECT COUNT(*) AS value FROM index_checkpoints WHERE checkpoint_kind = 'rebuild'",
        )
        .await,
        0,
    );
    assert_eq!(
        scalar_string(
            &fixture.db,
            "SELECT state AS value FROM index_jobs WHERE kind = 'rebuild'",
        )
        .await,
        "pending",
    );

    let resumed = fixture
        .runner
        .run(fixture.request("worker-b"))
        .await
        .expect("restart should replay interrupted page safely");
    assert_eq!(resumed.status(), IndexReplayRunStatus::Complete);
    assert_eq!(resumed.job_id(), first.job_id());
    assert_eq!(resumed.attempt_count(), Some(2));
    assert_eq!(resumed.applied_count(), 0);
    assert_eq!(resumed.duplicate_count(), 1);
    assert_eq!(fixture.source_calls.load(Ordering::SeqCst), 2);
    assert_eq!(
        scalar_i64(
            &fixture.db,
            "SELECT COUNT(*) AS value FROM index_inbox WHERE tenant_id = '11111111-1111-1111-1111-111111111111' AND source_name = 'shutdown-owner-primary' AND delivery_id = '00000000-0000-0000-0000-0000000003e9' AND state = 'applied'",
        )
        .await,
        1,
    );
    assert_eq!(
        scalar_i64(
            &fixture.db,
            "SELECT COUNT(*) AS value FROM index_checkpoints WHERE checkpoint_kind = 'rebuild' AND json_type(cursor) = 'null'",
        )
        .await,
        1,
    );
    assert_eq!(
        scalar_string(
            &fixture.db,
            "SELECT state AS value FROM index_jobs WHERE kind = 'rebuild'",
        )
        .await,
        "succeeded",
    );
}
