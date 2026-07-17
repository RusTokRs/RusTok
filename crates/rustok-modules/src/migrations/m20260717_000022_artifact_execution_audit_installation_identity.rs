use sea_orm::{ConnectionTrait, DbBackend, Statement};
use sea_orm_migration::prelude::*;

/// Retains the exact admitted installation identity in newly written execution
/// evidence. The column remains nullable only because older redacted rows have
/// no recoverable installation identity; the current observer always writes it.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let statements: &[&str] = match manager.get_database_backend() {
            DbBackend::Postgres => &[
                "ALTER TABLE module_artifact_execution_audit \
                 ADD COLUMN installation_id UUID NULL",
                "CREATE INDEX module_artifact_execution_audit_installation_idx \
                 ON module_artifact_execution_audit (installation_id, started_at DESC)",
            ],
            DbBackend::Sqlite => &[
                "ALTER TABLE module_artifact_execution_audit \
                 ADD COLUMN installation_id TEXT NULL",
                "CREATE INDEX module_artifact_execution_audit_installation_idx \
                 ON module_artifact_execution_audit (installation_id, started_at DESC)",
            ],
            backend => {
                return Err(DbErr::Migration(format!(
                    "artifact execution-audit identity migration does not support database backend {backend:?}"
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
            "DROP INDEX module_artifact_execution_audit_installation_idx",
            "ALTER TABLE module_artifact_execution_audit DROP COLUMN installation_id",
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
