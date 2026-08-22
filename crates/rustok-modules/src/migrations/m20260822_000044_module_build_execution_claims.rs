use sea_orm::{ConnectionTrait, DbBackend, Statement};
use sea_orm_migration::prelude::*;

/// Adds one durable execution claim and monotonic revision to each isolated
/// module-build request. A broker redelivery may recover only an expired claim;
/// it cannot start a second live worker for the same immutable request.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let connection = manager.get_connection();
        let statements: &[&str] = match manager.get_database_backend() {
            DbBackend::Postgres => &[
                "ALTER TABLE module_build_requests ADD COLUMN revision BIGINT NOT NULL DEFAULT 1 CHECK (revision > 0)",
                "ALTER TABLE module_build_requests ADD COLUMN execution_claim_id UUID NULL",
                "ALTER TABLE module_build_requests ADD COLUMN lease_expires_at TIMESTAMPTZ NULL",
                "ALTER TABLE module_build_requests ADD COLUMN claimed_at TIMESTAMPTZ NULL",
                "ALTER TABLE module_build_requests DROP CONSTRAINT module_build_requests_status_check",
                "ALTER TABLE module_build_requests ADD CONSTRAINT module_build_requests_status_check CHECK (status IN ('queued', 'running', 'completed'))",
                "ALTER TABLE module_build_requests ADD CONSTRAINT module_build_requests_state_check CHECK ((status = 'queued' AND result IS NULL AND result_hash IS NULL AND execution_claim_id IS NULL AND lease_expires_at IS NULL AND claimed_at IS NULL AND completed_at IS NULL) OR (status = 'running' AND result IS NULL AND result_hash IS NULL AND execution_claim_id IS NOT NULL AND lease_expires_at IS NOT NULL AND claimed_at IS NOT NULL AND completed_at IS NULL) OR (status = 'completed' AND result IS NOT NULL AND result_hash IS NOT NULL AND execution_claim_id IS NULL AND lease_expires_at IS NULL AND claimed_at IS NOT NULL AND completed_at IS NOT NULL))",
                "DROP INDEX module_build_requests_queue_idx",
                "CREATE INDEX module_build_requests_queue_idx ON module_build_requests (status, lease_expires_at, created_at, request_id)",
            ],
            DbBackend::Sqlite => &[
                "DROP INDEX module_build_requests_queue_idx",
                "ALTER TABLE module_build_requests RENAME TO module_build_requests_before_execution_claims",
                "CREATE TABLE module_build_requests (request_id TEXT PRIMARY KEY, tenant_id TEXT NOT NULL, project_id TEXT NOT NULL CHECK (length(trim(project_id)) BETWEEN 1 AND 256), idempotency_key TEXT NOT NULL CHECK (length(trim(idempotency_key)) BETWEEN 1 AND 256), request_hash TEXT NOT NULL CHECK (length(request_hash) = 71), request JSON NOT NULL, result JSON NULL, result_hash TEXT NULL CHECK (result_hash IS NULL OR length(result_hash) = 71), attempt INTEGER NOT NULL CHECK (attempt > 0), revision INTEGER NOT NULL DEFAULT 1 CHECK (revision > 0), status TEXT NOT NULL CHECK (status IN ('queued', 'running', 'completed')), execution_claim_id TEXT NULL, lease_expires_at TEXT NULL, claimed_at TEXT NULL, created_at TEXT NOT NULL, completed_at TEXT NULL, UNIQUE (tenant_id, project_id, idempotency_key), CHECK ((status = 'queued' AND result IS NULL AND result_hash IS NULL AND execution_claim_id IS NULL AND lease_expires_at IS NULL AND claimed_at IS NULL AND completed_at IS NULL) OR (status = 'running' AND result IS NULL AND result_hash IS NULL AND execution_claim_id IS NOT NULL AND lease_expires_at IS NOT NULL AND claimed_at IS NOT NULL AND completed_at IS NULL) OR (status = 'completed' AND result IS NOT NULL AND result_hash IS NOT NULL AND execution_claim_id IS NULL AND lease_expires_at IS NULL AND claimed_at IS NOT NULL AND completed_at IS NOT NULL)))",
                "INSERT INTO module_build_requests (request_id, tenant_id, project_id, idempotency_key, request_hash, request, result, result_hash, attempt, revision, status, execution_claim_id, lease_expires_at, claimed_at, created_at, completed_at) SELECT request_id, tenant_id, project_id, idempotency_key, request_hash, request, result, result_hash, attempt, 1, status, NULL, NULL, NULL, created_at, completed_at FROM module_build_requests_before_execution_claims",
                "DROP TABLE module_build_requests_before_execution_claims",
                "CREATE INDEX module_build_requests_queue_idx ON module_build_requests (status, lease_expires_at, created_at, request_id)",
            ],
            backend => {
                return Err(DbErr::Migration(format!(
                    "module build execution claim migration does not support database backend {backend:?}"
                )));
            }
        };

        for statement in statements {
            connection
                .execute(Statement::from_string(
                    manager.get_database_backend(),
                    (*statement).to_string(),
                ))
                .await?;
        }
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let connection = manager.get_connection();
        let backend = manager.get_database_backend();
        if connection
            .query_one(Statement::from_string(
                backend,
                "SELECT 1 FROM module_build_requests WHERE status = 'running' LIMIT 1".to_string(),
            ))
            .await?
            .is_some()
        {
            return Err(DbErr::Migration(
                "module build execution-claim migration cannot roll back while a build is running"
                    .to_string(),
            ));
        }

        let statements: &[&str] = match backend {
            DbBackend::Postgres => &[
                "ALTER TABLE module_build_requests DROP CONSTRAINT module_build_requests_state_check",
                "DROP INDEX module_build_requests_queue_idx",
                "ALTER TABLE module_build_requests DROP CONSTRAINT module_build_requests_status_check",
                "ALTER TABLE module_build_requests DROP COLUMN claimed_at",
                "ALTER TABLE module_build_requests DROP COLUMN lease_expires_at",
                "ALTER TABLE module_build_requests DROP COLUMN execution_claim_id",
                "ALTER TABLE module_build_requests DROP COLUMN revision",
                "ALTER TABLE module_build_requests ADD CONSTRAINT module_build_requests_status_check CHECK (status IN ('queued', 'completed'))",
                "CREATE INDEX module_build_requests_queue_idx ON module_build_requests (created_at, request_id)",
            ],
            DbBackend::Sqlite => &[
                "DROP INDEX module_build_requests_queue_idx",
                "ALTER TABLE module_build_requests RENAME TO module_build_requests_with_execution_claims",
                "CREATE TABLE module_build_requests (request_id TEXT PRIMARY KEY, tenant_id TEXT NOT NULL, project_id TEXT NOT NULL CHECK (length(trim(project_id)) BETWEEN 1 AND 256), idempotency_key TEXT NOT NULL CHECK (length(trim(idempotency_key)) BETWEEN 1 AND 256), request_hash TEXT NOT NULL CHECK (length(request_hash) = 71), request JSON NOT NULL, result JSON NULL, result_hash TEXT NULL CHECK (result_hash IS NULL OR length(result_hash) = 71), attempt INTEGER NOT NULL CHECK (attempt > 0), status TEXT NOT NULL CHECK (status IN ('queued', 'completed')), created_at TEXT NOT NULL, completed_at TEXT NULL, UNIQUE (tenant_id, project_id, idempotency_key))",
                "INSERT INTO module_build_requests (request_id, tenant_id, project_id, idempotency_key, request_hash, request, result, result_hash, attempt, status, created_at, completed_at) SELECT request_id, tenant_id, project_id, idempotency_key, request_hash, request, result, result_hash, attempt, status, created_at, completed_at FROM module_build_requests_with_execution_claims",
                "DROP TABLE module_build_requests_with_execution_claims",
                "CREATE INDEX module_build_requests_queue_idx ON module_build_requests (created_at, request_id)",
            ],
            _ => unreachable!("the backend was validated above"),
        };

        for statement in statements {
            connection
                .execute(Statement::from_string(backend, (*statement).to_string()))
                .await?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use sea_orm::{ConnectionTrait, Database, DbBackend, Statement};
    use sea_orm_migration::prelude::{MigrationTrait, SchemaManager};

    use super::Migration;

    #[tokio::test]
    async fn sqlite_upgrade_enforces_one_live_build_execution_claim() {
        let database = Database::connect("sqlite::memory:")
            .await
            .expect("database");
        crate::migrations::m20260716_000012_module_build_requests::Migration
            .up(&SchemaManager::new(&database))
            .await
            .expect("historical build-request schema");
        database
            .execute(Statement::from_string(
                DbBackend::Sqlite,
                "INSERT INTO module_build_requests (request_id, tenant_id, project_id, idempotency_key, request_hash, request, attempt, status, created_at) VALUES ('request', 'tenant', 'project', 'idempotency', 'sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa', '{}', 1, 'queued', CURRENT_TIMESTAMP)".to_string(),
            ))
            .await
            .expect("historical queued request");

        Migration
            .up(&SchemaManager::new(&database))
            .await
            .expect("claim migration");

        assert!(
            database
                .execute(Statement::from_string(
                    DbBackend::Sqlite,
                    "UPDATE module_build_requests SET status = 'running' WHERE request_id = 'request'"
                        .to_string(),
                ))
                .await
                .is_err(),
            "a running build must retain a durable claim and lease"
        );
        Migration
            .down(&SchemaManager::new(&database))
            .await
            .expect("clean queue rolls back");
    }
}
