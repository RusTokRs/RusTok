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
                    tenant_id UUID NOT NULL,\
                    build_request_id UUID NOT NULL,\
                    source_digest TEXT NOT NULL CHECK (length(source_digest) = 71),\
                    component_digest TEXT NOT NULL CHECK (length(component_digest) = 71),\
                    artifact_manifest_digest TEXT NOT NULL CHECK (length(artifact_manifest_digest) = 71),\
                    sbom_manifest_digest TEXT NOT NULL CHECK (length(sbom_manifest_digest) = 71),\
                    provenance_manifest_digest TEXT NOT NULL CHECK (length(provenance_manifest_digest) = 71),\
                    signature_manifest_digest TEXT NOT NULL CHECK (length(signature_manifest_digest) = 71),\
                    staged_by_principal JSONB NOT NULL,\
                    idempotency_key UUID NOT NULL,\
                    staged_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,\
                    UNIQUE (request_id, idempotency_key)\
                )",
                "CREATE INDEX registry_publish_build_staging_request_current_idx \
                 ON registry_publish_build_staging (request_id, component_digest, staged_at DESC)",
            ],
            DbBackend::Sqlite => &[
                "CREATE TABLE registry_publish_build_staging (\
                    id TEXT PRIMARY KEY NOT NULL,\
                    request_id TEXT NOT NULL REFERENCES registry_publish_requests(id),\
                    tenant_id TEXT NOT NULL,\
                    build_request_id TEXT NOT NULL,\
                    source_digest TEXT NOT NULL CHECK (length(source_digest) = 71),\
                    component_digest TEXT NOT NULL CHECK (length(component_digest) = 71),\
                    artifact_manifest_digest TEXT NOT NULL CHECK (length(artifact_manifest_digest) = 71),\
                    sbom_manifest_digest TEXT NOT NULL CHECK (length(sbom_manifest_digest) = 71),\
                    provenance_manifest_digest TEXT NOT NULL CHECK (length(provenance_manifest_digest) = 71),\
                    signature_manifest_digest TEXT NOT NULL CHECK (length(signature_manifest_digest) = 71),\
                    staged_by_principal JSON NOT NULL,\
                    idempotency_key TEXT NOT NULL,\
                    staged_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,\
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
