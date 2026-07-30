use std::{
    collections::BTreeMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::Duration,
};

use async_trait::async_trait;
use rustok_core::MigrationSource;
use sea_orm::{
    ConnectionTrait, Database, DatabaseConnection, DbBackend, Statement, Value as SqlValue,
};
use sea_orm_migration::SchemaManager;
use serde_json::json;
use uuid::Uuid;

use super::{
    IndexReconciliationRunRequest, IndexReconciliationRunStatus,
    PostgresIndexReconciliationRunner,
};
use crate::{
    EntityKey, EntityName, FieldCardinality, FieldName, IndexField, IndexModule, IndexMutation,
    IndexRecord, IndexSchema, IndexSchemaSourceCatalog, IndexSource, IndexSourceCatalog,
    IndexSourceFailure, IndexSourceLoadBatch, IndexSourceLoadRequest, IndexSourcePage,
    IndexSourceScanRequest, IndexValue, IndexValueType, LocaleMode, ModuleName, SchemaRef,
    SchemaRegistry, SchemaVersion,
};

const TENANT: &str = "11111111-1111-1111-1111-111111111111";

struct ReconciliationSource {
    ids: Arc<Mutex<Vec<u128>>>,
    calls: Arc<AtomicUsize>,
    injected: Arc<AtomicBool>,
    inject_on_call: Option<usize>,
}

#[async_trait]
impl IndexSource for ReconciliationSource {
    async fn scan(
        &self,
        request: IndexSourceScanRequest,
    ) -> Result<IndexSourcePage, IndexSourceFailure> {
        let call = self.calls.fetch_add(1, Ordering::SeqCst);
        if self.inject_on_call == Some(call) && !self.injected.swap(true, Ordering::SeqCst) {
            self.ids
                .lock()
                .expect("source ids lock")
                .push(50);
        }
        let after = request
            .cursor()
            .map(|cursor| {
                cursor
                    .value()
                    .as_u64()
                    .expect("test cursor should be a positive integer") as u128
            })
            .unwrap_or(0);
        let mut ids = self.ids.lock().expect("source ids lock").clone();
        ids.sort_unstable();
        ids.dedup();
        let visible = ids
            .into_iter()
            .filter(|id| *id > after)
            .collect::<Vec<_>>();
        let selected = visible
            .iter()
            .copied()
            .take(request.limit())
            .collect::<Vec<_>>();
        let next_cursor = if visible.len() > selected.len() {
            selected.last().copied().map(|id| {
                crate::IndexSourceCursor::new(json!(id as u64))
                    .expect("test cursor should be valid")
            })
        } else {
            None
        };
        let mutations = selected
            .into_iter()
            .map(|id| mutation(request.tenant_id(), id))
            .collect();
        IndexSourcePage::new(&request, mutations, next_cursor)
            .map_err(|_| IndexSourceFailure::permanent("fixture_page_invalid").unwrap())
    }

    async fn load(
        &self,
        request: IndexSourceLoadRequest,
    ) -> Result<IndexSourceLoadBatch, IndexSourceFailure> {
        Ok(IndexSourceLoadBatch::new(&request, Vec::new()).expect("empty targeted load"))
    }
}

struct Fixture {
    db: DatabaseConnection,
    runner: PostgresIndexReconciliationRunner,
    calls: Arc<AtomicUsize>,
}

impl Fixture {
    async fn new(inject_on_call: Option<usize>) -> Self {
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
        let fingerprint = schema.fingerprint().unwrap().to_string();
        let schema_json = serde_json::to_value(&schema).unwrap();
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
        schema_catalog.register("product", schema.clone()).unwrap();
        let calls = Arc::new(AtomicUsize::new(0));
        let mut source_catalog = IndexSourceCatalog::new();
        source_catalog
            .register(
                "product",
                "product-primary",
                [schema.reference.clone()],
                ReconciliationSource {
                    ids: Arc::new(Mutex::new(vec![100, 200])),
                    calls: calls.clone(),
                    injected: Arc::new(AtomicBool::new(false)),
                    inject_on_call,
                },
            )
            .unwrap();
        let sources = source_catalog.materialize(&schema_catalog).unwrap();
        let mut registry = SchemaRegistry::new();
        registry.register(schema).unwrap();
        let runner =
            PostgresIndexReconciliationRunner::new(db.clone(), sources, Arc::new(registry));
        Self { db, runner, calls }
    }

    fn request(&self, worker: &str, max_pages: usize, pass_count: u32) -> IndexReconciliationRunRequest {
        IndexReconciliationRunRequest::new(
            Uuid::parse_str(TENANT).unwrap(),
            schema_ref(),
            worker,
            1,
            max_pages,
            1,
            pass_count,
            Duration::from_secs(60),
        )
        .unwrap()
    }
}

fn schema_ref() -> SchemaRef {
    SchemaRef {
        module: ModuleName::new("rustok-product").unwrap(),
        entity: EntityName::new("product").unwrap(),
        version: SchemaVersion::INITIAL,
    }
}

fn schema() -> IndexSchema {
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

fn mutation(tenant_id: Uuid, id: u128) -> IndexMutation {
    let entity_id = Uuid::from_u128(id);
    IndexMutation::Upsert {
        event_id: Uuid::from_u128(10_000 + id),
        record: IndexRecord {
            key: EntityKey {
                tenant_id,
                schema: schema_ref(),
                entity_id,
                locale: None,
            },
            source_version: 1,
            fields: BTreeMap::from([(
                FieldName::new("id").unwrap(),
                IndexValue::Uuid(entity_id),
            )]),
            links: Vec::new(),
        },
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

#[tokio::test]
async fn two_pass_reconciliation_catches_insert_behind_first_cursor() {
    let fixture = Fixture::new(Some(1)).await;
    let outcome = fixture
        .runner
        .run(fixture.request("worker-a", 8, 2))
        .await
        .unwrap();

    assert_eq!(outcome.status(), IndexReconciliationRunStatus::Complete);
    assert_eq!(outcome.passes_completed(), 2);
    assert_eq!(outcome.pages_processed(), 5);
    assert_eq!(outcome.mutation_count(), 5);
    assert_eq!(outcome.applied_count(), 3);
    assert_eq!(outcome.duplicate_count(), 2);
    assert_eq!(fixture.calls.load(Ordering::SeqCst), 5);
    assert_eq!(
        scalar_i64(&fixture.db, "SELECT COUNT(*) AS value FROM index_entities").await,
        3,
    );
    assert_eq!(
        scalar_i64(
            &fixture.db,
            "SELECT CAST(json_extract(cursor, '$.completed_passes') AS INTEGER) AS value FROM index_jobs WHERE kind = 'reconcile' AND state = 'succeeded'",
        )
        .await,
        2,
    );

    let repeated = fixture
        .runner
        .run(fixture.request("worker-b", 8, 2))
        .await
        .unwrap();
    assert_eq!(
        repeated.status(),
        IndexReconciliationRunStatus::AlreadyComplete
    );
    assert_eq!(fixture.calls.load(Ordering::SeqCst), 5);
}

#[tokio::test]
async fn bounded_reconciliation_yields_and_resumes_durable_pass_state() {
    let fixture = Fixture::new(Some(1)).await;

    let first = fixture
        .runner
        .run(fixture.request("worker-a", 2, 2))
        .await
        .unwrap();
    assert_eq!(first.status(), IndexReconciliationRunStatus::Yielded);
    assert_eq!(first.passes_completed(), 1);
    assert_eq!(first.attempt_count(), Some(1));

    let second = fixture
        .runner
        .run(fixture.request("worker-b", 2, 2))
        .await
        .unwrap();
    assert_eq!(second.status(), IndexReconciliationRunStatus::Yielded);
    assert_eq!(second.job_id(), first.job_id());
    assert_eq!(second.passes_completed(), 1);
    assert_eq!(second.attempt_count(), Some(2));

    let third = fixture
        .runner
        .run(fixture.request("worker-c", 2, 2))
        .await
        .unwrap();
    assert_eq!(third.status(), IndexReconciliationRunStatus::Complete);
    assert_eq!(third.job_id(), first.job_id());
    assert_eq!(third.passes_completed(), 2);
    assert_eq!(third.attempt_count(), Some(3));
    assert_eq!(fixture.calls.load(Ordering::SeqCst), 5);
    assert_eq!(
        scalar_i64(
            &fixture.db,
            "SELECT CAST(json_extract(cursor, '$.pages_processed') AS INTEGER) AS value FROM index_jobs WHERE kind = 'reconcile' AND state = 'succeeded'",
        )
        .await,
        5,
    );
}

#[test]
fn reconciliation_request_bounds_pages_passes_and_heartbeat_cadence() {
    let tenant_id = Uuid::parse_str(TENANT).unwrap();
    assert!(IndexReconciliationRunRequest::new(
        tenant_id,
        schema_ref(),
        "worker-a",
        10,
        0,
        1,
        2,
        Duration::from_secs(60),
    )
    .is_err());
    assert!(IndexReconciliationRunRequest::new(
        tenant_id,
        schema_ref(),
        "worker-a",
        10,
        2,
        3,
        2,
        Duration::from_secs(60),
    )
    .is_err());
    assert!(IndexReconciliationRunRequest::new(
        tenant_id,
        schema_ref(),
        "worker-a",
        10,
        2,
        1,
        0,
        Duration::from_secs(60),
    )
    .is_err());
    assert!(IndexReconciliationRunRequest::new(
        tenant_id,
        schema_ref(),
        "worker-a",
        10,
        2,
        1,
        9,
        Duration::from_secs(60),
    )
    .is_err());
}
