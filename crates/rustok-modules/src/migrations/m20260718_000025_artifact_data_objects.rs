use sea_orm::{ConnectionTrait, DbBackend, Statement};
use sea_orm_migration::prelude::*;

/// Durable owner metadata for private brokered artifact objects. Artifact code
/// addresses only `object_name`; the physical storage key remains host-private.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let statements: &[&str] = match manager.get_database_backend() {
            DbBackend::Postgres => &[
                "CREATE TABLE module_artifact_data_objects (\
                    tenant_id UUID NOT NULL,\
                    module_slug TEXT NOT NULL,\
                    data_contract_revision BIGINT NOT NULL CHECK (data_contract_revision > 0),\
                    object_name TEXT NOT NULL CHECK (length(object_name) BETWEEN 1 AND 256),\
                    storage_key TEXT NOT NULL UNIQUE,\
                    content_type TEXT NOT NULL CHECK (length(content_type) BETWEEN 1 AND 128),\
                    size_bytes BIGINT NOT NULL CHECK (size_bytes > 0 AND size_bytes <= 33554432),\
                    digest_sha256 TEXT NOT NULL CHECK (digest_sha256 ~ '^sha256:[0-9A-Fa-f]{64}$'),\
                    revision BIGINT NOT NULL CHECK (revision > 0),\
                    created_at TIMESTAMPTZ NOT NULL,\
                    updated_at TIMESTAMPTZ NOT NULL,\
                    PRIMARY KEY (tenant_id, module_slug, data_contract_revision, object_name)\
                )",
                "ALTER TABLE module_artifact_data_objects ENABLE ROW LEVEL SECURITY",
                "CREATE POLICY module_artifact_data_objects_scope ON module_artifact_data_objects \
                 USING (tenant_id::text = current_setting('rustok.tenant_id', true)) \
                 WITH CHECK (tenant_id::text = current_setting('rustok.tenant_id', true))",
            ],
            DbBackend::Sqlite => &["CREATE TABLE module_artifact_data_objects (\
                    tenant_id TEXT NOT NULL,\
                    module_slug TEXT NOT NULL,\
                    data_contract_revision INTEGER NOT NULL CHECK (data_contract_revision > 0),\
                    object_name TEXT NOT NULL CHECK (length(object_name) BETWEEN 1 AND 256),\
                    storage_key TEXT NOT NULL UNIQUE,\
                    content_type TEXT NOT NULL CHECK (length(content_type) BETWEEN 1 AND 128),\
                    size_bytes INTEGER NOT NULL CHECK (size_bytes > 0 AND size_bytes <= 33554432),\
                    digest_sha256 TEXT NOT NULL CHECK (length(digest_sha256) = 71 AND substr(digest_sha256, 1, 7) = 'sha256:' AND substr(digest_sha256, 8) NOT GLOB '*[^0-9A-Fa-f]*'),\
                    revision INTEGER NOT NULL CHECK (revision > 0),\
                    created_at TEXT NOT NULL,\
                    updated_at TEXT NOT NULL,\
                    PRIMARY KEY (tenant_id, module_slug, data_contract_revision, object_name)\
                )"],
            backend => {
                return Err(DbErr::Migration(format!(
                    "artifact data object migration does not support database backend {backend:?}"
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
            .execute_unprepared("DROP TABLE module_artifact_data_objects")
            .await?;
        Ok(())
    }
}
