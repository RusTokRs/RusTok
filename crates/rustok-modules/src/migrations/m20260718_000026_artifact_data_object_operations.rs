use sea_orm::{ConnectionTrait, DbBackend, Statement};
use sea_orm_migration::prelude::*;

/// Makes object uploads idempotent without persisting their binary payload.
/// Operation rows retain only the owner-generated storage key and verified
/// result metadata, both protected by the same tenant boundary as the object.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let statements: &[&str] = match manager.get_database_backend() {
            DbBackend::Postgres => &[
                "CREATE TABLE module_artifact_data_object_operations (\
                    tenant_id UUID NOT NULL,\
                    module_slug TEXT NOT NULL,\
                    data_contract_revision BIGINT NOT NULL CHECK (data_contract_revision > 0),\
                    idempotency_key UUID NOT NULL,\
                    object_name TEXT NOT NULL CHECK (length(object_name) BETWEEN 1 AND 256),\
                    storage_key TEXT NOT NULL,\
                    content_type TEXT NOT NULL CHECK (length(content_type) BETWEEN 1 AND 128),\
                    size_bytes BIGINT NOT NULL CHECK (size_bytes > 0 AND size_bytes <= 33554432),\
                    digest_sha256 TEXT NOT NULL CHECK (digest_sha256 ~ '^sha256:[0-9A-Fa-f]{64}$'),\
                    expected_revision BIGINT NULL CHECK (expected_revision > 0),\
                    revision BIGINT NOT NULL CHECK (revision > 0),\
                    completed_at TIMESTAMPTZ NOT NULL,\
                    PRIMARY KEY (tenant_id, module_slug, data_contract_revision, idempotency_key)\
                )",
                "ALTER TABLE module_artifact_data_object_operations ENABLE ROW LEVEL SECURITY",
                "CREATE POLICY module_artifact_data_object_operations_scope ON module_artifact_data_object_operations \
                 USING (tenant_id::text = current_setting('rustok.tenant_id', true)) \
                 WITH CHECK (tenant_id::text = current_setting('rustok.tenant_id', true))",
            ],
            DbBackend::Sqlite => &["CREATE TABLE module_artifact_data_object_operations (\
                    tenant_id TEXT NOT NULL,\
                    module_slug TEXT NOT NULL,\
                    data_contract_revision INTEGER NOT NULL CHECK (data_contract_revision > 0),\
                    idempotency_key TEXT NOT NULL,\
                    object_name TEXT NOT NULL CHECK (length(object_name) BETWEEN 1 AND 256),\
                    storage_key TEXT NOT NULL,\
                    content_type TEXT NOT NULL CHECK (length(content_type) BETWEEN 1 AND 128),\
                    size_bytes INTEGER NOT NULL CHECK (size_bytes > 0 AND size_bytes <= 33554432),\
                    digest_sha256 TEXT NOT NULL CHECK (length(digest_sha256) = 71 AND substr(digest_sha256, 1, 7) = 'sha256:' AND substr(digest_sha256, 8) NOT GLOB '*[^0-9A-Fa-f]*'),\
                    expected_revision INTEGER NULL CHECK (expected_revision > 0),\
                    revision INTEGER NOT NULL CHECK (revision > 0),\
                    completed_at TEXT NOT NULL,\
                    PRIMARY KEY (tenant_id, module_slug, data_contract_revision, idempotency_key)\
                )"],
            backend => {
                return Err(DbErr::Migration(format!(
                    "artifact data object operation migration does not support database backend {backend:?}"
                )))
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
            .execute_unprepared("DROP TABLE module_artifact_data_object_operations")
            .await
    }
}
