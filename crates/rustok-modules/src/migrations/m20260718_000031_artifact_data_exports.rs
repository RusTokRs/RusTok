use sea_orm::{ConnectionTrait, DbBackend, Statement};
use sea_orm_migration::prelude::*;

/// Durable, redacted audit facts for bounded owner-only structured-data export
/// pages. Exported values remain outside the audit record and outbox payload.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let statements: &[&str] = match manager.get_database_backend() {
            DbBackend::Postgres => &[
                "CREATE TABLE module_artifact_data_exports (\
                    export_id UUID PRIMARY KEY,\
                    tenant_id UUID NOT NULL,\
                    module_slug TEXT NOT NULL,\
                    data_contract_revision BIGINT NOT NULL CHECK (data_contract_revision > 0),\
                    namespace_revision BIGINT NOT NULL CHECK (namespace_revision > 0),\
                    actor_id UUID NOT NULL,\
                    prefix TEXT NOT NULL CHECK (length(prefix) BETWEEN 2 AND 256),\
                    after_key TEXT NULL CHECK (after_key IS NULL OR length(after_key) BETWEEN 1 AND 256),\
                    page_limit BIGINT NOT NULL CHECK (page_limit BETWEEN 1 AND 100),\
                    reason TEXT NOT NULL CHECK (length(trim(reason)) BETWEEN 1 AND 2000),\
                    exported_records BIGINT NOT NULL CHECK (exported_records BETWEEN 0 AND 100),\
                    completed_at TIMESTAMPTZ NOT NULL\
                )",
                "CREATE INDEX module_artifact_data_exports_scope_idx \
                 ON module_artifact_data_exports (tenant_id, module_slug, data_contract_revision, completed_at, export_id)",
                "ALTER TABLE module_artifact_data_exports ENABLE ROW LEVEL SECURITY",
                "CREATE POLICY module_artifact_data_exports_scope ON module_artifact_data_exports \
                 USING (tenant_id::text = current_setting('rustok.tenant_id', true)) \
                 WITH CHECK (tenant_id::text = current_setting('rustok.tenant_id', true))",
            ],
            DbBackend::Sqlite => &[
                "CREATE TABLE module_artifact_data_exports (\
                    export_id TEXT PRIMARY KEY,\
                    tenant_id TEXT NOT NULL,\
                    module_slug TEXT NOT NULL,\
                    data_contract_revision INTEGER NOT NULL CHECK (data_contract_revision > 0),\
                    namespace_revision INTEGER NOT NULL CHECK (namespace_revision > 0),\
                    actor_id TEXT NOT NULL,\
                    prefix TEXT NOT NULL CHECK (length(prefix) BETWEEN 2 AND 256),\
                    after_key TEXT NULL CHECK (after_key IS NULL OR length(after_key) BETWEEN 1 AND 256),\
                    page_limit INTEGER NOT NULL CHECK (page_limit BETWEEN 1 AND 100),\
                    reason TEXT NOT NULL CHECK (length(trim(reason)) BETWEEN 1 AND 2000),\
                    exported_records INTEGER NOT NULL CHECK (exported_records BETWEEN 0 AND 100),\
                    completed_at TEXT NOT NULL\
                )",
                "CREATE INDEX module_artifact_data_exports_scope_idx \
                 ON module_artifact_data_exports (tenant_id, module_slug, data_contract_revision, completed_at, export_id)",
            ],
            backend => {
                return Err(DbErr::Migration(format!(
                    "artifact data export migration does not support database backend {backend:?}"
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
            .execute_unprepared("DROP TABLE module_artifact_data_exports")
            .await?;
        Ok(())
    }
}
