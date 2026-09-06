use sea_orm::{ConnectionTrait, DbBackend, Statement};
use sea_orm_migration::prelude::*;

/// Durable data-migration state belongs to the admitted installation, not to a
/// guest payload. An irreversible checkpoint prevents an unsafe rollback or
/// purge path from pretending that data can be restored automatically.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let statements: &[&str] = match manager.get_database_backend() {
            DbBackend::Postgres => &[
                "ALTER TABLE module_artifact_installations ADD COLUMN migration_checkpoint JSONB NULL",
                "ALTER TABLE module_artifact_installations ADD COLUMN has_irreversible_migration BOOLEAN NOT NULL DEFAULT FALSE",
                "CREATE TABLE module_artifact_migration_checkpoint_operations (\
                    operation_id UUID PRIMARY KEY,\
                    installation_id UUID NOT NULL REFERENCES module_artifact_installations(installation_id),\
                    expected_revision BIGINT NOT NULL CHECK (expected_revision > 0),\
                    revision BIGINT NOT NULL CHECK (revision > expected_revision),\
                    request_digest TEXT NOT NULL CHECK (request_digest ~ '^sha256:[0-9a-f]{64}$'),\
                    actor_id UUID NOT NULL,\
                    trace_id TEXT NOT NULL CHECK (length(trim(trace_id)) > 0),\
                    correlation_id UUID NOT NULL,\
                    reason TEXT NOT NULL CHECK (length(trim(reason)) > 0),\
                    idempotency_key UUID NOT NULL,\
                    committed_at TIMESTAMPTZ NOT NULL,\
                    UNIQUE (installation_id, idempotency_key)\
                )",
                "ALTER TABLE module_artifact_migration_checkpoint_operations ENABLE ROW LEVEL SECURITY",
                "CREATE POLICY module_artifact_migration_checkpoint_operations_scope ON module_artifact_migration_checkpoint_operations \
                    USING (EXISTS (SELECT 1 FROM module_artifact_installations installation \
                        WHERE installation.installation_id = module_artifact_migration_checkpoint_operations.installation_id \
                        AND (installation.scope_kind = 'platform' OR installation.tenant_id::text = current_setting('rustok.tenant_id', true))))",
            ],
            DbBackend::Sqlite => &[
                "ALTER TABLE module_artifact_installations ADD COLUMN migration_checkpoint JSON NULL",
                "ALTER TABLE module_artifact_installations ADD COLUMN has_irreversible_migration INTEGER NOT NULL DEFAULT 0 CHECK (has_irreversible_migration IN (0, 1))",
                "CREATE TABLE module_artifact_migration_checkpoint_operations (\
                    operation_id TEXT PRIMARY KEY NOT NULL,\
                    installation_id TEXT NOT NULL REFERENCES module_artifact_installations(installation_id),\
                    expected_revision INTEGER NOT NULL CHECK (expected_revision > 0),\
                    revision INTEGER NOT NULL CHECK (revision > expected_revision),\
                    request_digest TEXT NOT NULL CHECK (length(request_digest) = 71 AND substr(request_digest, 1, 7) = 'sha256:' AND substr(request_digest, 8) NOT GLOB '*[^0-9a-f]*'),\
                    actor_id TEXT NOT NULL,\
                    trace_id TEXT NOT NULL CHECK (length(trim(trace_id)) > 0),\
                    correlation_id TEXT NOT NULL,\
                    reason TEXT NOT NULL CHECK (length(trim(reason)) > 0),\
                    idempotency_key TEXT NOT NULL,\
                    committed_at TEXT NOT NULL,\
                    UNIQUE (installation_id, idempotency_key)\
                )",
            ],
            backend => {
                return Err(DbErr::Migration(format!(
                    "artifact migration checkpoint does not support database backend {backend:?}"
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
            .execute_unprepared("DROP TABLE module_artifact_migration_checkpoint_operations")
            .await?;
        for column in ["migration_checkpoint", "has_irreversible_migration"] {
            manager
                .get_connection()
                .execute_unprepared(&format!(
                    "ALTER TABLE module_artifact_installations DROP COLUMN {column}"
                ))
                .await?;
        }
        Ok(())
    }
}
