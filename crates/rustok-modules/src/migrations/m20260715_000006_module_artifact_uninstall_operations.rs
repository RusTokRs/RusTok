use sea_orm::{ConnectionTrait, DbBackend, Statement};
use sea_orm_migration::prelude::*;

/// An uninstall removes a scoped selection but deliberately retains immutable
/// admission and rollback evidence. The operation is the durable source for
/// runtime exclusion and later retention-based CAS reclamation.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let statements: &[&str] = match manager.get_database_backend() {
            DbBackend::Postgres => &[
                "CREATE TABLE module_artifact_uninstall_operations (\
                    operation_id UUID PRIMARY KEY,\
                    installation_id UUID NOT NULL UNIQUE REFERENCES module_artifact_installations(installation_id),\
                    expected_revision BIGINT NOT NULL CHECK (expected_revision > 0),\
                    actor_id UUID NOT NULL,\
                    reason TEXT NOT NULL CHECK (length(trim(reason)) > 0),\
                    idempotency_key UUID NOT NULL UNIQUE,\
                    committed_at TIMESTAMPTZ NOT NULL\
                )",
                "ALTER TABLE module_artifact_uninstall_operations ENABLE ROW LEVEL SECURITY",
                "CREATE POLICY module_artifact_uninstall_operations_scope ON module_artifact_uninstall_operations \
                    USING (EXISTS (SELECT 1 FROM module_artifact_installations installation \
                        WHERE installation.installation_id = module_artifact_uninstall_operations.installation_id \
                        AND (installation.scope_kind = 'platform' OR installation.tenant_id::text = current_setting('rustok.tenant_id', true))))",
            ],
            DbBackend::Sqlite => &["CREATE TABLE module_artifact_uninstall_operations (\
                    operation_id TEXT PRIMARY KEY NOT NULL,\
                    installation_id TEXT NOT NULL UNIQUE REFERENCES module_artifact_installations(installation_id),\
                    expected_revision INTEGER NOT NULL CHECK (expected_revision > 0),\
                    actor_id TEXT NOT NULL,\
                    reason TEXT NOT NULL CHECK (length(trim(reason)) > 0),\
                    idempotency_key TEXT NOT NULL UNIQUE,\
                    committed_at TEXT NOT NULL\
                )"],
            backend => {
                return Err(DbErr::Migration(format!(
                    "module artifact uninstall-operation migration does not support database backend {backend:?}"
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
            .execute_unprepared("DROP TABLE module_artifact_uninstall_operations")
            .await
            .map(|_| ())
    }
}
