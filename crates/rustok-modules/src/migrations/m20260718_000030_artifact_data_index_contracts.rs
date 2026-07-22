use sea_orm::{ConnectionTrait, DbBackend, Statement};
use sea_orm_migration::prelude::*;

/// Binds an artifact data namespace to the immutable logical index declaration
/// that materialized its projection rows. Reusing a data-contract revision for
/// a different declaration fails closed instead of returning incomplete data.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let statements: &[&str] = match manager.get_database_backend() {
            DbBackend::Postgres => &[
                "CREATE TABLE module_artifact_data_index_contracts (\
                    tenant_id UUID NOT NULL,\
                    module_slug TEXT NOT NULL,\
                    data_contract_revision BIGINT NOT NULL CHECK (data_contract_revision > 0),\
                    contract_digest TEXT NOT NULL CHECK (contract_digest ~ '^sha256:[0-9a-f]{64}$'),\
                    bound_at TIMESTAMPTZ NOT NULL,\
                    PRIMARY KEY (tenant_id, module_slug, data_contract_revision)\
                )",
                "ALTER TABLE module_artifact_data_index_contracts ENABLE ROW LEVEL SECURITY",
                "CREATE POLICY module_artifact_data_index_contracts_scope ON module_artifact_data_index_contracts \
                 USING (tenant_id::text = current_setting('rustok.tenant_id', true)) \
                 WITH CHECK (tenant_id::text = current_setting('rustok.tenant_id', true))",
            ],
            DbBackend::Sqlite => &["CREATE TABLE module_artifact_data_index_contracts (\
                    tenant_id TEXT NOT NULL,\
                    module_slug TEXT NOT NULL,\
                    data_contract_revision INTEGER NOT NULL CHECK (data_contract_revision > 0),\
                    contract_digest TEXT NOT NULL CHECK (length(contract_digest) = 71 AND substr(contract_digest, 1, 7) = 'sha256:' AND substr(contract_digest, 8) NOT GLOB '*[^0-9a-f]*'),\
                    bound_at TEXT NOT NULL,\
                    PRIMARY KEY (tenant_id, module_slug, data_contract_revision)\
                )"],
            backend => {
                return Err(DbErr::Migration(format!(
                    "artifact data index-contract migration does not support database backend {backend:?}"
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
            .execute_unprepared("DROP TABLE module_artifact_data_index_contracts")
            .await?;
        Ok(())
    }
}
