use sea_orm::{ConnectionTrait, DbBackend, Statement};
use sea_orm_migration::prelude::*;

/// Durable, tenant-scoped references to private object bytes that are no
/// longer reachable from artifact data metadata. A retention-policy snapshot
/// must approve a candidate before the owner GC service deletes its bytes.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let statements: &[&str] = match manager.get_database_backend() {
            DbBackend::Postgres => &[
                "CREATE TABLE module_artifact_data_object_gc_candidates (\
                    candidate_id UUID PRIMARY KEY,\
                    tenant_id UUID NOT NULL,\
                    module_slug TEXT NOT NULL,\
                    data_contract_revision BIGINT NOT NULL CHECK (data_contract_revision > 0),\
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
            DbBackend::Sqlite => &[
                "CREATE TABLE module_artifact_data_object_gc_candidates (\
                    candidate_id TEXT PRIMARY KEY,\
                    tenant_id TEXT NOT NULL,\
                    module_slug TEXT NOT NULL,\
                    data_contract_revision INTEGER NOT NULL CHECK (data_contract_revision > 0),\
                    policy_revision INTEGER NOT NULL CHECK (policy_revision > 0),\
                    storage_key TEXT NOT NULL UNIQUE,\
                    queued_at TEXT NOT NULL\
                )",
                "CREATE INDEX module_artifact_data_object_gc_candidates_tenant_idx \
                 ON module_artifact_data_object_gc_candidates (tenant_id, queued_at, candidate_id)",
            ],
            backend => return Err(DbErr::Migration(format!(
                "artifact data object GC candidate migration does not support database backend {backend:?}"
            ))),
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
            .execute_unprepared("DROP TABLE module_artifact_data_object_gc_candidates")
            .await?;
        Ok(())
    }
}
