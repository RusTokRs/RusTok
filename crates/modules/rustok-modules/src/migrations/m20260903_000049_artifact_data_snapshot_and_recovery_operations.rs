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
                    snapshot_id UUID NOT NULL,\
                    operation_kind TEXT NOT NULL CHECK (operation_kind IN ('snapshot', 'restore')),\
                    object_name TEXT NOT NULL CHECK (length(object_name) BETWEEN 1 AND 256),\
                    source_storage_key TEXT NOT NULL,\
                    target_storage_key TEXT NOT NULL,\
                    digest_sha256 TEXT NOT NULL CHECK (digest_sha256 ~ '^sha256:[0-9a-f]{64}$'),\
                    size_bytes BIGINT NOT NULL CHECK (size_bytes >= 0),\
                    status TEXT NOT NULL CHECK (status IN ('intent', 'staging', 'committed', 'collected')),\
                    created_at TIMESTAMPTZ NOT NULL,\
                    committed_at TIMESTAMPTZ NULL,\
                    collected_at TIMESTAMPTZ NULL,\
                    UNIQUE (tenant_id, snapshot_id, operation_kind, object_name)\
                )",
                "CREATE INDEX idx_snapshot_copy_intents_scope \
                 ON module_artifact_data_snapshot_copy_intents (tenant_id, snapshot_id, status)",
                "ALTER TABLE module_artifact_data_snapshot_copy_intents ENABLE ROW LEVEL SECURITY",
                "CREATE POLICY module_artifact_data_snapshot_copy_intents_scope \
                 ON module_artifact_data_snapshot_copy_intents \
                 USING (tenant_id::text = current_setting('rustok.tenant_id', true)) \
                 WITH CHECK (tenant_id::text = current_setting('rustok.tenant_id', true))",
                "CREATE TABLE module_artifact_data_namespace_recovery_operations (\
                    recovery_id UUID PRIMARY KEY,\
                    tenant_id UUID NOT NULL,\
                    module_slug TEXT NOT NULL,\
                    data_contract_revision BIGINT NOT NULL CHECK (data_contract_revision > 0),\
                    source_snapshot_id UUID NOT NULL,\
                    tombstone_namespace_revision BIGINT NOT NULL CHECK (tombstone_namespace_revision > 0),\
                    target_namespace_revision BIGINT NOT NULL CHECK (target_namespace_revision > 0),\
                    status TEXT NOT NULL CHECK (status IN ('staging', 'verified', 'cutover', 'aborted')),\
                    records_restored BIGINT NOT NULL CHECK (records_restored >= 0),\
                    objects_restored BIGINT NOT NULL CHECK (objects_restored >= 0),\
                    manifest_digest TEXT NOT NULL CHECK (manifest_digest ~ '^sha256:[0-9a-f]{64}$'),\
                    actor_id UUID NOT NULL,\
                    trace_id TEXT NOT NULL CHECK (length(trim(trace_id)) BETWEEN 1 AND 512),\
                    correlation_id UUID NOT NULL,\
                    idempotency_key UUID NOT NULL,\
                    created_at TIMESTAMPTZ NOT NULL,\
                    verified_at TIMESTAMPTZ NULL,\
                    cutover_at TIMESTAMPTZ NULL,\
                    UNIQUE (tenant_id, module_slug, data_contract_revision, idempotency_key)\
                )",
                "CREATE INDEX idx_namespace_recovery_ops_scope \
                 ON module_artifact_data_namespace_recovery_operations (tenant_id, module_slug, data_contract_revision, status)",
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
                    snapshot_id TEXT NOT NULL,\
                    operation_kind TEXT NOT NULL CHECK (operation_kind IN ('snapshot', 'restore')),\
                    object_name TEXT NOT NULL CHECK (length(object_name) BETWEEN 1 AND 256),\
                    source_storage_key TEXT NOT NULL,\
                    target_storage_key TEXT NOT NULL,\
                    digest_sha256 TEXT NOT NULL CHECK (length(digest_sha256) = 71),\
                    size_bytes INTEGER NOT NULL CHECK (size_bytes >= 0),\
                    status TEXT NOT NULL CHECK (status IN ('intent', 'staging', 'committed', 'collected')),\
                    created_at TEXT NOT NULL,\
                    committed_at TEXT NULL,\
                    collected_at TEXT NULL,\
                    UNIQUE (tenant_id, snapshot_id, operation_kind, object_name)\
                )",
                "CREATE INDEX idx_snapshot_copy_intents_scope \
                 ON module_artifact_data_snapshot_copy_intents (tenant_id, snapshot_id, status)",
                "CREATE TABLE module_artifact_data_namespace_recovery_operations (\
                    recovery_id TEXT PRIMARY KEY NOT NULL,\
                    tenant_id TEXT NOT NULL,\
                    module_slug TEXT NOT NULL,\
                    data_contract_revision INTEGER NOT NULL CHECK (data_contract_revision > 0),\
                    source_snapshot_id TEXT NOT NULL,\
                    tombstone_namespace_revision INTEGER NOT NULL CHECK (tombstone_namespace_revision > 0),\
                    target_namespace_revision INTEGER NOT NULL CHECK (target_namespace_revision > 0),\
                    status TEXT NOT NULL CHECK (status IN ('staging', 'verified', 'cutover', 'aborted')),\
                    records_restored INTEGER NOT NULL CHECK (records_restored >= 0),\
                    objects_restored INTEGER NOT NULL CHECK (objects_restored >= 0),\
                    manifest_digest TEXT NOT NULL CHECK (length(manifest_digest) = 71),\
                    actor_id TEXT NOT NULL,\
                    trace_id TEXT NOT NULL CHECK (length(trim(trace_id)) BETWEEN 1 AND 512),\
                    correlation_id TEXT NOT NULL,\
                    idempotency_key TEXT NOT NULL,\
                    created_at TEXT NOT NULL,\
                    verified_at TEXT NULL,\
                    cutover_at TEXT NULL,\
                    UNIQUE (tenant_id, module_slug, data_contract_revision, idempotency_key)\
                )",
                "CREATE INDEX idx_namespace_recovery_ops_scope \
                 ON module_artifact_data_namespace_recovery_operations (tenant_id, module_slug, data_contract_revision, status)",
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
