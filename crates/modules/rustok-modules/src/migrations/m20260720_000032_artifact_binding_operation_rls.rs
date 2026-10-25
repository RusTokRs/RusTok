use sea_orm::{ConnectionTrait, Statement};
use sea_orm_migration::prelude::*;

use crate::backend::SupportedBackend;

/// Adds the missing tenant RLS boundary to durable artifact binding operations.
#[derive(DeriveMigrationName)]
pub struct Migration;

const POLICY: &str = "module_artifact_binding_operations_scope";
const TABLE: &str = "module_artifact_binding_operations";

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Row-level security exists only on PostgreSQL; SQLite is an
        // explicit no-op, anything else is rejected by the validation.
        let backend = SupportedBackend::new(manager.get_database_backend())?;
        if backend != SupportedBackend::Postgres {
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
                .execute_raw(Statement::from_string(backend.db_backend(), statement))
                .await?;
        }
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let backend = SupportedBackend::new(manager.get_database_backend())?;
        if backend != SupportedBackend::Postgres {
            return Ok(());
        }

        for statement in [
            format!("DROP POLICY {POLICY} ON {TABLE}"),
            format!("ALTER TABLE {TABLE} DISABLE ROW LEVEL SECURITY"),
        ] {
            manager
                .get_connection()
                .execute_raw(Statement::from_string(backend.db_backend(), statement))
                .await?;
        }
        Ok(())
    }
}
