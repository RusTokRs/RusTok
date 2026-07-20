use sea_orm::{ConnectionTrait, DbBackend, Statement};
use sea_orm_migration::prelude::*;

/// Durable, owner-owned resumable upload state. Chunks are private storage
/// objects; the tables retain only their verified metadata and ordering.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let statements: &[&str] = match manager.get_database_backend() {
            DbBackend::Postgres => &[
                "CREATE TABLE module_artifact_data_object_upload_sessions (\
                    session_id UUID PRIMARY KEY,\
                    tenant_id UUID NOT NULL,\
                    module_slug TEXT NOT NULL,\
                    data_contract_revision BIGINT NOT NULL CHECK (data_contract_revision > 0),\
                    policy_revision BIGINT NOT NULL CHECK (policy_revision > 0),\
                    object_name TEXT NOT NULL CHECK (length(object_name) BETWEEN 1 AND 256),\
                    content_type TEXT NOT NULL CHECK (length(content_type) BETWEEN 1 AND 128),\
                    expected_revision BIGINT NULL CHECK (expected_revision > 0),\
                    idempotency_key UUID NOT NULL,\
                    request_digest TEXT NOT NULL CHECK (request_digest ~ '^sha256:[0-9a-f]{64}$'),\
                    status TEXT NOT NULL CHECK (status IN ('open', 'completing', 'completed', 'abandoned')),\
                    expires_at TIMESTAMPTZ NOT NULL,\
                    completed_revision BIGINT NULL CHECK (completed_revision > 0),\
                    created_at TIMESTAMPTZ NOT NULL,\
                    updated_at TIMESTAMPTZ NOT NULL,\
                    completed_at TIMESTAMPTZ NULL,\
                    UNIQUE (tenant_id, module_slug, data_contract_revision, policy_revision, idempotency_key)\
                )",
                "CREATE INDEX module_artifact_data_object_upload_sessions_expiry_idx \
                 ON module_artifact_data_object_upload_sessions (tenant_id, expires_at, session_id)",
                "CREATE TABLE module_artifact_data_object_upload_chunks (\
                    tenant_id UUID NOT NULL,\
                    session_id UUID NOT NULL REFERENCES module_artifact_data_object_upload_sessions (session_id) ON DELETE CASCADE,\
                    sequence BIGINT NOT NULL CHECK (sequence >= 0),\
                    storage_key TEXT NOT NULL UNIQUE,\
                    size_bytes BIGINT NOT NULL CHECK (size_bytes > 0 AND size_bytes <= 45056),\
                    digest_sha256 TEXT NOT NULL CHECK (digest_sha256 ~ '^sha256:[0-9a-f]{64}$'),\
                    created_at TIMESTAMPTZ NOT NULL,\
                    PRIMARY KEY (tenant_id, session_id, sequence)\
                )",
                "ALTER TABLE module_artifact_data_object_upload_sessions ENABLE ROW LEVEL SECURITY",
                "ALTER TABLE module_artifact_data_object_upload_chunks ENABLE ROW LEVEL SECURITY",
                "CREATE POLICY module_artifact_data_object_upload_sessions_scope \
                 ON module_artifact_data_object_upload_sessions \
                 USING (tenant_id::text = current_setting('rustok.tenant_id', true)) \
                 WITH CHECK (tenant_id::text = current_setting('rustok.tenant_id', true))",
                "CREATE POLICY module_artifact_data_object_upload_chunks_scope \
                 ON module_artifact_data_object_upload_chunks \
                 USING (tenant_id::text = current_setting('rustok.tenant_id', true)) \
                 WITH CHECK (tenant_id::text = current_setting('rustok.tenant_id', true))",
            ],
            DbBackend::Sqlite => &[
                "CREATE TABLE module_artifact_data_object_upload_sessions (\
                    session_id TEXT PRIMARY KEY,\
                    tenant_id TEXT NOT NULL,\
                    module_slug TEXT NOT NULL,\
                    data_contract_revision INTEGER NOT NULL CHECK (data_contract_revision > 0),\
                    policy_revision INTEGER NOT NULL CHECK (policy_revision > 0),\
                    object_name TEXT NOT NULL CHECK (length(object_name) BETWEEN 1 AND 256),\
                    content_type TEXT NOT NULL CHECK (length(content_type) BETWEEN 1 AND 128),\
                    expected_revision INTEGER NULL CHECK (expected_revision > 0),\
                    idempotency_key TEXT NOT NULL,\
                    request_digest TEXT NOT NULL CHECK (length(request_digest) = 71 AND substr(request_digest, 1, 7) = 'sha256:' AND substr(request_digest, 8) NOT GLOB '*[^0-9a-f]*'),\
                    status TEXT NOT NULL CHECK (status IN ('open', 'completing', 'completed', 'abandoned')),\
                    expires_at TEXT NOT NULL,\
                    completed_revision INTEGER NULL CHECK (completed_revision > 0),\
                    created_at TEXT NOT NULL,\
                    updated_at TEXT NOT NULL,\
                    completed_at TEXT NULL,\
                    UNIQUE (tenant_id, module_slug, data_contract_revision, policy_revision, idempotency_key)\
                )",
                "CREATE INDEX module_artifact_data_object_upload_sessions_expiry_idx \
                 ON module_artifact_data_object_upload_sessions (tenant_id, expires_at, session_id)",
                "CREATE TABLE module_artifact_data_object_upload_chunks (\
                    tenant_id TEXT NOT NULL,\
                    session_id TEXT NOT NULL REFERENCES module_artifact_data_object_upload_sessions (session_id) ON DELETE CASCADE,\
                    sequence INTEGER NOT NULL CHECK (sequence >= 0),\
                    storage_key TEXT NOT NULL UNIQUE,\
                    size_bytes INTEGER NOT NULL CHECK (size_bytes > 0 AND size_bytes <= 45056),\
                    digest_sha256 TEXT NOT NULL CHECK (length(digest_sha256) = 71 AND substr(digest_sha256, 1, 7) = 'sha256:' AND substr(digest_sha256, 8) NOT GLOB '*[^0-9a-f]*'),\
                    created_at TEXT NOT NULL,\
                    PRIMARY KEY (tenant_id, session_id, sequence)\
                )",
            ],
            backend => return Err(DbErr::Migration(format!(
                "artifact data object upload session migration does not support database backend {backend:?}"
            ))),
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
            .execute_unprepared("DROP TABLE module_artifact_data_object_upload_chunks")
            .await?;
        manager
            .get_connection()
            .execute_unprepared("DROP TABLE module_artifact_data_object_upload_sessions")
            .await?;
        Ok(())
    }
}
