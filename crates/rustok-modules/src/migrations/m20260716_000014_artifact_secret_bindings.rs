use sea_orm::{ConnectionTrait, DbBackend, Statement};
use sea_orm_migration::prelude::*;

/// Persists tenant/module-scoped logical secret bindings separately from
/// structured artifact data. Rows contain resolver references only, never
/// resolved secret values.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let statements: &[&str] = match manager.get_database_backend() {
            DbBackend::Postgres => &[
                "CREATE TABLE module_artifact_secret_bindings (\
                    tenant_id UUID NOT NULL,\
                    module_slug TEXT NOT NULL,\
                    data_contract_revision BIGINT NOT NULL CHECK (data_contract_revision > 0),\
                    reference_name TEXT NOT NULL CHECK (length(reference_name) BETWEEN 1 AND 96),\
                    resolver_alias TEXT NOT NULL CHECK (length(resolver_alias) BETWEEN 1 AND 96),\
                    resolver_key TEXT NOT NULL CHECK (length(resolver_key) BETWEEN 1 AND 512),\
                    revision BIGINT NOT NULL CHECK (revision > 0),\
                    actor_id UUID NOT NULL,\
                    reason TEXT NOT NULL CHECK (length(trim(reason)) > 0),\
                    created_at TIMESTAMPTZ NOT NULL,\
                    updated_at TIMESTAMPTZ NOT NULL,\
                    PRIMARY KEY (tenant_id, module_slug, data_contract_revision, reference_name)\
                )",
                "ALTER TABLE module_artifact_secret_bindings ENABLE ROW LEVEL SECURITY",
                "CREATE POLICY module_artifact_secret_bindings_scope ON module_artifact_secret_bindings \
                 USING (tenant_id::text = current_setting('rustok.tenant_id', true)) \
                 WITH CHECK (tenant_id::text = current_setting('rustok.tenant_id', true))",
                "CREATE TABLE module_artifact_secret_binding_operations (\
                    tenant_id UUID NOT NULL,\
                    module_slug TEXT NOT NULL,\
                    data_contract_revision BIGINT NOT NULL CHECK (data_contract_revision > 0),\
                    idempotency_key UUID NOT NULL,\
                    reference_name TEXT NOT NULL CHECK (length(reference_name) BETWEEN 1 AND 96),\
                    resolver_alias TEXT NOT NULL CHECK (length(resolver_alias) BETWEEN 1 AND 96),\
                    resolver_key TEXT NOT NULL CHECK (length(resolver_key) BETWEEN 1 AND 512),\
                    expected_revision BIGINT NULL CHECK (expected_revision > 0),\
                    actor_id UUID NOT NULL,\
                    reason TEXT NOT NULL CHECK (length(trim(reason)) > 0),\
                    revision BIGINT NOT NULL CHECK (revision > 0),\
                    completed_at TIMESTAMPTZ NOT NULL,\
                    PRIMARY KEY (tenant_id, module_slug, data_contract_revision, idempotency_key)\
                )",
                "ALTER TABLE module_artifact_secret_binding_operations ENABLE ROW LEVEL SECURITY",
                "CREATE POLICY module_artifact_secret_binding_operations_scope \
                 ON module_artifact_secret_binding_operations \
                 USING (tenant_id::text = current_setting('rustok.tenant_id', true)) \
                 WITH CHECK (tenant_id::text = current_setting('rustok.tenant_id', true))",
            ],
            DbBackend::Sqlite => &[
                "CREATE TABLE module_artifact_secret_bindings (\
                    tenant_id TEXT NOT NULL,\
                    module_slug TEXT NOT NULL,\
                    data_contract_revision INTEGER NOT NULL CHECK (data_contract_revision > 0),\
                    reference_name TEXT NOT NULL CHECK (length(reference_name) BETWEEN 1 AND 96),\
                    resolver_alias TEXT NOT NULL CHECK (length(resolver_alias) BETWEEN 1 AND 96),\
                    resolver_key TEXT NOT NULL CHECK (length(resolver_key) BETWEEN 1 AND 512),\
                    revision INTEGER NOT NULL CHECK (revision > 0),\
                    actor_id TEXT NOT NULL,\
                    reason TEXT NOT NULL CHECK (length(trim(reason)) > 0),\
                    created_at TEXT NOT NULL,\
                    updated_at TEXT NOT NULL,\
                    PRIMARY KEY (tenant_id, module_slug, data_contract_revision, reference_name)\
                )",
                "CREATE TABLE module_artifact_secret_binding_operations (\
                    tenant_id TEXT NOT NULL,\
                    module_slug TEXT NOT NULL,\
                    data_contract_revision INTEGER NOT NULL CHECK (data_contract_revision > 0),\
                    idempotency_key TEXT NOT NULL,\
                    reference_name TEXT NOT NULL CHECK (length(reference_name) BETWEEN 1 AND 96),\
                    resolver_alias TEXT NOT NULL CHECK (length(resolver_alias) BETWEEN 1 AND 96),\
                    resolver_key TEXT NOT NULL CHECK (length(resolver_key) BETWEEN 1 AND 512),\
                    expected_revision INTEGER NULL CHECK (expected_revision > 0),\
                    actor_id TEXT NOT NULL,\
                    reason TEXT NOT NULL CHECK (length(trim(reason)) > 0),\
                    revision INTEGER NOT NULL CHECK (revision > 0),\
                    completed_at TEXT NOT NULL,\
                    PRIMARY KEY (tenant_id, module_slug, data_contract_revision, idempotency_key)\
                )",
            ],
            backend => {
                return Err(DbErr::Migration(format!(
                    "artifact secret-binding migration does not support database backend {backend:?}"
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
            "module_artifact_secret_binding_operations",
            "module_artifact_secret_bindings",
        ] {
            manager
                .get_connection()
                .execute_unprepared(&format!("DROP TABLE {table}"))
                .await?;
        }
        Ok(())
    }
}
