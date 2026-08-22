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
                    installation_id UUID NOT NULL,\
                    module_slug TEXT NOT NULL,\
                    module_version TEXT NOT NULL,\
                    artifact_digest TEXT NOT NULL,\
                    executor TEXT NOT NULL,\
                    phase TEXT NOT NULL,\
                    actor_id TEXT NULL,\
                    trace_id TEXT NULL,\
                    binding_id TEXT NULL,\
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
                "CREATE INDEX module_artifact_execution_audit_installation_binding_idx \
                 ON module_artifact_execution_audit (installation_id, binding_id, started_at DESC)",
            ],
            DbBackend::Sqlite => &[
                "CREATE TABLE module_artifact_execution_audit (\
                    execution_id TEXT PRIMARY KEY,\
                    tenant_id TEXT NULL,\
                    installation_id TEXT NOT NULL,\
                    module_slug TEXT NOT NULL,\
                    module_version TEXT NOT NULL,\
                    artifact_digest TEXT NOT NULL,\
                    executor TEXT NOT NULL,\
                    phase TEXT NOT NULL,\
                    actor_id TEXT NULL,\
                    trace_id TEXT NULL,\
                    binding_id TEXT NULL,\
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
                "CREATE INDEX module_artifact_execution_audit_installation_binding_idx \
                 ON module_artifact_execution_audit (installation_id, binding_id, started_at DESC)",
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

#[cfg(test)]
mod tests {
    use sea_orm::{ConnectionTrait, Database};
    use sea_orm_migration::prelude::{MigrationTrait, SchemaManager};

    use super::Migration;

    #[tokio::test]
    async fn sqlite_schema_starts_with_installation_and_binding_identity() {
        let database = Database::connect("sqlite::memory:")
            .await
            .expect("test database");
        Migration
            .up(&SchemaManager::new(&database))
            .await
            .expect("migration");

        database
            .execute_unprepared(
                "INSERT INTO module_artifact_execution_audit \
                 (execution_id, tenant_id, installation_id, module_slug, module_version, \
                  artifact_digest, executor, phase, binding_id, status, started_at) \
                 VALUES \
                 ('a0a26b70-e90f-4c02-9687-17c8b0dcd082', 'd6804a24-1df5-4934-98aa-2a6864f2579c', \
                  '8eac9c37-9c1c-4a02-8b1f-f4f8485ad2a1', 'payments', '1.0.0', \
                  'sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa', \
                  'wasm_component', 'http', 'admin_actions.reconcile', 'started', \
                  '2026-08-22T00:00:00Z')",
            )
            .await
            .expect("canonical audit identity insert");
    }
}
