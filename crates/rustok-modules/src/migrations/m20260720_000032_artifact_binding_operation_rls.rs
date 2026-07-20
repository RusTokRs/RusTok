use sea_orm::{ConnectionTrait, DbBackend, Statement};
use sea_orm_migration::prelude::*;

/// Adds the missing tenant RLS boundary to durable artifact binding operations.
#[derive(DeriveMigrationName)]
pub struct Migration;

const POLICY: &str = "module_artifact_binding_operations_scope";
const TABLE: &str = "module_artifact_binding_operations";

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if manager.get_database_backend() != DbBackend::Postgres {
            return Ok(());
        }

        for statement in [
            format!("ALTER TABLE {TABLE} ENABLE ROW LEVEL SECURITY"),
            format!(
                "CREATE POLICY {POLICY} ON {TABLE} \
                 USING (tenant_id::text = current_setting('rustok.tenant_id', true)) \
                 WITH CHECK (tenant_id::text = current_setting('rustok.tenant_id', true))"
            ),
        ] {
            manager
                .get_connection()
                .execute(Statement::from_string(DbBackend::Postgres, statement))
                .await?;
        }
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if manager.get_database_backend() != DbBackend::Postgres {
            return Ok(());
        }

        for statement in [
            format!("DROP POLICY {POLICY} ON {TABLE}"),
            format!("ALTER TABLE {TABLE} DISABLE ROW LEVEL SECURITY"),
        ] {
            manager
                .get_connection()
                .execute(Statement::from_string(DbBackend::Postgres, statement))
                .await?;
        }
        Ok(())
    }
}
