use sea_orm::{ConnectionTrait, DbBackend, Statement};
use sea_orm_migration::prelude::*;

/// Owner-materialized scalar projections for descriptor-declared artifact data
/// indexes. Artifact code never chooses a physical database index or submits a
/// JSON/SQL expression.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let statements: &[&str] = match manager.get_database_backend() {
            DbBackend::Postgres => &[
                "CREATE TABLE module_artifact_data_indexes (\
                    tenant_id UUID NOT NULL,\
                    module_slug TEXT NOT NULL,\
                    data_contract_revision BIGINT NOT NULL CHECK (data_contract_revision > 0),\
                    index_name TEXT NOT NULL CHECK (length(index_name) BETWEEN 1 AND 64),\
                    index_value TEXT NOT NULL CHECK (length(index_value) BETWEEN 1 AND 256),\
                    data_key TEXT NOT NULL CHECK (length(data_key) BETWEEN 1 AND 256),\
                    PRIMARY KEY (tenant_id, module_slug, data_contract_revision, index_name, index_value, data_key)\
                )",
                "CREATE INDEX module_artifact_data_indexes_lookup_idx \
                 ON module_artifact_data_indexes (tenant_id, module_slug, data_contract_revision, index_name, index_value, data_key)",
                "ALTER TABLE module_artifact_data_indexes ENABLE ROW LEVEL SECURITY",
                "CREATE POLICY module_artifact_data_indexes_scope ON module_artifact_data_indexes \
                 USING (tenant_id::text = current_setting('rustok.tenant_id', true)) \
                 WITH CHECK (tenant_id::text = current_setting('rustok.tenant_id', true))",
            ],
            DbBackend::Sqlite => &[
                "CREATE TABLE module_artifact_data_indexes (\
                    tenant_id TEXT NOT NULL,\
                    module_slug TEXT NOT NULL,\
                    data_contract_revision INTEGER NOT NULL CHECK (data_contract_revision > 0),\
                    index_name TEXT NOT NULL CHECK (length(index_name) BETWEEN 1 AND 64),\
                    index_value TEXT NOT NULL CHECK (length(index_value) BETWEEN 1 AND 256),\
                    data_key TEXT NOT NULL CHECK (length(data_key) BETWEEN 1 AND 256),\
                    PRIMARY KEY (tenant_id, module_slug, data_contract_revision, index_name, index_value, data_key)\
                )",
                "CREATE INDEX module_artifact_data_indexes_lookup_idx \
                 ON module_artifact_data_indexes (tenant_id, module_slug, data_contract_revision, index_name, index_value, data_key)",
            ],
            backend => return Err(DbErr::Migration(format!(
                "artifact data index migration does not support database backend {backend:?}"
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
            .execute_unprepared("DROP TABLE module_artifact_data_indexes")
            .await?;
        Ok(())
    }
}
