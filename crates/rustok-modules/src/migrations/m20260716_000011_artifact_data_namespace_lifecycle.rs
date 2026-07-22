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
                "CREATE TABLE module_artifact_data_namespaces (\
                    tenant_id UUID NOT NULL,\
                    module_slug TEXT NOT NULL,\
                    data_contract_revision BIGINT NOT NULL CHECK (data_contract_revision > 0),\
                    namespace_revision BIGINT NOT NULL CHECK (namespace_revision > 0),\
                    purged_at TIMESTAMPTZ NULL,\
                    created_at TIMESTAMPTZ NOT NULL,\
                    updated_at TIMESTAMPTZ NOT NULL,\
                    PRIMARY KEY (tenant_id, module_slug, data_contract_revision)\
                )",
                "ALTER TABLE module_artifact_data_namespaces ENABLE ROW LEVEL SECURITY",
                "CREATE POLICY module_artifact_data_namespaces_scope ON module_artifact_data_namespaces \
                 USING (tenant_id::text = current_setting('rustok.tenant_id', true)) \
                 WITH CHECK (tenant_id::text = current_setting('rustok.tenant_id', true))",
                "CREATE TABLE module_artifact_data_purge_operations (\
                    tenant_id UUID NOT NULL,\
                    module_slug TEXT NOT NULL,\
                    data_contract_revision BIGINT NOT NULL CHECK (data_contract_revision > 0),\
                    idempotency_key UUID NOT NULL,\
                    expected_namespace_revision BIGINT NOT NULL CHECK (expected_namespace_revision > 0),\
                    namespace_revision BIGINT NOT NULL CHECK (namespace_revision > 0),\
                    actor_id UUID NOT NULL,\
                    reason TEXT NOT NULL CHECK (length(trim(reason)) > 0),\
                    purged_records BIGINT NOT NULL CHECK (purged_records >= 0),\
                    completed_at TIMESTAMPTZ NOT NULL,\
                    PRIMARY KEY (tenant_id, module_slug, data_contract_revision, idempotency_key)\
                )",
                "ALTER TABLE module_artifact_data_purge_operations ENABLE ROW LEVEL SECURITY",
                "CREATE POLICY module_artifact_data_purge_operations_scope ON module_artifact_data_purge_operations \
                 USING (tenant_id::text = current_setting('rustok.tenant_id', true)) \
                 WITH CHECK (tenant_id::text = current_setting('rustok.tenant_id', true))",
            ],
            DbBackend::Sqlite => &[
                "CREATE TABLE module_artifact_data_namespaces (\
                    tenant_id TEXT NOT NULL,\
                    module_slug TEXT NOT NULL,\
                    data_contract_revision INTEGER NOT NULL CHECK (data_contract_revision > 0),\
                    namespace_revision INTEGER NOT NULL CHECK (namespace_revision > 0),\
                    purged_at TEXT NULL,\
                    created_at TEXT NOT NULL,\
                    updated_at TEXT NOT NULL,\
                    PRIMARY KEY (tenant_id, module_slug, data_contract_revision)\
                )",
                "CREATE TABLE module_artifact_data_purge_operations (\
                    tenant_id TEXT NOT NULL,\
                    module_slug TEXT NOT NULL,\
                    data_contract_revision INTEGER NOT NULL CHECK (data_contract_revision > 0),\
                    idempotency_key TEXT NOT NULL,\
                    expected_namespace_revision INTEGER NOT NULL CHECK (expected_namespace_revision > 0),\
                    namespace_revision INTEGER NOT NULL CHECK (namespace_revision > 0),\
                    actor_id TEXT NOT NULL,\
                    reason TEXT NOT NULL CHECK (length(trim(reason)) > 0),\
                    purged_records INTEGER NOT NULL CHECK (purged_records >= 0),\
                    completed_at TEXT NOT NULL,\
                    PRIMARY KEY (tenant_id, module_slug, data_contract_revision, idempotency_key)\
                )",
            ],
            backend => {
                return Err(DbErr::Migration(format!(
                    "artifact data namespace lifecycle migration does not support database backend {backend:?}"
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
        for table in [
            "module_artifact_data_purge_operations",
            "module_artifact_data_namespaces",
        ] {
            manager
                .get_connection()
                .execute_unprepared(&format!("DROP TABLE {table}"))
                .await?;
        }
        Ok(())
    }
}
