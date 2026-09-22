use sea_orm::{ConnectionTrait, DbBackend, Statement};
use sea_orm_migration::prelude::*;

/// Adds host-owned lifecycle and destructive-operation history around the
/// structured-value namespace. Data writes cannot recreate a purged namespace.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let statements: &[&str] = match manager.get_database_backend() {
            DbBackend::Postgres => &[
                "CREATE TABLE module_artifact_data_purge_operations (\
                    tenant_id UUID NOT NULL,\
                    installation_id UUID NOT NULL REFERENCES module_artifact_installations(installation_id),\
                    data_owner_id UUID NOT NULL,\
                    namespace_instance_id UUID NOT NULL,\
                    policy_revision BIGINT NOT NULL CHECK (policy_revision > 0),\
                    idempotency_key UUID NOT NULL,\
                    expected_namespace_revision BIGINT NOT NULL CHECK (expected_namespace_revision > 0),\
                    namespace_revision BIGINT NOT NULL CHECK (namespace_revision > 0),\
                    actor_id UUID NOT NULL,\
                    trace_id TEXT NOT NULL CHECK (length(trim(trace_id)) > 0 AND length(trace_id) <= 512),\
                    correlation_id UUID NOT NULL,\
                    reason TEXT NOT NULL CHECK (length(trim(reason)) > 0),\
                    purged_records BIGINT NOT NULL CHECK (purged_records >= 0),\
                    completed_at TIMESTAMPTZ NOT NULL,\
                    PRIMARY KEY (tenant_id, installation_id, idempotency_key)\
                )",
                "ALTER TABLE module_artifact_data_purge_operations ENABLE ROW LEVEL SECURITY",
                "CREATE POLICY module_artifact_data_purge_operations_scope ON module_artifact_data_purge_operations \
                 USING (tenant_id::text = current_setting('rustok.tenant_id', true)) \
                 WITH CHECK (tenant_id::text = current_setting('rustok.tenant_id', true))",
            ],
            DbBackend::Sqlite => &["CREATE TABLE module_artifact_data_purge_operations (\
                    tenant_id TEXT NOT NULL,\
                    installation_id TEXT NOT NULL REFERENCES module_artifact_installations(installation_id),\
                    data_owner_id TEXT NOT NULL,\
                    namespace_instance_id TEXT NOT NULL,\
                    policy_revision INTEGER NOT NULL CHECK (policy_revision > 0),\
                    idempotency_key TEXT NOT NULL,\
                    expected_namespace_revision INTEGER NOT NULL CHECK (expected_namespace_revision > 0),\
                    namespace_revision INTEGER NOT NULL CHECK (namespace_revision > 0),\
                    actor_id TEXT NOT NULL,\
                    trace_id TEXT NOT NULL CHECK (length(trim(trace_id)) > 0 AND length(trace_id) <= 512),\
                    correlation_id TEXT NOT NULL,\
                    reason TEXT NOT NULL CHECK (length(trim(reason)) > 0),\
                    purged_records INTEGER NOT NULL CHECK (purged_records >= 0),\
                    completed_at TEXT NOT NULL,\
                    PRIMARY KEY (tenant_id, installation_id, idempotency_key)\
                )"],
            backend => {
                return Err(DbErr::Migration(format!(
                    "artifact data namespace lifecycle migration does not support database backend {backend:?}"
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
        for table in ["module_artifact_data_purge_operations"] {
            manager
                .get_connection()
                .execute_unprepared(&format!("DROP TABLE {table}"))
                .await?;
        }
        Ok(())
    }
}
