use rustok_core::MigrationSource;
use rustok_index::IndexModule;
use sea_orm::{
    ConnectionTrait, DatabaseConnection, DbBackend, Statement, Value as SqlValue,
};
use sea_orm_migration::SchemaManager;
use uuid::Uuid;

use super::{
    connection::{TestResult, connect, database_url, scoped_connection},
    schema::schema,
};

pub struct TestDatabase {
    control: DatabaseConnection,
    pub database_url: String,
    pub schema_name: String,
    pub tenant_id: Uuid,
}

impl TestDatabase {
    pub async fn setup() -> TestResult<Option<Self>> {
        let Some(database_url) = database_url() else {
            eprintln!(
                "RUSTOK_INDEX_TEST_DATABASE_URL is not PostgreSQL; skipping reconciliation process restart harness"
            );
            return Ok(None);
        };
        let control = connect(&database_url).await?;
        let schema_name = format!(
            "rustok_index_reconciliation_process_restart_{}",
            Uuid::new_v4().simple()
        );
        control
            .execute_unprepared(&format!(r#"CREATE SCHEMA "{schema_name}""#))
            .await?;
        let tenant_id = Uuid::new_v4();
        let db = scoped_connection(&database_url, &schema_name).await?;
        prepare(&db, tenant_id).await?;
        Ok(Some(Self {
            control,
            database_url,
            schema_name,
            tenant_id,
        }))
    }

    pub async fn connection(&self) -> TestResult<DatabaseConnection> {
        scoped_connection(&self.database_url, &self.schema_name).await
    }

    pub async fn cleanup(self) -> TestResult<()> {
        self.control
            .execute_unprepared(&format!(
                r#"DROP SCHEMA IF EXISTS "{}" CASCADE"#,
                self.schema_name
            ))
            .await?;
        Ok(())
    }
}

async fn prepare(db: &DatabaseConnection, tenant_id: Uuid) -> TestResult<()> {
    db.execute_unprepared("CREATE TABLE tenants (id UUID NOT NULL PRIMARY KEY)")
        .await?;
    db.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "INSERT INTO tenants (id) VALUES ($1)",
        vec![tenant_id.into()],
    ))
    .await?;
    let manager = SchemaManager::new(db);
    for migration in IndexModule.migrations() {
        migration.up(&manager).await?;
    }
    persist_schema(db, tenant_id).await
}

async fn persist_schema(db: &DatabaseConnection, tenant_id: Uuid) -> TestResult<()> {
    let schema = schema();
    db.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "INSERT INTO index_schemas (tenant_id, module_name, entity_name, schema_version, schema_fingerprint, schema_json, status) VALUES ($1, $2, $3, $4, $5, $6, 'active')",
        vec![
            tenant_id.into(),
            schema.reference.module.as_str().to_owned().into(),
            schema.reference.entity.as_str().to_owned().into(),
            i64::from(schema.reference.version.get()).into(),
            schema.fingerprint()?.to_string().into(),
            SqlValue::Json(Some(Box::new(serde_json::to_value(&schema)?))),
        ],
    ))
    .await?;
    Ok(())
}
