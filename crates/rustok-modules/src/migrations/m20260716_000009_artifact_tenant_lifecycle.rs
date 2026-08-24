use sea_orm::{ConnectionTrait, DbBackend, Statement};
use sea_orm_migration::prelude::*;

/// Tenant intent is distinct from the admitted installation and its runtime
/// bindings. This state is the durable source for artifact enable/disable.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let statements: &[&str] = match manager.get_database_backend() {
            DbBackend::Postgres => &[
                "CREATE TABLE module_artifact_tenant_lifecycle (\
                    installation_id UUID NOT NULL REFERENCES module_artifact_installations(installation_id),\
                    tenant_id UUID NOT NULL,\
                    enabled BOOLEAN NOT NULL,\
                    revision BIGINT NOT NULL CHECK (revision > 0),\
                    expected_revision BIGINT NOT NULL CHECK (expected_revision > 0),\
                    actor_id UUID NOT NULL,\
                    trace_id TEXT NOT NULL CHECK (length(trim(trace_id)) > 0),\
                    correlation_id UUID NOT NULL,\
                    reason TEXT NOT NULL CHECK (length(trim(reason)) > 0),\
                    updated_at TIMESTAMPTZ NOT NULL,\
                    PRIMARY KEY (installation_id, tenant_id)\
                )",
                "ALTER TABLE module_artifact_tenant_lifecycle ENABLE ROW LEVEL SECURITY",
                "CREATE POLICY module_artifact_tenant_lifecycle_scope ON module_artifact_tenant_lifecycle \
                 USING (tenant_id::text = current_setting('rustok.tenant_id', true)) \
                 WITH CHECK (tenant_id::text = current_setting('rustok.tenant_id', true))",
                "CREATE TABLE module_artifact_tenant_lifecycle_operations (\
                    operation_id UUID PRIMARY KEY,\
                    installation_id UUID NOT NULL REFERENCES module_artifact_installations(installation_id),\
                    tenant_id UUID NOT NULL,\
                    requested_enabled BOOLEAN NOT NULL,\
                    expected_revision BIGINT NOT NULL CHECK (expected_revision > 0),\
                    committed_revision BIGINT NOT NULL CHECK (committed_revision > 0),\
                    actor_id UUID NOT NULL,\
                    trace_id TEXT NOT NULL CHECK (length(trim(trace_id)) > 0),\
                    correlation_id UUID NOT NULL,\
                    reason TEXT NOT NULL CHECK (length(trim(reason)) > 0),\
                    idempotency_key UUID NOT NULL,\
                    committed_at TIMESTAMPTZ NOT NULL,\
                    UNIQUE (tenant_id, idempotency_key)\
                )",
                "ALTER TABLE module_artifact_tenant_lifecycle_operations ENABLE ROW LEVEL SECURITY",
                "CREATE POLICY module_artifact_tenant_lifecycle_operations_scope ON module_artifact_tenant_lifecycle_operations \
                 USING (tenant_id::text = current_setting('rustok.tenant_id', true)) \
                 WITH CHECK (tenant_id::text = current_setting('rustok.tenant_id', true))",
            ],
            DbBackend::Sqlite => &[
                "CREATE TABLE module_artifact_tenant_lifecycle (\
                    installation_id TEXT NOT NULL REFERENCES module_artifact_installations(installation_id),\
                    tenant_id TEXT NOT NULL,\
                    enabled INTEGER NOT NULL CHECK (enabled IN (0, 1)),\
                    revision INTEGER NOT NULL CHECK (revision > 0),\
                    expected_revision INTEGER NOT NULL CHECK (expected_revision > 0),\
                    actor_id TEXT NOT NULL,\
                    trace_id TEXT NOT NULL CHECK (length(trim(trace_id)) > 0),\
                    correlation_id TEXT NOT NULL,\
                    reason TEXT NOT NULL CHECK (length(trim(reason)) > 0),\
                    updated_at TEXT NOT NULL,\
                    PRIMARY KEY (installation_id, tenant_id)\
                )",
                "CREATE TABLE module_artifact_tenant_lifecycle_operations (\
                    operation_id TEXT PRIMARY KEY,\
                    installation_id TEXT NOT NULL REFERENCES module_artifact_installations(installation_id),\
                    tenant_id TEXT NOT NULL,\
                    requested_enabled INTEGER NOT NULL CHECK (requested_enabled IN (0, 1)),\
                    expected_revision INTEGER NOT NULL CHECK (expected_revision > 0),\
                    committed_revision INTEGER NOT NULL CHECK (committed_revision > 0),\
                    actor_id TEXT NOT NULL,\
                    trace_id TEXT NOT NULL CHECK (length(trim(trace_id)) > 0),\
                    correlation_id TEXT NOT NULL,\
                    reason TEXT NOT NULL CHECK (length(trim(reason)) > 0),\
                    idempotency_key TEXT NOT NULL,\
                    committed_at TEXT NOT NULL,\
                    UNIQUE (tenant_id, idempotency_key)\
                )",
            ],
            backend => {
                return Err(DbErr::Migration(format!(
                    "artifact tenant lifecycle migration does not support database backend {backend:?}"
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
            .execute_unprepared("DROP TABLE module_artifact_tenant_lifecycle_operations")
            .await?;
        manager
            .get_connection()
            .execute_unprepared("DROP TABLE module_artifact_tenant_lifecycle")
            .await
            .map(|_| ())
    }
}
