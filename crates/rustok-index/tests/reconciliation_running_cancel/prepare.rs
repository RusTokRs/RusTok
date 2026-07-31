use rustok_core::MigrationSource;
use rustok_index::IndexModule;
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, Statement, Value as SqlValue};
use sea_orm_migration::SchemaManager;
use uuid::Uuid;

use super::{connection::TestResult, schema::schema};

pub async fn prepare(db: &DatabaseConnection, tenant_id: Uuid) -> TestResult<()> {
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
