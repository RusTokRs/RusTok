use sea_orm::{ConnectionTrait, DbBackend, Statement};
use sea_orm_migration::prelude::*;

/// Makes artifact origin an explicit durable release fact and records the
/// stricter provenance/quarantine decision required for external prebuilts and
/// production-sandbox evidence required for Alloy-authored artifacts.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let statements: &[&str] = match manager.get_database_backend() {
            DbBackend::Postgres => &[
                "ALTER TABLE registry_publish_requests \
                 ADD COLUMN artifact_origin TEXT NOT NULL DEFAULT 'unclassified' \
                 CHECK (artifact_origin IN ('unclassified', 'platform_built', 'external_prebuilt', 'alloy_authored'))",
                "ALTER TABLE registry_module_releases \
                 ADD COLUMN artifact_origin TEXT NOT NULL DEFAULT 'unclassified' \
                 CHECK (artifact_origin IN ('unclassified', 'platform_built', 'external_prebuilt', 'alloy_authored'))",
                "CREATE TABLE registry_publish_external_staging (\
                    id TEXT PRIMARY KEY,\
                    request_id TEXT NOT NULL REFERENCES registry_publish_requests(id),\
                    expected_revision BIGINT NOT NULL CHECK (expected_revision > 0),\
                    artifact_digest TEXT NOT NULL CHECK (length(artifact_digest) = 71),\
                    source_evidence_kind TEXT NOT NULL CHECK (source_evidence_kind IN ('reproducible', 'unavailable')),\
                    source_reference TEXT NULL,\
                    source_digest TEXT NULL CHECK (source_digest IS NULL OR length(source_digest) = 71),\
                    source_absence_reason TEXT NULL,\
                    provenance_reference TEXT NOT NULL,\
                    provenance_digest TEXT NOT NULL CHECK (length(provenance_digest) = 71),\
                    provenance_policy_revision TEXT NOT NULL,\
                    quarantine_review_reference TEXT NOT NULL,\
                    quarantine_policy_revision TEXT NOT NULL,\
                    quarantine_approved_by_principal JSONB NOT NULL,\
                    staged_by_principal JSONB NOT NULL,\
                    actor_id UUID NOT NULL,\
                    trace_id TEXT NOT NULL CHECK (length(trim(trace_id)) BETWEEN 1 AND 512),\
                    correlation_id UUID NOT NULL,\
                    actor_can_manage_modules BOOLEAN NOT NULL,\
                    idempotency_key UUID NOT NULL,\
                    staged_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,\
                    CHECK (\
                        (source_evidence_kind = 'reproducible' AND source_reference IS NOT NULL AND source_digest IS NOT NULL AND source_absence_reason IS NULL)\
                        OR (source_evidence_kind = 'unavailable' AND source_reference IS NULL AND source_digest IS NULL AND source_absence_reason IS NOT NULL)\
                    ),\
                    UNIQUE (request_id, idempotency_key)\
                )",
                "CREATE INDEX registry_publish_external_staging_request_current_idx \
                 ON registry_publish_external_staging (request_id, artifact_digest, staged_at DESC)",
            ],
            DbBackend::Sqlite => &[
                "ALTER TABLE registry_publish_requests \
                 ADD COLUMN artifact_origin TEXT NOT NULL DEFAULT 'unclassified' \
                 CHECK (artifact_origin IN ('unclassified', 'platform_built', 'external_prebuilt', 'alloy_authored'))",
                "ALTER TABLE registry_module_releases \
                 ADD COLUMN artifact_origin TEXT NOT NULL DEFAULT 'unclassified' \
                 CHECK (artifact_origin IN ('unclassified', 'platform_built', 'external_prebuilt', 'alloy_authored'))",
                "CREATE TABLE registry_publish_external_staging (\
                    id TEXT PRIMARY KEY NOT NULL,\
                    request_id TEXT NOT NULL REFERENCES registry_publish_requests(id),\
                    expected_revision INTEGER NOT NULL CHECK (expected_revision > 0),\
                    artifact_digest TEXT NOT NULL CHECK (length(artifact_digest) = 71),\
                    source_evidence_kind TEXT NOT NULL CHECK (source_evidence_kind IN ('reproducible', 'unavailable')),\
                    source_reference TEXT NULL,\
                    source_digest TEXT NULL CHECK (source_digest IS NULL OR length(source_digest) = 71),\
                    source_absence_reason TEXT NULL,\
                    provenance_reference TEXT NOT NULL,\
                    provenance_digest TEXT NOT NULL CHECK (length(provenance_digest) = 71),\
                    provenance_policy_revision TEXT NOT NULL,\
                    quarantine_review_reference TEXT NOT NULL,\
                    quarantine_policy_revision TEXT NOT NULL,\
                    quarantine_approved_by_principal JSON NOT NULL,\
                    staged_by_principal JSON NOT NULL,\
                    actor_id TEXT NOT NULL,\
                    trace_id TEXT NOT NULL CHECK (length(trim(trace_id)) BETWEEN 1 AND 512),\
                    correlation_id TEXT NOT NULL,\
                    actor_can_manage_modules BOOLEAN NOT NULL,\
                    idempotency_key TEXT NOT NULL,\
                    staged_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,\
                    CHECK (\
                        (source_evidence_kind = 'reproducible' AND source_reference IS NOT NULL AND source_digest IS NOT NULL AND source_absence_reason IS NULL)\
                        OR (source_evidence_kind = 'unavailable' AND source_reference IS NULL AND source_digest IS NULL AND source_absence_reason IS NOT NULL)\
                    ),\
                    UNIQUE (request_id, idempotency_key)\
                )",
                "CREATE INDEX registry_publish_external_staging_request_current_idx \
                 ON registry_publish_external_staging (request_id, artifact_digest, staged_at DESC)",
            ],
            backend => {
                return Err(DbErr::Migration(format!(
                    "registry external artifact staging does not support database backend {backend:?}"
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
        let backend = manager.get_database_backend();
        for statement in [
            "DROP TABLE registry_publish_external_staging",
            "ALTER TABLE registry_module_releases DROP COLUMN artifact_origin",
            "ALTER TABLE registry_publish_requests DROP COLUMN artifact_origin",
        ] {
            manager
                .get_connection()
                .execute(Statement::from_string(backend, statement.to_string()))
                .await?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::Migration;
    use sea_orm::{ConnectionTrait, Database, DbBackend, Statement};
    use sea_orm_migration::{MigrationTrait, SchemaManager};

    #[tokio::test]
    async fn sqlite_external_prebuilt_staging_persists_command_context_receipt_fields() {
        let database = Database::connect("sqlite::memory:")
            .await
            .expect("database");
        for statement in [
            "CREATE TABLE registry_publish_requests (id TEXT PRIMARY KEY)",
            "CREATE TABLE registry_module_releases (id TEXT PRIMARY KEY)",
        ] {
            database
                .execute(Statement::from_string(
                    DbBackend::Sqlite,
                    statement.to_string(),
                ))
                .await
                .expect("migration prerequisite schema");
        }

        let manager = SchemaManager::new(&database);
        Migration
            .up(&manager)
            .await
            .expect("external prebuilt staging migration");
        let fields = database
            .query_all(Statement::from_string(
                DbBackend::Sqlite,
                "PRAGMA table_info(registry_publish_external_staging)".to_string(),
            ))
            .await
            .expect("staging schema query")
            .into_iter()
            .map(|row| row.try_get::<String>("", "name").expect("field name"))
            .collect::<Vec<_>>();

        for field in [
            "expected_revision",
            "actor_id",
            "trace_id",
            "correlation_id",
            "actor_can_manage_modules",
            "idempotency_key",
        ] {
            assert!(
                fields.iter().any(|candidate| candidate == field),
                "missing {field}"
            );
        }
    }
}
