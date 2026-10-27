use sea_orm::{ConnectionTrait, Statement};
use sea_orm_migration::prelude::*;

use crate::backend::SupportedBackend;

/// Makes the lifecycle journal's tenant-scoped idempotency identity a
/// database invariant and supports bounded stale-operation scans.
///
/// Historical rows may have a null idempotency key, so uniqueness is partial.
/// Existing duplicate non-null keys deliberately make index creation fail:
/// choosing a winner would silently rewrite command history, and an operator
/// must reconcile those rows before this migration can proceed.
#[derive(DeriveMigrationName)]
pub struct Migration;

const UNIQUE_INDEX: &str = "uq_module_operations_tenant_idempotency_key";
const STALE_INDEX: &str = "idx_module_operations_status_updated_at";

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let backend = SupportedBackend::new(manager.get_database_backend())?;
        let connection = manager.get_connection();

        connection
            .execute_raw(Statement::from_string(
                backend.db_backend(),
                format!(
                    "CREATE UNIQUE INDEX IF NOT EXISTS {UNIQUE_INDEX} \
                     ON module_operations (tenant_id, idempotency_key) \
                     WHERE idempotency_key IS NOT NULL"
                ),
            ))
            .await?;
        connection
            .execute_raw(Statement::from_string(
                backend.db_backend(),
                format!(
                    "CREATE INDEX IF NOT EXISTS {STALE_INDEX} \
                     ON module_operations (status, updated_at)"
                ),
            ))
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let backend = SupportedBackend::new(manager.get_database_backend())?;
        let connection = manager.get_connection();

        connection
            .execute_raw(Statement::from_string(
                backend.db_backend(),
                format!("DROP INDEX IF EXISTS {STALE_INDEX}"),
            ))
            .await?;
        connection
            .execute_raw(Statement::from_string(
                backend.db_backend(),
                format!("DROP INDEX IF EXISTS {UNIQUE_INDEX}"),
            ))
            .await?;
        Ok(())
    }
}
