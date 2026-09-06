use sea_orm::{ConnectionTrait, DbBackend, Statement};
use sea_orm_migration::prelude::*;

/// Persists crash-safe page request intents, receipts, and idempotency for
/// cross-revision maintenance-only artifact data copy operations.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let statements: &[&str] = match manager.get_database_backend() {
            DbBackend::Postgres => &[
                "CREATE TABLE module_artifact_data_copy_operations (\
                    operation_id UUID PRIMARY KEY,\
                    tenant_id UUID NOT NULL,\
                    module_slug TEXT NOT NULL,\
                    source_contract_revision BIGINT NOT NULL CHECK (source_contract_revision > 0),\
                    target_contract_revision BIGINT NOT NULL CHECK (target_contract_revision > 0),\
                    page_cursor TEXT NULL,\
                    page_digest TEXT NOT NULL CHECK (page_digest ~ '^sha256:[0-9a-f]{64}$'),\
                    items_count BIGINT NOT NULL CHECK (items_count >= 0),\
                    status TEXT NOT NULL CHECK (status IN ('intent', 'committed', 'failed')),\
                    actor_id UUID NOT NULL,\
                    trace_id TEXT NOT NULL CHECK (length(trim(trace_id)) BETWEEN 1 AND 512),\
                    correlation_id UUID NOT NULL,\
                    idempotency_key UUID NOT NULL,\
                    reason TEXT NOT NULL CHECK (length(trim(reason)) BETWEEN 1 AND 2000),\
                    created_at TIMESTAMPTZ NOT NULL,\
                    committed_at TIMESTAMPTZ NULL,\
                    UNIQUE (tenant_id, module_slug, source_contract_revision, target_contract_revision, idempotency_key)\
                )",
                "CREATE INDEX idx_artifact_data_copy_ops_scope \
                 ON module_artifact_data_copy_operations (tenant_id, module_slug, source_contract_revision, target_contract_revision, status)",
                "ALTER TABLE module_artifact_data_copy_operations ENABLE ROW LEVEL SECURITY",
                "CREATE POLICY module_artifact_data_copy_operations_scope \
                 ON module_artifact_data_copy_operations \
                 USING (tenant_id::text = current_setting('rustok.tenant_id', true)) \
                 WITH CHECK (tenant_id::text = current_setting('rustok.tenant_id', true))",
            ],
            DbBackend::Sqlite => &[
                "CREATE TABLE module_artifact_data_copy_operations (\
                    operation_id TEXT PRIMARY KEY NOT NULL,\
                    tenant_id TEXT NOT NULL,\
                    module_slug TEXT NOT NULL,\
                    source_contract_revision INTEGER NOT NULL CHECK (source_contract_revision > 0),\
                    target_contract_revision INTEGER NOT NULL CHECK (target_contract_revision > 0),\
                    page_cursor TEXT NULL,\
                    page_digest TEXT NOT NULL CHECK (length(page_digest) = 71),\
                    items_count INTEGER NOT NULL CHECK (items_count >= 0),\
                    status TEXT NOT NULL CHECK (status IN ('intent', 'committed', 'failed')),\
                    actor_id TEXT NOT NULL,\
                    trace_id TEXT NOT NULL CHECK (length(trim(trace_id)) BETWEEN 1 AND 512),\
                    correlation_id TEXT NOT NULL,\
                    idempotency_key TEXT NOT NULL,\
                    reason TEXT NOT NULL CHECK (length(trim(reason)) BETWEEN 1 AND 2000),\
                    created_at TEXT NOT NULL,\
                    committed_at TEXT NULL,\
                    UNIQUE (tenant_id, module_slug, source_contract_revision, target_contract_revision, idempotency_key)\
                )",
                "CREATE INDEX idx_artifact_data_copy_ops_scope \
                 ON module_artifact_data_copy_operations (tenant_id, module_slug, source_contract_revision, target_contract_revision, status)",
            ],
            backend => {
                return Err(DbErr::Migration(format!(
                    "artifact data copy operations migration does not support database backend {backend:?}"
                )));
            }
        };

        for statement in statements {
            manager
                .get_connection()
                .execute_raw(Statement::from_string(
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
            .execute_unprepared("DROP TABLE IF EXISTS module_artifact_data_copy_operations")
            .await?;
        Ok(())
    }
}
