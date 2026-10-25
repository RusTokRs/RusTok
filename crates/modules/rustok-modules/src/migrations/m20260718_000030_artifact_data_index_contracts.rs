use sea_orm::{ConnectionTrait, Statement};
use sea_orm_migration::prelude::*;

use crate::backend::SupportedBackend;

/// Binds an artifact data namespace to the immutable logical index declaration
/// that materialized its projection rows. Reusing a data-contract revision for
/// a different declaration fails closed instead of returning incomplete data.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let backend = SupportedBackend::new(manager.get_database_backend())?;
        let statements: &[&str] = match backend {
            SupportedBackend::Postgres => &[
                "CREATE TABLE module_artifact_data_index_contracts (\
                    tenant_id UUID NOT NULL,\
                    data_owner_id UUID NOT NULL,\
                    namespace_instance_id UUID NOT NULL,\
                    contract_digest TEXT NOT NULL CHECK (contract_digest ~ '^sha256:[0-9a-f]{64}$'),\
                    bound_at TIMESTAMPTZ NOT NULL,\
                    PRIMARY KEY (tenant_id, data_owner_id, namespace_instance_id)\
                )",
                "ALTER TABLE module_artifact_data_index_contracts ENABLE ROW LEVEL SECURITY",
                "CREATE POLICY module_artifact_data_index_contracts_scope ON module_artifact_data_index_contracts \
                 USING (tenant_id::text = current_setting('rustok.tenant_id', true)) \
                 WITH CHECK (tenant_id::text = current_setting('rustok.tenant_id', true))",
            ],
            SupportedBackend::Sqlite => &["CREATE TABLE module_artifact_data_index_contracts (\
                    tenant_id TEXT NOT NULL,\
                    data_owner_id TEXT NOT NULL,\
                    namespace_instance_id TEXT NOT NULL,\
                    contract_digest TEXT NOT NULL CHECK (length(contract_digest) = 71 AND substr(contract_digest, 1, 7) = 'sha256:' AND substr(contract_digest, 8) NOT GLOB '*[^0-9a-f]*'),\
                    bound_at TEXT NOT NULL,\
                    PRIMARY KEY (tenant_id, data_owner_id, namespace_instance_id)\
                )"],
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
        super::protect_artifact_data_namespace_writes(
            manager,
            "module_artifact_data_index_contracts",
        )
        .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared("DROP TABLE module_artifact_data_index_contracts")
            .await?;
        super::drop_artifact_data_namespace_write_guard(
            manager,
            "module_artifact_data_index_contracts",
        )
        .await?;
        Ok(())
    }
}
