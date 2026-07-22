use sea_orm::{ConnectionTrait, DbBackend, Statement};
use sea_orm_migration::prelude::*;

/// Stores the complete immutable rollback command fingerprint and response so
/// an idempotent retry can replay after the source installation has changed.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let statements: &[&str] = match manager.get_database_backend() {
            DbBackend::Postgres => &[
                "ALTER TABLE module_artifact_rollback_operations \
                 ADD COLUMN target_capability_grant_revision BIGINT NULL \
                 CHECK (target_capability_grant_revision > 0)",
                "ALTER TABLE module_artifact_rollback_operations \
                 ADD COLUMN migration_rollback_mode TEXT NULL \
                 CHECK (migration_rollback_mode IN ('reversible', 'compensating', 'prohibited'))",
                "ALTER TABLE module_artifact_rollback_operations \
                 ADD COLUMN source_revision BIGINT NULL CHECK (source_revision > 0)",
                "ALTER TABLE module_artifact_rollback_operations \
                 ADD COLUMN target_revision BIGINT NULL CHECK (target_revision > 0)",
            ],
            DbBackend::Sqlite => &[
                "ALTER TABLE module_artifact_rollback_operations \
                 ADD COLUMN target_capability_grant_revision INTEGER NULL \
                 CHECK (target_capability_grant_revision > 0)",
                "ALTER TABLE module_artifact_rollback_operations \
                 ADD COLUMN migration_rollback_mode TEXT NULL \
                 CHECK (migration_rollback_mode IN ('reversible', 'compensating', 'prohibited'))",
                "ALTER TABLE module_artifact_rollback_operations \
                 ADD COLUMN source_revision INTEGER NULL CHECK (source_revision > 0)",
                "ALTER TABLE module_artifact_rollback_operations \
                 ADD COLUMN target_revision INTEGER NULL CHECK (target_revision > 0)",
            ],
            backend => {
                return Err(DbErr::Migration(format!(
                    "artifact rollback idempotency migration does not support database backend {backend:?}"
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
        for column in [
            "target_revision",
            "source_revision",
            "migration_rollback_mode",
            "target_capability_grant_revision",
        ] {
            manager
                .get_connection()
                .execute(Statement::from_string(
                    manager.get_database_backend(),
                    format!("ALTER TABLE module_artifact_rollback_operations DROP COLUMN {column}"),
                ))
                .await?;
        }
        Ok(())
    }
}
