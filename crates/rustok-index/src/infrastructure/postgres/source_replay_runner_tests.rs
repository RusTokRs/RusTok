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
use serde_json::json;
use uuid::Uuid;

use super::{
    IndexReplayCancelOutcome, IndexReplayJobAcquireOutcome, IndexReplayJobLeaseRequest,
    IndexReplayRunError, IndexReplayRunRequest, IndexReplayRunStatus, PostgresIndexReplayJobStore,
    PostgresIndexReplayRunner,
};
use crate::{
    EntityKey, EntityName, FieldCardinality, FieldName, IndexField, IndexModule, IndexMutation,
    IndexRecord, IndexSchema, IndexSchemaSourceCatalog, IndexSource, IndexSourceCatalog,
    IndexSourceFailure, IndexSourceLoadBatch, IndexSourceLoadRequest, IndexSourcePage,
    IndexSourceScanRequest, IndexValue, IndexValueType, LocaleMode, ModuleName, SchemaRef,
    SchemaRegistry, SchemaVersion,
};

const TENANT: &str = "11111111-1111-1111-1111-111111111111";

struct PagedSource {
    db: DatabaseConnection,
    calls: Arc<AtomicUsize>,
    page_count: usize,
    expire_lease_on_call: Option<usize>,
    request_cancel_on_call: Option<usize>,
}

#[async_trait]
impl IndexSource for PagedSource {
    async fn scan(
        &self,
        request: IndexSourceScanRequest,
    ) -> Result<IndexSourcePage, IndexSourceFailure> {
        let call = self.calls.fetch_add(1, Ordering::SeqCst);
        if self.expire_lease_on_call == Some(call) {
            self.db
                .execute_raw(Statement::from_string(
                    DbBackend::Sqlite,
                    "UPDATE index_jobs SET lease_expires_at = datetime('now', '-1 second') WHERE kind = 'rebuild' AND state = 'running'".to_owned(),
                ))
                .await
                .expect("test source should expire the active replay lease");
        }
        if self.request_cancel_on_call == Some(call) {
            self.db
                .execute_raw(Statement::from_string(
                    DbBackend::Sqlite,
                    "UPDATE index_jobs SET cancel_requested = TRUE WHERE kind = 'rebuild' AND state = 'running'".to_owned(),
                ))
                .await
                .expect("test source should request cancellation");
        }

        let page = request
            .cursor()
            .map(|cursor| {
                cursor
                    .value()
                    .as_u64()
                    .expect("test cursor should be an integer") as usize
            })
            .unwrap_or(0);
        let entity_id = Uuid::from_u128(100 + page as u128);
        let event_id = Uuid::from_u128(1_000 + page as u128);
        let mutation = mutation(request.tenant_id(), entity_id, event_id, page as u64 + 1);
        let next_cursor = if page + 1 < self.page_count {
            Some(crate::IndexSourceCursor::new(json!(page + 1)).unwrap())
        } else {
            None
        };
        Ok(IndexSourcePage::new(&request, vec![mutation], next_cursor)
            .expect("test source page should satisfy the bounded contract"))
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
    runner: PostgresIndexReplayRunner,
    calls: Arc<AtomicUsize>,
}

impl Fixture {
    async fn new(
        page_count: usize,
        expire_lease_on_call: Option<usize>,
        request_cancel_on_call: Option<usize>,
    ) -> Self {
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
        db.execute_raw(Statement::from_sql_and_values(
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
                PagedSource {
                    db: db.clone(),
                    calls: calls.clone(),
                    page_count,
                    expire_lease_on_call,
                    request_cancel_on_call,
                },
            )
            .unwrap();
        let sources = source_catalog.materialize(&schema_catalog).unwrap();
        let mut registry = SchemaRegistry::new();
        registry.register(schema).unwrap();
        let runner = PostgresIndexReplayRunner::new(db.clone(), sources, Arc::new(registry));

        Self { db, runner, calls }
    }

    fn request(
        &self,
        worker_id: &str,
        max_pages: usize,
        heartbeat_every_pages: usize,
    ) -> IndexReplayRunRequest {
        IndexReplayRunRequest::new(
            Uuid::parse_str(TENANT).unwrap(),
            schema_ref(),
            worker_id,
            10,
            max_pages,
            heartbeat_every_pages,
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

fn mutation(
    tenant_id: Uuid,
    entity_id: Uuid,
    event_id: Uuid,
    source_version: u64,
) -> IndexMutation {
    IndexMutation::Upsert {
        event_id,
        record: IndexRecord {
            key: EntityKey {
                tenant_id,
                schema: schema_ref(),
                entity_id,
                locale: None,
            },
            source_version,
            fields: BTreeMap::from([(FieldName::new("id").unwrap(), IndexValue::Uuid(entity_id))]),
            links: Vec::new(),
        },
    }
}

async fn scalar_i64(db: &DatabaseConnection, sql: &str) -> i64 {
    db.query_one_raw(Statement::from_string(DbBackend::Sqlite, sql.to_owned()))
        .await
        .expect("scalar query should execute")
        .expect("scalar query should return one row")
        .try_get("", "value")
        .expect("scalar value should be integer")
}

async fn scalar_string(db: &DatabaseConnection, sql: &str) -> String {
    db.query_one_raw(Statement::from_string(DbBackend::Sqlite, sql.to_owned()))
        .await
        .expect("scalar query should execute")
        .expect("scalar query should return one row")
        .try_get("", "value")
        .expect("scalar value should be text")
}

#[tokio::test]
async fn bounded_run_yields_pending_and_resumes_with_a_new_attempt() {
    let fixture = Fixture::new(3, None, None).await;
    let first = fixture
        .runner
        .run(fixture.request("worker-a", 2, 1))
        .await
        .unwrap();
    assert_eq!(first.status(), IndexReplayRunStatus::Yielded);
    assert_eq!(first.pages_processed(), 2);
    assert_eq!(first.heartbeat_count(), 1);
    assert_eq!(first.mutation_count(), 2);
    assert_eq!(first.attempt_count(), Some(1));
    assert_eq!(
        scalar_i64(
            &fixture.db,
            "SELECT COUNT(*) AS value FROM index_jobs WHERE kind = 'rebuild' AND state = 'pending' AND lease_owner IS NULL AND lease_expires_at IS NULL AND completed_at IS NULL",
        )
        .await,
        1,
    );

    let second = fixture
        .runner
        .run(fixture.request("worker-b", 2, 1))
        .await
        .unwrap();
    assert_eq!(second.status(), IndexReplayRunStatus::Complete);
    assert_eq!(second.job_id(), first.job_id());
    assert_eq!(second.attempt_count(), Some(2));
    assert_eq!(second.pages_processed(), 1);
    assert_eq!(fixture.calls.load(Ordering::SeqCst), 3);
    assert_eq!(
        scalar_i64(
            &fixture.db,
            "SELECT COUNT(*) AS value FROM index_jobs WHERE kind = 'rebuild' AND state = 'succeeded' AND completed_at IS NOT NULL AND cancel_requested = FALSE",
        )
        .await,
        1,
    );
}

#[tokio::test]
async fn pending_cancel_request_terminalizes_without_a_worker() {
    let fixture = Fixture::new(3, None, None).await;
    let first = fixture
        .runner
        .run(fixture.request("worker-a", 1, 1))
        .await
        .unwrap();
    assert_eq!(first.status(), IndexReplayRunStatus::Yielded);
    let job_id = first.job_id().unwrap();
    assert_eq!(
        fixture
            .runner
            .request_cancel(Uuid::parse_str(TENANT).unwrap(), job_id)
            .await
            .unwrap(),
        IndexReplayCancelOutcome::Cancelled,
    );
    assert_eq!(
        scalar_i64(
            &fixture.db,
            "SELECT COUNT(*) AS value FROM index_jobs WHERE kind = 'rebuild' AND state = 'cancelled' AND cancel_requested = TRUE AND completed_at IS NOT NULL AND lease_owner IS NULL",
        )
        .await,
        1,
    );
}

#[tokio::test]
async fn running_cancel_request_is_observed_after_the_current_page() {
    let fixture = Fixture::new(3, None, Some(0)).await;
    let outcome = fixture
        .runner
        .run(fixture.request("worker-a", 3, 1))
        .await
        .unwrap();
    assert_eq!(outcome.status(), IndexReplayRunStatus::Cancelled);
    assert_eq!(outcome.pages_processed(), 1);
    assert_eq!(fixture.calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        scalar_i64(
            &fixture.db,
            "SELECT COUNT(*) AS value FROM index_jobs WHERE kind = 'rebuild' AND state = 'cancelled' AND cancel_requested = TRUE AND completed_at IS NOT NULL",
        )
        .await,
        1,
    );
    assert_eq!(
        scalar_string(
            &fixture.db,
            "SELECT CAST(cursor AS TEXT) AS value FROM index_checkpoints WHERE checkpoint_kind = 'rebuild'",
        )
        .await,
        "1",
    );
}

#[tokio::test]
async fn requested_running_cancel_survives_reclaim_and_fences_the_old_attempt() {
    let fixture = Fixture::new(3, None, None).await;
    let job_store = PostgresIndexReplayJobStore::new(fixture.db.clone());
    let lease_request = IndexReplayJobLeaseRequest::new(
        Uuid::parse_str(TENANT).unwrap(),
        schema_ref(),
        "product-primary",
        "worker-a",
        Duration::from_secs(60),
    )
    .unwrap();
    let first = match job_store.acquire(&lease_request).await.unwrap() {
        IndexReplayJobAcquireOutcome::Acquired(lease) => lease,
        outcome => panic!("first worker should acquire replay job, got {outcome:?}"),
    };
    assert_eq!(
        fixture
            .runner
            .request_cancel(Uuid::parse_str(TENANT).unwrap(), first.job_id())
            .await
            .unwrap(),
        IndexReplayCancelOutcome::Requested,
    );
    fixture
        .db
        .execute_raw(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "UPDATE index_jobs SET lease_expires_at = datetime('now', '-1 second') WHERE tenant_id = ?1 AND job_id = ?2",
            vec![TENANT.to_owned().into(), first.job_id().to_string().into()],
        ))
        .await
        .unwrap();

    let second = fixture
        .runner
        .run(fixture.request("worker-b", 3, 1))
        .await
        .unwrap();
    assert_eq!(second.status(), IndexReplayRunStatus::Cancelled);
    assert_eq!(second.job_id(), Some(first.job_id()));
    assert_eq!(second.attempt_count(), Some(2));
    assert_eq!(second.pages_processed(), 0);
    assert_eq!(fixture.calls.load(Ordering::SeqCst), 0);
    assert_eq!(
        job_store.heartbeat(&first, Duration::from_secs(60)).await,
        Err(super::IndexReplayJobError::LeaseLost),
    );
}

#[tokio::test]
async fn lease_loss_during_a_page_does_not_publish_failure_or_advance_cursor() {
    let fixture = Fixture::new(2, Some(1), None).await;
    let error = fixture
        .runner
        .run(fixture.request("worker-a", 2, 1))
        .await
        .expect_err("expired attempt must stop without terminal publication");
    assert!(matches!(error, IndexReplayRunError::LeaseLost { .. }));
    assert_eq!(fixture.calls.load(Ordering::SeqCst), 2);
    assert_eq!(
        scalar_i64(
            &fixture.db,
            "SELECT COUNT(*) AS value FROM index_jobs WHERE kind = 'rebuild' AND state = 'running' AND completed_at IS NULL AND last_error_code IS NULL",
        )
        .await,
        1,
    );
    assert_eq!(
        scalar_string(
            &fixture.db,
            "SELECT CAST(cursor AS TEXT) AS value FROM index_checkpoints WHERE checkpoint_kind = 'rebuild'",
        )
        .await,
        "1",
    );
}

#[test]
fn run_request_bounds_pages_and_heartbeat_cadence() {
    let tenant_id = Uuid::parse_str(TENANT).unwrap();
    assert!(
        IndexReplayRunRequest::new(
            tenant_id,
            schema_ref(),
            "worker-a",
            10,
            0,
            1,
            Duration::from_secs(60),
        )
        .is_err()
    );
    assert!(
        IndexReplayRunRequest::new(
            tenant_id,
            schema_ref(),
            "worker-a",
            10,
            2,
            3,
            Duration::from_secs(60),
        )
        .is_err()
    );
}
