//! Separate deployment readiness provenance from tenant serving policy.

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
        // NULL is an explicit legacy marker. Never reinterpret historical
        // observations as owner-derived deployment evidence.
        let sql = match backend {
            SupportedBackend::Postgres => "ALTER TABLE module_artifact_node_reconciliations ADD COLUMN deployment_intent_revision TEXT NULL CHECK (deployment_intent_revision IS NULL OR deployment_intent_revision ~ '^sha256:[0-9a-f]{64}$')",
            SupportedBackend::Sqlite => "ALTER TABLE module_artifact_node_reconciliations ADD COLUMN deployment_intent_revision TEXT NULL CHECK (deployment_intent_revision IS NULL OR (length(deployment_intent_revision) = 71 AND substr(deployment_intent_revision, 1, 7) = 'sha256:' AND substr(deployment_intent_revision, 8) NOT GLOB '*[^0-9a-f]*'))",
        };
        manager.get_connection().execute_unprepared(sql).await?;
        Ok(())
    }

    async fn down(&self, _: &SchemaManager) -> Result<(), DbErr> {
        Err(DbErr::Migration(
            "deployment readiness provenance cannot be downgraded safely".into(),
        ))
    }
}
