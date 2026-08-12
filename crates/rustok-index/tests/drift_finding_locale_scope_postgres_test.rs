use std::{env, error::Error};

use rustok_core::MigrationSource;
use rustok_index::{
    EntityName, IndexModule, LocaleKey, ModuleName, SchemaRef, SchemaVersion,
    infrastructure::postgres::{
        IndexDriftDigestFindingRequest, IndexDriftFindingScope, IndexDriftFindingSeverity,
        IndexDriftFindingWriteOutcome, PostgresIndexDriftFindingInspector,
        PostgresIndexDriftFindingWriter,
    },
};
use sea_orm::{
    ConnectOptions, ConnectionTrait, Database, DatabaseConnection, DbBackend, Statement,
};
use sea_orm_migration::SchemaManager;
use uuid::Uuid;

const DATABASE_ENV: &str = "RUSTOK_INDEX_TEST_DATABASE_URL";
const CHECK_NAME: &str = "source_index_digest_mismatch";
const DIGEST_BYTES: usize = 64;

type TestResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

#[tokio::test]
async fn migration_writer_and_inspector_support_locale_free_entity_findings() -> TestResult<()> {
    let Some(database_url) = database_url() else {
        eprintln!(
            "{DATABASE_ENV} is not set to a PostgreSQL URL; skipping locale-optional finding harness"
        );
        return Ok(());
    };
    let control = connect(&database_url).await?;
    let schema_name = format!(
        "rustok_index_drift_locale_scope_{}",
        Uuid::new_v4().simple()
    );
    control
        .execute_unprepared(&format!(r#"CREATE SCHEMA "{schema_name}""#))
        .await?;

    let result = run_scenario(&database_url, &schema_name).await;
    let cleanup = control
        .execute_unprepared(&format!(r#"DROP SCHEMA IF EXISTS "{schema_name}" CASCADE"#))
        .await;
    result?;
    cleanup?;
    Ok(())
}

async fn run_scenario(database_url: &str, schema_name: &str) -> TestResult<()> {
    let db = scoped_connection(database_url, schema_name).await?;
    db.execute_unprepared("CREATE TABLE tenants (id UUID NOT NULL PRIMARY KEY)")
        .await?;
    let tenant_id = Uuid::new_v4();
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

    let schema = SchemaRef {
        module: ModuleName::new("rustok-product").unwrap(),
        entity: EntityName::new("product").unwrap(),
        version: SchemaVersion::new(2),
    };
    let entity_id = Uuid::new_v4();
    let locale_request = request(
        tenant_id,
        IndexDriftFindingScope::Entity {
            schema: schema.clone(),
            entity_id,
            locale: LocaleKey::new("en-US").unwrap(),
        },
        'b',
    );
    let no_locale_request = request(
        tenant_id,
        IndexDriftFindingScope::EntityWithoutLocale {
            schema: schema.clone(),
            entity_id,
        },
        'c',
    );
    assert_ne!(
        locale_request.finding_key(),
        no_locale_request.finding_key()
    );

    let writer = PostgresIndexDriftFindingWriter::new(db.clone());
    let locale_created = writer.record_digest_mismatch(&locale_request).await?;
    let no_locale_created = writer.record_digest_mismatch(&no_locale_request).await?;
    assert!(matches!(
        &locale_created,
        IndexDriftFindingWriteOutcome::Created { .. }
    ));
    assert!(matches!(
        &no_locale_created,
        IndexDriftFindingWriteOutcome::Created { .. }
    ));

    let rows = db
        .query_all(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT finding_id, finding_key, locale_key FROM index_consistency_findings WHERE tenant_id = $1 ORDER BY locale_key NULLS FIRST",
            vec![tenant_id.into()],
        ))
        .await?;
    assert_eq!(rows.len(), 2);
    let first_locale: Option<String> = rows[0].try_get("", "locale_key")?;
    let second_locale: Option<String> = rows[1].try_get("", "locale_key")?;
    assert_eq!(first_locale, None);
    assert_eq!(second_locale.as_deref(), Some("en-US"));

    let inspector = PostgresIndexDriftFindingInspector::new(db.clone());
    let no_locale = inspector
        .inspect(tenant_id, no_locale_created.finding_id())
        .await?
        .expect("locale-free finding must remain inspectable");
    assert_eq!(no_locale.finding_key(), no_locale_request.finding_key());
    match no_locale.scope() {
        IndexDriftFindingScope::EntityWithoutLocale {
            schema: inspected_schema,
            entity_id: inspected_entity,
        } => {
            assert_eq!(inspected_schema, &schema);
            assert_eq!(*inspected_entity, entity_id);
        }
        other => panic!("unexpected locale-free scope: {other:?}"),
    }

    let refreshed = writer
        .record_digest_mismatch(&request(
            tenant_id,
            IndexDriftFindingScope::EntityWithoutLocale { schema, entity_id },
            'd',
        ))
        .await?;
    assert!(matches!(
        &refreshed,
        IndexDriftFindingWriteOutcome::Refreshed { .. }
    ));
    assert_eq!(refreshed.finding_id(), no_locale_created.finding_id());
    Ok(())
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
