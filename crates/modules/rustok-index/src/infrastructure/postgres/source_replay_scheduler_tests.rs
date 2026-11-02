//! Focused generic worker definitions; no test/SQL execution is claimed in the source packet.
use std::collections::BTreeMap;

use super::*;
use crate::{
    EntityKey, FieldCardinality, FieldName, IndexField, IndexModule, IndexMutation, IndexRecord,
    IndexReplayJobEnqueueRequest, IndexSchema, IndexSource, IndexSourceCursor, IndexSourceFailure,
    IndexSourceLoadBatch, IndexSourceLoadRequest, IndexSourcePage, IndexSourceScanRequest,
    IndexValue, IndexValueType, LocaleMode, PostgresIndexReplayJobStore,
    PostgresSchemaRegistrationStore, materialize_index_schema_registry,
    materialize_index_source_registry, materialize_postgres_index_replay_runtime,
    register_index_schema_source, register_index_source,
};
use rustok_core::MigrationSource;
use sea_orm::Database;
use sea_orm_migration::SchemaManager;
use serde_json::json;

const TENANT: Uuid = Uuid::from_u128(1);
const ORIGINAL: Uuid = Uuid::from_u128(900);
const SOURCE: &str = "scheduler-test-source";

struct Pages {
    fail: bool,
    pages: u64,
}

#[async_trait]
impl IndexSource for Pages {
    async fn scan(
        &self,
        request: IndexSourceScanRequest,
    ) -> Result<IndexSourcePage, IndexSourceFailure> {
        if self.fail {
            return Err(IndexSourceFailure::retryable("fixture_owner_unavailable").unwrap());
        }
        if self.pages == 0 {
            return Ok(IndexSourcePage::new(&request, Vec::new(), None).unwrap());
        }
        let page = request
            .cursor()
            .map(|cursor| cursor.value()["page"].as_u64().unwrap())
            .unwrap_or(0);
        let entity_id = Uuid::from_u128(2000 + u128::from(page));
        let mutation = IndexMutation::Upsert {
            event_id: Uuid::from_u128(1000 + u128::from(page)),
            record: IndexRecord {
                key: EntityKey {
                    tenant_id: request.tenant_id(),
                    schema: request.schema().clone(),
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
        };
        let next = (page + 1 < self.pages)
            .then(|| IndexSourceCursor::new(json!({"page": page + 1})).unwrap());
        Ok(IndexSourcePage::new(&request, vec![mutation], next).unwrap())
    }

    async fn load(
        &self,
        request: IndexSourceLoadRequest,
    ) -> Result<IndexSourceLoadBatch, IndexSourceFailure> {
        IndexSourceLoadBatch::new(&request, Vec::new())
            .map_err(|_| IndexSourceFailure::permanent("fixture_invalid").unwrap())
    }
}

struct Fixture {
    adapter: PostgresIndexReplayWorkAdapter,
    jobs: PostgresIndexReplayJobStore,
    schema: IndexSchema,
}

impl Fixture {
    async fn new(fail: bool) -> Self {
        Self::with_pages(fail, 9).await
    }

    async fn with_pages(fail: bool, pages: u64) -> Self {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        db.execute_unprepared(&format!(
            "CREATE TABLE tenants (id TEXT PRIMARY KEY); INSERT INTO tenants VALUES ('{TENANT}')"
        ))
        .await
        .unwrap();
        let manager = SchemaManager::new(&db);
        for migration in IndexModule.migrations() {
            migration.up(&manager).await.unwrap();
        }
        let schema = IndexSchema {
            reference: SchemaRef {
                module: ModuleName::new("scheduler_test").unwrap(),
                entity: EntityName::new("record").unwrap(),
                version: SchemaVersion::INITIAL,
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
        };
        PostgresSchemaRegistrationStore::new(db.clone())
            .register(TENANT, &schema)
            .await
            .unwrap();
        let mut extensions = ModuleRuntimeExtensions::default();
        register_index_schema_source(&mut extensions, "scheduler_test", schema.clone()).unwrap();
        register_index_source(
            &mut extensions,
            "scheduler_test",
            SOURCE,
            [schema.reference.clone()],
            Pages { fail, pages },
        )
        .unwrap();
        let schemas = materialize_index_schema_registry(&extensions)
            .unwrap()
            .unwrap();
        let sources = materialize_index_source_registry(&extensions)
            .unwrap()
            .unwrap();
        extensions.insert(schemas.clone());
        extensions.insert(sources.clone());
        let runtime = materialize_postgres_index_replay_runtime(&mut extensions, db.clone())
            .unwrap()
            .unwrap();
        let jobs = PostgresIndexReplayJobStore::new(db.clone());
        jobs.enqueue(
            &IndexReplayJobEnqueueRequest::new(TENANT, ORIGINAL, schema.reference.clone(), SOURCE)
                .unwrap(),
        )
        .await
        .unwrap();
        Self {
            adapter: PostgresIndexReplayWorkAdapter {
                db,
                schemas,
                sources,
                runtime,
            },
            jobs,
            schema,
        }
    }
}

#[tokio::test]
async fn worker_yields_original_job_with_durable_checkpoint_then_finishes_without_fake_completion()
{
    let fixture = Fixture::new(false).await;
    let first = fixture
        .adapter
        .claim(INDEX_REPLAY_WORKER)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(first.id, ORIGINAL);
    assert!(
        fixture
            .adapter
            .complete(&first, ModuleWorkOutcome::Completed)
            .await
            .is_err()
    );
    let yielded = fixture.adapter.execute(first.clone()).await.unwrap();
    assert!(matches!(yielded, ModuleWorkOutcome::Retryable { .. }));
    fixture.adapter.complete(&first, yielded).await.unwrap();
    let second = fixture
        .adapter
        .claim(INDEX_REPLAY_WORKER)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(second.id, first.id);
    assert_ne!(second.lease_token, first.lease_token);
    let complete = fixture.adapter.execute(second.clone()).await.unwrap();
    assert_eq!(complete, ModuleWorkOutcome::Completed);
    fixture.adapter.complete(&second, complete).await.unwrap();
    assert!(
        fixture
            .adapter
            .claim(INDEX_REPLAY_WORKER)
            .await
            .unwrap()
            .is_none()
    );
    let row = fixture
        .adapter
        .db
        .query_one_raw(Statement::from_string(
            DbBackend::Sqlite,
            "SELECT COUNT(*) AS n FROM index_entities",
        ))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(row.try_get::<i64>("", "n").unwrap(), 9);
    let job = fixture
        .adapter
        .db
        .query_one_raw(Statement::from_string(
            DbBackend::Sqlite,
            "SELECT state, attempt_count FROM index_jobs WHERE kind='rebuild'",
        ))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(job.try_get::<String>("", "state").unwrap(), "succeeded");
    assert_eq!(job.try_get::<i64>("", "attempt_count").unwrap(), 2);
}

#[tokio::test]
async fn unselected_due_source_cannot_starve_selected_work_or_gain_an_ack() {
    let fixture = Fixture::new(false).await;
    let mut unselected = fixture.schema.clone();
    unselected.reference.entity = EntityName::new("unselected").unwrap();
    PostgresSchemaRegistrationStore::new(fixture.adapter.db.clone())
        .register(TENANT, &unselected)
        .await
        .unwrap();
    fixture
        .jobs
        .enqueue(
            &IndexReplayJobEnqueueRequest::new(
                TENANT,
                Uuid::from_u128(800),
                unselected.reference,
                "unselected-source",
            )
            .unwrap(),
        )
        .await
        .unwrap();
    fixture.adapter.db.execute_unprepared("UPDATE index_jobs SET available_at=datetime('now', '-1 hour') WHERE entity_name='unselected'").await.unwrap();
    let selected = fixture
        .adapter
        .claim(INDEX_REPLAY_WORKER)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(selected.id, ORIGINAL);
    let retained = fixture
        .adapter
        .db
        .query_one_raw(Statement::from_string(
            DbBackend::Sqlite,
            "SELECT state, attempt_count FROM index_jobs WHERE entity_name='unselected'",
        ))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(retained.try_get::<String>("", "state").unwrap(), "pending");
    assert_eq!(retained.try_get::<i64>("", "attempt_count").unwrap(), 0);
}

#[tokio::test]
async fn owner_failure_parks_original_job_without_claiming_success_or_rotating_its_identity() {
    let fixture = Fixture::new(true).await;
    let item = fixture
        .adapter
        .claim(INDEX_REPLAY_WORKER)
        .await
        .unwrap()
        .unwrap();
    assert!(fixture.adapter.execute(item.clone()).await.is_err());
    assert!(
        fixture
            .adapter
            .complete(&item, ModuleWorkOutcome::Completed)
            .await
            .is_err()
    );
    fixture
        .adapter
        .complete(
            &item,
            ModuleWorkOutcome::Retryable {
                message: "fixture_native_failure".into(),
            },
        )
        .await
        .unwrap();
    assert!(
        fixture
            .adapter
            .claim(INDEX_REPLAY_WORKER)
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        fixture
            .jobs
            .enqueue(
                &IndexReplayJobEnqueueRequest::new(
                    TENANT,
                    ORIGINAL,
                    fixture.schema.reference,
                    SOURCE
                )
                .unwrap()
            )
            .await
            .is_err()
    );
}

#[test]
fn replay_work_registration_is_absent_without_sources() {
    let mut extensions = ModuleRuntimeExtensions::default();
    assert!(!register_postgres_index_replay_work(&mut extensions).unwrap());
    assert!(!extensions.contains::<ModuleWorkRegistrations>());
}

#[tokio::test]
async fn replay_work_registration_rejects_duplicate_owner_selection() {
    let fixture = Fixture::new(false).await;
    let mut extensions = ModuleRuntimeExtensions::default();
    extensions.insert(fixture.adapter.sources.clone());
    extensions.insert(fixture.adapter.schemas.clone());
    assert!(register_postgres_index_replay_work(&mut extensions).unwrap());
    assert!(matches!(
        register_postgres_index_replay_work(&mut extensions),
        Err(ModuleWorkError::DuplicateHandler(_))
    ));
}

#[tokio::test]
async fn empty_initial_source_still_completes_original_job_only_after_durable_null_checkpoint() {
    let fixture = Fixture::with_pages(false, 0).await;
    let item = fixture
        .adapter
        .claim(INDEX_REPLAY_WORKER)
        .await
        .unwrap()
        .unwrap();
    let outcome = fixture.adapter.execute(item.clone()).await.unwrap();
    assert_eq!(outcome, ModuleWorkOutcome::Completed);
    fixture.adapter.complete(&item, outcome).await.unwrap();
    let checkpoint = fixture
        .adapter
        .db
        .query_one_raw(Statement::from_string(
            DbBackend::Sqlite,
            "SELECT cursor FROM index_checkpoints",
        ))
        .await
        .unwrap()
        .unwrap();
    assert!(
        checkpoint
            .try_get::<serde_json::Value>("", "cursor")
            .unwrap()
            .is_null()
    );
    let count = fixture
        .adapter
        .db
        .query_one_raw(Statement::from_string(
            DbBackend::Sqlite,
            "SELECT COUNT(*) AS n FROM index_entities",
        ))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(count.try_get::<i64>("", "n").unwrap(), 0);
}
