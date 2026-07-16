use sea_orm::{ConnectionTrait, DbBackend, Statement};
use sea_orm_migration::prelude::*;

/// Durable owner command identity for artifact admission. A reservation is
/// inserted before installation metadata so concurrent retries converge on one
/// admitted installation instead of creating duplicate selections.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let statements: &[&str] = match manager.get_database_backend() {
            DbBackend::Postgres => &[
                "CREATE TABLE module_artifact_admission_commands (\
                    scope_kind TEXT NOT NULL CHECK (scope_kind IN ('platform', 'tenant')),\
                    scope_tenant_key TEXT NOT NULL,\
                    actor_id UUID NOT NULL,\
                    idempotency_key UUID NOT NULL,\
                    request_digest TEXT NOT NULL,\
                    installation_id UUID NULL REFERENCES module_artifact_installations(installation_id),\
                    committed_at TIMESTAMPTZ NOT NULL,\
                    PRIMARY KEY (scope_kind, scope_tenant_key, actor_id, idempotency_key)\
                )",
                "CREATE INDEX module_artifact_admission_commands_installation_idx \
                 ON module_artifact_admission_commands (installation_id)",
                "ALTER TABLE module_artifact_admission_commands ENABLE ROW LEVEL SECURITY",
                "CREATE POLICY module_artifact_admission_commands_scope \
                 ON module_artifact_admission_commands USING (\
                    scope_kind = 'platform' OR scope_tenant_key = current_setting('rustok.tenant_id', true)\
                 ) WITH CHECK (\
                    scope_kind = 'platform' OR scope_tenant_key = current_setting('rustok.tenant_id', true)\
                 )",
            ],
            DbBackend::Sqlite => &[
                "CREATE TABLE module_artifact_admission_commands (\
                    scope_kind TEXT NOT NULL CHECK (scope_kind IN ('platform', 'tenant')),\
                    scope_tenant_key TEXT NOT NULL,\
                    actor_id TEXT NOT NULL,\
                    idempotency_key TEXT NOT NULL,\
                    request_digest TEXT NOT NULL,\
                    installation_id TEXT NULL REFERENCES module_artifact_installations(installation_id),\
                    committed_at TEXT NOT NULL,\
                    PRIMARY KEY (scope_kind, scope_tenant_key, actor_id, idempotency_key)\
                )",
                "CREATE INDEX module_artifact_admission_commands_installation_idx \
                 ON module_artifact_admission_commands (installation_id)",
            ],
            backend => {
                return Err(DbErr::Migration(format!(
                    "artifact admission-command migration does not support database backend {backend:?}"
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
            .execute_unprepared("DROP TABLE module_artifact_admission_commands")
            .await
            .map(|_| ())
    }
}
