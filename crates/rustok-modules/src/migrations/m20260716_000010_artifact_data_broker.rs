use sea_orm::{ConnectionTrait, DbBackend, Statement};
use sea_orm_migration::prelude::*;

/// Creates the host-owned structured-value namespace for untrusted artifacts.
/// Guest artifacts address logical keys only; physical tables and storage paths
/// remain outside their contract.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let statements: &[&str] = match manager.get_database_backend() {
            DbBackend::Postgres => &[
                "CREATE TABLE module_artifact_data (\
                    tenant_id UUID NOT NULL,\
                    module_slug TEXT NOT NULL,\
                    data_contract_revision BIGINT NOT NULL CHECK (data_contract_revision > 0),\
                    data_key TEXT NOT NULL CHECK (length(data_key) BETWEEN 1 AND 256),\
                    value JSONB NOT NULL,\
                    revision BIGINT NOT NULL CHECK (revision > 0),\
                    updated_at TIMESTAMPTZ NOT NULL,\
                    PRIMARY KEY (tenant_id, module_slug, data_contract_revision, data_key)\
                )",
                "CREATE INDEX module_artifact_data_namespace_idx ON module_artifact_data (tenant_id, module_slug, data_contract_revision)",
                "ALTER TABLE module_artifact_data ENABLE ROW LEVEL SECURITY",
                "CREATE POLICY module_artifact_data_scope ON module_artifact_data \
                 USING (tenant_id::text = current_setting('rustok.tenant_id', true)) \
                 WITH CHECK (tenant_id::text = current_setting('rustok.tenant_id', true))",
                "CREATE TABLE module_artifact_data_operations (\
                    tenant_id UUID NOT NULL,\
                    module_slug TEXT NOT NULL,\
                    data_contract_revision BIGINT NOT NULL CHECK (data_contract_revision > 0),\
                    idempotency_key UUID NOT NULL,\
                    data_key TEXT NOT NULL CHECK (length(data_key) BETWEEN 1 AND 256),\
                    value JSONB NOT NULL,\
                    expected_revision BIGINT NULL CHECK (expected_revision > 0),\
                    revision BIGINT NOT NULL CHECK (revision > 0),\
                    completed_at TIMESTAMPTZ NOT NULL,\
                    PRIMARY KEY (tenant_id, module_slug, data_contract_revision, idempotency_key)\
                )",
                "ALTER TABLE module_artifact_data_operations ENABLE ROW LEVEL SECURITY",
                "CREATE POLICY module_artifact_data_operations_scope ON module_artifact_data_operations \
                 USING (tenant_id::text = current_setting('rustok.tenant_id', true)) \
                 WITH CHECK (tenant_id::text = current_setting('rustok.tenant_id', true))",
            ],
            DbBackend::Sqlite => &[
                "CREATE TABLE module_artifact_data (\
                    tenant_id TEXT NOT NULL,\
                    module_slug TEXT NOT NULL,\
                    data_contract_revision INTEGER NOT NULL CHECK (data_contract_revision > 0),\
                    data_key TEXT NOT NULL CHECK (length(data_key) BETWEEN 1 AND 256),\
                    value JSON NOT NULL,\
                    revision INTEGER NOT NULL CHECK (revision > 0),\
                    updated_at TEXT NOT NULL,\
                    PRIMARY KEY (tenant_id, module_slug, data_contract_revision, data_key)\
                )",
                "CREATE INDEX module_artifact_data_namespace_idx ON module_artifact_data (tenant_id, module_slug, data_contract_revision)",
                "CREATE TABLE module_artifact_data_operations (\
                    tenant_id TEXT NOT NULL,\
                    module_slug TEXT NOT NULL,\
                    data_contract_revision INTEGER NOT NULL CHECK (data_contract_revision > 0),\
                    idempotency_key TEXT NOT NULL,\
                    data_key TEXT NOT NULL CHECK (length(data_key) BETWEEN 1 AND 256),\
                    value JSON NOT NULL,\
                    expected_revision INTEGER NULL CHECK (expected_revision > 0),\
                    revision INTEGER NOT NULL CHECK (revision > 0),\
                    completed_at TEXT NOT NULL,\
                    PRIMARY KEY (tenant_id, module_slug, data_contract_revision, idempotency_key)\
                )",
            ],
            backend => {
                return Err(DbErr::Migration(format!(
                    "artifact data broker migration does not support database backend {backend:?}"
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
        for table in ["module_artifact_data_operations", "module_artifact_data"] {
            manager
                .get_connection()
                .execute_unprepared(&format!("DROP TABLE {table}"))
                .await?;
        }
        Ok(())
    }
}
