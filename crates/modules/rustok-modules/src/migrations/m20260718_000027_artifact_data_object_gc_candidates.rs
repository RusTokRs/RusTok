use sea_orm::{ConnectionTrait, DbBackend, Statement};
use sea_orm_migration::prelude::*;

use crate::backend::SupportedBackend;

/// Durable, tenant-scoped references to private object bytes that are no
/// longer reachable from artifact data metadata. A retention-policy snapshot
/// must approve a candidate before the owner GC service deletes its bytes.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let backend = SupportedBackend::new(manager.get_database_backend())?;
        let statements: &[&str] = match backend {
            SupportedBackend::Postgres => &[
                "CREATE TABLE module_artifact_data_object_gc_candidates (\
                    candidate_id UUID PRIMARY KEY,\
                    tenant_id UUID NOT NULL,\
                    data_owner_id UUID NOT NULL,\
                    namespace_instance_id UUID NOT NULL,\
                    policy_revision BIGINT NOT NULL CHECK (policy_revision > 0),\
                    storage_key TEXT NOT NULL UNIQUE,\
                    queued_at TIMESTAMPTZ NOT NULL\
                )",
                "CREATE INDEX module_artifact_data_object_gc_candidates_tenant_idx \
                 ON module_artifact_data_object_gc_candidates (tenant_id, queued_at, candidate_id)",
                "ALTER TABLE module_artifact_data_object_gc_candidates ENABLE ROW LEVEL SECURITY",
                "CREATE POLICY module_artifact_data_object_gc_candidates_scope \
                 ON module_artifact_data_object_gc_candidates \
                 USING (tenant_id::text = current_setting('rustok.tenant_id', true)) \
                 WITH CHECK (tenant_id::text = current_setting('rustok.tenant_id', true))",
            ],
            SupportedBackend::Sqlite => &[
                "CREATE TABLE module_artifact_data_object_gc_candidates (\
                    candidate_id TEXT PRIMARY KEY,\
                    tenant_id TEXT NOT NULL,\
                    data_owner_id TEXT NOT NULL,\
                    namespace_instance_id TEXT NOT NULL,\
                    policy_revision INTEGER NOT NULL CHECK (policy_revision > 0),\
                    storage_key TEXT NOT NULL UNIQUE,\
                    queued_at TEXT NOT NULL\
                )",
                "CREATE INDEX module_artifact_data_object_gc_candidates_tenant_idx \
                 ON module_artifact_data_object_gc_candidates (tenant_id, queued_at, candidate_id)",
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

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared("DROP TABLE module_artifact_data_object_gc_candidates")
            .await?;
        Ok(())
    }
}
