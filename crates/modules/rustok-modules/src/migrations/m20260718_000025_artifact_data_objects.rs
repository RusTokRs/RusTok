use sea_orm::{ConnectionTrait, DbBackend, Statement};
use sea_orm_migration::prelude::*;

use crate::backend::SupportedBackend;

/// Durable owner metadata for private brokered artifact objects. Artifact code
/// addresses only `object_name`; the physical storage key remains host-private.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let backend = SupportedBackend::new(manager.get_database_backend())?;
        let statements: &[&str] = match backend {
            SupportedBackend::Postgres => &[
                "CREATE TABLE module_artifact_data_objects (\
                    tenant_id UUID NOT NULL,\
                    data_owner_id UUID NOT NULL,\
                    namespace_instance_id UUID NOT NULL,\
                    object_name TEXT NOT NULL CHECK (length(object_name) BETWEEN 1 AND 256),\
                    storage_key TEXT NOT NULL UNIQUE,\
                    content_type TEXT NOT NULL CHECK (length(content_type) BETWEEN 1 AND 128),\
                    size_bytes BIGINT NOT NULL CHECK (size_bytes > 0 AND size_bytes <= 33554432),\
                    digest_sha256 TEXT NOT NULL CHECK (digest_sha256 ~ '^sha256:[0-9a-f]{64}$'),\
                    revision BIGINT NOT NULL CHECK (revision > 0),\
                    created_at TIMESTAMPTZ NOT NULL,\
                    updated_at TIMESTAMPTZ NOT NULL,\
                    PRIMARY KEY (tenant_id, data_owner_id, namespace_instance_id, object_name)\
                )",
                "ALTER TABLE module_artifact_data_objects ENABLE ROW LEVEL SECURITY",
                "CREATE POLICY module_artifact_data_objects_scope ON module_artifact_data_objects \
                 USING (tenant_id::text = current_setting('rustok.tenant_id', true)) \
                 WITH CHECK (tenant_id::text = current_setting('rustok.tenant_id', true))",
            ],
            SupportedBackend::Sqlite => &["CREATE TABLE module_artifact_data_objects (\
                    tenant_id TEXT NOT NULL,\
                    data_owner_id TEXT NOT NULL,\
                    namespace_instance_id TEXT NOT NULL,\
                    object_name TEXT NOT NULL CHECK (length(object_name) BETWEEN 1 AND 256),\
                    storage_key TEXT NOT NULL UNIQUE,\
                    content_type TEXT NOT NULL CHECK (length(content_type) BETWEEN 1 AND 128),\
                    size_bytes INTEGER NOT NULL CHECK (size_bytes > 0 AND size_bytes <= 33554432),\
                    digest_sha256 TEXT NOT NULL CHECK (length(digest_sha256) = 71 AND substr(digest_sha256, 1, 7) = 'sha256:' AND substr(digest_sha256, 8) NOT GLOB '*[^0-9a-f]*'),\
                    revision INTEGER NOT NULL CHECK (revision > 0),\
                    created_at TEXT NOT NULL,\
                    updated_at TEXT NOT NULL,\
                    PRIMARY KEY (tenant_id, data_owner_id, namespace_instance_id, object_name)\
                )"],
        };
        for statement in statements {
            manager
                .get_connection()
                .execute_raw(Statement::from_string(
                    backend.db_backend(),
                    (*statement).to_string(),
                ))
                .await?;
        }
        super::protect_artifact_data_namespace_writes(manager, "module_artifact_data_objects")
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared("DROP TABLE module_artifact_data_objects")
            .await?;
        super::drop_artifact_data_namespace_write_guard(manager, "module_artifact_data_objects")
            .await?;
        Ok(())
    }
}
