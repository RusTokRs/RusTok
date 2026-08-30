use sea_orm::{ConnectionTrait, DbBackend, Statement};
use sea_orm_migration::prelude::*;

/// Adds immutable source lineage for every publication origin, including
/// reviewed Alloy forks, and stores the exact installable artifact contract
/// beside each published registry release.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let statements: &[&str] = match manager.get_database_backend() {
            DbBackend::Postgres => &[
                "ALTER TABLE registry_publish_build_staging \
                 ADD COLUMN source_reference TEXT NULL \
                 CHECK (source_reference IS NULL OR length(trim(source_reference)) BETWEEN 1 AND 512)",
                "CREATE TABLE registry_publish_alloy_staging (\
                    id TEXT PRIMARY KEY,\
                    request_id TEXT NOT NULL REFERENCES registry_publish_requests(id) ON DELETE RESTRICT,\
                    expected_revision BIGINT NOT NULL CHECK (expected_revision > 0),\
                    alloy_tenant_id UUID NOT NULL,\
                    alloy_script_id UUID NOT NULL,\
                    artifact_digest TEXT NOT NULL CHECK (length(artifact_digest) = 71),\
                    source_digest TEXT NOT NULL CHECK (length(source_digest) = 71),\
                    source_revision BIGINT NOT NULL CHECK (source_revision > 0),\
                    parent_release_slug TEXT NULL CHECK (parent_release_slug IS NULL OR length(trim(parent_release_slug)) BETWEEN 1 AND 128),\
                    parent_release_version TEXT NULL CHECK (parent_release_version IS NULL OR length(trim(parent_release_version)) BETWEEN 1 AND 128),\
                    parent_release_digest TEXT NULL CHECK (parent_release_digest IS NULL OR length(parent_release_digest) = 71),\
                    review_reference TEXT NOT NULL CHECK (length(trim(review_reference)) BETWEEN 1 AND 512),\
                    review_digest TEXT NOT NULL CHECK (length(review_digest) = 71),\
                    review_policy_revision TEXT NOT NULL CHECK (length(trim(review_policy_revision)) BETWEEN 1 AND 128),\
                    reviewed_by_principal JSONB NOT NULL,\
                    sandbox_execution_id UUID NOT NULL,\
                    sandbox_test_path TEXT NOT NULL CHECK (length(trim(sandbox_test_path)) BETWEEN 1 AND 512),\
                    sandbox_executor TEXT NOT NULL CHECK (length(trim(sandbox_executor)) BETWEEN 1 AND 64),\
                    sandbox_scenario_digest TEXT NOT NULL CHECK (length(sandbox_scenario_digest) = 71),\
                    sandbox_runtime_abi TEXT NOT NULL CHECK (length(trim(sandbox_runtime_abi)) BETWEEN 1 AND 128),\
                    sandbox_policy_digest TEXT NOT NULL CHECK (length(sandbox_policy_digest) = 71),\
                    sandbox_capability_grants INTEGER NOT NULL CHECK (sandbox_capability_grants >= 0),\
                    staged_by_principal JSONB NOT NULL,\
                    actor_id UUID NOT NULL,\
                    trace_id TEXT NOT NULL CHECK (length(trim(trace_id)) BETWEEN 1 AND 512),\
                    correlation_id UUID NOT NULL,\
                    idempotency_key UUID NOT NULL,\
                    staged_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,\
                    CHECK ((parent_release_slug IS NULL AND parent_release_version IS NULL AND parent_release_digest IS NULL) \
                           OR (parent_release_slug IS NOT NULL AND parent_release_version IS NOT NULL AND parent_release_digest IS NOT NULL)),\
                    UNIQUE (request_id, idempotency_key)\
                )",
                "CREATE INDEX registry_publish_alloy_staging_request_current_idx \
                 ON registry_publish_alloy_staging (request_id, artifact_digest, staged_at DESC)",
                "CREATE TABLE registry_module_release_artifacts (\
                    release_id TEXT PRIMARY KEY REFERENCES registry_module_releases(id) ON DELETE RESTRICT,\
                    request_id TEXT NOT NULL UNIQUE REFERENCES registry_publish_requests(id) ON DELETE RESTRICT,\
                    artifact JSONB NOT NULL,\
                    descriptor JSONB NOT NULL,\
                    lineage JSONB NOT NULL,\
                    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP\
                )",
            ],
            DbBackend::Sqlite => &[
                "ALTER TABLE registry_publish_build_staging \
                 ADD COLUMN source_reference TEXT NULL \
                 CHECK (source_reference IS NULL OR length(trim(source_reference)) BETWEEN 1 AND 512)",
                "CREATE TABLE registry_publish_alloy_staging (\
                    id TEXT PRIMARY KEY NOT NULL,\
                    request_id TEXT NOT NULL REFERENCES registry_publish_requests(id) ON DELETE RESTRICT,\
                    expected_revision INTEGER NOT NULL CHECK (expected_revision > 0),\
                    alloy_tenant_id TEXT NOT NULL,\
                    alloy_script_id TEXT NOT NULL,\
                    artifact_digest TEXT NOT NULL CHECK (length(artifact_digest) = 71),\
                    source_digest TEXT NOT NULL CHECK (length(source_digest) = 71),\
                    source_revision INTEGER NOT NULL CHECK (source_revision > 0),\
                    parent_release_slug TEXT NULL CHECK (parent_release_slug IS NULL OR length(trim(parent_release_slug)) BETWEEN 1 AND 128),\
                    parent_release_version TEXT NULL CHECK (parent_release_version IS NULL OR length(trim(parent_release_version)) BETWEEN 1 AND 128),\
                    parent_release_digest TEXT NULL CHECK (parent_release_digest IS NULL OR length(parent_release_digest) = 71),\
                    review_reference TEXT NOT NULL CHECK (length(trim(review_reference)) BETWEEN 1 AND 512),\
                    review_digest TEXT NOT NULL CHECK (length(review_digest) = 71),\
                    review_policy_revision TEXT NOT NULL CHECK (length(trim(review_policy_revision)) BETWEEN 1 AND 128),\
                    reviewed_by_principal JSON NOT NULL,\
                    sandbox_execution_id TEXT NOT NULL,\
                    sandbox_test_path TEXT NOT NULL CHECK (length(trim(sandbox_test_path)) BETWEEN 1 AND 512),\
                    sandbox_executor TEXT NOT NULL CHECK (length(trim(sandbox_executor)) BETWEEN 1 AND 64),\
                    sandbox_scenario_digest TEXT NOT NULL CHECK (length(sandbox_scenario_digest) = 71),\
                    sandbox_runtime_abi TEXT NOT NULL CHECK (length(trim(sandbox_runtime_abi)) BETWEEN 1 AND 128),\
                    sandbox_policy_digest TEXT NOT NULL CHECK (length(sandbox_policy_digest) = 71),\
                    sandbox_capability_grants INTEGER NOT NULL CHECK (sandbox_capability_grants >= 0),\
                    staged_by_principal JSON NOT NULL,\
                    actor_id TEXT NOT NULL,\
                    trace_id TEXT NOT NULL CHECK (length(trim(trace_id)) BETWEEN 1 AND 512),\
                    correlation_id TEXT NOT NULL,\
                    idempotency_key TEXT NOT NULL,\
                    staged_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,\
                    CHECK ((parent_release_slug IS NULL AND parent_release_version IS NULL AND parent_release_digest IS NULL) \
                           OR (parent_release_slug IS NOT NULL AND parent_release_version IS NOT NULL AND parent_release_digest IS NOT NULL)),\
                    UNIQUE (request_id, idempotency_key)\
                )",
                "CREATE INDEX registry_publish_alloy_staging_request_current_idx \
                 ON registry_publish_alloy_staging (request_id, artifact_digest, staged_at DESC)",
                "CREATE TABLE registry_module_release_artifacts (\
                    release_id TEXT PRIMARY KEY NOT NULL REFERENCES registry_module_releases(id) ON DELETE RESTRICT,\
                    request_id TEXT NOT NULL UNIQUE REFERENCES registry_publish_requests(id) ON DELETE RESTRICT,\
                    artifact JSON NOT NULL,\
                    descriptor JSON NOT NULL,\
                    lineage JSON NOT NULL,\
                    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP\
                )",
            ],
            backend => {
                return Err(DbErr::Migration(format!(
                    "registry release artifact contracts do not support database backend {backend:?}"
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
            .execute_unprepared("DROP TABLE registry_module_release_artifacts")
            .await?;
        manager
            .get_connection()
            .execute_unprepared("DROP TABLE registry_publish_alloy_staging")
            .await?;
        manager
            .get_connection()
            .execute_unprepared(
                "ALTER TABLE registry_publish_build_staging DROP COLUMN source_reference",
            )
            .await
            .map(|_| ())
    }
}

#[cfg(test)]
mod tests {
    use sea_orm::{ConnectionTrait, Database, DbBackend, Statement};
    use sea_orm_migration::prelude::{MigrationTrait, SchemaManager};

    use super::Migration;

    #[tokio::test]
    async fn sqlite_alloy_staging_receipt_preserves_command_context_columns() {
        let database = Database::connect("sqlite::memory:")
            .await
            .expect("database");
        for statement in [
            "CREATE TABLE registry_publish_build_staging (id TEXT PRIMARY KEY)",
            "CREATE TABLE registry_publish_requests (id TEXT PRIMARY KEY)",
            "CREATE TABLE registry_module_releases (id TEXT PRIMARY KEY)",
        ] {
            database
                .execute(Statement::from_string(
                    DbBackend::Sqlite,
                    statement.to_string(),
                ))
                .await
                .expect("migration prerequisite");
        }
        Migration
            .up(&SchemaManager::new(&database))
            .await
            .expect("release artifact migration");

        let columns = database
            .query_all(Statement::from_string(
                DbBackend::Sqlite,
                "PRAGMA table_info(registry_publish_alloy_staging)".to_string(),
            ))
            .await
            .expect("alloy staging columns")
            .into_iter()
            .map(|row| row.try_get::<String>("", "name").expect("column name"))
            .collect::<Vec<_>>();
        for column in [
            "expected_revision",
            "actor_id",
            "trace_id",
            "correlation_id",
            "idempotency_key",
            "sandbox_scenario_digest",
        ] {
            assert!(columns.iter().any(|name| name == column), "{column}");
        }
    }
}
