use std::{
    env,
    error::Error,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

use async_trait::async_trait;
use rustok_core::MigrationSource;
use rustok_index::{
    EntityName, FieldCardinality, FieldName, IndexField, IndexModule, IndexReconciliationRunError,
    IndexReconciliationRunRequest, IndexReconciliationRunStatus, IndexSchema,
    IndexSchemaSourceCatalog, IndexSource, IndexSourceCatalog, IndexSourceFailure,
    IndexSourceLoadBatch, IndexSourceLoadRequest, IndexSourcePage, IndexSourceScanRequest,
    IndexValueType, LocaleMode, ModuleName, PostgresIndexReconciliationRunner, SchemaRef,
    SchemaRegistry, SchemaVersion,
};
use sea_orm::{
    ConnectOptions, ConnectionTrait, Database, DatabaseConnection, DbBackend, QueryResult,
    Statement, Value as SqlValue,
};
use sea_orm_migration::SchemaManager;
use serde_json::{Value as JsonValue, json};
use uuid::Uuid;

const DATABASE_ENV: &str = "RUSTOK_INDEX_TEST_DATABASE_URL";
const SOURCE_NAME: &str = "reconciliation-dead-letter-primary";
const MODULE_NAME: &str = "reconciliation-dead-letter-harness";
const DEPENDENCY_CODE: &str = "owner_source_permanent_dead_letter";
const PAGE_FAILURE_CODE: &str = "index.reconciliation_page_failed";
const PRIVATE_MARKER: &str = "private-reconciliation-failure-detail";

type TestResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

#[derive(Clone)]
struct CountedFailingSource {
    calls: Arc<AtomicUsize>,
}

#[async_trait]
impl IndexSource for CountedFailingSource {
    async fn scan(
        &self,
        _request: IndexSourceScanRequest,
    ) -> Result<IndexSourcePage, IndexSourceFailure> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Err(IndexSourceFailure::permanent(DEPENDENCY_CODE)
            .expect("fixture dependency code must be valid"))
    }

    async fn load(
        &self,
        request: IndexSourceLoadRequest,
    ) -> Result<IndexSourceLoadBatch, IndexSourceFailure> {
        Ok(IndexSourceLoadBatch::new(&request, Vec::new()).expect("empty targeted load"))
    }
}

struct TestDatabase {
    control: DatabaseConnection,
    database_url: String,
    schema_name: String,
    tenant_id: Uuid,
}

impl TestDatabase {
    async fn setup() -> TestResult<Option<Self>> {
        let Some(database_url) = database_url() else {
            eprintln!(
                "{DATABASE_ENV} is not set to a PostgreSQL URL; skipping reconciliation dead-letter admission harness"
            );
            return Ok(None);
        };
        let control = connect(&database_url).await?;
        let schema_name = format!(
            "rustok_index_reconciliation_dead_letter_{}",
            Uuid::new_v4().simple()
        );
        control
            .execute_unprepared(&format!(r#"CREATE SCHEMA "{schema_name}""#))
            .await?;

        let tenant_id = Uuid::new_v4();
        let db = scoped_connection(&database_url, &schema_name).await?;
        db.execute_unprepared("CREATE TABLE tenants (id UUID NOT NULL PRIMARY KEY)")
            .await?;
        db.execute_raw(Statement::from_sql_and_values(
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
        }))
    }

    async fn connection(&self) -> TestResult<DatabaseConnection> {
        scoped_connection(&self.database_url, &self.schema_name).await
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

#[derive(Debug)]
struct FailedJobEvidence {
    job_id: Uuid,
    state: String,
    attempt_count: i64,
    completed_passes: i64,
    pages_processed: i64,
    last_error_code: String,
    last_error_details: JsonValue,
    lease_released: bool,
    completed: bool,
}

#[tokio::test]
async fn failed_reconciliation_scope_blocks_new_jobs_without_exposing_details() -> TestResult<()> {
    let Some(database) = TestDatabase::setup().await? else {
        return Ok(());
    };
    let calls = Arc::new(AtomicUsize::new(0));
    let first_runner = runner(database.connection().await?, calls.clone());

    let first_outcome = first_runner
        .run(request(database.tenant_id, "dead-letter-worker-a"))
        .await
        .expect("permanent source failure must terminalize through a typed outcome");
    assert_eq!(
        first_outcome.status(),
        IndexReconciliationRunStatus::FailedPermanent
    );
    assert_eq!(first_outcome.attempt_count(), Some(1));
    assert_eq!(first_outcome.retry_after(), None);
    assert_eq!(first_outcome.next_attempt(), None);
    assert_eq!(first_outcome.pages_processed(), 0);
    assert_eq!(calls.load(Ordering::SeqCst), 1);

    let evidence_db = database.connection().await?;
    let failed = read_failed_job(&evidence_db, database.tenant_id).await?;
    assert_eq!(first_outcome.job_id(), Some(failed.job_id));
    assert_eq!(failed.state, "failed");
    assert_eq!(failed.attempt_count, 1);
    assert_eq!(failed.completed_passes, 0);
    assert_eq!(failed.pages_processed, 0);
    assert_eq!(failed.last_error_code, PAGE_FAILURE_CODE);
    assert_eq!(
        failed.last_error_details,
        json!({
            "contract": "index_reconciliation_run_failure_v1",
            "dependency_code": DEPENDENCY_CODE,
            "retryable": false,
        })
    );
    assert!(failed.lease_released);
    assert!(failed.completed);
    assert_eq!(count(&evidence_db, "index_jobs").await?, 1);
    assert_eq!(count(&evidence_db, "index_entities").await?, 0);
    assert_eq!(count(&evidence_db, "index_inbox").await?, 0);

    replace_private_failure_details(&evidence_db, database.tenant_id, failed.job_id).await?;

    let second_runner = runner(database.connection().await?, calls.clone());
    let blocked = second_runner
        .run(request(database.tenant_id, "dead-letter-worker-b"))
        .await
        .expect_err("failed reconciliation scope must remain blocked");
    let debug = format!("{blocked:?}");
    let display = blocked.to_string();
    match blocked {
        IndexReconciliationRunError::DeadLettered {
            job_id,
            attempt_count,
            error_code,
        } => {
            assert_eq!(job_id, failed.job_id);
            assert_eq!(attempt_count, 1);
            assert_eq!(error_code.as_deref(), Some(PAGE_FAILURE_CODE));
        }
        other => panic!("unexpected blocked reconciliation error: {other:?}"),
    }
    assert!(!debug.contains(PRIVATE_MARKER));
    assert!(!debug.contains(DEPENDENCY_CODE));
    assert!(!display.contains(PRIVATE_MARKER));
    assert!(!display.contains(DEPENDENCY_CODE));
    assert_eq!(calls.load(Ordering::SeqCst), 1);

    let retained = read_failed_job(&evidence_db, database.tenant_id).await?;
    assert_eq!(retained.job_id, failed.job_id);
    assert_eq!(retained.state, "failed");
    assert_eq!(retained.attempt_count, 1);
    assert_eq!(retained.last_error_code, PAGE_FAILURE_CODE);
    assert_eq!(
        retained.last_error_details,
        json!({ "private": PRIVATE_MARKER })
    );
    assert!(retained.lease_released);
    assert!(retained.completed);
    assert_eq!(count(&evidence_db, "index_jobs").await?, 1);
    assert_eq!(count(&evidence_db, "index_entities").await?, 0);
    assert_eq!(count(&evidence_db, "index_inbox").await?, 0);

    database.cleanup().await
}

fn runner(db: DatabaseConnection, calls: Arc<AtomicUsize>) -> PostgresIndexReconciliationRunner {
    let schema = schema();
    let mut schema_catalog = IndexSchemaSourceCatalog::new();
    schema_catalog
        .register(MODULE_NAME, schema.clone())
        .expect("fixture schema source must register");

    let mut source_catalog = IndexSourceCatalog::new();
    source_catalog
        .register(
            MODULE_NAME,
            SOURCE_NAME,
            [schema.reference.clone()],
            CountedFailingSource { calls },
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

fn request(tenant_id: Uuid, worker_id: &str) -> IndexReconciliationRunRequest {
    IndexReconciliationRunRequest::new(
        tenant_id,
        schema_ref(),
        worker_id,
        1,
        1,
        1,
        1,
        Duration::from_secs(60),
    )
    .expect("fixture reconciliation request must be valid")
}

fn schema_ref() -> SchemaRef {
    SchemaRef {
        module: ModuleName::new(MODULE_NAME).unwrap(),
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

async fn persist_schema(db: &DatabaseConnection, tenant_id: Uuid) -> TestResult<()> {
    let schema = schema();
    let fingerprint = schema.fingerprint()?.to_string();
    let schema_json = serde_json::to_value(&schema)?;
    db.execute_raw(Statement::from_sql_and_values(
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

async fn read_failed_job(
    db: &DatabaseConnection,
    tenant_id: Uuid,
) -> TestResult<FailedJobEvidence> {
    let row = db
        .query_one_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT job_id, state, attempt_count::bigint AS attempt_count_value, (cursor->>'completed_passes')::bigint AS completed_passes, (cursor->>'pages_processed')::bigint AS pages_processed, last_error_code, last_error_details, (lease_owner IS NULL AND lease_expires_at IS NULL) AS lease_released, (completed_at IS NOT NULL) AS completed FROM index_jobs WHERE tenant_id = $1 AND kind = 'reconcile' AND state = 'failed' ORDER BY created_at DESC LIMIT 1",
            vec![tenant_id.into()],
        ))
        .await?
        .ok_or_else(|| std::io::Error::other("failed reconciliation job is missing"))?;
    Ok(FailedJobEvidence {
        job_id: row.try_get("", "job_id")?,
        state: row.try_get("", "state")?,
        attempt_count: row.try_get("", "attempt_count_value")?,
        completed_passes: row.try_get("", "completed_passes")?,
        pages_processed: row.try_get("", "pages_processed")?,
        last_error_code: row.try_get("", "last_error_code")?,
        last_error_details: row.try_get("", "last_error_details")?,
        lease_released: row.try_get("", "lease_released")?,
        completed: row.try_get("", "completed")?,
    })
}

async fn replace_private_failure_details(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    job_id: Uuid,
) -> TestResult<()> {
    let updated = db
        .execute_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "UPDATE index_jobs SET last_error_details = $3 WHERE tenant_id = $1 AND job_id = $2 AND kind = 'reconcile' AND state = 'failed'",
            vec![
                tenant_id.into(),
                job_id.into(),
                SqlValue::Json(Some(Box::new(json!({ "private": PRIVATE_MARKER })))),
            ],
        ))
        .await?;
    if updated.rows_affected() != 1 {
        return Err(
            std::io::Error::other("failed reconciliation details update lost scope").into(),
        );
    }
    Ok(())
}

async fn count(db: &DatabaseConnection, table: &str) -> TestResult<i64> {
    let sql = match table {
        "index_jobs" => "SELECT COUNT(*)::bigint AS value FROM index_jobs WHERE kind = 'reconcile'",
        "index_entities" => "SELECT COUNT(*)::bigint AS value FROM index_entities",
        "index_inbox" => "SELECT COUNT(*)::bigint AS value FROM index_inbox",
        _ => panic!("unsupported fixture table"),
    };
    let row: QueryResult = db
        .query_one_raw(Statement::from_string(DbBackend::Postgres, sql.to_owned()))
        .await?
        .ok_or_else(|| std::io::Error::other("count query returned no row"))?;
    Ok(row.try_get("", "value")?)
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
