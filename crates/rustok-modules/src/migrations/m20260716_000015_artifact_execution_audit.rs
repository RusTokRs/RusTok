use sea_orm::{ConnectionTrait, DbBackend, Statement};
use sea_orm_migration::prelude::*;

/// Persists redacted lifecycle/runtime execution records for admitted artifacts.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let statements: &[&str] = match manager.get_database_backend() {
            DbBackend::Postgres => &[
                "CREATE TABLE module_artifact_execution_audit (\
                    execution_id UUID PRIMARY KEY,\
                    tenant_id UUID NULL,\
                    module_slug TEXT NOT NULL,\
                    module_version TEXT NOT NULL,\
                    artifact_digest TEXT NOT NULL,\
                    executor TEXT NOT NULL,\
                    phase TEXT NOT NULL,\
                    actor_id TEXT NULL,\
                    trace_id TEXT NULL,\
                    status TEXT NOT NULL CHECK (status IN ('started', 'succeeded', 'failed')),\
                    started_at TIMESTAMPTZ NOT NULL,\
                    finished_at TIMESTAMPTZ NULL,\
                    duration_ms BIGINT NULL CHECK (duration_ms >= 0),\
                    instructions_consumed BIGINT NULL CHECK (instructions_consumed >= 0),\
                    peak_memory_bytes BIGINT NULL CHECK (peak_memory_bytes >= 0),\
                    output_bytes BIGINT NULL CHECK (output_bytes >= 0),\
                    error_code TEXT NULL\
                )",
                "ALTER TABLE module_artifact_execution_audit ENABLE ROW LEVEL SECURITY",
                "CREATE POLICY module_artifact_execution_audit_scope ON module_artifact_execution_audit \
                 USING (tenant_id IS NULL OR tenant_id::text = current_setting('rustok.tenant_id', true)) \
                 WITH CHECK (tenant_id IS NULL OR tenant_id::text = current_setting('rustok.tenant_id', true))",
                "CREATE INDEX module_artifact_execution_audit_subject_idx \
                 ON module_artifact_execution_audit (tenant_id, module_slug, started_at DESC)",
            ],
            DbBackend::Sqlite => &[
                "CREATE TABLE module_artifact_execution_audit (\
                    execution_id TEXT PRIMARY KEY,\
                    tenant_id TEXT NULL,\
                    module_slug TEXT NOT NULL,\
                    module_version TEXT NOT NULL,\
                    artifact_digest TEXT NOT NULL,\
                    executor TEXT NOT NULL,\
                    phase TEXT NOT NULL,\
                    actor_id TEXT NULL,\
                    trace_id TEXT NULL,\
                    status TEXT NOT NULL CHECK (status IN ('started', 'succeeded', 'failed')),\
                    started_at TEXT NOT NULL,\
                    finished_at TEXT NULL,\
                    duration_ms INTEGER NULL CHECK (duration_ms >= 0),\
                    instructions_consumed INTEGER NULL CHECK (instructions_consumed >= 0),\
                    peak_memory_bytes INTEGER NULL CHECK (peak_memory_bytes >= 0),\
                    output_bytes INTEGER NULL CHECK (output_bytes >= 0),\
                    error_code TEXT NULL\
                )",
                "CREATE INDEX module_artifact_execution_audit_subject_idx \
                 ON module_artifact_execution_audit (tenant_id, module_slug, started_at DESC)",
            ],
            backend => {
                return Err(DbErr::Migration(format!(
                    "artifact execution-audit migration does not support database backend {backend:?}"
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
        manager
            .get_connection()
            .execute_unprepared("DROP TABLE module_artifact_execution_audit")
            .await
            .map(|_| ())
    }
}
