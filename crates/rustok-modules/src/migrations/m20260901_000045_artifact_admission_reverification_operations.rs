use sea_orm::{ConnectionTrait, DbBackend, Statement};
use sea_orm_migration::prelude::*;

/// Durable exact-replay receipts for owner-authorized admission reverification.
/// The evidence replaces mutable trust facts but never selects a new artifact
/// identity, so the receipt binds the existing installation and resulting CAS
/// revision to one authenticated command context.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let statements: &[&str] = match manager.get_database_backend() {
            DbBackend::Postgres => &[
                "CREATE TABLE module_artifact_admission_reverification_operations (\
                    scope_kind TEXT NOT NULL CHECK (scope_kind IN ('platform', 'tenant')),\
                    scope_tenant_key TEXT NOT NULL,\
                    actor_id UUID NOT NULL,\
                    idempotency_key UUID NOT NULL,\
                    trace_id TEXT NOT NULL CHECK (length(trim(trace_id)) BETWEEN 1 AND 512),\
                    correlation_id UUID NOT NULL,\
                    request_digest TEXT NOT NULL CHECK (request_digest ~ '^sha256:[0-9a-f]{64}$'),\
                    installation_id UUID NOT NULL REFERENCES module_artifact_installations(installation_id) ON DELETE RESTRICT,\
                    expected_revision BIGINT NOT NULL CHECK (expected_revision > 0),\
                    resulting_revision BIGINT NOT NULL CHECK (resulting_revision > expected_revision),\
                    committed_at TIMESTAMPTZ NOT NULL,\
                    PRIMARY KEY (scope_kind, scope_tenant_key, actor_id, idempotency_key)\
                )",
                "CREATE INDEX module_artifact_admission_reverification_operations_installation_idx \
                 ON module_artifact_admission_reverification_operations (installation_id)",
                "ALTER TABLE module_artifact_admission_reverification_operations ENABLE ROW LEVEL SECURITY",
                "CREATE POLICY module_artifact_admission_reverification_operations_scope \
                 ON module_artifact_admission_reverification_operations USING (\
                    scope_kind = 'platform' OR scope_tenant_key = current_setting('rustok.tenant_id', true)\
                 ) WITH CHECK (\
                    scope_kind = 'platform' OR scope_tenant_key = current_setting('rustok.tenant_id', true)\
                 )",
            ],
            DbBackend::Sqlite => &[
                "CREATE TABLE module_artifact_admission_reverification_operations (\
                    scope_kind TEXT NOT NULL CHECK (scope_kind IN ('platform', 'tenant')),\
                    scope_tenant_key TEXT NOT NULL,\
                    actor_id TEXT NOT NULL,\
                    idempotency_key TEXT NOT NULL,\
                    trace_id TEXT NOT NULL CHECK (length(trim(trace_id)) BETWEEN 1 AND 512),\
                    correlation_id TEXT NOT NULL,\
                    request_digest TEXT NOT NULL CHECK (length(request_digest) = 71 AND substr(request_digest, 1, 7) = 'sha256:' AND substr(request_digest, 8) NOT GLOB '*[^0-9a-f]*'),\
                    installation_id TEXT NOT NULL REFERENCES module_artifact_installations(installation_id) ON DELETE RESTRICT,\
                    expected_revision INTEGER NOT NULL CHECK (expected_revision > 0),\
                    resulting_revision INTEGER NOT NULL CHECK (resulting_revision > expected_revision),\
                    committed_at TEXT NOT NULL,\
                    PRIMARY KEY (scope_kind, scope_tenant_key, actor_id, idempotency_key)\
                )",
                "CREATE INDEX module_artifact_admission_reverification_operations_installation_idx \
                 ON module_artifact_admission_reverification_operations (installation_id)",
            ],
            backend => {
                return Err(DbErr::Migration(format!(
                    "artifact admission reverification migration does not support database backend {backend:?}"
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
            .execute_unprepared("DROP TABLE module_artifact_admission_reverification_operations")
            .await
            .map(|_| ())
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use sea_orm::{ConnectionTrait, Database, DbBackend, Statement};
    use sea_orm_migration::prelude::{MigrationTrait, SchemaManager};

    use super::Migration;

    #[tokio::test]
    async fn sqlite_schema_records_complete_command_replay_evidence() {
        let database = Database::connect("sqlite::memory:")
            .await
            .expect("database");
        crate::migrations::m20260711_000001_module_artifact_installations::Migration
            .up(&SchemaManager::new(&database))
            .await
            .expect("installation schema");
        Migration
            .up(&SchemaManager::new(&database))
            .await
            .expect("reverification receipt schema");

        let columns = database
            .query_all_raw(Statement::from_string(
                DbBackend::Sqlite,
                "PRAGMA table_info(module_artifact_admission_reverification_operations)"
                    .to_string(),
            ))
            .await
            .expect("table info")
            .into_iter()
            .map(|row| row.try_get("", "name").expect("column name"))
            .collect::<HashSet<String>>();
        for column in [
            "actor_id",
            "idempotency_key",
            "trace_id",
            "correlation_id",
            "request_digest",
            "expected_revision",
            "resulting_revision",
        ] {
            assert!(columns.contains(column), "missing {column}");
        }
    }
}
