use sea_orm::{ConnectionTrait, Statement};
use sea_orm_migration::prelude::*;

use crate::backend::SupportedBackend;

/// Adds the durable predecessor link required for a later atomic rollback
/// operation. The link is deliberately nullable: a module's first admitted
/// installation has no predecessor.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let backend = SupportedBackend::new(manager.get_database_backend())?;
        let statement = match backend {
            SupportedBackend::Postgres => {
                "ALTER TABLE module_artifact_installations \
                ADD COLUMN previous_installation_id UUID NULL \
                REFERENCES module_artifact_installations(installation_id)"
            }
            SupportedBackend::Sqlite => {
                "ALTER TABLE module_artifact_installations \
                ADD COLUMN previous_installation_id TEXT NULL \
                REFERENCES module_artifact_installations(installation_id)"
            }
        };
        manager
            .get_connection()
            .execute_raw(Statement::from_string(backend.db_backend(), statement.to_string()))
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let backend = SupportedBackend::new(manager.get_database_backend())?;
        let statement = match backend {
            SupportedBackend::Postgres => {
                "ALTER TABLE module_artifact_installations \
                DROP COLUMN previous_installation_id"
            }
            SupportedBackend::Sqlite => {
                "ALTER TABLE module_artifact_installations DROP COLUMN previous_installation_id"
            }
        };
        manager
            .get_connection()
            .execute_raw(Statement::from_string(backend.db_backend(), statement.to_string()))
            .await?;
        Ok(())
    }
}
