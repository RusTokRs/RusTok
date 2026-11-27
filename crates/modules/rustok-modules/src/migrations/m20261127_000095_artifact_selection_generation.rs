//! Monotonic serving selection continuity on the permanent activation fence.
use sea_orm::ConnectionTrait;
use sea_orm_migration::prelude::*;

use crate::backend::SupportedBackend;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    fn use_transaction(&self) -> Option<bool> {
        Some(true)
    }

    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let backend = SupportedBackend::new(manager.get_database_backend())?;
        let sql = match backend {
            SupportedBackend::Postgres => {
                "ALTER TABLE module_artifact_activation_locks ADD COLUMN selection_generation BIGINT NOT NULL DEFAULT 0 CHECK (selection_generation >= 0)"
            }
            SupportedBackend::Sqlite => {
                "ALTER TABLE module_artifact_activation_locks ADD COLUMN selection_generation INTEGER NOT NULL DEFAULT 0 CHECK (selection_generation >= 0)"
            }
        };
        manager.get_connection().execute_unprepared(sql).await?;
        Ok(())
    }

    async fn down(&self, _: &SchemaManager) -> Result<(), DbErr> {
        Err(DbErr::Migration(
            "artifact selection continuity cannot be downgraded safely".into(),
        ))
    }
}
