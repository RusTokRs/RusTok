use sea_orm::{ConnectionTrait, DbBackend, Statement};
use sea_orm_migration::prelude::*;

/// Persists exact replay results for revision-guarded structured-data deletes.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let statements: &[&str] = match manager.get_database_backend() {
            DbBackend::Postgres => &[
                "CREATE TABLE module_artifact_data_delete_operations (\
                    tenant_id UUID NOT NULL,\
                    module_slug TEXT NOT NULL,\
                    data_contract_revision BIGINT NOT NULL CHECK (data_contract_revision > 0),\
                    policy_revision BIGINT NOT NULL CHECK (policy_revision > 0),\
                    idempotency_key UUID NOT NULL,\
                    data_key TEXT NOT NULL CHECK (length(data_key) BETWEEN 1 AND 256),\
                    expected_revision BIGINT NOT NULL CHECK (expected_revision > 0),\
                    deleted_revision BIGINT NOT NULL CHECK (deleted_revision > 0),\
                    completed_at TIMESTAMPTZ NOT NULL,\
                    CHECK (expected_revision = deleted_revision),\
                    PRIMARY KEY (tenant_id, module_slug, data_contract_revision, policy_revision, idempotency_key)\
                )",
                "ALTER TABLE module_artifact_data_delete_operations ENABLE ROW LEVEL SECURITY",
                "CREATE POLICY module_artifact_data_delete_operations_scope \
                 ON module_artifact_data_delete_operations \
                 USING (tenant_id::text = current_setting('rustok.tenant_id', true)) \
                 WITH CHECK (tenant_id::text = current_setting('rustok.tenant_id', true))",
            ],
            DbBackend::Sqlite => &["CREATE TABLE module_artifact_data_delete_operations (\
                    tenant_id TEXT NOT NULL,\
                    module_slug TEXT NOT NULL,\
                    data_contract_revision INTEGER NOT NULL CHECK (data_contract_revision > 0),\
                    policy_revision INTEGER NOT NULL CHECK (policy_revision > 0),\
                    idempotency_key TEXT NOT NULL,\
                    data_key TEXT NOT NULL CHECK (length(data_key) BETWEEN 1 AND 256),\
                    expected_revision INTEGER NOT NULL CHECK (expected_revision > 0),\
                    deleted_revision INTEGER NOT NULL CHECK (deleted_revision > 0),\
                    completed_at TEXT NOT NULL,\
                    CHECK (expected_revision = deleted_revision),\
                    PRIMARY KEY (tenant_id, module_slug, data_contract_revision, policy_revision, idempotency_key)\
                )"],
            backend => {
                return Err(DbErr::Migration(format!(
                    "artifact data record deletion migration does not support database backend {backend:?}"
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
            .execute_unprepared("DROP TABLE module_artifact_data_delete_operations")
            .await?;
        Ok(())
    }
}
