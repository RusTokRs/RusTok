use sea_orm::{ConnectionTrait, Statement};
use sea_orm_migration::prelude::*;

use crate::backend::SupportedBackend;

/// Persists the owner clock watermark for each effective immutable Schedule
/// binding. Slot records remain the execution evidence; this table only
/// prevents a restart from silently forgetting a materialization window.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let backend = SupportedBackend::new(manager.get_database_backend())?;
        let statements: &[&str] = match backend {
            SupportedBackend::Postgres => &[
                "CREATE TABLE module_artifact_schedule_cursors (\
                    tenant_id UUID NOT NULL,\
                    installation_id UUID NOT NULL REFERENCES module_artifact_installations(installation_id),\
                    binding_id TEXT NOT NULL CHECK (length(trim(binding_id)) BETWEEN 1 AND 128),\
                    schedule_digest TEXT NOT NULL CHECK (length(schedule_digest) = 71),\
                    materialized_through TIMESTAMPTZ NOT NULL,\
                    updated_at TIMESTAMPTZ NOT NULL,\
                    PRIMARY KEY (tenant_id, installation_id, binding_id)\
                )",
                "ALTER TABLE module_artifact_schedule_cursors ENABLE ROW LEVEL SECURITY",
                "CREATE POLICY module_artifact_schedule_cursors_scope \
                 ON module_artifact_schedule_cursors \
                 USING (tenant_id::text = current_setting('rustok.tenant_id', true)) \
                 WITH CHECK (tenant_id::text = current_setting('rustok.tenant_id', true))",
            ],
            SupportedBackend::Sqlite => &["CREATE TABLE module_artifact_schedule_cursors (\
                    tenant_id TEXT NOT NULL,\
                    installation_id TEXT NOT NULL REFERENCES module_artifact_installations(installation_id),\
                    binding_id TEXT NOT NULL CHECK (length(trim(binding_id)) BETWEEN 1 AND 128),\
                    schedule_digest TEXT NOT NULL CHECK (length(schedule_digest) = 71),\
                    materialized_through TEXT NOT NULL,\
                    updated_at TEXT NOT NULL,\
                    PRIMARY KEY (tenant_id, installation_id, binding_id)\
                )"],
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
        manager
            .get_connection()
            .execute_unprepared("DROP TABLE module_artifact_schedule_cursors")
            .await
            .map(|_| ())
    }
}
