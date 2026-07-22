use sea_orm::{ConnectionTrait, DbBackend, Statement};
use sea_orm_migration::prelude::*;

/// Durable data-migration state belongs to the admitted installation, not to a
/// guest payload. An irreversible checkpoint prevents an unsafe rollback or
/// purge path from pretending that data can be restored automatically.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let statements: &[&str] = match manager.get_database_backend() {
            DbBackend::Postgres => &[
                "ALTER TABLE module_artifact_installations ADD COLUMN migration_checkpoint JSONB NULL",
                "ALTER TABLE module_artifact_installations ADD COLUMN has_irreversible_migration BOOLEAN NOT NULL DEFAULT FALSE",
            ],
            DbBackend::Sqlite => &[
                "ALTER TABLE module_artifact_installations ADD COLUMN migration_checkpoint JSON NULL",
                "ALTER TABLE module_artifact_installations ADD COLUMN has_irreversible_migration INTEGER NOT NULL DEFAULT 0 CHECK (has_irreversible_migration IN (0, 1))",
            ],
            backend => {
                return Err(DbErr::Migration(format!(
                    "artifact migration checkpoint does not support database backend {backend:?}"
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
        for column in ["migration_checkpoint", "has_irreversible_migration"] {
            manager
                .get_connection()
                .execute_unprepared(&format!(
                    "ALTER TABLE module_artifact_installations DROP COLUMN {column}"
                ))
                .await?;
        }
        Ok(())
    }
}
