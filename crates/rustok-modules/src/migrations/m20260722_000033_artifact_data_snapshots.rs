use sea_orm::{ConnectionTrait, DbBackend, Statement};
use sea_orm_migration::prelude::*;

/// Durable owner snapshots for one artifact data namespace. Logical values and
/// index projections remain in PostgreSQL while private object bytes are copied
/// to snapshot-owned storage keys before a snapshot becomes ready.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let statements: &[&str] = match manager.get_database_backend() {
            DbBackend::Postgres => &[
                "CREATE TABLE module_artifact_data_snapshots (\
                    snapshot_id UUID PRIMARY KEY,\
                    tenant_id UUID NOT NULL,\
                    module_slug TEXT NOT NULL,\
                    data_contract_revision BIGINT NOT NULL CHECK (data_contract_revision > 0),\
                    policy_revision BIGINT NOT NULL CHECK (policy_revision > 0),\
                    source_namespace_revision BIGINT NOT NULL CHECK (source_namespace_revision > 0),\
                    status TEXT NOT NULL CHECK (status IN ('staging', 'ready', 'collecting')),\
                    retention_revision BIGINT NOT NULL CHECK (retention_revision > 0),\
                    request_digest TEXT NOT NULL CHECK (request_digest ~ '^sha256:[0-9a-f]{64}$'),\
                    manifest_digest TEXT NULL CHECK (manifest_digest IS NULL OR manifest_digest ~ '^sha256:[0-9a-f]{64}$'),\
                    actor_id UUID NOT NULL,\
                    reason TEXT NOT NULL CHECK (length(trim(reason)) BETWEEN 1 AND 2000),\
                    idempotency_key UUID NOT NULL,\
                    structured_record_count BIGINT NOT NULL CHECK (structured_record_count >= 0),\
                    object_count BIGINT NOT NULL CHECK (object_count >= 0),\
                    total_object_bytes BIGINT NOT NULL CHECK (total_object_bytes >= 0),\
                    retain_until TIMESTAMPTZ NOT NULL,\
                    legal_hold BOOLEAN NOT NULL DEFAULT FALSE,\
                    created_at TIMESTAMPTZ NOT NULL,\
                    ready_at TIMESTAMPTZ NULL,\
                    UNIQUE (tenant_id, module_slug, data_contract_revision, idempotency_key)\
                )",
                "CREATE INDEX module_artifact_data_snapshots_scope_idx ON module_artifact_data_snapshots (tenant_id, module_slug, data_contract_revision, created_at, snapshot_id)",
                "ALTER TABLE module_artifact_data_snapshots ENABLE ROW LEVEL SECURITY",
                "CREATE POLICY module_artifact_data_snapshots_scope ON module_artifact_data_snapshots USING (tenant_id::text = current_setting('rustok.tenant_id', true)) WITH CHECK (tenant_id::text = current_setting('rustok.tenant_id', true))",
                "CREATE TABLE module_artifact_data_snapshot_records (\
                    tenant_id UUID NOT NULL,\
                    snapshot_id UUID NOT NULL REFERENCES module_artifact_data_snapshots(snapshot_id) ON DELETE CASCADE,\
                    data_key TEXT NOT NULL CHECK (length(data_key) BETWEEN 1 AND 256),\
                    value JSONB NOT NULL,\
                    revision BIGINT NOT NULL CHECK (revision > 0),\
                    PRIMARY KEY (snapshot_id, data_key)\
                )",
                "ALTER TABLE module_artifact_data_snapshot_records ENABLE ROW LEVEL SECURITY",
                "CREATE POLICY module_artifact_data_snapshot_records_scope ON module_artifact_data_snapshot_records USING (tenant_id::text = current_setting('rustok.tenant_id', true)) WITH CHECK (tenant_id::text = current_setting('rustok.tenant_id', true))",
                "CREATE TABLE module_artifact_data_snapshot_objects (\
                    tenant_id UUID NOT NULL,\
                    snapshot_id UUID NOT NULL REFERENCES module_artifact_data_snapshots(snapshot_id) ON DELETE CASCADE,\
                    object_name TEXT NOT NULL CHECK (length(object_name) BETWEEN 1 AND 256),\
                    content_type TEXT NOT NULL CHECK (length(content_type) BETWEEN 1 AND 128),\
                    size_bytes BIGINT NOT NULL CHECK (size_bytes > 0 AND size_bytes <= 33554432),\
                    digest_sha256 TEXT NOT NULL CHECK (digest_sha256 ~ '^sha256:[0-9a-f]{64}$'),\
                    revision BIGINT NOT NULL CHECK (revision > 0),\
                    source_storage_key TEXT NOT NULL,\
                    snapshot_storage_key TEXT NULL UNIQUE,\
                    PRIMARY KEY (snapshot_id, object_name)\
                )",
                "CREATE INDEX module_artifact_data_snapshot_objects_source_idx ON module_artifact_data_snapshot_objects (tenant_id, source_storage_key)",
                "ALTER TABLE module_artifact_data_snapshot_objects ENABLE ROW LEVEL SECURITY",
                "CREATE POLICY module_artifact_data_snapshot_objects_scope ON module_artifact_data_snapshot_objects USING (tenant_id::text = current_setting('rustok.tenant_id', true)) WITH CHECK (tenant_id::text = current_setting('rustok.tenant_id', true))",
                "CREATE TABLE module_artifact_data_snapshot_indexes (\
                    tenant_id UUID NOT NULL,\
                    snapshot_id UUID NOT NULL REFERENCES module_artifact_data_snapshots(snapshot_id) ON DELETE CASCADE,\
                    index_name TEXT NOT NULL CHECK (length(index_name) BETWEEN 1 AND 64),\
                    index_value TEXT NOT NULL CHECK (length(index_value) BETWEEN 1 AND 256),\
                    data_key TEXT NOT NULL CHECK (length(data_key) BETWEEN 1 AND 256),\
                    PRIMARY KEY (snapshot_id, index_name, index_value, data_key)\
                )",
                "ALTER TABLE module_artifact_data_snapshot_indexes ENABLE ROW LEVEL SECURITY",
                "CREATE POLICY module_artifact_data_snapshot_indexes_scope ON module_artifact_data_snapshot_indexes USING (tenant_id::text = current_setting('rustok.tenant_id', true)) WITH CHECK (tenant_id::text = current_setting('rustok.tenant_id', true))",
                "CREATE TABLE module_artifact_data_snapshot_index_contracts (\
                    tenant_id UUID NOT NULL,\
                    snapshot_id UUID PRIMARY KEY REFERENCES module_artifact_data_snapshots(snapshot_id) ON DELETE CASCADE,\
                    contract_digest TEXT NOT NULL CHECK (contract_digest ~ '^sha256:[0-9a-f]{64}$')\
                )",
                "ALTER TABLE module_artifact_data_snapshot_index_contracts ENABLE ROW LEVEL SECURITY",
                "CREATE POLICY module_artifact_data_snapshot_index_contracts_scope ON module_artifact_data_snapshot_index_contracts USING (tenant_id::text = current_setting('rustok.tenant_id', true)) WITH CHECK (tenant_id::text = current_setting('rustok.tenant_id', true))",
                "CREATE TABLE module_artifact_data_snapshot_retention_operations (\
                    tenant_id UUID NOT NULL,\
                    snapshot_id UUID NOT NULL,\
                    idempotency_key UUID NOT NULL,\
                    request_digest TEXT NOT NULL CHECK (request_digest ~ '^sha256:[0-9a-f]{64}$'),\
                    expected_retention_revision BIGINT NOT NULL CHECK (expected_retention_revision > 0),\
                    retention_revision BIGINT NOT NULL CHECK (retention_revision > 0),\
                    retain_until TIMESTAMPTZ NOT NULL,\
                    legal_hold BOOLEAN NOT NULL,\
                    actor_id UUID NOT NULL,\
                    reason TEXT NOT NULL CHECK (length(trim(reason)) BETWEEN 1 AND 2000),\
                    completed_at TIMESTAMPTZ NOT NULL,\
                    PRIMARY KEY (tenant_id, snapshot_id, idempotency_key)\
                )",
                "ALTER TABLE module_artifact_data_snapshot_retention_operations ENABLE ROW LEVEL SECURITY",
                "CREATE POLICY module_artifact_data_snapshot_retention_operations_scope ON module_artifact_data_snapshot_retention_operations USING (tenant_id::text = current_setting('rustok.tenant_id', true)) WITH CHECK (tenant_id::text = current_setting('rustok.tenant_id', true))",
                "CREATE TABLE module_artifact_data_snapshot_collections (\
                    collection_id UUID PRIMARY KEY,\
                    tenant_id UUID NOT NULL,\
                    snapshot_id UUID NOT NULL UNIQUE,\
                    module_slug TEXT NOT NULL,\
                    data_contract_revision BIGINT NOT NULL CHECK (data_contract_revision > 0),\
                    policy_snapshot_id TEXT NOT NULL CHECK (length(policy_snapshot_id) BETWEEN 1 AND 128),\
                    actor_id UUID NOT NULL,\
                    reason TEXT NOT NULL CHECK (length(trim(reason)) BETWEEN 1 AND 2000),\
                    object_count BIGINT NOT NULL CHECK (object_count >= 0),\
                    collecting_at TIMESTAMPTZ NOT NULL,\
                    completed_at TIMESTAMPTZ NULL\
                )",
                "CREATE INDEX module_artifact_data_snapshot_collections_tenant_idx ON module_artifact_data_snapshot_collections (tenant_id, collecting_at, collection_id)",
                "ALTER TABLE module_artifact_data_snapshot_collections ENABLE ROW LEVEL SECURITY",
                "CREATE POLICY module_artifact_data_snapshot_collections_scope ON module_artifact_data_snapshot_collections USING (tenant_id::text = current_setting('rustok.tenant_id', true)) WITH CHECK (tenant_id::text = current_setting('rustok.tenant_id', true))",
                "CREATE TABLE module_artifact_data_restore_operations (\
                    tenant_id UUID NOT NULL,\
                    module_slug TEXT NOT NULL,\
                    data_contract_revision BIGINT NOT NULL CHECK (data_contract_revision > 0),\
                    idempotency_key UUID NOT NULL,\
                    request_digest TEXT NOT NULL CHECK (request_digest ~ '^sha256:[0-9a-f]{64}$'),\
                    snapshot_id UUID NOT NULL,\
                    expected_namespace_revision BIGINT NOT NULL CHECK (expected_namespace_revision > 0),\
                    namespace_revision BIGINT NOT NULL CHECK (namespace_revision > 0),\
                    restored_records BIGINT NOT NULL CHECK (restored_records >= 0),\
                    restored_objects BIGINT NOT NULL CHECK (restored_objects >= 0),\
                    actor_id UUID NOT NULL,\
                    reason TEXT NOT NULL CHECK (length(trim(reason)) BETWEEN 1 AND 2000),\
                    completed_at TIMESTAMPTZ NOT NULL,\
                    PRIMARY KEY (tenant_id, module_slug, data_contract_revision, idempotency_key)\
                )",
                "ALTER TABLE module_artifact_data_restore_operations ENABLE ROW LEVEL SECURITY",
                "CREATE POLICY module_artifact_data_restore_operations_scope ON module_artifact_data_restore_operations USING (tenant_id::text = current_setting('rustok.tenant_id', true)) WITH CHECK (tenant_id::text = current_setting('rustok.tenant_id', true))",
            ],
            DbBackend::Sqlite => &[
                "CREATE TABLE module_artifact_data_snapshots (snapshot_id TEXT PRIMARY KEY, tenant_id TEXT NOT NULL, module_slug TEXT NOT NULL, data_contract_revision INTEGER NOT NULL CHECK (data_contract_revision > 0), policy_revision INTEGER NOT NULL CHECK (policy_revision > 0), source_namespace_revision INTEGER NOT NULL CHECK (source_namespace_revision > 0), status TEXT NOT NULL CHECK (status IN ('staging', 'ready', 'collecting')), retention_revision INTEGER NOT NULL CHECK (retention_revision > 0), request_digest TEXT NOT NULL CHECK (length(request_digest) = 71), manifest_digest TEXT NULL CHECK (manifest_digest IS NULL OR length(manifest_digest) = 71), actor_id TEXT NOT NULL, reason TEXT NOT NULL CHECK (length(trim(reason)) BETWEEN 1 AND 2000), idempotency_key TEXT NOT NULL, structured_record_count INTEGER NOT NULL CHECK (structured_record_count >= 0), object_count INTEGER NOT NULL CHECK (object_count >= 0), total_object_bytes INTEGER NOT NULL CHECK (total_object_bytes >= 0), retain_until TEXT NOT NULL, legal_hold INTEGER NOT NULL DEFAULT 0, created_at TEXT NOT NULL, ready_at TEXT NULL, UNIQUE (tenant_id, module_slug, data_contract_revision, idempotency_key))",
                "CREATE INDEX module_artifact_data_snapshots_scope_idx ON module_artifact_data_snapshots (tenant_id, module_slug, data_contract_revision, created_at, snapshot_id)",
                "CREATE TABLE module_artifact_data_snapshot_records (tenant_id TEXT NOT NULL, snapshot_id TEXT NOT NULL REFERENCES module_artifact_data_snapshots(snapshot_id) ON DELETE CASCADE, data_key TEXT NOT NULL CHECK (length(data_key) BETWEEN 1 AND 256), value JSON NOT NULL, revision INTEGER NOT NULL CHECK (revision > 0), PRIMARY KEY (snapshot_id, data_key))",
                "CREATE TABLE module_artifact_data_snapshot_objects (tenant_id TEXT NOT NULL, snapshot_id TEXT NOT NULL REFERENCES module_artifact_data_snapshots(snapshot_id) ON DELETE CASCADE, object_name TEXT NOT NULL CHECK (length(object_name) BETWEEN 1 AND 256), content_type TEXT NOT NULL CHECK (length(content_type) BETWEEN 1 AND 128), size_bytes INTEGER NOT NULL CHECK (size_bytes > 0 AND size_bytes <= 33554432), digest_sha256 TEXT NOT NULL CHECK (length(digest_sha256) = 71), revision INTEGER NOT NULL CHECK (revision > 0), source_storage_key TEXT NOT NULL, snapshot_storage_key TEXT NULL UNIQUE, PRIMARY KEY (snapshot_id, object_name))",
                "CREATE INDEX module_artifact_data_snapshot_objects_source_idx ON module_artifact_data_snapshot_objects (tenant_id, source_storage_key)",
                "CREATE TABLE module_artifact_data_snapshot_indexes (tenant_id TEXT NOT NULL, snapshot_id TEXT NOT NULL REFERENCES module_artifact_data_snapshots(snapshot_id) ON DELETE CASCADE, index_name TEXT NOT NULL CHECK (length(index_name) BETWEEN 1 AND 64), index_value TEXT NOT NULL CHECK (length(index_value) BETWEEN 1 AND 256), data_key TEXT NOT NULL CHECK (length(data_key) BETWEEN 1 AND 256), PRIMARY KEY (snapshot_id, index_name, index_value, data_key))",
                "CREATE TABLE module_artifact_data_snapshot_index_contracts (tenant_id TEXT NOT NULL, snapshot_id TEXT PRIMARY KEY REFERENCES module_artifact_data_snapshots(snapshot_id) ON DELETE CASCADE, contract_digest TEXT NOT NULL CHECK (length(contract_digest) = 71))",
                "CREATE TABLE module_artifact_data_snapshot_retention_operations (tenant_id TEXT NOT NULL, snapshot_id TEXT NOT NULL, idempotency_key TEXT NOT NULL, request_digest TEXT NOT NULL CHECK (length(request_digest) = 71), expected_retention_revision INTEGER NOT NULL CHECK (expected_retention_revision > 0), retention_revision INTEGER NOT NULL CHECK (retention_revision > 0), retain_until TEXT NOT NULL, legal_hold INTEGER NOT NULL, actor_id TEXT NOT NULL, reason TEXT NOT NULL CHECK (length(trim(reason)) BETWEEN 1 AND 2000), completed_at TEXT NOT NULL, PRIMARY KEY (tenant_id, snapshot_id, idempotency_key))",
                "CREATE TABLE module_artifact_data_snapshot_collections (collection_id TEXT PRIMARY KEY, tenant_id TEXT NOT NULL, snapshot_id TEXT NOT NULL UNIQUE, module_slug TEXT NOT NULL, data_contract_revision INTEGER NOT NULL CHECK (data_contract_revision > 0), policy_snapshot_id TEXT NOT NULL CHECK (length(policy_snapshot_id) BETWEEN 1 AND 128), actor_id TEXT NOT NULL, reason TEXT NOT NULL CHECK (length(trim(reason)) BETWEEN 1 AND 2000), object_count INTEGER NOT NULL CHECK (object_count >= 0), collecting_at TEXT NOT NULL, completed_at TEXT NULL)",
                "CREATE INDEX module_artifact_data_snapshot_collections_tenant_idx ON module_artifact_data_snapshot_collections (tenant_id, collecting_at, collection_id)",
                "CREATE TABLE module_artifact_data_restore_operations (tenant_id TEXT NOT NULL, module_slug TEXT NOT NULL, data_contract_revision INTEGER NOT NULL CHECK (data_contract_revision > 0), idempotency_key TEXT NOT NULL, request_digest TEXT NOT NULL CHECK (length(request_digest) = 71), snapshot_id TEXT NOT NULL, expected_namespace_revision INTEGER NOT NULL CHECK (expected_namespace_revision > 0), namespace_revision INTEGER NOT NULL CHECK (namespace_revision > 0), restored_records INTEGER NOT NULL CHECK (restored_records >= 0), restored_objects INTEGER NOT NULL CHECK (restored_objects >= 0), actor_id TEXT NOT NULL, reason TEXT NOT NULL CHECK (length(trim(reason)) BETWEEN 1 AND 2000), completed_at TEXT NOT NULL, PRIMARY KEY (tenant_id, module_slug, data_contract_revision, idempotency_key))",
            ],
            backend => {
                return Err(DbErr::Migration(format!(
                    "artifact data snapshot migration does not support database backend {backend:?}"
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
        for table in [
            "module_artifact_data_restore_operations",
            "module_artifact_data_snapshot_collections",
            "module_artifact_data_snapshot_retention_operations",
            "module_artifact_data_snapshot_index_contracts",
            "module_artifact_data_snapshot_indexes",
            "module_artifact_data_snapshot_objects",
            "module_artifact_data_snapshot_records",
            "module_artifact_data_snapshots",
        ] {
            manager
                .get_connection()
                .execute_unprepared(&format!("DROP TABLE {table}"))
                .await?;
        }
        Ok(())
    }
}
