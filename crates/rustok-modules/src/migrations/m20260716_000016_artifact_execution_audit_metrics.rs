use sea_orm::{ConnectionTrait, DbBackend, Statement};
use sea_orm_migration::prelude::*;

/// Adds runtime-observable queue and capability-call metrics to artifact audit.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let statements: &[&str] = match manager.get_database_backend() {
            DbBackend::Postgres => &[
                "ALTER TABLE module_artifact_execution_audit \
                 ADD COLUMN queue_time_ms BIGINT NULL CHECK (queue_time_ms >= 0)",
                "ALTER TABLE module_artifact_execution_audit \
                 ADD COLUMN capability_calls BIGINT NULL CHECK (capability_calls >= 0)",
            ],
            DbBackend::Sqlite => &[
                "ALTER TABLE module_artifact_execution_audit \
                 ADD COLUMN queue_time_ms INTEGER NULL CHECK (queue_time_ms >= 0)",
                "ALTER TABLE module_artifact_execution_audit \
                 ADD COLUMN capability_calls INTEGER NULL CHECK (capability_calls >= 0)",
            ],
            backend => {
                return Err(DbErr::Migration(format!(
                    "artifact execution-audit metrics migration does not support database backend {backend:?}"
                )));
            }
        };
        for statement in statements {
            manager
                .get_connection()
                .execute(Statement::from_string(
                    manager.get_database_backend(),
                    (*statement).to_string(),
                ))
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
