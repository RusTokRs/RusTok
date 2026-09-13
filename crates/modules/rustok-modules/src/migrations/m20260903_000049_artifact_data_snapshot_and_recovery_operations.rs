use sea_orm::{ConnectionTrait, DbBackend, Statement};
use sea_orm_migration::prelude::*;

/// Persists crash-safe snapshot/restore intents, staging receipts, and post-purge
/// namespace recovery operations with verified CAS cutover.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let statements: &[&str] = match manager.get_database_backend() {
            DbBackend::Postgres => &[
                "CREATE TABLE module_artifact_data_snapshot_copy_intents (\
                    intent_id UUID PRIMARY KEY,\
                    tenant_id UUID NOT NULL,\
                    data_owner_id UUID NOT NULL,\
                    namespace_instance_id UUID NOT NULL,\
                    snapshot_id UUID NOT NULL,\
                    operation_id UUID NOT NULL,\
                    operation_request_digest TEXT NOT NULL CHECK (length(operation_request_digest) = 71),\
                    request_digest TEXT NOT NULL CHECK (length(request_digest) = 71),\
                    operation_kind TEXT NOT NULL CHECK (operation_kind IN ('snapshot', 'restore')),\
                    object_name TEXT NOT NULL CHECK (length(object_name) BETWEEN 1 AND 256),\
                    source_storage_key TEXT NOT NULL,\
                    target_storage_key TEXT NOT NULL,\
                    digest_sha256 TEXT NOT NULL CHECK (digest_sha256 ~ '^sha256:[0-9a-f]{64}$'),\
                    size_bytes BIGINT NOT NULL CHECK (size_bytes >= 0),\
                    status TEXT NOT NULL CHECK (status IN ('intent', 'staging', 'committed')),\
                    created_at TIMESTAMPTZ NOT NULL,\
                    committed_at TIMESTAMPTZ NULL,\
                    UNIQUE (tenant_id, data_owner_id, namespace_instance_id, operation_id, operation_kind, object_name)\
                )",
                "CREATE INDEX idx_snapshot_copy_intents_scope \
                 ON module_artifact_data_snapshot_copy_intents (tenant_id, snapshot_id, status)",
                "ALTER TABLE module_artifact_data_snapshot_copy_intents ENABLE ROW LEVEL SECURITY",
                "CREATE POLICY module_artifact_data_snapshot_copy_intents_scope \
                 ON module_artifact_data_snapshot_copy_intents \
                 USING (tenant_id::text = current_setting('rustok.tenant_id', true)) \
                 WITH CHECK (tenant_id::text = current_setting('rustok.tenant_id', true))",
                "CREATE TABLE module_artifact_data_snapshot_holds (\
                    tenant_id UUID NOT NULL,\
                    snapshot_id UUID NOT NULL,\
                    holder_kind TEXT NOT NULL CHECK (holder_kind IN ('restore', 'recovery')),\
                    holder_id UUID NOT NULL,\
                    data_owner_id UUID NOT NULL,\
                    namespace_instance_id UUID NOT NULL,\
                    request_digest TEXT NOT NULL CHECK (length(request_digest) = 71),\
                    created_at TIMESTAMPTZ NOT NULL,\
                    released_at TIMESTAMPTZ NULL,\
                    PRIMARY KEY (tenant_id, snapshot_id, holder_kind, holder_id)\
                )",
                "CREATE INDEX idx_artifact_data_snapshot_active_holds \
                 ON module_artifact_data_snapshot_holds (tenant_id, snapshot_id, released_at)",
                "ALTER TABLE module_artifact_data_snapshot_holds ENABLE ROW LEVEL SECURITY",
                "CREATE POLICY module_artifact_data_snapshot_holds_scope ON module_artifact_data_snapshot_holds \
                 USING (tenant_id::text = current_setting('rustok.tenant_id', true)) \
                 WITH CHECK (tenant_id::text = current_setting('rustok.tenant_id', true))",
                "CREATE TABLE module_artifact_data_namespace_recovery_operations (\
                    recovery_id UUID PRIMARY KEY,\
                    tenant_id UUID NOT NULL,\
                    data_owner_id UUID NOT NULL,\
                    namespace_instance_id UUID NOT NULL,\
                    source_snapshot_id UUID NOT NULL,\
                    tombstone_namespace_revision BIGINT NOT NULL CHECK (tombstone_namespace_revision > 0),\
                    target_namespace_revision BIGINT NOT NULL CHECK (target_namespace_revision > 0),\
                    status TEXT NOT NULL CHECK (status IN ('staging', 'verified', 'cutover', 'aborted')),\
                    records_restored BIGINT NOT NULL CHECK (records_restored >= 0),\
                    objects_restored BIGINT NOT NULL CHECK (objects_restored >= 0),\
                    manifest_digest TEXT NOT NULL CHECK (manifest_digest ~ '^sha256:[0-9a-f]{64}$'),\
                    request_digest TEXT NOT NULL CHECK (request_digest ~ '^sha256:[0-9a-f]{64}$'),\
                    actor_id UUID NOT NULL,\
                    trace_id TEXT NOT NULL CHECK (length(trim(trace_id)) BETWEEN 1 AND 512),\
                    correlation_id UUID NOT NULL,\
                    idempotency_key UUID NOT NULL,\
                    created_at TIMESTAMPTZ NOT NULL,\
                    verified_at TIMESTAMPTZ NULL,\
                    cutover_at TIMESTAMPTZ NULL,\
                    UNIQUE (tenant_id, data_owner_id, namespace_instance_id, idempotency_key)\
                )",
                "CREATE INDEX idx_namespace_recovery_ops_scope \
                 ON module_artifact_data_namespace_recovery_operations (tenant_id, data_owner_id, namespace_instance_id, status)",
                "ALTER TABLE module_artifact_data_namespace_recovery_operations ENABLE ROW LEVEL SECURITY",
                "CREATE POLICY module_artifact_data_namespace_recovery_operations_scope \
                 ON module_artifact_data_namespace_recovery_operations \
                 USING (tenant_id::text = current_setting('rustok.tenant_id', true)) \
                 WITH CHECK (tenant_id::text = current_setting('rustok.tenant_id', true))",
            ],
            DbBackend::Sqlite => &[
                "CREATE TABLE module_artifact_data_snapshot_copy_intents (\
                    intent_id TEXT PRIMARY KEY NOT NULL,\
                    tenant_id TEXT NOT NULL,\
                    data_owner_id TEXT NOT NULL,\
                    namespace_instance_id TEXT NOT NULL,\
                    snapshot_id TEXT NOT NULL,\
                    operation_id TEXT NOT NULL,\
                    operation_request_digest TEXT NOT NULL CHECK (length(operation_request_digest) = 71),\
                    request_digest TEXT NOT NULL CHECK (length(request_digest) = 71),\
                    operation_kind TEXT NOT NULL CHECK (operation_kind IN ('snapshot', 'restore')),\
                    object_name TEXT NOT NULL CHECK (length(object_name) BETWEEN 1 AND 256),\
                    source_storage_key TEXT NOT NULL,\
                    target_storage_key TEXT NOT NULL,\
                    digest_sha256 TEXT NOT NULL CHECK (length(digest_sha256) = 71),\
                    size_bytes INTEGER NOT NULL CHECK (size_bytes >= 0),\
                    status TEXT NOT NULL CHECK (status IN ('intent', 'staging', 'committed')),\
                    created_at TEXT NOT NULL,\
                    committed_at TEXT NULL,\
                    UNIQUE (tenant_id, data_owner_id, namespace_instance_id, operation_id, operation_kind, object_name)\
                )",
                "CREATE INDEX idx_snapshot_copy_intents_scope \
                 ON module_artifact_data_snapshot_copy_intents (tenant_id, snapshot_id, status)",
                "CREATE TABLE module_artifact_data_snapshot_holds (\
                    tenant_id TEXT NOT NULL,\
                    snapshot_id TEXT NOT NULL,\
                    holder_kind TEXT NOT NULL CHECK (holder_kind IN ('restore', 'recovery')),\
                    holder_id TEXT NOT NULL,\
                    data_owner_id TEXT NOT NULL,\
                    namespace_instance_id TEXT NOT NULL,\
                    request_digest TEXT NOT NULL CHECK (length(request_digest) = 71),\
                    created_at TEXT NOT NULL,\
                    released_at TEXT NULL,\
                    PRIMARY KEY (tenant_id, snapshot_id, holder_kind, holder_id)\
                )",
                "CREATE INDEX idx_artifact_data_snapshot_active_holds \
                 ON module_artifact_data_snapshot_holds (tenant_id, snapshot_id, released_at)",
                "CREATE TABLE module_artifact_data_namespace_recovery_operations (\
                    recovery_id TEXT PRIMARY KEY NOT NULL,\
                    tenant_id TEXT NOT NULL,\
                    data_owner_id TEXT NOT NULL,\
                    namespace_instance_id TEXT NOT NULL,\
                    source_snapshot_id TEXT NOT NULL,\
                    tombstone_namespace_revision INTEGER NOT NULL CHECK (tombstone_namespace_revision > 0),\
                    target_namespace_revision INTEGER NOT NULL CHECK (target_namespace_revision > 0),\
                    status TEXT NOT NULL CHECK (status IN ('staging', 'verified', 'cutover', 'aborted')),\
                    records_restored INTEGER NOT NULL CHECK (records_restored >= 0),\
                    objects_restored INTEGER NOT NULL CHECK (objects_restored >= 0),\
                    manifest_digest TEXT NOT NULL CHECK (length(manifest_digest) = 71),\
                    request_digest TEXT NOT NULL CHECK (length(request_digest) = 71 AND substr(request_digest, 1, 7) = 'sha256:' AND substr(request_digest, 8) NOT GLOB '*[^0-9a-f]*'),\
                    actor_id TEXT NOT NULL,\
                    trace_id TEXT NOT NULL CHECK (length(trim(trace_id)) BETWEEN 1 AND 512),\
                    correlation_id TEXT NOT NULL,\
                    idempotency_key TEXT NOT NULL,\
                    created_at TEXT NOT NULL,\
                    verified_at TEXT NULL,\
                    cutover_at TEXT NULL,\
                    UNIQUE (tenant_id, data_owner_id, namespace_instance_id, idempotency_key)\
                )",
                "CREATE INDEX idx_namespace_recovery_ops_scope \
                 ON module_artifact_data_namespace_recovery_operations (tenant_id, data_owner_id, namespace_instance_id, status)",
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
            "DROP TABLE IF EXISTS module_artifact_data_namespace_recovery_operations",
            "DROP TABLE IF EXISTS module_artifact_data_snapshot_copy_intents",
            "DROP TABLE IF EXISTS module_artifact_data_snapshot_holds",
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
