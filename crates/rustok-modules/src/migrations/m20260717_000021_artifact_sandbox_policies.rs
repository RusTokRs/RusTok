use sea_orm::{ConnectionTrait, DbBackend, Statement};
use sea_orm_migration::prelude::*;

/// Persists host-selected sandbox policy independently from artifact
/// declarations. A missing row denies runtime execution rather than turning a
/// descriptor capability into an implicit grant.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let statements: &[&str] = match manager.get_database_backend() {
            DbBackend::Postgres => &[
                "CREATE TABLE module_artifact_sandbox_policies (\
                    installation_id UUID NOT NULL REFERENCES module_artifact_installations(installation_id),\
                    tenant_id UUID NULL,\
                    capability_grant_revision BIGINT NOT NULL CHECK (capability_grant_revision > 0),\
                    policy JSONB NOT NULL,\
                    created_at TIMESTAMPTZ NOT NULL\
                )",
                "CREATE UNIQUE INDEX module_artifact_sandbox_policy_default_identity \
                 ON module_artifact_sandbox_policies (installation_id) WHERE tenant_id IS NULL",
                "CREATE UNIQUE INDEX module_artifact_sandbox_policy_tenant_identity \
                 ON module_artifact_sandbox_policies (installation_id, tenant_id) WHERE tenant_id IS NOT NULL",
                "ALTER TABLE module_artifact_sandbox_policies ENABLE ROW LEVEL SECURITY",
                "CREATE POLICY module_artifact_sandbox_policies_tenant_scope \
                 ON module_artifact_sandbox_policies \
                 USING (tenant_id IS NULL OR tenant_id::text = current_setting('rustok.tenant_id', true)) \
                 WITH CHECK (tenant_id IS NULL OR tenant_id::text = current_setting('rustok.tenant_id', true))",
            ],
            DbBackend::Sqlite => &[
                "CREATE TABLE module_artifact_sandbox_policies (\
                    installation_id TEXT NOT NULL REFERENCES module_artifact_installations(installation_id),\
                    tenant_id TEXT NULL,\
                    capability_grant_revision INTEGER NOT NULL CHECK (capability_grant_revision > 0),\
                    policy JSON NOT NULL,\
                    created_at TEXT NOT NULL\
                )",
                "CREATE UNIQUE INDEX module_artifact_sandbox_policy_default_identity \
                 ON module_artifact_sandbox_policies (installation_id) WHERE tenant_id IS NULL",
                "CREATE UNIQUE INDEX module_artifact_sandbox_policy_tenant_identity \
                 ON module_artifact_sandbox_policies (installation_id, tenant_id) WHERE tenant_id IS NOT NULL",
            ],
            backend => {
                return Err(DbErr::Migration(format!(
                    "artifact sandbox policy migration does not support database backend {backend:?}"
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
            .execute_unprepared("DROP TABLE module_artifact_sandbox_policies")
            .await
            .map(|_| ())
    }
}
