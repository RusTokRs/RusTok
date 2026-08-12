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
use tokio::sync::Notify;
use uuid::Uuid;

use super::{
    IndexReplayRunError, IndexReplayRunRequest, IndexReplayRunStatus, PostgresIndexReplayRunner,
};
use crate::{
    EntityKey, EntityName, FieldCardinality, FieldName, IndexField, IndexModule, IndexMutation,
    IndexRecord, IndexSchema, IndexSchemaSourceCatalog, IndexSource, IndexSourceCatalog,
    IndexSourceFailure, IndexSourceLoadBatch, IndexSourceLoadRequest, IndexSourcePage,
    IndexSourceScanRequest, IndexValue, IndexValueType, LocaleMode, ModuleName, SchemaRef,
    SchemaRegistry, SchemaVersion,
};

const TENANT: &str = "22222222-2222-2222-2222-222222222222";
const ENTITY_ID: Uuid = Uuid::from_u128(202);
const EVENT_ID: Uuid = Uuid::from_u128(2_002);
const EVIDENCE_LEASE_DURATION: Duration = Duration::from_secs(86_400);

struct BlockingFirstScanSource {
    calls: Arc<AtomicUsize>,
    first_host_scan_started: Arc<Notify>,
    release_first_host_scan: Arc<Notify>,
}

#[async_trait]
impl IndexSource for BlockingFirstScanSource {
    async fn scan(
        &self,
        request: IndexSourceScanRequest,
    ) -> Result<IndexSourcePage, IndexSourceFailure> {
        let call = self.calls.fetch_add(1, Ordering::SeqCst);
        if call == 0 {
            self.first_host_scan_started.notify_one();
            self.release_first_host_scan.notified().await;
        }

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
        Ok(IndexSourcePage::new(&request, vec![mutation], None)
            .expect("stable multi-host page should satisfy replay source contract"))
    }

    async fn load(
        &self,
        request: IndexSourceLoadRequest,
    ) -> Result<IndexSourceLoadBatch, IndexSourceFailure> {
        Ok(IndexSourceLoadBatch::new(&request, Vec::new())
            .expect("empty targeted load should satisfy source contract"))
    }
}

struct Fixture {
    db: DatabaseConnection,
    host_a: PostgresIndexReplayRunner,
    host_b: PostgresIndexReplayRunner,
    source_calls: Arc<AtomicUsize>,
    first_host_scan_started: Arc<Notify>,
    release_first_host_scan: Arc<Notify>,
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
        let fingerprint = schema
            .fingerprint()
            .expect("schema fingerprint")
            .to_string();
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

        let first_host_scan_started = Arc::new(Notify::new());
        let release_first_host_scan = Arc::new(Notify::new());
        let source_calls = Arc::new(AtomicUsize::new(0));
        let mut schema_catalog = IndexSchemaSourceCatalog::new();
        schema_catalog
            .register("multihost-owner", schema.clone())
            .expect("schema source registration");
        let mut source_catalog = IndexSourceCatalog::new();
        source_catalog
            .register(
                "multihost-owner",
                "multihost-owner-primary",
                [schema.reference.clone()],
                BlockingFirstScanSource {
                    calls: source_calls.clone(),
                    first_host_scan_started: first_host_scan_started.clone(),
                    release_first_host_scan: release_first_host_scan.clone(),
                },
            )
            .expect("source registration");
        let sources = source_catalog
            .materialize(&schema_catalog)
            .expect("source registry");
        let mut registry = SchemaRegistry::new();
        registry.register(schema).expect("schema registry");
        let registry = Arc::new(registry);

        let host_a = PostgresIndexReplayRunner::new(db.clone(), sources.clone(), registry.clone());
        let host_b = PostgresIndexReplayRunner::new(db.clone(), sources, registry);

        Self {
            db,
            host_a,
            host_b,
            source_calls,
            first_host_scan_started,
            release_first_host_scan,
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
            EVIDENCE_LEASE_DURATION,
        )
        .expect("bounded replay request")
    }
}

fn tenant_id() -> Uuid {
    Uuid::parse_str(TENANT).expect("tenant UUID")
}

fn schema_ref() -> SchemaRef {
    SchemaRef {
        module: ModuleName::new("multihost-owner").expect("module name"),
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
async fn expired_host_is_reclaimed_by_second_runner_and_stale_host_cannot_publish() {
    let fixture = Fixture::new().await;
    let host_a = fixture.host_a.clone();
    let host_a_request = fixture.request("host-a");
    let host_a_task = tokio::spawn(async move { host_a.run(host_a_request).await });

    fixture.first_host_scan_started.notified().await;
    assert_eq!(fixture.source_calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        scalar_string(
            &fixture.db,
            "SELECT lease_owner AS value FROM index_jobs WHERE kind = 'rebuild' AND state = 'running'",
        )
        .await,
        "host-a",
    );
    assert_eq!(
        scalar_i64(
            &fixture.db,
            "SELECT CAST(attempt_count AS INTEGER) AS value FROM index_jobs WHERE kind = 'rebuild'",
        )
        .await,
        1,
    );

    let expired = fixture
        .db
        .execute(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "UPDATE index_jobs SET lease_expires_at = datetime('now', '-1 second') WHERE tenant_id = ?1 AND kind = 'rebuild' AND state = 'running' AND lease_owner = ?2 AND attempt_count = 1",
            vec![TENANT.to_owned().into(), "host-a".to_owned().into()],
        ))
        .await
        .expect("evidence fixture should expire host-a lease deterministically");
    assert_eq!(expired.rows_affected(), 1);

    let second = fixture
        .host_b
        .run(fixture.request("host-b"))
        .await
        .expect("host-b should reclaim the expired replay attempt");
    assert_eq!(second.status(), IndexReplayRunStatus::Complete);
    assert_eq!(second.attempt_count(), Some(2));
    assert_eq!(second.applied_count(), 1);
    assert_eq!(second.duplicate_count(), 0);
    assert_eq!(fixture.source_calls.load(Ordering::SeqCst), 2);
    let job_id = second
        .job_id()
        .expect("completed replay should expose job id");

    fixture.release_first_host_scan.notify_one();
    let first_error = host_a_task
        .await
        .expect("host-a task should join")
        .expect_err("stale host-a attempt must fail closed after host-b reclaim");
    match first_error {
        IndexReplayRunError::LeaseLost {
            job_id: stale_job_id,
            attempt_count,
        } => {
            assert_eq!(stale_job_id, job_id);
            assert_eq!(attempt_count, 1);
        }
        other => panic!("stale host-a should lose its lease fence, got {other:?}"),
    }

    assert_eq!(
        scalar_i64(
            &fixture.db,
            "SELECT COUNT(*) AS value FROM index_jobs WHERE kind = 'rebuild' AND state = 'succeeded' AND attempt_count = 2 AND lease_owner IS NULL AND lease_expires_at IS NULL AND completed_at IS NOT NULL AND last_error_code IS NULL",
        )
        .await,
        1,
    );
    assert_eq!(
        scalar_i64(
            &fixture.db,
            "SELECT COUNT(*) AS value FROM index_checkpoints WHERE checkpoint_kind = 'rebuild' AND CAST(cursor AS TEXT) = 'null'",
        )
        .await,
        1,
    );
    assert_eq!(
        scalar_i64(
            &fixture.db,
            "SELECT COUNT(*) AS value FROM index_inbox WHERE tenant_id = '22222222-2222-2222-2222-222222222222' AND source_name = 'multihost-owner-primary' AND delivery_id = '00000000-0000-0000-0000-0000000007d2' AND state = 'applied'",
        )
        .await,
        1,
    );
    assert_eq!(
        scalar_i64(
            &fixture.db,
            "SELECT COUNT(*) AS value FROM index_entities WHERE tenant_id = '22222222-2222-2222-2222-222222222222' AND module_name = 'multihost-owner' AND entity_name = 'item' AND schema_version = 1 AND entity_id = '00000000-0000-0000-0000-0000000000ca' AND is_deleted = 0",
        )
        .await,
        1,
    );
}
