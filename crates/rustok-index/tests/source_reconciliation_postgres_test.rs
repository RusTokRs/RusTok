use std::{
    collections::BTreeMap,
    error::Error,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

use async_trait::async_trait;
use rustok_core::MigrationSource;
use rustok_index::{
    EntityKey, EntityName, FieldCardinality, FieldName, IndexField, IndexModule, IndexMutation,
    IndexRecord, IndexReconciliationCancelOutcome, IndexReconciliationRunRequest,
    IndexReconciliationRunStatus, IndexReconciliationTerminalState, IndexSchema,
    IndexSchemaSourceCatalog, IndexSource, IndexSourceCatalog, IndexSourceCursor,
    IndexSourceFailure, IndexSourceLoadBatch, IndexSourceLoadRequest, IndexSourcePage,
    IndexSourceScanRequest, IndexValue, IndexValueType, LocaleMode, ModuleName,
    PostgresIndexReconciliationRunner, SchemaRef, SchemaRegistry, SchemaVersion,
};
use sea_orm::{
    ConnectOptions, ConnectionTrait, Database, DatabaseConnection, DbBackend, Statement,
    Value as SqlValue,
};
use sea_orm_migration::SchemaManager;
use serde_json::json;
use uuid::Uuid;

type TestResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

const DATABASE_ENV: &str = "RUSTOK_INDEX_TEST_DATABASE_URL";

#[derive(Clone)]
struct ReconciliationSource {
    ids: Arc<Mutex<Vec<u128>>>,
    calls: Arc<AtomicUsize>,
}

#[async_trait]
impl IndexSource for ReconciliationSource {
    async fn scan(
        &self,
        request: IndexSourceScanRequest,
    ) -> Result<IndexSourcePage, IndexSourceFailure> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        let after = request
            .cursor()
            .map(|cursor| {
                cursor
                    .value()
                    .as_u64()
                    .expect("fixture cursor must be a positive integer") as u128
            })
            .unwrap_or(0);
        let mut ids = self.ids.lock().expect("fixture source ids lock").clone();
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
                IndexSourceCursor::new(json!(id as u64))
                    .expect("fixture cursor must remain bounded")
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

struct PostgresReconciliationTestDb {
    control: DatabaseConnection,
    database_url: String,
    schema_name: String,
    tenant_id: Uuid,
    source_ids: Arc<Mutex<Vec<u128>>>,
    source_calls: Arc<AtomicUsize>,
}

impl PostgresReconciliationTestDb {
    async fn setup(case: &str) -> TestResult<Option<Self>> {
        let Some(database_url) = postgres_database_url() else {
            eprintln!(
                "{DATABASE_ENV} is not set to a PostgreSQL URL; skipping rustok-index reconciliation PostgreSQL harness"
            );
            return Ok(None);
        };
        let control = connect(&database_url).await?;
        let schema_name = format!(
            "rustok_index_reconciliation_{}_{}",
            case,
            Uuid::new_v4().simple()
        );
        control
            .execute_unprepared(&format!(r#"CREATE SCHEMA "{schema_name}""#))
            .await?;

        let tenant_id = Uuid::new_v4();
        let db = scoped_connection(&database_url, &schema_name).await?;
        db.execute_unprepared("CREATE TABLE tenants (id UUID NOT NULL PRIMARY KEY)")
            .await?;
        db.execute(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "INSERT INTO tenants (id) VALUES ($1)",
            vec![tenant_id.into()],
        ))
        .await?;

        let manager = SchemaManager::new(&db);
        for migration in IndexModule.migrations() {
            migration.up(&manager).await?;
        }
        persist_schema(&db, tenant_id).await?;

        Ok(Some(Self {
            control,
            database_url,
            schema_name,
            tenant_id,
            source_ids: Arc::new(Mutex::new(vec![100, 200])),
            source_calls: Arc::new(AtomicUsize::new(0)),
        }))
    }

    async fn connection(&self) -> TestResult<DatabaseConnection> {
        scoped_connection(&self.database_url, &self.schema_name).await
    }

    fn runner(&self, db: DatabaseConnection) -> PostgresIndexReconciliationRunner {
        let schema = schema();
        let mut schema_catalog = IndexSchemaSourceCatalog::new();
        schema_catalog
            .register("postgres-harness", schema.clone())
            .expect("fixture schema source must register");
        let mut source_catalog = IndexSourceCatalog::new();
        source_catalog
            .register(
                "postgres-harness",
                "postgres-harness-primary",
                [schema.reference.clone()],
                ReconciliationSource {
                    ids: self.source_ids.clone(),
                    calls: self.source_calls.clone(),
                },
            )
            .expect("fixture source must register");
        let sources = source_catalog
            .materialize(&schema_catalog)
            .expect("fixture source registry must materialize");
        let mut registry = SchemaRegistry::new();
        registry
            .register(schema)
            .expect("fixture schema registry must materialize");
        PostgresIndexReconciliationRunner::new(db, sources, Arc::new(registry))
    }

    fn request(&self, worker_id: &str, max_pages: usize) -> IndexReconciliationRunRequest {
        IndexReconciliationRunRequest::new(
            self.tenant_id,
            schema_ref(),
            worker_id,
            1,
            max_pages,
            1,
            1,
            Duration::from_secs(60),
        )
        .expect("fixture request must be valid")
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

fn postgres_database_url() -> Option<String> {
    std::env::var(DATABASE_ENV)
        .or_else(|_| std::env::var("DATABASE_URL"))
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

async fn persist_schema(db: &DatabaseConnection, tenant_id: Uuid) -> TestResult<()> {
    let schema = schema();
    let fingerprint = schema.fingerprint()?.to_string();
    let schema_json = serde_json::to_value(&schema)?;
    db.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "INSERT INTO index_schemas (tenant_id, module_name, entity_name, schema_version, schema_fingerprint, schema_json, status) VALUES ($1, $2, $3, $4, $5, $6, 'active')",
        vec![
            tenant_id.into(),
            schema.reference.module.as_str().to_owned().into(),
            schema.reference.entity.as_str().to_owned().into(),
            i64::from(schema.reference.version.get()).into(),
            fingerprint.into(),
            SqlValue::Json(Some(Box::new(schema_json))),
        ],
    ))
    .await?;
    Ok(())
}

fn schema_ref() -> SchemaRef {
    SchemaRef {
        module: ModuleName::new("postgres-harness").unwrap(),
        entity: EntityName::new("item").unwrap(),
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

async fn scalar_i64(db: &DatabaseConnection, sql: &str) -> TestResult<i64> {
    Ok(db
        .query_one(Statement::from_string(DbBackend::Postgres, sql.to_owned()))
        .await?
        .ok_or("scalar query returned no row")?
        .try_get("", "value")?)
}

async fn scalar_string(db: &DatabaseConnection, sql: &str) -> TestResult<String> {
    Ok(db
        .query_one(Statement::from_string(DbBackend::Postgres, sql.to_owned()))
        .await?
        .ok_or("scalar query returned no row")?
        .try_get("", "value")?)
}

#[tokio::test]
async fn reconciliation_yield_resumes_across_new_connection_and_preserves_job_identity(
) -> TestResult<()> {
    let Some(fixture) = PostgresReconciliationTestDb::setup("resume").await? else {
        return Ok(());
    };

    let first_runner = fixture.runner(fixture.connection().await?);
    let first = first_runner
        .run(fixture.request("postgres-worker-a", 1))
        .await?;
    assert_eq!(first.status(), IndexReconciliationRunStatus::Yielded);
    assert_eq!(first.attempt_count(), Some(1));
    assert_eq!(first.pages_processed(), 1);
    assert_eq!(first.passes_completed(), 0);
    let job_id = first.job_id().expect("yielded job id");
    drop(first_runner);

    let second_runner = fixture.runner(fixture.connection().await?);
    let second = second_runner
        .run(fixture.request("postgres-worker-b", 1))
        .await?;
    assert_eq!(second.status(), IndexReconciliationRunStatus::Complete);
    assert_eq!(second.job_id(), Some(job_id));
    assert_eq!(second.attempt_count(), Some(2));
    assert_eq!(second.pages_processed(), 1);
    assert_eq!(second.passes_completed(), 1);
    assert_eq!(fixture.source_calls.load(Ordering::SeqCst), 2);

    let evidence = fixture.connection().await?;
    assert_eq!(
        scalar_i64(&evidence, "SELECT COUNT(*)::bigint AS value FROM index_entities").await?,
        2
    );
    assert_eq!(
        scalar_i64(
            &evidence,
            "SELECT COUNT(*)::bigint AS value FROM index_jobs WHERE kind = 'reconcile'",
        )
        .await?,
        1
    );
    assert_eq!(
        scalar_string(
            &evidence,
            "SELECT state AS value FROM index_jobs WHERE kind = 'reconcile'",
        )
        .await?,
        "succeeded"
    );
    assert_eq!(
        scalar_i64(
            &evidence,
            "SELECT (cursor->>'completed_passes')::bigint AS value FROM index_jobs WHERE kind = 'reconcile'",
        )
        .await?,
        1
    );
    assert_eq!(
        scalar_i64(
            &evidence,
            "SELECT (cursor->>'pages_processed')::bigint AS value FROM index_jobs WHERE kind = 'reconcile'",
        )
        .await?,
        2
    );

    let repeated_runner = fixture.runner(fixture.connection().await?);
    let repeated = repeated_runner
        .run(fixture.request("postgres-worker-c", 1))
        .await?;
    assert_eq!(
        repeated.status(),
        IndexReconciliationRunStatus::AlreadyComplete
    );
    assert_eq!(repeated.job_id(), Some(job_id));
    assert_eq!(fixture.source_calls.load(Ordering::SeqCst), 2);

    fixture.cleanup().await
}

#[tokio::test]
async fn pending_reconciliation_cancel_is_durable_and_tenant_scoped() -> TestResult<()> {
    let Some(fixture) = PostgresReconciliationTestDb::setup("cancel").await? else {
        return Ok(());
    };

    let first_runner = fixture.runner(fixture.connection().await?);
    let first = first_runner
        .run(fixture.request("postgres-worker-a", 1))
        .await?;
    assert_eq!(first.status(), IndexReconciliationRunStatus::Yielded);
    let job_id = first.job_id().expect("yielded job id");
    drop(first_runner);

    let cancelling_runner = fixture.runner(fixture.connection().await?);
    assert_eq!(
        cancelling_runner
            .request_cancel(Uuid::new_v4(), job_id)
            .await?,
        IndexReconciliationCancelOutcome::NotFound
    );
    assert_eq!(
        cancelling_runner
            .request_cancel(fixture.tenant_id, job_id)
            .await?,
        IndexReconciliationCancelOutcome::Cancelled
    );
    assert_eq!(
        cancelling_runner
            .request_cancel(fixture.tenant_id, job_id)
            .await?,
        IndexReconciliationCancelOutcome::AlreadyTerminal(
            IndexReconciliationTerminalState::Cancelled
        )
    );

    let evidence = fixture.connection().await?;
    assert_eq!(
        scalar_string(
            &evidence,
            "SELECT state AS value FROM index_jobs WHERE kind = 'reconcile'",
        )
        .await?,
        "cancelled"
    );
    assert_eq!(
        scalar_i64(&evidence, "SELECT COUNT(*)::bigint AS value FROM index_entities").await?,
        1
    );
    assert_eq!(
        scalar_i64(
            &evidence,
            "SELECT COUNT(*)::bigint AS value FROM index_jobs WHERE kind = 'reconcile'",
        )
        .await?,
        1
    );

    fixture.cleanup().await
}
