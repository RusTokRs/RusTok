use sea_orm::{ConnectionTrait, DbBackend, Statement};
use sea_orm_migration::prelude::*;

/// Creates the control-plane queue for remote untrusted module builds. The
/// worker receives submissions through the transactional outbox, never through
/// a server-local Cargo invocation.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let statements: &[&str] = match manager.get_database_backend() {
            DbBackend::Postgres => &[
                "CREATE TABLE module_build_requests (\
                    request_id UUID PRIMARY KEY,\
                    tenant_id UUID NOT NULL,\
                    project_id TEXT NOT NULL CHECK (length(trim(project_id)) BETWEEN 1 AND 256),\
                    idempotency_key TEXT NOT NULL CHECK (length(trim(idempotency_key)) BETWEEN 1 AND 256),\
                    request_hash TEXT NOT NULL CHECK (length(request_hash) = 71),\
                    request JSONB NOT NULL,\
                    result JSONB NULL,\
                    result_hash TEXT NULL CHECK (result_hash IS NULL OR length(result_hash) = 71),\
                    attempt INTEGER NOT NULL CHECK (attempt > 0),\
                    status TEXT NOT NULL CHECK (status IN ('queued', 'completed')),\
                    created_at TIMESTAMPTZ NOT NULL,\
                    completed_at TIMESTAMPTZ NULL,\
                    UNIQUE (tenant_id, project_id, idempotency_key)\
                )",
                "CREATE INDEX module_build_requests_queue_idx ON module_build_requests (created_at, request_id)",
                "ALTER TABLE module_build_requests ENABLE ROW LEVEL SECURITY",
                "CREATE POLICY module_build_requests_scope ON module_build_requests \
                 USING (tenant_id::text = current_setting('rustok.tenant_id', true)) \
                 WITH CHECK (tenant_id::text = current_setting('rustok.tenant_id', true))",
            ],
            DbBackend::Sqlite => &[
                "CREATE TABLE module_build_requests (\
                    request_id TEXT PRIMARY KEY,\
                    tenant_id TEXT NOT NULL,\
                    project_id TEXT NOT NULL CHECK (length(trim(project_id)) BETWEEN 1 AND 256),\
                    idempotency_key TEXT NOT NULL CHECK (length(trim(idempotency_key)) BETWEEN 1 AND 256),\
                    request_hash TEXT NOT NULL CHECK (length(request_hash) = 71),\
                    request JSON NOT NULL,\
                    result JSON NULL,\
                    result_hash TEXT NULL CHECK (result_hash IS NULL OR length(result_hash) = 71),\
                    attempt INTEGER NOT NULL CHECK (attempt > 0),\
                    status TEXT NOT NULL CHECK (status IN ('queued', 'completed')),\
                    created_at TEXT NOT NULL,\
                    completed_at TEXT NULL,\
                    UNIQUE (tenant_id, project_id, idempotency_key)\
                )",
                "CREATE INDEX module_build_requests_queue_idx ON module_build_requests (created_at, request_id)",
            ],
            backend => {
                return Err(DbErr::Migration(format!(
                    "module build request migration does not support database backend {backend:?}"
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
            .execute_unprepared("DROP TABLE module_build_requests")
            .await
            .map(|_| ())
    }
}
