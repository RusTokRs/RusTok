use sea_orm::{ConnectionTrait, Statement};
use sea_orm_migration::prelude::*;

use crate::backend::SupportedBackend;

/// Persists the owner-selected capability-grant revision independently of an
/// artifact's declared capabilities and the policy revision used to evaluate it.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let backend = SupportedBackend::new(manager.get_database_backend())?;
        let statement = match backend {
            SupportedBackend::Postgres => {
                "ALTER TABLE module_artifact_installations \
                ADD COLUMN capability_grant_revision BIGINT NOT NULL DEFAULT 1 \
                CHECK (capability_grant_revision > 0)"
            }
            SupportedBackend::Sqlite => {
                "ALTER TABLE module_artifact_installations \
                ADD COLUMN capability_grant_revision INTEGER NOT NULL DEFAULT 1 \
                CHECK (capability_grant_revision > 0)"
            }
        };
        manager
            .get_connection()
            .execute_raw(Statement::from_string(backend.db_backend(), statement))
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let backend = SupportedBackend::new(manager.get_database_backend())?;
        let statement = "ALTER TABLE module_artifact_installations \
            DROP COLUMN capability_grant_revision";
        manager
            .get_connection()
            .execute_raw(Statement::from_string(backend.db_backend(), statement))
            .await?;
        Ok(())
    }
}
