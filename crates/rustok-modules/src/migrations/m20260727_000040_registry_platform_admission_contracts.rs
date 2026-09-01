use sea_orm::{ConnectionTrait, DbBackend, Statement};
use sea_orm_migration::prelude::*;

/// Retains the complete immutable platform-admission contract needed to
/// materialize an installable registry release. The generic evidence ledger
/// remains append-only audit evidence; it is not a descriptor or OCI identity
/// store.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let statements: &[&str] = match manager.get_database_backend() {
            DbBackend::Postgres => &["CREATE TABLE registry_publish_platform_admissions (\
                    request_id TEXT PRIMARY KEY REFERENCES registry_publish_requests(id) ON DELETE RESTRICT,\
                    registry_id TEXT NOT NULL CHECK (length(trim(registry_id)) BETWEEN 1 AND 96),\
                    registry TEXT NOT NULL CHECK (length(trim(registry)) BETWEEN 1 AND 255),\
                    repository TEXT NOT NULL CHECK (length(trim(repository)) BETWEEN 1 AND 512),\
                    manifest_digest TEXT NOT NULL CHECK (length(manifest_digest) = 71),\
                    payload_digest TEXT NOT NULL CHECK (length(payload_digest) = 71),\
                    descriptor_digest TEXT NOT NULL CHECK (length(descriptor_digest) = 71),\
                    descriptor JSONB NOT NULL,\
                    runtime_kind TEXT NOT NULL CHECK (runtime_kind IN ('rhai', 'wasm_component', 'sidecar')),\
                    media_type TEXT NOT NULL CHECK (length(trim(media_type)) BETWEEN 1 AND 255),\
                    signature_reference TEXT NOT NULL CHECK (length(trim(signature_reference)) BETWEEN 1 AND 512),\
                    signature_digest TEXT NOT NULL CHECK (length(signature_digest) = 71),\
                    provenance_reference TEXT NOT NULL CHECK (length(trim(provenance_reference)) BETWEEN 1 AND 512),\
                    provenance_digest TEXT NOT NULL CHECK (length(provenance_digest) = 71),\
                    sbom_reference TEXT NOT NULL CHECK (length(trim(sbom_reference)) BETWEEN 1 AND 512),\
                    sbom_digest TEXT NOT NULL CHECK (length(sbom_digest) = 71),\
                    admission_reference TEXT NOT NULL CHECK (length(trim(admission_reference)) BETWEEN 1 AND 512),\
                    admission_digest TEXT NOT NULL CHECK (length(admission_digest) = 64),\
                    recorded_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP\
                )"],
            DbBackend::Sqlite => &["CREATE TABLE registry_publish_platform_admissions (\
                    request_id TEXT PRIMARY KEY NOT NULL REFERENCES registry_publish_requests(id) ON DELETE RESTRICT,\
                    registry_id TEXT NOT NULL CHECK (length(trim(registry_id)) BETWEEN 1 AND 96),\
                    registry TEXT NOT NULL CHECK (length(trim(registry)) BETWEEN 1 AND 255),\
                    repository TEXT NOT NULL CHECK (length(trim(repository)) BETWEEN 1 AND 512),\
                    manifest_digest TEXT NOT NULL CHECK (length(manifest_digest) = 71),\
                    payload_digest TEXT NOT NULL CHECK (length(payload_digest) = 71),\
                    descriptor_digest TEXT NOT NULL CHECK (length(descriptor_digest) = 71),\
                    descriptor JSON NOT NULL,\
                    runtime_kind TEXT NOT NULL CHECK (runtime_kind IN ('rhai', 'wasm_component', 'sidecar')),\
                    media_type TEXT NOT NULL CHECK (length(trim(media_type)) BETWEEN 1 AND 255),\
                    signature_reference TEXT NOT NULL CHECK (length(trim(signature_reference)) BETWEEN 1 AND 512),\
                    signature_digest TEXT NOT NULL CHECK (length(signature_digest) = 71),\
                    provenance_reference TEXT NOT NULL CHECK (length(trim(provenance_reference)) BETWEEN 1 AND 512),\
                    provenance_digest TEXT NOT NULL CHECK (length(provenance_digest) = 71),\
                    sbom_reference TEXT NOT NULL CHECK (length(trim(sbom_reference)) BETWEEN 1 AND 512),\
                    sbom_digest TEXT NOT NULL CHECK (length(sbom_digest) = 71),\
                    admission_reference TEXT NOT NULL CHECK (length(trim(admission_reference)) BETWEEN 1 AND 512),\
                    admission_digest TEXT NOT NULL CHECK (length(admission_digest) = 64),\
                    recorded_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP\
                )"],
            backend => {
                return Err(DbErr::Migration(format!(
                    "registry platform admission contracts do not support database backend {backend:?}"
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
            .execute_unprepared("DROP TABLE registry_publish_platform_admissions")
            .await
            .map(|_| ())
    }
}
