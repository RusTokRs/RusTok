use sea_orm::{ConnectionTrait, Statement};
use sea_orm_migration::prelude::*;

use crate::backend::SupportedBackend;

/// Adds the deterministic owner sequence to policy revision cursors.
///
/// Existing cursors start at sequence zero. That value does not claim to
/// reconstruct pre-migration history; it is the start of the durable sequence
/// epoch in which every later advance or explicit resync increments once.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let backend = SupportedBackend::new(manager.get_database_backend())?;
        let statement = match backend {
            SupportedBackend::Postgres => {
                "ALTER TABLE module_policy_revision_cursors \
                 ADD COLUMN IF NOT EXISTS current_sequence BIGINT NOT NULL DEFAULT 0 \
                 CHECK (current_sequence >= 0)"
            }
            SupportedBackend::Sqlite => {
                "ALTER TABLE module_policy_revision_cursors \
                 ADD COLUMN current_sequence INTEGER NOT NULL DEFAULT 0 \
                 CHECK (current_sequence >= 0)"
            }
        };
        manager
            .get_connection()
            .execute_raw(Statement::from_string(
                backend.db_backend(),
                statement.to_string(),
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
                "ALTER TABLE module_policy_revision_cursors DROP COLUMN current_sequence"
                    .to_string(),
            ))
            .await?;
        Ok(())
    }
}
