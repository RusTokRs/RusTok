use sea_orm::{ConnectionTrait, Statement};
use sea_orm_migration::prelude::*;

use crate::backend::SupportedBackend;

/// Adds runtime-observable queue and capability-call metrics to artifact audit.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let backend = SupportedBackend::new(manager.get_database_backend())?;
        let statements: &[&str] = match backend {
            SupportedBackend::Postgres => &[
                "ALTER TABLE module_artifact_execution_audit \
                 ADD COLUMN queue_time_ms BIGINT NULL CHECK (queue_time_ms >= 0)",
                "ALTER TABLE module_artifact_execution_audit \
                 ADD COLUMN capability_calls BIGINT NULL CHECK (capability_calls >= 0)",
            ],
            SupportedBackend::Sqlite => &[
                "ALTER TABLE module_artifact_execution_audit \
                 ADD COLUMN queue_time_ms INTEGER NULL CHECK (queue_time_ms >= 0)",
                "ALTER TABLE module_artifact_execution_audit \
                 ADD COLUMN capability_calls INTEGER NULL CHECK (capability_calls >= 0)",
            ],
        };
        for statement in statements {
            manager
                .get_connection()
                .execute_raw(Statement::from_string(backend.db_backend(), (*statement).to_string()))
                .await?;
        }
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let statements = [
            "ALTER TABLE module_artifact_execution_audit DROP COLUMN capability_calls",
            "ALTER TABLE module_artifact_execution_audit DROP COLUMN queue_time_ms",
        ];
        for statement in statements {
            manager
                .get_connection()
                .execute_unprepared(statement)
                .await?;
        }
        Ok(())
    }
}
