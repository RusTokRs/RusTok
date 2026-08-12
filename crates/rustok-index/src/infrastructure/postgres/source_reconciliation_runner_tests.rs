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
    IndexReconciliationRunError, IndexReconciliationRunRequest, IndexReconciliationRunStatus,
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

#[derive(Clone, Copy)]
enum FailureMode {
    Retryable,
    Permanent,
}

struct ReconciliationSource {
    ids: Arc<Mutex<Vec<u128>>>,
    calls: Arc<AtomicUsize>,
    injected: Arc<AtomicBool>,
    inject_on_call: Option<usize>,
    failure: Option<FailureMode>,
}

#[async_trait]
impl IndexSource for ReconciliationSource {
    async fn scan(
        &self,
        request: IndexSourceScanRequest,
    ) -> Result<IndexSourcePage, IndexSourceFailure> {
        let call = self.calls.fetch_add(1, Ordering::SeqCst);
        if let Some(failure) = self.failure {
            return Err(match failure {
                FailureMode::Retryable => {
                    IndexSourceFailure::retryable("fixture_source_retryable").unwrap()
                }
                FailureMode::Permanent => {
                    IndexSourceFailure::permanent("fixture_source_permanent").unwrap()
                }
            });
        }
        if self.inject_on_call == Some(call) && !self.injected.swap(true, Ordering::SeqCst) {
            self.ids.lock().expect("source ids lock").push(50);
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
        let visible = ids.into_iter().filter(|id| *id > after).collect::<Vec<_>>();
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
        Self::build(inject_on_call, None).await
    }

    async fn failing(failure: FailureMode) -> Self {
        Self::build(None, Some(failure)).await
    }

    async fn build(inject_on_call: Option<usize>, failure: Option<FailureMode>) -> Self {
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
                    failure,
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

    fn request(
        &self,
        worker: &str,
        max_pages: usize,
        pass_count: u32,
    ) -> IndexReconciliationRunRequest {
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

    async fn make_retry_due(&self) {
        let updated = self
            .db
            .execute(Statement::from_string(
                DbBackend::Sqlite,
                "UPDATE index_jobs SET available_at = CURRENT_TIMESTAMP WHERE kind = 'reconcile' AND state = 'pending'"
                    .to_owned(),
            ))
            .await
            .expect("retry fixture should become due");
        assert_eq!(updated.rows_affected(), 1);
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
            fields: BTreeMap::from([(FieldName::new("id").unwrap(), IndexValue::Uuid(entity_id))]),
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

async fn scalar_string(db: &DatabaseConnection, sql: &str) -> String {
    db.query_one(Statement::from_string(DbBackend::Sqlite, sql.to_owned()))
        .await
        .expect("scalar query should execute")
        .expect("scalar query should return one row")
        .try_get("", "value")
        .expect("scalar value should be text")
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
    assert_eq!(outcome.retry_after(), None);
    assert_eq!(outcome.next_attempt(), None);
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

#[tokio::test]
async fn retryable_failure_schedules_due_attempts_and_terminally_exhausts() {
    let fixture = Fixture::failing(FailureMode::Retryable).await;
    let mut job_id = None;

    for (index, (attempt, seconds, next_attempt)) in
        [(1_u32, 5_u64, 2_u32), (2, 10, 3), (3, 20, 4), (4, 40, 5)]
            .into_iter()
            .enumerate()
    {
        let worker = format!("retry-worker-{index}");
        let outcome = fixture
            .runner
            .run(fixture.request(&worker, 1, 1))
            .await
            .unwrap();
        assert_eq!(
            outcome.status(),
            IndexReconciliationRunStatus::RetryScheduled
        );
        assert_eq!(outcome.attempt_count(), Some(attempt));
        assert_eq!(outcome.retry_after(), Some(Duration::from_secs(seconds)));
        assert_eq!(outcome.next_attempt(), Some(next_attempt));
        assert_eq!(outcome.pages_processed(), 0);
        if let Some(expected) = job_id {
            assert_eq!(outcome.job_id(), Some(expected));
        } else {
            job_id = outcome.job_id();
        }
        assert_eq!(
            scalar_string(
                &fixture.db,
                "SELECT state AS value FROM index_jobs WHERE kind = 'reconcile'",
            )
            .await,
            "pending",
        );
        assert_eq!(
            scalar_i64(
                &fixture.db,
                "SELECT CAST(attempt_count AS INTEGER) AS value FROM index_jobs WHERE kind = 'reconcile'",
            )
            .await,
            i64::from(attempt),
        );
        assert_eq!(
            scalar_i64(
                &fixture.db,
                "SELECT CAST(json_extract(last_error_details, '$.retryable') AS INTEGER) AS value FROM index_jobs WHERE kind = 'reconcile'",
            )
            .await,
            1,
        );

        let busy_worker = format!("retry-busy-{index}");
        let busy = fixture
            .runner
            .run(fixture.request(&busy_worker, 1, 1))
            .await
            .unwrap();
        assert_eq!(busy.status(), IndexReconciliationRunStatus::Busy);
        assert_eq!(fixture.calls.load(Ordering::SeqCst), attempt as usize);
        fixture.make_retry_due().await;
    }

    let exhausted = fixture
        .runner
        .run(fixture.request("retry-worker-final", 1, 1))
        .await
        .unwrap();
    assert_eq!(
        exhausted.status(),
        IndexReconciliationRunStatus::FailedExhausted
    );
    assert_eq!(exhausted.job_id(), job_id);
    assert_eq!(exhausted.attempt_count(), Some(5));
    assert_eq!(exhausted.retry_after(), None);
    assert_eq!(exhausted.next_attempt(), None);
    assert_eq!(fixture.calls.load(Ordering::SeqCst), 5);
    assert_eq!(
        scalar_string(
            &fixture.db,
            "SELECT state AS value FROM index_jobs WHERE kind = 'reconcile'",
        )
        .await,
        "failed",
    );

    let blocked = fixture
        .runner
        .run(fixture.request("retry-worker-blocked", 1, 1))
        .await
        .expect_err("exhausted reconciliation must remain dead-lettered");
    assert!(matches!(
        blocked,
        IndexReconciliationRunError::DeadLettered {
            attempt_count: 5,
            ..
        }
    ));
    assert_eq!(fixture.calls.load(Ordering::SeqCst), 5);
}

#[tokio::test]
async fn permanent_failure_terminalizes_without_retry_metadata() {
    let fixture = Fixture::failing(FailureMode::Permanent).await;
    let outcome = fixture
        .runner
        .run(fixture.request("permanent-worker", 1, 1))
        .await
        .unwrap();

    assert_eq!(
        outcome.status(),
        IndexReconciliationRunStatus::FailedPermanent
    );
    assert_eq!(outcome.attempt_count(), Some(1));
    assert_eq!(outcome.retry_after(), None);
    assert_eq!(outcome.next_attempt(), None);
    assert_eq!(outcome.pages_processed(), 0);
    assert_eq!(fixture.calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        scalar_string(
            &fixture.db,
            "SELECT state AS value FROM index_jobs WHERE kind = 'reconcile'",
        )
        .await,
        "failed",
    );

    let blocked = fixture
        .runner
        .run(fixture.request("permanent-worker-blocked", 1, 1))
        .await
        .expect_err("permanent reconciliation failure must remain dead-lettered");
    assert!(matches!(
        blocked,
        IndexReconciliationRunError::DeadLettered {
            attempt_count: 1,
            ..
        }
    ));
    assert_eq!(fixture.calls.load(Ordering::SeqCst), 1);
}

#[test]
fn reconciliation_request_bounds_pages_passes_and_heartbeat_cadence() {
    let tenant_id = Uuid::parse_str(TENANT).unwrap();
    assert!(
        IndexReconciliationRunRequest::new(
            tenant_id,
            schema_ref(),
            "worker-a",
            10,
            0,
            1,
            2,
            Duration::from_secs(60),
        )
        .is_err()
    );
    assert!(
        IndexReconciliationRunRequest::new(
            tenant_id,
            schema_ref(),
            "worker-a",
            10,
            2,
            3,
            2,
            Duration::from_secs(60),
        )
        .is_err()
    );
    assert!(
        IndexReconciliationRunRequest::new(
            tenant_id,
            schema_ref(),
            "worker-a",
            10,
            2,
            1,
            0,
            Duration::from_secs(60),
        )
        .is_err()
    );
    assert!(
        IndexReconciliationRunRequest::new(
            tenant_id,
            schema_ref(),
            "worker-a",
            10,
            2,
            1,
            9,
            Duration::from_secs(60),
        )
        .is_err()
    );
}
