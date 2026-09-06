use sea_orm::{ConnectionTrait, DbBackend, Statement};
use sea_orm_migration::prelude::*;

/// Persists immutable Rhai authoring packages, linking reviewed Alloy revisions,
/// deterministic source-CAS receipts, and finalized artifact descriptors.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let statements: &[&str] = match manager.get_database_backend() {
            DbBackend::Postgres => &[
                "CREATE TABLE module_artifact_rhai_authoring_packages (\
                    package_id UUID PRIMARY KEY,\
                    tenant_id UUID NOT NULL,\
                    slug TEXT NOT NULL,\
                    version TEXT NOT NULL,\
                    alloy_script_id UUID NOT NULL,\
                    alloy_revision BIGINT NOT NULL CHECK (alloy_revision > 0),\
                    review_decision_id UUID NOT NULL,\
                    review_digest TEXT NOT NULL CHECK (review_digest ~ '^sha256:[0-9a-f]{64}$'),\
                    source_digest TEXT NOT NULL CHECK (source_digest ~ '^sha256:[0-9a-f]{64}$'),\
                    descriptor_digest TEXT NOT NULL CHECK (descriptor_digest ~ '^sha256:[0-9a-f]{64}$'),\
                    descriptor_json TEXT NOT NULL,\
                    idempotency_key UUID NOT NULL,\
                    actor_id UUID NOT NULL,\
                    trace_id TEXT NOT NULL CHECK (length(trim(trace_id)) BETWEEN 1 AND 512),\
                    created_at TIMESTAMPTZ NOT NULL,\
                    UNIQUE (tenant_id, slug, version),\
                    UNIQUE (tenant_id, alloy_script_id, alloy_revision, idempotency_key)\
                )",
                "CREATE INDEX idx_rhai_authoring_packages_scope \
                 ON module_artifact_rhai_authoring_packages (tenant_id, slug, version)",
                "ALTER TABLE module_artifact_rhai_authoring_packages ENABLE ROW LEVEL SECURITY",
                "CREATE POLICY module_artifact_rhai_authoring_packages_scope \
                 ON module_artifact_rhai_authoring_packages \
                 USING (tenant_id::text = current_setting('rustok.tenant_id', true)) \
                 WITH CHECK (tenant_id::text = current_setting('rustok.tenant_id', true))",
            ],
            DbBackend::Sqlite => &[
                "CREATE TABLE module_artifact_rhai_authoring_packages (\
                    package_id TEXT PRIMARY KEY NOT NULL,\
                    tenant_id TEXT NOT NULL,\
                    slug TEXT NOT NULL,\
                    version TEXT NOT NULL,\
                    alloy_script_id TEXT NOT NULL,\
                    alloy_revision INTEGER NOT NULL CHECK (alloy_revision > 0),\
                    review_decision_id TEXT NOT NULL,\
                    review_digest TEXT NOT NULL CHECK (length(review_digest) = 71),\
                    source_digest TEXT NOT NULL CHECK (length(source_digest) = 71),\
                    descriptor_digest TEXT NOT NULL CHECK (length(descriptor_digest) = 71),\
                    descriptor_json TEXT NOT NULL,\
                    idempotency_key TEXT NOT NULL,\
                    actor_id TEXT NOT NULL,\
                    trace_id TEXT NOT NULL CHECK (length(trim(trace_id)) BETWEEN 1 AND 512),\
                    created_at TEXT NOT NULL,\
                    UNIQUE (tenant_id, slug, version),\
                    UNIQUE (tenant_id, alloy_script_id, alloy_revision, idempotency_key)\
                )",
                "CREATE INDEX idx_rhai_authoring_packages_scope \
                 ON module_artifact_rhai_authoring_packages (tenant_id, slug, version)",
            ],
            _ => return Err(DbErr::Custom("Unsupported database backend".to_string())),
        };

        for sql in statements {
            manager
                .get_connection()
                .execute_raw(Statement::from_string(
                    manager.get_database_backend(),
                    (*sql).to_string(),
                ))
                .await?;
        }

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let statements = &[
            "DROP TABLE IF EXISTS module_artifact_rhai_authoring_packages",
        ];

        for sql in statements {
            manager
                .get_connection()
                .execute_raw(Statement::from_string(
                    manager.get_database_backend(),
                    (*sql).to_string(),
                ))
                .await?;
        }

        Ok(())
    }
}
