use sea_orm::{ConnectionTrait, Statement};
use sea_orm_migration::prelude::*;

use crate::backend::SupportedBackend;

/// Drops `module_production_operations` (audit remediation, Phase E
/// follow-up).
///
/// The table was the operation journal of the parallel
/// `dynamic_lifecycle` service. That service is deleted: work-generation
/// creation/retirement moved into the canonical admission/uninstall
/// owners in `installation.rs`, and the journal had no readers. The
/// canonical operation history of record is the module operation store
/// plus the domain outbox, so the table is removed instead of being left
/// as an orphaned second ledger.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let backend = SupportedBackend::new(manager.get_database_backend())?;
        match backend {
            SupportedBackend::Postgres | SupportedBackend::Sqlite => {}
        }
        manager
            .get_connection()
            .execute_raw(Statement::from_string(
                backend.db_backend(),
                "DROP TABLE IF EXISTS module_production_operations",
            ))
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let backend = SupportedBackend::new(manager.get_database_backend())?;
        let statements: &[_] = match backend {
            SupportedBackend::Postgres => &[
                "CREATE TABLE module_production_operations (\
                    operation_id UUID PRIMARY KEY,\
                    installation_id UUID NOT NULL REFERENCES module_artifact_installations(installation_id) ON DELETE RESTRICT,\
                    action TEXT NOT NULL CHECK (action IN ('install', 'enable', 'update', 'disable', 'remove', 'uninstall', 'rollback', 'dynamic_artifact_data_purge', 'dynamic_artifact_settings_purge')),\
                    scope_kind TEXT NOT NULL CHECK (scope_kind IN ('platform', 'tenant')),\
                    tenant_id UUID NULL,\
                    module_slug TEXT NOT NULL CHECK (length(trim(module_slug)) > 0),\
                    release_digest TEXT NOT NULL CHECK (release_digest ~ '^sha256:[0-9a-f]{64}$'),\
                    predecessor_installation_id UUID NULL,\
                    data_owner_id UUID NOT NULL,\
                    settings_instance_id UUID NOT NULL,\
                    work_generation BIGINT NOT NULL CHECK (work_generation > 0),\
                    status TEXT NOT NULL CHECK (status IN ('in_progress', 'converged', 'rolled_back', 'failed')),\
                    actor_id UUID NOT NULL,\
                    idempotency_key UUID NOT NULL UNIQUE,\
                    trace_id TEXT NOT NULL CHECK (length(trim(trace_id)) BETWEEN 1 AND 512),\
                    correlation_id UUID NOT NULL,\
                    created_at TIMESTAMPTZ NOT NULL,\
                    updated_at TIMESTAMPTZ NOT NULL\
                )",
                "CREATE INDEX idx_production_operations_slug \
                 ON module_production_operations (module_slug, tenant_id, work_generation)",
                "ALTER TABLE module_production_operations ENABLE ROW LEVEL SECURITY",
                "CREATE POLICY module_production_operations_scope \
                 ON module_production_operations \
                 USING (scope_kind = 'platform' OR tenant_id::text = current_setting('rustok.tenant_id', true)) \
                 WITH CHECK (scope_kind = 'platform' OR tenant_id::text = current_setting('rustok.tenant_id', true))",
            ],
            SupportedBackend::Sqlite => &[
                "CREATE TABLE module_production_operations (\
                    operation_id TEXT PRIMARY KEY NOT NULL,\
                    installation_id TEXT NOT NULL REFERENCES module_artifact_installations(installation_id) ON DELETE RESTRICT,\
                    action TEXT NOT NULL CHECK (action IN ('install', 'enable', 'update', 'disable', 'remove', 'uninstall', 'rollback', 'dynamic_artifact_data_purge', 'dynamic_artifact_settings_purge')),\
                    scope_kind TEXT NOT NULL CHECK (scope_kind IN ('platform', 'tenant')),\
                    tenant_id TEXT NULL,\
                    module_slug TEXT NOT NULL CHECK (length(trim(module_slug)) > 0),\
                    release_digest TEXT NOT NULL CHECK (length(release_digest) = 71),\
                    predecessor_installation_id TEXT NULL,\
                    data_owner_id TEXT NOT NULL,\
                    settings_instance_id TEXT NOT NULL,\
                    work_generation INTEGER NOT NULL CHECK (work_generation > 0),\
                    status TEXT NOT NULL CHECK (status IN ('in_progress', 'converged', 'rolled_back', 'failed')),\
                    actor_id TEXT NOT NULL,\
                    idempotency_key TEXT NOT NULL UNIQUE,\
                    trace_id TEXT NOT NULL CHECK (length(trim(trace_id)) BETWEEN 1 AND 512),\
                    correlation_id TEXT NOT NULL,\
                    created_at TEXT NOT NULL,\
                    updated_at TEXT NOT NULL\
                )",
                "CREATE INDEX idx_production_operations_slug \
                 ON module_production_operations (module_slug, tenant_id, work_generation)",
            ],
        };
        for statement in statements {
            manager
                .get_connection()
                .execute_raw(Statement::from_string(
                    backend.db_backend(),
                    (*statement).to_string(),
                ))
                .await?;
        }
        Ok(())
    }
}
