use sea_orm::{ConnectionTrait, Statement};
use sea_orm_migration::prelude::*;

use crate::backend::SupportedBackend;

/// Stores the exact toggle result returned to the original caller so an
/// idempotent replay can return operation-time evidence (state, revision,
/// settings) instead of reconstructing the response from whatever the
/// current tenant state happens to be at replay time. Rows committed before
/// this migration keep `NULL` and replay falls back to the documented legacy
/// reconstruction. The payload is the serialized executor result contract;
/// it is stored as text on both backends for an exact round-trip.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let backend = SupportedBackend::new(manager.get_database_backend())?;
        manager
            .get_connection()
            .execute_raw(Statement::from_string(
                backend.db_backend(),
                "ALTER TABLE module_operations ADD COLUMN result_snapshot TEXT".to_string(),
            ))
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let backend = SupportedBackend::new(manager.get_database_backend())?;
        manager
            .get_connection()
            .execute_raw(Statement::from_string(
                backend.db_backend(),
                "ALTER TABLE module_operations DROP COLUMN result_snapshot".to_string(),
            ))
            .await?;
        Ok(())
    }
}
