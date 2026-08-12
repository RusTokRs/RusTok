use std::{env, error::Error};

use rustok_core::MigrationSource;
use rustok_index::{
    EntityName, IndexModule, LocaleKey, ModuleName, SchemaRef, SchemaVersion,
    infrastructure::postgres::{
        IndexDriftDigestFindingRequest, IndexDriftFindingScope, IndexDriftFindingSeverity,
        IndexDriftFindingWriteOutcome, PostgresIndexDriftFindingWriter,
    },
};
use sea_orm::{
    ConnectOptions, ConnectionTrait, Database, DatabaseConnection, DbBackend, QueryResult,
    Statement,
};
use sea_orm_migration::SchemaManager;
use serde_json::{Value as JsonValue, json};
use uuid::Uuid;

const DATABASE_ENV: &str = "RUSTOK_INDEX_TEST_DATABASE_URL";
const CHECK_NAME: &str = "source_index_digest_mismatch";
const DETAILS_CONTRACT: &str = "index_drift_digest_finding_v1";
const DIGEST_BYTES: usize = 64;

type TestResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

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
                "{DATABASE_ENV} is not set to a PostgreSQL URL; skipping drift-finding writer harness"
            );
            return Ok(None);
        };
        let control = connect(&database_url).await?;
        let schema_name = format!(
            "rustok_index_drift_finding_writer_{}",
            Uuid::new_v4().simple()
        );
        control
            .execute_unprepared(&format!(r#"CREATE SCHEMA "{schema_name}""#))
            .await?;

        let tenant_id = Uuid::new_v4();
        let db = scoped_connection(&database_url, &schema_name).await?;
        db.execute_unprepared("CREATE TABLE tenants (id UUID NOT NULL PRIMARY KEY)")
            .await?;
        insert_tenant(&db, tenant_id).await?;

        let manager = SchemaManager::new(&db);
        for migration in IndexModule.migrations() {
            migration.up(&manager).await?;
        }

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
struct FindingEvidence {
    finding_id: Uuid,
    finding_key: String,
    check_name: String,
    severity: String,
    state: String,
    scope_kind: String,
    module_name: String,
    entity_name: String,
    schema_version: i64,
    entity_id: Uuid,
    locale_key: String,
    expected_digest: String,
    actual_digest: String,
    details: JsonValue,
    closed: bool,
}

#[tokio::test]
async fn writer_serializes_identity_and_preserves_lifecycle_on_postgres() -> TestResult<()> {
    let Some(database) = TestDatabase::setup().await? else {
        return Ok(());
    };
    let scope = entity_scope();
    let first_request = request(database.tenant_id, scope.clone(), 'b');

    let writer_a = PostgresIndexDriftFindingWriter::new(database.connection().await?);
    let writer_b = PostgresIndexDriftFindingWriter::new(database.connection().await?);
    let (first, second) = tokio::join!(
        writer_a.record_digest_mismatch(&first_request),
        writer_b.record_digest_mismatch(&first_request),
    );
    let first = first?;
    let second = second?;
    assert!(matches!(
        (&first, &second),
        (
            IndexDriftFindingWriteOutcome::Created { .. },
            IndexDriftFindingWriteOutcome::Refreshed { .. }
        ) | (
            IndexDriftFindingWriteOutcome::Refreshed { .. },
            IndexDriftFindingWriteOutcome::Created { .. }
        )
    ));
    assert_eq!(first.finding_id(), second.finding_id());
    assert_eq!(first.finding_key(), second.finding_key());

    let evidence_db = database.connection().await?;
    let created = read_finding(&evidence_db, database.tenant_id).await?;
    assert_eq!(created.finding_id, first.finding_id());
    assert_eq!(created.finding_key, first.finding_key());
    assert_eq!(created.check_name, CHECK_NAME);
    assert_eq!(created.severity, "error");
    assert_eq!(created.state, "open");
    assert_eq!(created.scope_kind, "entity");
    assert_eq!(created.module_name, "rustok-product");
    assert_eq!(created.entity_name, "product");
    assert_eq!(created.schema_version, 2);
    assert_eq!(created.entity_id, scope_entity_id(&scope));
    assert_eq!(created.locale_key, "en-US");
    assert_eq!(created.expected_digest, "a".repeat(DIGEST_BYTES));
    assert_eq!(created.actual_digest, "b".repeat(DIGEST_BYTES));
    assert_eq!(created.details, json!({ "contract": DETAILS_CONTRACT }));
    assert!(!created.closed);
    assert_eq!(count_findings(&evidence_db, database.tenant_id).await?, 1);

    let refreshed = PostgresIndexDriftFindingWriter::new(database.connection().await?)
        .record_digest_mismatch(&request(database.tenant_id, scope.clone(), 'c'))
        .await?;
    assert!(matches!(
        refreshed,
        IndexDriftFindingWriteOutcome::Refreshed { .. }
    ));
    assert_eq!(refreshed.finding_id(), created.finding_id);
    let refreshed_row = read_finding(&evidence_db, database.tenant_id).await?;
    assert_eq!(refreshed_row.actual_digest, "c".repeat(DIGEST_BYTES));
    assert_eq!(refreshed_row.state, "open");

    set_state(
        &evidence_db,
        database.tenant_id,
        created.finding_id,
        "resolved",
    )
    .await?;
    let reopened = PostgresIndexDriftFindingWriter::new(database.connection().await?)
        .record_digest_mismatch(&request(database.tenant_id, scope.clone(), 'd'))
        .await?;
    assert!(matches!(
        reopened,
        IndexDriftFindingWriteOutcome::Reopened { .. }
    ));
    assert_eq!(reopened.finding_id(), created.finding_id);
    let reopened_row = read_finding(&evidence_db, database.tenant_id).await?;
    assert_eq!(reopened_row.state, "open");
    assert!(!reopened_row.closed);
    assert_eq!(reopened_row.actual_digest, "d".repeat(DIGEST_BYTES));

    set_state(
        &evidence_db,
        database.tenant_id,
        created.finding_id,
        "ignored",
    )
    .await?;
    let suppressed = PostgresIndexDriftFindingWriter::new(database.connection().await?)
        .record_digest_mismatch(&request(database.tenant_id, scope.clone(), 'e'))
        .await?;
    assert!(matches!(
        suppressed,
        IndexDriftFindingWriteOutcome::Suppressed { .. }
    ));
    assert_eq!(suppressed.finding_id(), created.finding_id);
    let ignored_row = read_finding(&evidence_db, database.tenant_id).await?;
    assert_eq!(ignored_row.state, "ignored");
    assert!(ignored_row.closed);
    assert_eq!(ignored_row.actual_digest, "e".repeat(DIGEST_BYTES));
    assert_eq!(count_findings(&evidence_db, database.tenant_id).await?, 1);

    let other_tenant = Uuid::new_v4();
    insert_tenant(&evidence_db, other_tenant).await?;
    let other = PostgresIndexDriftFindingWriter::new(database.connection().await?)
        .record_digest_mismatch(&request(other_tenant, scope, 'b'))
        .await?;
    assert!(matches!(
        other,
        IndexDriftFindingWriteOutcome::Created { .. }
    ));
    assert_ne!(other.finding_key(), created.finding_key);
    assert_eq!(count_findings(&evidence_db, other_tenant).await?, 1);
    assert_eq!(count_all_findings(&evidence_db).await?, 2);

    database.cleanup().await
}

fn entity_scope() -> IndexDriftFindingScope {
    IndexDriftFindingScope::Entity {
        schema: SchemaRef {
            module: ModuleName::new("rustok-product").unwrap(),
            entity: EntityName::new("product").unwrap(),
            version: SchemaVersion::new(2),
        },
        entity_id: Uuid::new_v4(),
        locale: LocaleKey::new("en-US").unwrap(),
    }
}

fn scope_entity_id(scope: &IndexDriftFindingScope) -> Uuid {
    match scope {
        IndexDriftFindingScope::Entity { entity_id, .. } => *entity_id,
        _ => panic!("fixture scope must be entity-scoped"),
    }
}

fn request(
    tenant_id: Uuid,
    scope: IndexDriftFindingScope,
    actual: char,
) -> IndexDriftDigestFindingRequest {
    IndexDriftDigestFindingRequest::new(
        tenant_id,
        CHECK_NAME,
        IndexDriftFindingSeverity::Error,
        scope,
        "a".repeat(DIGEST_BYTES),
        actual.to_string().repeat(DIGEST_BYTES),
    )
    .expect("fixture mismatch request must be valid")
}

async fn insert_tenant(db: &DatabaseConnection, tenant_id: Uuid) -> TestResult<()> {
    db.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "INSERT INTO tenants (id) VALUES ($1)",
        vec![tenant_id.into()],
    ))
    .await?;
    Ok(())
}

async fn read_finding(db: &DatabaseConnection, tenant_id: Uuid) -> TestResult<FindingEvidence> {
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT finding_id, finding_key, check_name, severity, state, scope_kind, module_name, entity_name, schema_version::bigint AS schema_version_value, entity_id, locale_key, expected_digest, actual_digest, details, (closed_at IS NOT NULL) AS closed FROM index_consistency_findings WHERE tenant_id = $1 ORDER BY first_detected_at, finding_id LIMIT 1",
            vec![tenant_id.into()],
        ))
        .await?
        .ok_or_else(|| std::io::Error::other("drift finding row is missing"))?;
    finding_evidence(row)
}

fn finding_evidence(row: QueryResult) -> TestResult<FindingEvidence> {
    Ok(FindingEvidence {
        finding_id: row.try_get("", "finding_id")?,
        finding_key: row.try_get("", "finding_key")?,
        check_name: row.try_get("", "check_name")?,
        severity: row.try_get("", "severity")?,
        state: row.try_get("", "state")?,
        scope_kind: row.try_get("", "scope_kind")?,
        module_name: row.try_get("", "module_name")?,
        entity_name: row.try_get("", "entity_name")?,
        schema_version: row.try_get("", "schema_version_value")?,
        entity_id: row.try_get("", "entity_id")?,
        locale_key: row.try_get("", "locale_key")?,
        expected_digest: row.try_get("", "expected_digest")?,
        actual_digest: row.try_get("", "actual_digest")?,
        details: row.try_get("", "details")?,
        closed: row.try_get("", "closed")?,
    })
}

async fn set_state(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    finding_id: Uuid,
    state: &str,
) -> TestResult<()> {
    let updated = db
        .execute(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "UPDATE index_consistency_findings SET state = $3, closed_at = CURRENT_TIMESTAMP WHERE tenant_id = $1 AND finding_id = $2",
            vec![tenant_id.into(), finding_id.into(), state.to_owned().into()],
        ))
        .await?;
    if updated.rows_affected() != 1 {
        return Err(std::io::Error::other("drift finding lifecycle update lost scope").into());
    }
    Ok(())
}

async fn count_findings(db: &DatabaseConnection, tenant_id: Uuid) -> TestResult<i64> {
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT COUNT(*)::bigint AS value FROM index_consistency_findings WHERE tenant_id = $1",
            vec![tenant_id.into()],
        ))
        .await?
        .ok_or_else(|| std::io::Error::other("tenant finding count returned no row"))?;
    Ok(row.try_get("", "value")?)
}

async fn count_all_findings(db: &DatabaseConnection) -> TestResult<i64> {
    let row = db
        .query_one(Statement::from_string(
            DbBackend::Postgres,
            "SELECT COUNT(*)::bigint AS value FROM index_consistency_findings".to_owned(),
        ))
        .await?
        .ok_or_else(|| std::io::Error::other("finding count returned no row"))?;
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
