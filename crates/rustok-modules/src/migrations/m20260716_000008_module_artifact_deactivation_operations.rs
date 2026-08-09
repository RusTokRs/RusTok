use sea_orm::{ConnectionTrait, DbBackend, Statement};
use sea_orm_migration::prelude::*;

/// Records durable scoped artifact activation and deactivation operations.
/// Both preserve immutable admission evidence and make their lifecycle result
/// replayable without selecting a mutable release again.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let statements: &[&str] = match manager.get_database_backend() {
            DbBackend::Postgres => &[
                "CREATE TABLE module_artifact_deactivation_operations (\
                    operation_id UUID PRIMARY KEY,\
                    installation_id UUID NOT NULL REFERENCES module_artifact_installations(installation_id),\
                    expected_revision BIGINT NOT NULL CHECK (expected_revision > 0),\
                    actor_id UUID NOT NULL,\
                    reason TEXT NOT NULL CHECK (length(trim(reason)) > 0),\
                    idempotency_key UUID NOT NULL UNIQUE,\
                    committed_at TIMESTAMPTZ NOT NULL\
                )",
                "CREATE INDEX module_artifact_deactivation_operations_installation_idx \
                 ON module_artifact_deactivation_operations (installation_id, committed_at DESC)",
                "ALTER TABLE module_artifact_deactivation_operations ENABLE ROW LEVEL SECURITY",
                "CREATE POLICY module_artifact_deactivation_operations_scope \
                 ON module_artifact_deactivation_operations USING (EXISTS (\
                    SELECT 1 FROM module_artifact_installations installation \
                    WHERE installation.installation_id = module_artifact_deactivation_operations.installation_id \
                    AND (installation.scope_kind = 'platform' OR installation.tenant_id::text = current_setting('rustok.tenant_id', true))\
                 ))",
                "CREATE TABLE module_artifact_activation_locks (\
                    scope_kind TEXT NOT NULL CHECK (scope_kind IN ('platform', 'tenant')),\
                    scope_tenant_key TEXT NOT NULL,\
                    slug TEXT NOT NULL CHECK (length(trim(slug)) > 0),\
                    PRIMARY KEY (scope_kind, scope_tenant_key, slug)\
                )",
                "ALTER TABLE module_artifact_activation_locks ENABLE ROW LEVEL SECURITY",
                "CREATE POLICY module_artifact_activation_locks_scope \
                 ON module_artifact_activation_locks USING (\
                    scope_kind = 'platform' OR scope_tenant_key = current_setting('rustok.tenant_id', true)\
                 ) WITH CHECK (\
                    scope_kind = 'platform' OR scope_tenant_key = current_setting('rustok.tenant_id', true)\
                 )",
                "CREATE TABLE module_artifact_activation_operations (\
                    operation_id UUID PRIMARY KEY,\
                    installation_id UUID NOT NULL REFERENCES module_artifact_installations(installation_id),\
                    predecessor_installation_id UUID NULL REFERENCES module_artifact_installations(installation_id),\
                    expected_revision BIGINT NOT NULL CHECK (expected_revision > 0),\
                    installation_revision BIGINT NOT NULL CHECK (installation_revision > 0),\
                    predecessor_revision BIGINT NULL CHECK (predecessor_revision IS NULL OR predecessor_revision > 0),\
                    actor_id UUID NOT NULL,\
                    reason TEXT NOT NULL CHECK (length(trim(reason)) > 0),\
                    idempotency_key UUID NOT NULL UNIQUE,\
                    committed_at TIMESTAMPTZ NOT NULL,\
                    CHECK ((predecessor_installation_id IS NULL AND predecessor_revision IS NULL) \
                        OR (predecessor_installation_id IS NOT NULL AND predecessor_revision IS NOT NULL))\
                )",
                "CREATE INDEX module_artifact_activation_operations_installation_idx \
                 ON module_artifact_activation_operations (installation_id, committed_at DESC)",
                "ALTER TABLE module_artifact_activation_operations ENABLE ROW LEVEL SECURITY",
                "CREATE POLICY module_artifact_activation_operations_scope \
                 ON module_artifact_activation_operations USING (EXISTS (\
                    SELECT 1 FROM module_artifact_installations installation \
                    WHERE installation.installation_id = module_artifact_activation_operations.installation_id \
                    AND (installation.scope_kind = 'platform' OR installation.tenant_id::text = current_setting('rustok.tenant_id', true))\
                 ))",
                "CREATE TABLE module_artifact_settings_instances (\
                    tenant_id UUID NOT NULL,\
                    data_owner_id UUID NOT NULL,\
                    settings_instance_id UUID NOT NULL,\
                    schema_digest TEXT NOT NULL CHECK (schema_digest ~ '^sha256:[0-9a-f]{64}$'),\
                    settings JSONB NOT NULL,\
                    revision BIGINT NOT NULL CHECK (revision > 0),\
                    created_at TIMESTAMPTZ NOT NULL,\
                    updated_at TIMESTAMPTZ NOT NULL,\
                    PRIMARY KEY (tenant_id, data_owner_id, settings_instance_id)\
                )",
                "ALTER TABLE module_artifact_settings_instances ENABLE ROW LEVEL SECURITY",
                "CREATE POLICY module_artifact_settings_instances_scope \
                 ON module_artifact_settings_instances \
                 USING (tenant_id::text = current_setting('rustok.tenant_id', true)) \
                 WITH CHECK (tenant_id::text = current_setting('rustok.tenant_id', true))",
            ],
            DbBackend::Sqlite => &[
                "CREATE TABLE module_artifact_deactivation_operations (\
                    operation_id TEXT PRIMARY KEY NOT NULL,\
                    installation_id TEXT NOT NULL REFERENCES module_artifact_installations(installation_id),\
                    expected_revision INTEGER NOT NULL CHECK (expected_revision > 0),\
                    actor_id TEXT NOT NULL,\
                    reason TEXT NOT NULL CHECK (length(trim(reason)) > 0),\
                    idempotency_key TEXT NOT NULL UNIQUE,\
                    committed_at TEXT NOT NULL\
                )",
                "CREATE INDEX module_artifact_deactivation_operations_installation_idx \
                 ON module_artifact_deactivation_operations (installation_id, committed_at DESC)",
                "CREATE TABLE module_artifact_activation_locks (\
                    scope_kind TEXT NOT NULL CHECK (scope_kind IN ('platform', 'tenant')),\
                    scope_tenant_key TEXT NOT NULL,\
                    slug TEXT NOT NULL CHECK (length(trim(slug)) > 0),\
                    PRIMARY KEY (scope_kind, scope_tenant_key, slug)\
                )",
                "CREATE TABLE module_artifact_activation_operations (\
                    operation_id TEXT PRIMARY KEY NOT NULL,\
                    installation_id TEXT NOT NULL REFERENCES module_artifact_installations(installation_id),\
                    predecessor_installation_id TEXT NULL REFERENCES module_artifact_installations(installation_id),\
                    expected_revision INTEGER NOT NULL CHECK (expected_revision > 0),\
                    installation_revision INTEGER NOT NULL CHECK (installation_revision > 0),\
                    predecessor_revision INTEGER NULL CHECK (predecessor_revision IS NULL OR predecessor_revision > 0),\
                    actor_id TEXT NOT NULL,\
                    reason TEXT NOT NULL CHECK (length(trim(reason)) > 0),\
                    idempotency_key TEXT NOT NULL UNIQUE,\
                    committed_at TEXT NOT NULL,\
                    CHECK ((predecessor_installation_id IS NULL AND predecessor_revision IS NULL) \
                        OR (predecessor_installation_id IS NOT NULL AND predecessor_revision IS NOT NULL))\
                )",
                "CREATE INDEX module_artifact_activation_operations_installation_idx \
                 ON module_artifact_activation_operations (installation_id, committed_at DESC)",
                "CREATE TABLE module_artifact_settings_instances (\
                    tenant_id TEXT NOT NULL,\
                    data_owner_id TEXT NOT NULL,\
                    settings_instance_id TEXT NOT NULL,\
                    schema_digest TEXT NOT NULL CHECK (length(schema_digest) = 71 AND substr(schema_digest, 1, 7) = 'sha256:' AND substr(schema_digest, 8) NOT GLOB '*[^0-9a-f]*'),\
                    settings JSON NOT NULL,\
                    revision INTEGER NOT NULL CHECK (revision > 0),\
                    created_at TEXT NOT NULL,\
                    updated_at TEXT NOT NULL,\
                    PRIMARY KEY (tenant_id, data_owner_id, settings_instance_id)\
                )",
            ],
            backend => {
                return Err(DbErr::Migration(format!(
                    "module artifact deactivation-operation migration does not support database backend {backend:?}"
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
        for statement in [
            "DROP TABLE module_artifact_settings_instances",
            "DROP TABLE module_artifact_activation_operations",
            "DROP TABLE module_artifact_activation_locks",
            "DROP TABLE module_artifact_deactivation_operations",
        ] {
            manager
                .get_connection()
                .execute_unprepared(statement)
                .await?;
        }
        Ok(())
    }
}
