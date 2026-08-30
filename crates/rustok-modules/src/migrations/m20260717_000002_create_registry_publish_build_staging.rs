use sea_orm::{ConnectionTrait, DbBackend, Statement};
use sea_orm_migration::prelude::*;

/// Records the immutable build result selected for one registry publication
/// stage. The record is append-only: a reupload must create a new stage rather
/// than rewriting the source/build identity behind an approved release.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let statements: &[&str] = match manager.get_database_backend() {
            DbBackend::Postgres => &[
                "CREATE TABLE registry_publish_build_staging (\
                    id TEXT PRIMARY KEY,\
                    request_id TEXT NOT NULL REFERENCES registry_publish_requests(id),\
                    expected_revision BIGINT NOT NULL CHECK (expected_revision > 0),\
                    tenant_id UUID NOT NULL,\
                    build_request_id UUID NOT NULL,\
                    source_digest TEXT NOT NULL CHECK (length(source_digest) = 71),\
                    parent_release_slug TEXT NULL CHECK (parent_release_slug IS NULL OR length(trim(parent_release_slug)) BETWEEN 1 AND 128),\
                    parent_release_version TEXT NULL CHECK (parent_release_version IS NULL OR length(trim(parent_release_version)) BETWEEN 1 AND 128),\
                    parent_release_digest TEXT NULL CHECK (parent_release_digest IS NULL OR length(parent_release_digest) = 71),\
                    component_digest TEXT NOT NULL CHECK (length(component_digest) = 71),\
                    artifact_manifest_digest TEXT NOT NULL CHECK (length(artifact_manifest_digest) = 71),\
                    sbom_manifest_digest TEXT NOT NULL CHECK (length(sbom_manifest_digest) = 71),\
                    provenance_manifest_digest TEXT NOT NULL CHECK (length(provenance_manifest_digest) = 71),\
                    signature_manifest_digest TEXT NOT NULL CHECK (length(signature_manifest_digest) = 71),\
                    staged_by_principal JSONB NOT NULL,\
                    actor_id UUID NOT NULL,\
                    trace_id TEXT NOT NULL CHECK (length(trim(trace_id)) BETWEEN 1 AND 512),\
                    correlation_id UUID NOT NULL,\
                    actor_can_manage_modules BOOLEAN NOT NULL,\
                    idempotency_key UUID NOT NULL,\
                    staged_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,\
                    CHECK ((parent_release_slug IS NULL AND parent_release_version IS NULL AND parent_release_digest IS NULL) \
                           OR (parent_release_slug IS NOT NULL AND parent_release_version IS NOT NULL AND parent_release_digest IS NOT NULL)),\
                    UNIQUE (request_id, idempotency_key)\
                )",
                "CREATE INDEX registry_publish_build_staging_request_current_idx \
                 ON registry_publish_build_staging (request_id, component_digest, staged_at DESC)",
            ],
            DbBackend::Sqlite => &[
                "CREATE TABLE registry_publish_build_staging (\
                    id TEXT PRIMARY KEY NOT NULL,\
                    request_id TEXT NOT NULL REFERENCES registry_publish_requests(id),\
                    expected_revision INTEGER NOT NULL CHECK (expected_revision > 0),\
                    tenant_id TEXT NOT NULL,\
                    build_request_id TEXT NOT NULL,\
                    source_digest TEXT NOT NULL CHECK (length(source_digest) = 71),\
                    parent_release_slug TEXT NULL CHECK (parent_release_slug IS NULL OR length(trim(parent_release_slug)) BETWEEN 1 AND 128),\
                    parent_release_version TEXT NULL CHECK (parent_release_version IS NULL OR length(trim(parent_release_version)) BETWEEN 1 AND 128),\
                    parent_release_digest TEXT NULL CHECK (parent_release_digest IS NULL OR length(parent_release_digest) = 71),\
                    component_digest TEXT NOT NULL CHECK (length(component_digest) = 71),\
                    artifact_manifest_digest TEXT NOT NULL CHECK (length(artifact_manifest_digest) = 71),\
                    sbom_manifest_digest TEXT NOT NULL CHECK (length(sbom_manifest_digest) = 71),\
                    provenance_manifest_digest TEXT NOT NULL CHECK (length(provenance_manifest_digest) = 71),\
                    signature_manifest_digest TEXT NOT NULL CHECK (length(signature_manifest_digest) = 71),\
                    staged_by_principal JSON NOT NULL,\
                    actor_id TEXT NOT NULL,\
                    trace_id TEXT NOT NULL CHECK (length(trim(trace_id)) BETWEEN 1 AND 512),\
                    correlation_id TEXT NOT NULL,\
                    actor_can_manage_modules BOOLEAN NOT NULL,\
                    idempotency_key TEXT NOT NULL,\
                    staged_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,\
                    CHECK ((parent_release_slug IS NULL AND parent_release_version IS NULL AND parent_release_digest IS NULL) \
                           OR (parent_release_slug IS NOT NULL AND parent_release_version IS NOT NULL AND parent_release_digest IS NOT NULL)),\
                    UNIQUE (request_id, idempotency_key)\
                )",
                "CREATE INDEX registry_publish_build_staging_request_current_idx \
                 ON registry_publish_build_staging (request_id, component_digest, staged_at DESC)",
            ],
            backend => {
                return Err(DbErr::Migration(format!(
                    "registry publish build staging does not support database backend {backend:?}"
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
            .execute_unprepared("DROP TABLE registry_publish_build_staging")
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
    async fn sqlite_staging_receipt_preserves_command_evidence_columns() {
        let database = Database::connect("sqlite::memory:")
            .await
            .expect("database");
        database
            .execute(Statement::from_string(
                DbBackend::Sqlite,
                "CREATE TABLE registry_publish_requests (id TEXT PRIMARY KEY)".to_string(),
            ))
            .await
            .expect("publish-request prerequisite");
        Migration
            .up(&SchemaManager::new(&database))
            .await
            .expect("staging migration");

        let columns = database
            .query_all(Statement::from_string(
                DbBackend::Sqlite,
                "PRAGMA table_info(registry_publish_build_staging)".to_string(),
            ))
            .await
            .expect("staging columns")
            .into_iter()
            .map(|row| row.try_get::<String>("", "name").expect("column name"))
            .collect::<Vec<_>>();
        for column in [
            "expected_revision",
            "tenant_id",
            "actor_id",
            "trace_id",
            "correlation_id",
            "actor_can_manage_modules",
            "idempotency_key",
            "parent_release_slug",
            "parent_release_version",
            "parent_release_digest",
        ] {
            assert!(columns.iter().any(|name| name == column), "{column}");
        }
    }
}
